use super::daemon::daemon_health_check;
use super::integration::{hook_checks, mcp_checks, pointer_check};
use super::package::expected_binaries;
use super::project::same_path_argument;
use super::runtime::{
    endpoint_check, endpoint_valid, executable_checks_at, operator_credential_outcome,
    runtime_check,
};
use super::*;
use precept_ipc::{API_VERSION, Error};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        let nonce = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "precept-doctor-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn find<'a>(checks: &'a [Check], id: &str) -> &'a Check {
    checks.iter().find(|check| check.id == id).unwrap()
}

fn report(checks: Vec<Check>) -> Value {
    serde_json::from_str(&Report::new(checks).render(true)).unwrap()
}

#[test]
fn report_shape_is_versioned_and_counts_every_distinct_state() {
    let checks = vec![
        Check::new("test.ok", Status::Ok, Reason::RuntimeOk, Remediation::None),
        Check::new(
            "test.attention",
            Status::Attention,
            Reason::PointerOutdated,
            Remediation::ReviewPointer,
        ),
        Check::new(
            "test.problem",
            Status::Problem,
            Reason::RuntimeUnsafe,
            Remediation::UseAbsolutePrivateRuntime,
        ),
        Check::new(
            "test.off",
            Status::Off,
            Reason::WritesDisabled,
            Remediation::ReviewWriteGrant,
        ),
        Check::new(
            "test.unknown",
            Status::Unknown,
            Reason::CredentialNotIssued,
            Remediation::StartDaemonExplicitly,
        ),
    ];
    let value = report(checks);
    assert_eq!(value["format"], FORMAT);
    assert_eq!(value["version"], VERSION);
    assert_eq!(value["product_version"], PRODUCT_VERSION);
    assert!(matches!(
        value["platform"].as_str(),
        Some("windows" | "linux")
    ));
    assert_eq!(value["summary"]["status"], "problem");
    for (status, count) in [
        ("ok", 1),
        ("attention", 1),
        ("problem", 1),
        ("off", 1),
        ("unknown", 1),
    ] {
        assert_eq!(value["summary"][status], count);
    }
    for row in value["checks"].as_array().unwrap() {
        assert!(row["id"].is_string());
        assert!(row["status"].is_string());
        assert!(row["reason"].is_string());
        assert!(row["remediation"].is_string());
        for key in row.as_object().unwrap().keys() {
            assert!(matches!(
                key.as_str(),
                "id" | "status" | "reason" | "remediation" | "facts"
            ));
        }
    }
    let text = Report::new(vec![Check::new(
        "test.off",
        Status::Off,
        Reason::WritesDisabled,
        Remediation::ReviewWriteGrant,
    )])
    .render(false);
    assert!(text.contains("[off] test.off: writes_disabled -> review_write_grant"));
}

#[tokio::test]
async fn run_redacts_path_value_token_and_raw_error_canaries() {
    let canaries = [
        "SECRET-PATH-CANARY/private/profile.sqlite3",
        "SECRET-VALUE-CANARY",
        "SECRET-TOKEN-CANARY-0123456789",
        "RAW-OS-ERROR-CANARY-access-denied",
    ];
    let location = Location {
        runtime_dir: PathBuf::from(canaries[0]),
        endpoint: canaries[2].into(),
    };
    let output = run(&location, Some(canaries[1]), true).await;
    let value: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(value["format"], FORMAT);
    for canary in canaries {
        assert!(!output.contains(canary), "redaction failed for {canary}");
    }
    assert!(value["summary"]["problem"].as_u64().unwrap() > 0);
}

#[test]
fn executable_layers_distinguish_missing_unsafe_and_unavailable() {
    let temp = TestDir::new("executable");
    let bins = temp.0.join("bin");
    fs::create_dir(&bins).unwrap();
    let current = bins.join(format!("precept{}", std::env::consts::EXE_SUFFIX));
    fs::write(&current, b"test").unwrap();
    let checks = executable_checks_at(Ok(current.clone()));
    assert_eq!(find(&checks, "executable.precept").status, Status::Ok);
    for id in [
        "executable.preceptd",
        "executable.precept_mcp",
        "executable.precept_hook",
    ] {
        assert_eq!(find(&checks, id).reason, Reason::ExecutableMissing);
    }
    fs::remove_file(&current).unwrap();
    fs::create_dir(&current).unwrap();
    assert_eq!(
        find(&executable_checks_at(Ok(current)), "executable.precept").reason,
        Reason::ExecutableUnsafe
    );
    assert_eq!(
        find(
            &executable_checks_at(Err(Error::Unavailable)),
            "executable.precept"
        )
        .reason,
        Reason::ExecutableUnavailable
    );
}

#[test]
fn runtime_endpoint_and_credential_layers_are_fixed_and_actionable() {
    assert_eq!(
        runtime_check(&Err(Error::UnsafeRuntime), Path::new(&std::env::temp_dir())).status,
        Status::Problem
    );
    let missing = TestDir::new("runtime").0.join("not-created");
    assert_eq!(
        runtime_check(&Err(Error::UnsafeRuntime), &missing).status,
        Status::Unknown
    );
    assert_eq!(
        runtime_check(&Err(Error::UnsafeRuntime), &missing).reason,
        Reason::RuntimeNotInitialized
    );
    let runtime = TestDir::new("endpoint");
    let root = runtime.0.canonicalize().unwrap();
    let location = Location {
        runtime_dir: root.clone(),
        endpoint: if cfg!(windows) {
            "doctor".into()
        } else {
            root.join("control.sock").to_str().unwrap().into()
        },
    };
    assert!(endpoint_valid(&location, Some(&root)));
    let invalid = Location {
        endpoint: if cfg!(windows) {
            "bad endpoint".into()
        } else {
            "relative.sock".into()
        },
        ..location.clone()
    };
    assert!(!endpoint_valid(&invalid, Some(&root)));
    assert_eq!(endpoint_check(false, true).reason, Reason::EndpointInvalid);
    assert_eq!(endpoint_check(false, false).status, Status::Unknown);
    assert_eq!(endpoint_check(false, false).reason, Reason::EndpointUnknown);

    assert_eq!(
        operator_credential_outcome(Some(&Access::Operator), None, true).status,
        Status::Ok
    );
    assert_eq!(
        operator_credential_outcome(
            Some(&Access::Agent {
                project: "alpha".into()
            }),
            None,
            true
        )
        .reason,
        Reason::CredentialWrongRole
    );
    assert_eq!(
        operator_credential_outcome(None, Some(Error::Unauthorized), true).reason,
        Reason::CredentialInvalid
    );
    assert_eq!(
        operator_credential_outcome(None, Some(Error::UnsafeRuntime), true).reason,
        Reason::CredentialUnsafe
    );
    assert_eq!(
        operator_credential_outcome(None, Some(Error::Unauthorized), false).status,
        Status::Unknown
    );
    let missing = operator_credential_outcome(None, Some(Error::UnsafeRuntime), false);
    assert_eq!(missing.status, Status::Unknown);
    assert_eq!(missing.reason, Reason::CredentialNotIssued);
    assert_eq!(missing.remediation, Remediation::StartDaemonExplicitly);
}

#[test]
fn daemon_layers_never_echo_malformed_values() {
    let canary = "SECRET-DAEMON-VALUE-CANARY";
    let health = Some(Ok(json!({
        "ready": true,
        "service": "preceptd",
        "api_version": API_VERSION,
        "version": canary,
        "schema": canary,
        "profile_selected": true
    })));
    let checks = daemon_checks(&health, Blocker::None);
    assert_eq!(find(&checks, "daemon.health").status, Status::Ok);
    assert_eq!(
        find(&checks, "daemon.product").reason,
        Reason::ProductIncompatible
    );
    assert_eq!(
        find(&checks, "profile.schema").reason,
        Reason::SchemaIncompatible
    );
    let output = serde_json::to_string(&checks).unwrap();
    assert!(!output.contains(canary));

    for error in [Error::Unavailable, Error::Timeout, Error::NotFound] {
        assert_eq!(
            daemon_health_check(&Some(Err(error)), Blocker::None).status,
            Status::Unknown
        );
    }
    for error in [
        Error::Unauthorized,
        Error::Version,
        Error::EndpointCollision,
    ] {
        assert_eq!(
            daemon_health_check(&Some(Err(error)), Blocker::None).status,
            Status::Problem
        );
    }
    assert_eq!(
        daemon_health_check(&None, Blocker::Credential).reason,
        Reason::DaemonNotChecked
    );
    assert_eq!(
        daemon_health_check(
            &Some(Ok(json!({"ready":false,"service":"preceptd"}))),
            Blocker::None
        )
        .reason,
        Reason::DaemonNotReady
    );
}

#[test]
fn profile_and_project_layers_keep_off_unknown_and_problem_distinct() {
    let health = Some(Ok(json!({"profile_selected":true,"schema":CURRENT_SCHEMA})));
    let selected = Some(Ok(json!({"selected":true,"write_enabled":false})));
    let checks = profile_checks(&selected, &health, Blocker::None);
    assert_eq!(find(&checks, "profile.selected").status, Status::Ok);
    assert_eq!(find(&checks, "profile.writes").status, Status::Off);
    let startup_canary = "SECRET-STARTUP-ERROR-CANARY";
    let unselected = Some(Ok(json!({
        "selected":false,
        "write_enabled":false,
        "startup_failure":startup_canary
    })));
    let checks = profile_checks(
        &unselected,
        &Some(Ok(json!({"profile_selected":false,"schema":"Unselected"}))),
        Blocker::None,
    );
    assert_eq!(
        find(&checks, "profile.selected").reason,
        Reason::ProfileStartupFailed
    );
    assert!(
        !serde_json::to_string(&checks)
            .unwrap()
            .contains(startup_canary)
    );

    let projects = Some(Ok(json!([{
        "name":"alpha","state":"ready","agents":true,"propose":true,"gate":true
    }])));
    let checks = project_checks(&projects, Some("alpha"), true, Some(false), Blocker::None);
    assert_eq!(
        find(&checks, "project.count").facts.as_ref().unwrap().count,
        Some(1)
    );
    assert_eq!(find(&checks, "project.state").status, Status::Ok);
    assert_eq!(find(&checks, "project.sharing").status, Status::Ok);
    assert_eq!(
        find(&checks, "project.propose").reason,
        Reason::ProposeRequiresWrites
    );
    assert_eq!(find(&checks, "project.gate").status, Status::Ok);
    assert_eq!(
        find(&checks, "project.gate")
            .facts
            .as_ref()
            .unwrap()
            .setup_only,
        Some(true)
    );

    let checks = project_checks(&projects, Some("missing"), true, Some(true), Blocker::None);
    assert_eq!(
        find(&checks, "project.binding").reason,
        Reason::ProjectNotFound
    );
    assert_eq!(find(&checks, "project.state").status, Status::Unknown);
    let checks = project_checks(&projects, None, true, Some(true), Blocker::None);
    assert_eq!(find(&checks, "project.binding").status, Status::Off);
    let checks = project_checks(
        &None,
        Some("alpha"),
        true,
        None,
        Blocker::Daemon(Error::Unavailable),
    );
    assert_eq!(find(&checks, "project.binding").status, Status::Unknown);

    for (state, status) in [
        ("unregistered", Status::Problem),
        ("unavailable", Status::Problem),
        ("future-state", Status::Unknown),
    ] {
        let projects = Some(Ok(json!([{
            "name":"alpha","state":state,"agents":false,"propose":false,"gate":false
        }])));
        let checks = project_checks(&projects, Some("alpha"), true, Some(true), Blocker::None);
        assert_eq!(find(&checks, "project.state").status, status);
        assert_eq!(find(&checks, "project.sharing").status, Status::Off);
        assert_eq!(find(&checks, "project.propose").status, Status::Off);
        assert_eq!(find(&checks, "project.gate").status, Status::Off);
    }
}

#[test]
fn integration_layers_cover_unavailable_off_outdated_malformed_and_unknown() {
    let unavailable = Some(Ok(json!({"available":false,"clients":[]})));
    let checks = mcp_checks(&unavailable, Blocker::None);
    assert_eq!(
        find(&checks, "integration.mcp.availability").status,
        Status::Problem
    );
    assert_eq!(
        find(&checks, "integration.mcp.claude").status,
        Status::Unknown
    );

    for (installed, current, malformed, status, reason) in [
        (false, false, false, Status::Off, Reason::McpNotInstalled),
        (true, false, false, Status::Attention, Reason::McpOutdated),
        (true, true, false, Status::Ok, Reason::McpCurrent),
        (false, false, true, Status::Problem, Reason::McpMalformed),
    ] {
        let value = Some(Ok(json!({"available":true,"clients":[
            {"client":"claude","installed":installed,"current":current,"malformed":malformed,"broken":false},
            {"client":"codex","installed":false,"current":false,"malformed":false}
        ]})));
        let checks = mcp_checks(&value, Blocker::None);
        assert_eq!(find(&checks, "integration.mcp.claude").status, status);
        assert_eq!(find(&checks, "integration.mcp.claude").reason, reason);
    }
    let checks = mcp_checks(
        &Some(Ok(json!({"available":true,"clients":[]}))),
        Blocker::None,
    );
    assert_eq!(
        find(&checks, "integration.mcp.claude").status,
        Status::Unknown
    );

    let hooks = Some(Ok(json!({
        "available":true,
        "hooks":[
            {"vendor":"claude","installed":true,"current":true,"malformed":false},
            {"vendor":"codex","installed":true,"current":false,"malformed":false}
        ],
        "gate_hooks":[
            {"vendor":"claude","installed":false,"current":false,"malformed":false},
            {"vendor":"codex","installed":false,"current":false,"malformed":true}
        ]
    })));
    let checks = hook_checks(&hooks, Blocker::None);
    assert_eq!(find(&checks, "integration.hooks.claude").status, Status::Ok);
    assert_eq!(
        find(&checks, "integration.hooks.codex").remediation,
        Remediation::UpdateAgentEntries
    );
    assert_eq!(
        find(&checks, "integration.hooks.codex").reason,
        Reason::HookOutdated
    );
    assert_eq!(find(&checks, "integration.gate.claude").status, Status::Off);
    assert_eq!(
        find(&checks, "integration.gate.codex").reason,
        Reason::HookMalformed
    );

    for (malformed, state, status, reason) in [
        (false, "current", Status::Ok, Reason::PointerCurrent),
        (
            false,
            "outdated",
            Status::Attention,
            Reason::PointerOutdated,
        ),
        (false, "absent", Status::Off, Reason::PointerAbsent),
        (
            false,
            "future",
            Status::Unknown,
            Reason::PointerStateUnknown,
        ),
        (true, "absent", Status::Problem, Reason::PointerMalformed),
    ] {
        let pointer = Some(Ok(json!({"malformed":malformed,"state":state})));
        let check = pointer_check(&pointer, Blocker::None);
        assert_eq!(check.status, status);
        assert_eq!(check.reason, reason);
    }
}

#[test]
fn path_arguments_compare_components_without_widening_scope() {
    let expected = PathBuf::from(if cfg!(windows) {
        r"C:\Precept\runtime"
    } else {
        "/precept/runtime"
    });
    let equivalent = if cfg!(windows) {
        "C:/Precept//runtime"
    } else {
        "/precept//runtime"
    };
    let different = if cfg!(windows) {
        "C:/Precept/other"
    } else {
        "/precept/other"
    };
    assert!(same_path_argument(&expected, equivalent));
    assert!(!same_path_argument(&expected, different));
    assert!(!same_path_argument(&expected, "runtime"));
}

#[cfg(windows)]
#[test]
fn project_mcp_accepts_equivalent_windows_separator_spellings() {
    let temp = TestDir::new("mcp-paths");
    let runtime = temp.0.join("runtime");
    precept_ipc::transport::private_dir(&runtime, true).unwrap();
    let location = Location {
        runtime_dir: PathBuf::from(runtime.to_string_lossy().replace('\\', "/")),
        endpoint: "doctor".into(),
    };
    let root = location.root().unwrap();
    let capability = location.credential_path(Some("alpha")).unwrap();
    let native_runtime = runtime.to_string_lossy().replace('/', "\\");
    let slash_capability = capability.to_string_lossy().replace('\\', "/");
    assert!(same_path_argument(&location.runtime_dir, &native_runtime));
    assert!(same_path_argument(&capability, &slash_capability));
    let value = json!({
        "executable":"precept-mcp.exe",
        "hook":"precept-hook.exe",
        "shared":true,
        "args":["--runtime-dir",native_runtime,"--endpoint","doctor"],
        "project_args":["--project","alpha","--capability-file",slash_capability]
    });
    let checks = project_mcp_check(
        &Some(Ok(value.clone())),
        Some("alpha"),
        &location,
        Some(&root),
        Blocker::None,
    );
    assert_eq!(find(&checks, "project.mcp").status, Status::Ok);

    let mut different = value;
    different["args"][1] = json!(runtime.join("other").to_string_lossy().replace('\\', "/"));
    let checks = project_mcp_check(
        &Some(Ok(different)),
        Some("alpha"),
        &location,
        Some(&root),
        Blocker::None,
    );
    assert_eq!(find(&checks, "project.mcp").status, Status::Problem);
    assert_eq!(
        find(&checks, "project.mcp").reason,
        Reason::ProjectMcpMismatch
    );
}

fn write_package(temp: &TestDir) -> PathBuf {
    let root = temp.0.join("Precept");
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("runtime")).unwrap();
    let current = root.join("bin/precept.exe");
    fs::write(&current, b"test").unwrap();
    let mut files = Vec::new();
    let mut binaries = Vec::new();
    for name in expected_binaries(false) {
        let file = format!("bin/{name}");
        fs::write(root.join(&file), b"").unwrap();
        files.push(json!({"file":file,"size":0,"sha256":"0".repeat(64)}));
        binaries.push(json!({"file":file,"size":0,"sha256":"0".repeat(64)}));
    }
    let manifest = json!({
        "format":2,
        "layout":"Precept/{bin,runtime,notices}",
        "version":PRODUCT_VERSION,
        "files":files,
        "binaries":binaries
    });
    fs::write(
        root.join("runtime/manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    current
}

#[test]
fn package_layers_are_bounded_redacted_and_do_not_claim_hash_verification() {
    let temp = TestDir::new("package");
    let absent = temp.0.join("absent/bin/precept.exe");
    let checks = package_checks_at(&absent);
    assert_eq!(find(&checks, "package.manifest").status, Status::Unknown);

    let current = write_package(&temp);
    let checks = package_checks_at(&current);
    assert_eq!(find(&checks, "package.manifest").status, Status::Ok);
    assert_eq!(find(&checks, "package.version").status, Status::Ok);
    assert_eq!(find(&checks, "package.inventory").status, Status::Ok);
    assert_eq!(
        find(&checks, "package.inventory")
            .facts
            .as_ref()
            .unwrap()
            .content_hashes_verified,
        Some(false)
    );

    let manifest_path = current.parent().unwrap().join("../runtime/manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["version"] = json!("SECRET-PACKAGE-VERSION-CANARY");
    manifest["source_sha"] = json!("SECRET-PACKAGE-PATH-CANARY");
    fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let checks = package_checks_at(&current);
    let output = serde_json::to_string(&checks).unwrap();
    assert_eq!(
        find(&checks, "package.version").reason,
        Reason::PackageVersionIncompatible
    );
    assert!(!output.contains("SECRET-PACKAGE-VERSION-CANARY"));
    assert!(!output.contains("SECRET-PACKAGE-PATH-CANARY"));

    fs::write(
        current
            .parent()
            .unwrap()
            .join(format!("preceptd{}", std::env::consts::EXE_SUFFIX)),
        b"changed",
    )
    .unwrap();
    assert_eq!(
        find(&package_checks_at(&current), "package.inventory").reason,
        Reason::PackageSizeMismatch
    );
    fs::write(&manifest_path, b"{SECRET-RAW-MANIFEST-CANARY").unwrap();
    let checks = package_checks_at(&current);
    assert_eq!(
        find(&checks, "package.manifest").reason,
        Reason::ManifestInvalid
    );
    assert!(
        !serde_json::to_string(&checks)
            .unwrap()
            .contains("SECRET-RAW-MANIFEST-CANARY")
    );
}

#[test]
fn an_entry_whose_program_is_gone_is_a_problem_with_one_remediation() {
    let hooks = Some(Ok(json!({
        "available":true,
        "hooks":[
            {"vendor":"claude","installed":true,"current":false,"malformed":false,"broken":true},
            {"vendor":"codex","installed":true,"current":true,"malformed":false,"broken":false}
        ],
        "gate_hooks":[]
    })));
    let checks = hook_checks(&hooks, Blocker::None);
    let claude = find(&checks, "integration.hooks.claude");
    assert_eq!(claude.status, Status::Problem);
    assert_eq!(claude.reason, Reason::HookBroken);
    assert_eq!(claude.remediation, Remediation::UpdateAgentEntries);
    assert_eq!(find(&checks, "integration.hooks.codex").status, Status::Ok);
    let mcp = Some(Ok(json!({"available":true,"clients":[
        {"client":"claude","installed":true,"current":false,"malformed":false,"broken":true}
    ]})));
    let checks = mcp_checks(&mcp, Blocker::None);
    assert_eq!(
        find(&checks, "integration.mcp.claude").reason,
        Reason::McpBroken
    );
}
