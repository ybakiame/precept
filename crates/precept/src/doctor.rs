//! Bounded, read-only installation diagnostics. Every emitted string is either
//! a fixed report token or a known-safe version/schema value; paths, operating
//! system errors, credentials, commands and Profile contents never enter a report.

use precept_ipc::{
    API_VERSION, Access, Client, Error, Location, Result, load_credential, valid_project,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const FORMAT: &str = "precept-doctor";
const VERSION: u32 = 1;
const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");
const PLATFORM: &str = if cfg!(windows) { "windows" } else { "linux" };
const CURRENT_SCHEMA: &str = "Version1";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const TOTAL_IPC_TIMEOUT: Duration = Duration::from_secs(12);
const MAX_MANIFEST_BYTES: u64 = 256 * 1024;
const MAX_MANIFEST_FILES: usize = 512;
const MAX_MANIFEST_BINARIES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Ok,
    Attention,
    Problem,
    Off,
    Unknown,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Attention => "attention",
            Self::Problem => "problem",
            Self::Off => "off",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Reason {
    ExecutableOk,
    ExecutableMissing,
    ExecutableUnavailable,
    ExecutableUnsafe,
    RuntimeOk,
    RuntimeUnsafe,
    RuntimeNotInitialized,
    EndpointOk,
    EndpointInvalid,
    EndpointUnknown,
    EndpointCollision,
    CredentialOk,
    CredentialNotIssued,
    CredentialInvalid,
    CredentialUnsafe,
    CredentialWrongRole,
    DaemonOk,
    DaemonUnavailable,
    DaemonAuthorizationFailed,
    DaemonApiIncompatible,
    DaemonNotReady,
    DaemonServiceMismatch,
    DaemonRequestFailed,
    DaemonNotChecked,
    ApiCompatible,
    ApiIncompatible,
    ApiUnknown,
    ProductCompatible,
    ProductIncompatible,
    ProductUnknown,
    ProfileSelected,
    ProfileNotSelected,
    ProfileStartupFailed,
    ProfileStateMismatch,
    ProfileStateUnknown,
    SchemaCompatible,
    SchemaIncompatible,
    SchemaUnselected,
    SchemaUnknown,
    WritesEnabled,
    WritesDisabled,
    WritesUnknown,
    ProjectsListed,
    ProjectsListUnavailable,
    ProjectBound,
    ProjectNotRequested,
    ProjectNameInvalid,
    ProjectNotFound,
    ProjectReady,
    ProjectUnregistered,
    ProjectUnavailable,
    ProjectStateUnknown,
    ProjectMcpAvailable,
    ProjectMcpUnavailable,
    ProjectMcpMismatch,
    SharingEnabled,
    SharingDisabled,
    SharingUnknown,
    ProposeEnabled,
    ProposeDisabled,
    ProposeRequiresWrites,
    ProposeUnknown,
    GateConfigured,
    GateDisabled,
    GateUnknown,
    AgentCredentialOk,
    AgentCredentialNotIssued,
    AgentCredentialWithheld,
    AgentCredentialInvalid,
    AgentCredentialUnsafe,
    AgentCredentialWrongRole,
    AgentCredentialUnavailable,
    McpAvailable,
    McpUnavailable,
    McpCurrent,
    McpNotInstalled,
    McpOutdated,
    McpMalformed,
    McpStateUnknown,
    HookAvailable,
    HookUnavailable,
    HookCurrent,
    HookNotInstalled,
    HookOutdated,
    HookMalformed,
    HookStateUnknown,
    PointerCurrent,
    PointerOutdated,
    PointerAbsent,
    PointerMalformed,
    PointerStateUnknown,
    ManifestPresent,
    ManifestAbsent,
    ManifestUnreadable,
    ManifestInvalid,
    ManifestUnavailable,
    PackageVersionCompatible,
    PackageVersionIncompatible,
    PackageVersionUnknown,
    PackageMetadataConsistent,
    PackageInventoryMismatch,
    PackageSizeMismatch,
    PackageHashMetadataInvalid,
    PackageMetadataUnknown,
    ResponseInvalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Remediation {
    None,
    ReinstallPackage,
    UseAbsolutePrivateRuntime,
    CorrectEndpoint,
    StartDaemonExplicitly,
    RestartDaemonExplicitly,
    RepairOperatorCredential,
    ReinstallMatchingRelease,
    SelectProfileExplicitly,
    ReviewStartupProfile,
    ReviewWriteGrant,
    RegisterProjectExplicitly,
    RestoreProjectPath,
    ReviewProject,
    ReviewProjectMcp,
    ReviewProjectSharing,
    ReviewPropose,
    ReviewGate,
    ReviewAgentCredential,
    ReviewMcpIntegration,
    ReviewHookIntegration,
    ReviewPointer,
    ReextractPackage,
    UseSupportedProfile,
    ReviewInstallation,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "snake_case")]
struct Facts {
    #[serde(skip_serializing_if = "Option::is_none")]
    available: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    installed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    malformed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    write_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shared: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    propose: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    setup_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_requested: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_api_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    product_version: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_product_version: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_schema: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manifest_format: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    binary_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sizes_match: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hashes_recorded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_hashes_verified: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
struct Check {
    id: &'static str,
    status: Status,
    reason: Reason,
    remediation: Remediation,
    #[serde(skip_serializing_if = "Option::is_none")]
    facts: Option<Facts>,
}

impl Check {
    fn new(id: &'static str, status: Status, reason: Reason, remediation: Remediation) -> Self {
        Self {
            id,
            status,
            reason,
            remediation,
            facts: None,
        }
    }

    fn with_facts(mut self, facts: Facts) -> Self {
        self.facts = Some(facts);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum OverallStatus {
    Ok,
    Attention,
    Problem,
    Incomplete,
}

#[derive(Debug, Serialize)]
struct Summary {
    status: OverallStatus,
    ok: usize,
    attention: usize,
    problem: usize,
    off: usize,
    unknown: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    format: &'static str,
    version: u32,
    product_version: &'static str,
    platform: &'static str,
    summary: Summary,
    checks: Vec<Check>,
}

impl Report {
    fn new(checks: Vec<Check>) -> Self {
        let count = |status| checks.iter().filter(|check| check.status == status).count();
        let problem = count(Status::Problem);
        let unknown = count(Status::Unknown);
        let attention = count(Status::Attention);
        let status = if problem > 0 {
            OverallStatus::Problem
        } else if unknown > 0 {
            OverallStatus::Incomplete
        } else if attention > 0 {
            OverallStatus::Attention
        } else {
            OverallStatus::Ok
        };
        Self {
            format: FORMAT,
            version: VERSION,
            product_version: PRODUCT_VERSION,
            platform: PLATFORM,
            summary: Summary {
                status,
                ok: count(Status::Ok),
                attention,
                problem,
                off: count(Status::Off),
                unknown,
            },
            checks,
        }
    }

    fn render(&self, json: bool) -> String {
        if json {
            return serde_json::to_string(self).unwrap_or_else(|_| {
                json!({
                    "format": FORMAT,
                    "version": VERSION,
                    "product_version": PRODUCT_VERSION,
                    "platform": PLATFORM,
                    "summary": {"status":"problem","ok":0,"attention":0,"problem":1,"off":0,"unknown":0},
                    "checks": [{"id":"report.render","status":"problem","reason":"response_invalid","remediation":"review_installation"}]
                })
                .to_string()
            });
        }
        let mut out = format!("Precept doctor {PLATFORM} {PRODUCT_VERSION}\n");
        for check in &self.checks {
            out.push_str(&format!(
                "[{}] {}: {} -> {}\n",
                check.status.as_str(),
                check.id,
                serde_json::to_value(check.reason)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_else(|| "response_invalid".into()),
                serde_json::to_value(check.remediation)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_else(|| "review_installation".into()),
            ));
        }
        out.push_str(&format!(
            "summary: {}; problem={}; unknown={}; attention={}; off={}; ok={}\n",
            serde_json::to_value(self.summary.status)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "problem".into()),
            self.summary.problem,
            self.summary.unknown,
            self.summary.attention,
            self.summary.off,
            self.summary.ok,
        ));
        out
    }
}

/// Diagnose one explicit installation without starting, selecting, enabling,
/// repairing or scanning anything. Failures are report rows, never a `Result`.
pub async fn run(location: &Location, project: Option<&str>, json: bool) -> String {
    let mut checks = executable_checks();
    let root = location.root();
    checks.push(runtime_check(&root, &location.runtime_dir));
    let endpoint_ok = endpoint_valid(location, root.as_deref().ok());
    checks.push(endpoint_check(endpoint_ok, root.is_ok()));

    let credential = root
        .as_ref()
        .ok()
        .and_then(|_| location.credential_path(None).ok())
        .map(|path| {
            let exists = fs::symlink_metadata(&path).is_ok();
            match load_credential(location, &path) {
                Ok(cap) => Ok((cap, exists)),
                Err(error) => Err((error, exists)),
            }
        });
    checks.push(operator_credential_check(credential.as_ref()));
    let operator_ready = credential
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .is_some_and(|(cap, _)| cap.access == Access::Operator);

    let project_valid = project.is_none_or(valid_project);
    let deadline = Instant::now() + TOTAL_IPC_TIMEOUT;
    let mut health = None;
    let mut profile = None;
    let mut projects = None;
    let mut mcp = None;
    let mut project_mcp = None;
    let mut hooks = None;
    let mut pointer = None;
    if operator_ready && endpoint_ok {
        health = Some(request(location, deadline, "daemon.health", json!({})).await);
        if health.as_ref().is_some_and(Result::is_ok) {
            profile = Some(request(location, deadline, "profile.info", json!({})).await);
            projects = Some(request(location, deadline, "project.list", json!({})).await);
            mcp = Some(request(location, deadline, "integration.mcp", json!({})).await);
            if project_valid && let Some(name) = project {
                project_mcp =
                    Some(request(location, deadline, "project.mcp", json!({"name":name})).await);
                hooks = Some(
                    request(
                        location,
                        deadline,
                        "integration.hooks",
                        json!({"project":name}),
                    )
                    .await,
                );
                pointer = Some(
                    request(
                        location,
                        deadline,
                        "integration.pointer",
                        json!({"project":name}),
                    )
                    .await,
                );
            }
        }
    }

    let blocker = if root.is_err() {
        match fs::symlink_metadata(&location.runtime_dir) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Blocker::RuntimeAbsent,
            _ => Blocker::Runtime,
        }
    } else if !endpoint_ok {
        Blocker::Endpoint
    } else if !operator_ready {
        Blocker::Credential
    } else {
        match health.as_ref() {
            Some(Err(error)) => Blocker::Daemon(*error),
            _ => Blocker::None,
        }
    };
    checks.extend(daemon_checks(&health, blocker));
    let writes = profile
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|value| value.get("write_enabled"))
        .and_then(Value::as_bool);
    checks.extend(profile_checks(&profile, &health, blocker));
    let project_checks = project_checks(&projects, project, project_valid, writes, blocker);
    let selected_project = project_checks
        .iter()
        .find(|check| check.id == "project.state")
        .is_some_and(|check| check.status == Status::Ok);
    let sharing = project_checks
        .iter()
        .find(|check| check.id == "project.sharing")
        .is_some_and(|check| check.status == Status::Ok);
    checks.extend(project_checks);
    checks.extend(project_mcp_check(
        &project_mcp,
        project,
        location,
        root.as_deref().ok(),
        blocker,
    ));
    checks.extend(agent_credential_check(
        location,
        root.as_deref().ok(),
        project,
        selected_project,
        sharing,
        blocker,
    ));
    checks.extend(integration_checks(
        &mcp,
        &hooks,
        &pointer,
        project,
        project_valid,
        blocker,
    ));
    if let Ok(exe) = std::env::current_exe() {
        checks.extend(package_checks_at(&exe));
    } else {
        checks.extend(package_checks_at(Path::new("")));
    }
    Report::new(checks).render(json)
}

#[derive(Clone, Copy)]
enum Blocker {
    None,
    Runtime,
    RuntimeAbsent,
    Endpoint,
    Credential,
    Daemon(Error),
}

fn not_checked(id: &'static str, blocker: Blocker) -> Check {
    let (reason, remediation) = match blocker {
        Blocker::Runtime => (
            Reason::DaemonNotChecked,
            Remediation::UseAbsolutePrivateRuntime,
        ),
        Blocker::RuntimeAbsent => (Reason::DaemonNotChecked, Remediation::StartDaemonExplicitly),
        Blocker::Endpoint => (Reason::DaemonNotChecked, Remediation::CorrectEndpoint),
        Blocker::Credential => (
            Reason::DaemonNotChecked,
            Remediation::RepairOperatorCredential,
        ),
        Blocker::Daemon(error) => daemon_failure(error),
        Blocker::None => (Reason::DaemonNotChecked, Remediation::ReviewInstallation),
    };
    Check::new(id, Status::Unknown, reason, remediation)
}

async fn request(
    location: &Location,
    deadline: Instant,
    method: &'static str,
    params: Value,
) -> Result<Value> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(Error::Timeout);
    }
    tokio::time::timeout(remaining.min(REQUEST_TIMEOUT), async move {
        let credential = load_credential(location, &location.credential_path(None)?)?;
        if credential.access != Access::Operator {
            return Err(Error::Unauthorized);
        }
        let mut client = Client::connect(location, &credential).await?;
        client.request(method, params).await
    })
    .await
    .unwrap_or(Err(Error::Timeout))
}

fn runtime_check(root: &Result<PathBuf>, runtime_dir: &Path) -> Check {
    match root {
        Ok(_) => Check::new(
            "runtime.location",
            Status::Ok,
            Reason::RuntimeOk,
            Remediation::None,
        ),
        Err(_) => {
            let absent = runtime_dir.is_absolute()
                && matches!(
                    fs::symlink_metadata(runtime_dir),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound
                );
            if absent {
                Check::new(
                    "runtime.location",
                    Status::Unknown,
                    Reason::RuntimeNotInitialized,
                    Remediation::StartDaemonExplicitly,
                )
            } else {
                Check::new(
                    "runtime.location",
                    Status::Problem,
                    Reason::RuntimeUnsafe,
                    Remediation::UseAbsolutePrivateRuntime,
                )
            }
        }
    }
}

fn endpoint_valid(location: &Location, root: Option<&Path>) -> bool {
    #[cfg(windows)]
    {
        let _ = root;
        !location.endpoint.is_empty()
            && location.endpoint.len() <= 48
            && location
                .endpoint
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
    }
    #[cfg(not(windows))]
    {
        let endpoint = Path::new(&location.endpoint);
        endpoint.is_absolute()
            && !location.endpoint.contains('\0')
            && location.endpoint.len() <= 100
            && root.is_some_and(|root| {
                endpoint
                    .parent()
                    .and_then(|parent| parent.canonicalize().ok())
                    .as_ref()
                    == Some(&root.to_path_buf())
            })
    }
}

fn endpoint_check(valid: bool, runtime_observed: bool) -> Check {
    match (valid, runtime_observed) {
        (true, _) => Check::new(
            "endpoint.location",
            Status::Ok,
            Reason::EndpointOk,
            Remediation::None,
        ),
        (false, false) => Check::new(
            "endpoint.location",
            Status::Unknown,
            Reason::EndpointUnknown,
            Remediation::CorrectEndpoint,
        ),
        (false, true) => Check::new(
            "endpoint.location",
            Status::Problem,
            Reason::EndpointInvalid,
            Remediation::CorrectEndpoint,
        ),
    }
}

fn executable_checks() -> Vec<Check> {
    match std::env::current_exe() {
        Ok(path) => executable_checks_at(Ok(path)),
        Err(_) => vec![Check::new(
            "executable.precept",
            Status::Problem,
            Reason::ExecutableUnavailable,
            Remediation::ReinstallPackage,
        )],
    }
}

fn executable_checks_at(current: Result<PathBuf>) -> Vec<Check> {
    let Ok(current) = current else {
        return vec![Check::new(
            "executable.precept",
            Status::Problem,
            Reason::ExecutableUnavailable,
            Remediation::ReinstallPackage,
        )];
    };
    let mut checks = vec![executable_check("executable.precept", &current)];
    let Some(parent) = current.parent() else {
        return checks;
    };
    for (id, name) in [
        ("executable.preceptd", "preceptd"),
        ("executable.precept_mcp", "precept-mcp"),
        ("executable.precept_hook", "precept-hook"),
    ] {
        checks.push(executable_check(
            id,
            &parent.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
        ));
    }
    let bridge = parent.join(format!("precept-bridge{}", std::env::consts::EXE_SUFFIX));
    if fs::symlink_metadata(&bridge).is_ok() {
        checks.push(executable_check("executable.precept_bridge", &bridge));
    }
    checks
}

fn executable_check(id: &'static str, path: &Path) -> Check {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_file() && !meta.file_type().is_symlink() => {
            Check::new(id, Status::Ok, Reason::ExecutableOk, Remediation::None)
        }
        Ok(_) => Check::new(
            id,
            Status::Problem,
            Reason::ExecutableUnsafe,
            Remediation::ReinstallPackage,
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Check::new(
            id,
            Status::Problem,
            Reason::ExecutableMissing,
            Remediation::ReinstallPackage,
        ),
        Err(_) => Check::new(
            id,
            Status::Problem,
            Reason::ExecutableUnavailable,
            Remediation::ReinstallPackage,
        ),
    }
}

type CredentialObservation = std::result::Result<(precept_ipc::Credential, bool), (Error, bool)>;

fn operator_credential_check(credential: Option<&CredentialObservation>) -> Check {
    let Some(credential) = credential else {
        return operator_credential_outcome(None, None, false);
    };
    match credential {
        Ok((cap, exists)) => operator_credential_outcome(Some(&cap.access), None, *exists),
        Err((error, exists)) => operator_credential_outcome(None, Some(*error), *exists),
    }
}

fn operator_credential_outcome(
    access: Option<&Access>,
    error: Option<Error>,
    exists: bool,
) -> Check {
    match (access, error) {
        (Some(Access::Operator), _) => Check::new(
            "credential.operator",
            Status::Ok,
            Reason::CredentialOk,
            Remediation::None,
        ),
        (Some(_), _) => Check::new(
            "credential.operator",
            Status::Problem,
            Reason::CredentialWrongRole,
            Remediation::RestartDaemonExplicitly,
        ),
        (None, Some(Error::UnsafeRuntime)) => Check::new(
            "credential.operator",
            Status::Problem,
            Reason::CredentialUnsafe,
            Remediation::RestartDaemonExplicitly,
        ),
        (None, Some(_)) if !exists => Check::new(
            "credential.operator",
            Status::Unknown,
            Reason::CredentialNotIssued,
            Remediation::StartDaemonExplicitly,
        ),
        (None, Some(_)) => Check::new(
            "credential.operator",
            Status::Problem,
            Reason::CredentialInvalid,
            Remediation::RestartDaemonExplicitly,
        ),
        (None, None) => Check::new(
            "credential.operator",
            Status::Unknown,
            if exists {
                Reason::CredentialInvalid
            } else {
                Reason::CredentialNotIssued
            },
            if exists {
                Remediation::RestartDaemonExplicitly
            } else {
                Remediation::StartDaemonExplicitly
            },
        ),
    }
}

fn daemon_failure(error: Error) -> (Reason, Remediation) {
    match error {
        Error::Unavailable | Error::Timeout | Error::NotFound => (
            Reason::DaemonUnavailable,
            Remediation::StartDaemonExplicitly,
        ),
        Error::Unauthorized => (
            Reason::DaemonAuthorizationFailed,
            Remediation::RepairOperatorCredential,
        ),
        Error::Version => (
            Reason::DaemonApiIncompatible,
            Remediation::ReinstallMatchingRelease,
        ),
        Error::UnsafeRuntime => (
            Reason::RuntimeUnsafe,
            Remediation::UseAbsolutePrivateRuntime,
        ),
        Error::EndpointCollision => (Reason::EndpointCollision, Remediation::CorrectEndpoint),
        _ => (Reason::DaemonRequestFailed, Remediation::ReviewInstallation),
    }
}

fn daemon_health_check(health: &Option<Result<Value>>, blocker: Blocker) -> Check {
    let Some(health) = health else {
        return not_checked("daemon.health", blocker);
    };
    match health {
        Err(error) => {
            let (reason, remediation) = daemon_failure(*error);
            let status = if matches!(error, Error::Unavailable | Error::Timeout | Error::NotFound) {
                Status::Unknown
            } else {
                Status::Problem
            };
            Check::new("daemon.health", status, reason, remediation)
        }
        Ok(value) => match (
            value.get("ready").and_then(Value::as_bool),
            value.get("service").and_then(Value::as_str),
        ) {
            (Some(true), Some("preceptd")) => Check::new(
                "daemon.health",
                Status::Ok,
                Reason::DaemonOk,
                Remediation::None,
            ),
            (Some(false), _) => Check::new(
                "daemon.health",
                Status::Problem,
                Reason::DaemonNotReady,
                Remediation::ReviewInstallation,
            ),
            (_, Some(_)) => Check::new(
                "daemon.health",
                Status::Problem,
                Reason::DaemonServiceMismatch,
                Remediation::ReinstallMatchingRelease,
            ),
            _ => Check::new(
                "daemon.health",
                Status::Problem,
                Reason::ResponseInvalid,
                Remediation::ReinstallMatchingRelease,
            ),
        },
    }
}

fn daemon_api_check(health: &Option<Result<Value>>, blocker: Blocker) -> Check {
    let Some(health) = health else {
        return not_checked("daemon.api", blocker);
    };
    match health {
        Err(error) => Check::new(
            "daemon.api",
            Status::Unknown,
            Reason::ApiUnknown,
            daemon_failure(*error).1,
        ),
        Ok(value) => match value.get("api_version").and_then(Value::as_u64) {
            Some(version) if version == u64::from(API_VERSION) => Check::new(
                "daemon.api",
                Status::Ok,
                Reason::ApiCompatible,
                Remediation::None,
            )
            .with_facts(Facts {
                api_version: Some(API_VERSION),
                expected_api_version: Some(API_VERSION),
                ..Facts::default()
            }),
            Some(_) => Check::new(
                "daemon.api",
                Status::Problem,
                Reason::ApiIncompatible,
                Remediation::ReinstallMatchingRelease,
            )
            .with_facts(Facts {
                expected_api_version: Some(API_VERSION),
                ..Facts::default()
            }),
            None => Check::new(
                "daemon.api",
                Status::Problem,
                Reason::ResponseInvalid,
                Remediation::ReinstallMatchingRelease,
            ),
        },
    }
}

fn daemon_product_check(health: &Option<Result<Value>>, blocker: Blocker) -> Check {
    let Some(health) = health else {
        return not_checked("daemon.product", blocker);
    };
    match health {
        Err(error) => Check::new(
            "daemon.product",
            Status::Unknown,
            Reason::ProductUnknown,
            daemon_failure(*error).1,
        ),
        Ok(value) => match value.get("version").and_then(Value::as_str) {
            Some(version) if version == PRODUCT_VERSION => Check::new(
                "daemon.product",
                Status::Ok,
                Reason::ProductCompatible,
                Remediation::None,
            )
            .with_facts(Facts {
                product_version: Some(PRODUCT_VERSION),
                expected_product_version: Some(PRODUCT_VERSION),
                ..Facts::default()
            }),
            Some(_) => Check::new(
                "daemon.product",
                Status::Problem,
                Reason::ProductIncompatible,
                Remediation::ReinstallMatchingRelease,
            )
            .with_facts(Facts {
                expected_product_version: Some(PRODUCT_VERSION),
                ..Facts::default()
            }),
            None => Check::new(
                "daemon.product",
                Status::Problem,
                Reason::ProductUnknown,
                Remediation::ReinstallMatchingRelease,
            ),
        },
    }
}

fn profile_schema_check(health: &Option<Result<Value>>, blocker: Blocker) -> Check {
    let Some(health) = health else {
        return not_checked("profile.schema", blocker);
    };
    let value = match health {
        Ok(value) => value,
        Err(error) => {
            return Check::new(
                "profile.schema",
                Status::Unknown,
                Reason::SchemaUnknown,
                daemon_failure(*error).1,
            );
        }
    };
    match value.get("profile_selected").and_then(Value::as_bool) {
        Some(false) => Check::new(
            "profile.schema",
            Status::Off,
            Reason::SchemaUnselected,
            Remediation::SelectProfileExplicitly,
        ),
        Some(true) => match value.get("schema").and_then(Value::as_str) {
            Some(CURRENT_SCHEMA) => Check::new(
                "profile.schema",
                Status::Ok,
                Reason::SchemaCompatible,
                Remediation::None,
            )
            .with_facts(Facts {
                schema: Some(CURRENT_SCHEMA),
                expected_schema: Some(CURRENT_SCHEMA),
                ..Facts::default()
            }),
            Some(_) => Check::new(
                "profile.schema",
                Status::Problem,
                Reason::SchemaIncompatible,
                Remediation::UseSupportedProfile,
            )
            .with_facts(Facts {
                expected_schema: Some(CURRENT_SCHEMA),
                ..Facts::default()
            }),
            None => Check::new(
                "profile.schema",
                Status::Unknown,
                Reason::SchemaUnknown,
                Remediation::UseSupportedProfile,
            ),
        },
        None => Check::new(
            "profile.schema",
            Status::Problem,
            Reason::ResponseInvalid,
            Remediation::ReinstallMatchingRelease,
        ),
    }
}

fn daemon_checks(health: &Option<Result<Value>>, blocker: Blocker) -> Vec<Check> {
    vec![
        daemon_health_check(health, blocker),
        daemon_api_check(health, blocker),
        daemon_product_check(health, blocker),
        profile_schema_check(health, blocker),
    ]
}

fn profile_checks(
    profile: &Option<Result<Value>>,
    health: &Option<Result<Value>>,
    blocker: Blocker,
) -> Vec<Check> {
    let Some(profile) = profile else {
        return vec![
            not_checked("profile.selected", blocker),
            not_checked("profile.writes", blocker),
        ];
    };
    let value = match profile {
        Ok(value) => value,
        Err(error) => {
            return vec![
                Check::new(
                    "profile.selected",
                    if matches!(error, Error::Unavailable | Error::Timeout) {
                        Status::Unknown
                    } else {
                        Status::Problem
                    },
                    Reason::ProfileStateUnknown,
                    daemon_failure(*error).1,
                ),
                Check::new(
                    "profile.writes",
                    Status::Unknown,
                    Reason::WritesUnknown,
                    daemon_failure(*error).1,
                ),
            ];
        }
    };
    let selected = value.get("selected").and_then(Value::as_bool);
    let health_selected = health
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|value| value.get("profile_selected"))
        .and_then(Value::as_bool);
    let mismatch = matches!((selected, health_selected), (Some(a), Some(b)) if a != b);
    let selected_check = match (selected, mismatch) {
        (Some(true), false) => Check::new(
            "profile.selected",
            Status::Ok,
            Reason::ProfileSelected,
            Remediation::None,
        )
        .with_facts(Facts {
            selected: Some(true),
            ..Facts::default()
        }),
        (Some(false), false) if !value.get("startup_failure").is_none_or(Value::is_null) => {
            Check::new(
                "profile.selected",
                Status::Problem,
                Reason::ProfileStartupFailed,
                Remediation::ReviewStartupProfile,
            )
            .with_facts(Facts {
                selected: Some(false),
                ..Facts::default()
            })
        }
        (Some(false), false) => Check::new(
            "profile.selected",
            Status::Off,
            Reason::ProfileNotSelected,
            Remediation::SelectProfileExplicitly,
        )
        .with_facts(Facts {
            selected: Some(false),
            ..Facts::default()
        }),
        (Some(_), true) => Check::new(
            "profile.selected",
            Status::Problem,
            Reason::ProfileStateMismatch,
            Remediation::ReviewInstallation,
        ),
        _ => Check::new(
            "profile.selected",
            Status::Problem,
            Reason::ResponseInvalid,
            Remediation::ReinstallMatchingRelease,
        ),
    };
    let writes_check = match value.get("write_enabled").and_then(Value::as_bool) {
        Some(true) => Check::new(
            "profile.writes",
            Status::Ok,
            Reason::WritesEnabled,
            Remediation::None,
        )
        .with_facts(Facts {
            write_enabled: Some(true),
            ..Facts::default()
        }),
        Some(false) => Check::new(
            "profile.writes",
            Status::Off,
            Reason::WritesDisabled,
            Remediation::ReviewWriteGrant,
        )
        .with_facts(Facts {
            write_enabled: Some(false),
            ..Facts::default()
        }),
        None => Check::new(
            "profile.writes",
            Status::Problem,
            Reason::ResponseInvalid,
            Remediation::ReinstallMatchingRelease,
        ),
    };
    vec![selected_check, writes_check]
}

fn project_refused(count: Check, binding: Check) -> Vec<Check> {
    let mut checks = vec![count, binding];
    checks.extend(unknown_project_capabilities());
    checks
}

fn project_checks(
    projects: &Option<Result<Value>>,
    project: Option<&str>,
    project_valid: bool,
    writes: Option<bool>,
    blocker: Blocker,
) -> Vec<Check> {
    let count_check = match projects {
        Some(Ok(Value::Array(entries))) => Check::new(
            "project.count",
            Status::Ok,
            Reason::ProjectsListed,
            Remediation::None,
        )
        .with_facts(Facts {
            count: Some(entries.len()),
            ..Facts::default()
        }),
        Some(Ok(_)) => Check::new(
            "project.count",
            Status::Problem,
            Reason::ResponseInvalid,
            Remediation::ReinstallMatchingRelease,
        ),
        Some(Err(error)) => Check::new(
            "project.count",
            if matches!(error, Error::Unavailable | Error::Timeout) {
                Status::Unknown
            } else {
                Status::Problem
            },
            Reason::ProjectsListUnavailable,
            daemon_failure(*error).1,
        ),
        None => not_checked("project.count", blocker),
    };
    let Some(name) = project else {
        return vec![
            count_check,
            Check::new(
                "project.binding",
                Status::Off,
                Reason::ProjectNotRequested,
                Remediation::None,
            )
            .with_facts(Facts {
                project_requested: Some(false),
                ..Facts::default()
            }),
        ];
    };
    if !project_valid {
        return project_refused(
            count_check,
            Check::new(
                "project.binding",
                Status::Problem,
                Reason::ProjectNameInvalid,
                Remediation::ReviewProject,
            ),
        );
    }
    let entries = match projects {
        Some(Ok(Value::Array(entries))) => entries,
        Some(Ok(_)) => {
            return project_refused(
                count_check,
                Check::new(
                    "project.binding",
                    Status::Problem,
                    Reason::ResponseInvalid,
                    Remediation::ReinstallMatchingRelease,
                ),
            );
        }
        Some(Err(error)) => {
            return project_refused(
                count_check,
                Check::new(
                    "project.binding",
                    Status::Unknown,
                    Reason::ProjectStateUnknown,
                    daemon_failure(*error).1,
                ),
            );
        }
        None => {
            return project_refused(count_check, not_checked("project.binding", blocker));
        }
    };
    let Some(entry) = entries
        .iter()
        .find(|entry| entry.get("name").and_then(Value::as_str) == Some(name))
    else {
        return project_refused(
            count_check,
            Check::new(
                "project.binding",
                Status::Problem,
                Reason::ProjectNotFound,
                Remediation::RegisterProjectExplicitly,
            ),
        );
    };
    let bound = Check::new(
        "project.binding",
        Status::Ok,
        Reason::ProjectBound,
        Remediation::None,
    );
    let state = match entry.get("state").and_then(Value::as_str) {
        Some("ready") => Check::new(
            "project.state",
            Status::Ok,
            Reason::ProjectReady,
            Remediation::None,
        ),
        Some("unregistered") => Check::new(
            "project.state",
            Status::Problem,
            Reason::ProjectUnregistered,
            Remediation::RegisterProjectExplicitly,
        ),
        Some("unavailable") => Check::new(
            "project.state",
            Status::Problem,
            Reason::ProjectUnavailable,
            Remediation::RestoreProjectPath,
        ),
        Some(_) => Check::new(
            "project.state",
            Status::Unknown,
            Reason::ProjectStateUnknown,
            Remediation::ReviewProject,
        ),
        None => Check::new(
            "project.state",
            Status::Problem,
            Reason::ResponseInvalid,
            Remediation::ReinstallMatchingRelease,
        ),
    };
    let shared = boolean_capability(
        "project.sharing",
        entry.get("agents").and_then(Value::as_bool),
        Reason::SharingEnabled,
        Reason::SharingDisabled,
        Reason::SharingUnknown,
        Remediation::ReviewProjectSharing,
        |facts, value| facts.shared = Some(value),
    );
    let gate = boolean_capability(
        "project.gate",
        entry.get("gate").and_then(Value::as_bool),
        Reason::GateConfigured,
        Reason::GateDisabled,
        Reason::GateUnknown,
        Remediation::ReviewGate,
        |facts, value| facts.gate = Some(value),
    );
    let gate = if gate.status == Status::Ok {
        gate.with_facts(Facts {
            gate: Some(true),
            setup_only: Some(true),
            ..Facts::default()
        })
    } else {
        gate
    };
    let propose = match (entry.get("propose").and_then(Value::as_bool), writes) {
        (Some(true), Some(true)) => Check::new(
            "project.propose",
            Status::Ok,
            Reason::ProposeEnabled,
            Remediation::None,
        )
        .with_facts(Facts {
            propose: Some(true),
            ..Facts::default()
        }),
        (Some(true), Some(false)) => Check::new(
            "project.propose",
            Status::Attention,
            Reason::ProposeRequiresWrites,
            Remediation::ReviewWriteGrant,
        )
        .with_facts(Facts {
            propose: Some(true),
            write_enabled: Some(false),
            ..Facts::default()
        }),
        (Some(true), None) => Check::new(
            "project.propose",
            Status::Unknown,
            Reason::ProposeUnknown,
            Remediation::ReviewWriteGrant,
        ),
        (Some(false), _) => Check::new(
            "project.propose",
            Status::Off,
            Reason::ProposeDisabled,
            Remediation::ReviewPropose,
        )
        .with_facts(Facts {
            propose: Some(false),
            ..Facts::default()
        }),
        (None, _) => Check::new(
            "project.propose",
            Status::Problem,
            Reason::ResponseInvalid,
            Remediation::ReinstallMatchingRelease,
        ),
    };
    vec![count_check, bound, state, shared, propose, gate]
}

fn unknown_project_capabilities() -> Vec<Check> {
    let unknown = |id, reason, remediation| Check::new(id, Status::Unknown, reason, remediation);
    vec![
        unknown(
            "project.state",
            Reason::ProjectStateUnknown,
            Remediation::ReviewProject,
        ),
        unknown(
            "project.sharing",
            Reason::SharingUnknown,
            Remediation::ReviewProjectSharing,
        ),
        unknown(
            "project.propose",
            Reason::ProposeUnknown,
            Remediation::ReviewPropose,
        ),
        unknown("project.gate", Reason::GateUnknown, Remediation::ReviewGate),
    ]
}

fn boolean_capability(
    id: &'static str,
    value: Option<bool>,
    enabled: Reason,
    disabled: Reason,
    unknown: Reason,
    remediation: Remediation,
    fact: fn(&mut Facts, bool),
) -> Check {
    match value {
        Some(value) => {
            let mut facts = Facts::default();
            fact(&mut facts, value);
            Check::new(
                id,
                if value { Status::Ok } else { Status::Off },
                if value { enabled } else { disabled },
                if value {
                    Remediation::None
                } else {
                    remediation
                },
            )
            .with_facts(facts)
        }
        None => Check::new(id, Status::Unknown, unknown, remediation),
    }
}

#[cfg(windows)]
fn normalized_windows_path(path: &Path) -> PathBuf {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    // Windows accepts both separators, including in verbatim paths.
    let units = path
        .as_os_str()
        .encode_wide()
        .map(|unit| {
            if unit == b'/' as u16 {
                b'\\' as u16
            } else {
                unit
            }
        })
        .collect::<Vec<_>>();
    PathBuf::from(std::ffi::OsString::from_wide(&units))
}

fn same_path_argument(expected: &Path, actual: &str) -> bool {
    #[cfg(windows)]
    {
        let expected = normalized_windows_path(expected);
        let actual = normalized_windows_path(Path::new(actual));
        expected.is_absolute()
            && actual.is_absolute()
            && expected.components().eq(actual.components())
    }
    #[cfg(not(windows))]
    {
        let actual = Path::new(actual);
        expected.is_absolute()
            && actual.is_absolute()
            && expected.components().eq(actual.components())
    }
}

fn project_mcp_check(
    result: &Option<Result<Value>>,
    project: Option<&str>,
    location: &Location,
    root: Option<&Path>,
    blocker: Blocker,
) -> Vec<Check> {
    let Some(name) = project else {
        return vec![];
    };
    let Some(result) = result else {
        return vec![not_checked("project.mcp", blocker)];
    };
    let check = match result {
        Err(Error::NotFound) => Check::new(
            "project.mcp",
            Status::Problem,
            Reason::ProjectNotFound,
            Remediation::RegisterProjectExplicitly,
        ),
        Err(error) => Check::new(
            "project.mcp",
            Status::Unknown,
            Reason::ProjectMcpUnavailable,
            daemon_failure(*error).1,
        ),
        Ok(value) => {
            let capability = root.and_then(|_root| location.credential_path(Some(name)).ok());
            let args_ok = value
                .get("args")
                .and_then(Value::as_array)
                .is_some_and(|args| {
                    args.len() == 4
                        && args[0].as_str() == Some("--runtime-dir")
                        && args[1]
                            .as_str()
                            .is_some_and(|actual| same_path_argument(&location.runtime_dir, actual))
                        && args[2].as_str() == Some("--endpoint")
                        && args[3].as_str() == Some(location.endpoint.as_str())
                });
            let project_args_ok = capability.as_ref().is_some_and(|capability| {
                value
                    .get("project_args")
                    .and_then(Value::as_array)
                    .is_some_and(|args| {
                        args.len() == 4
                            && args[0].as_str() == Some("--project")
                            && args[1].as_str() == Some(name)
                            && args[2].as_str() == Some("--capability-file")
                            && args[3]
                                .as_str()
                                .is_some_and(|actual| same_path_argument(capability, actual))
                    })
            });
            let available = ["executable", "hook"].iter().all(|key| {
                value
                    .get(key)
                    .and_then(Value::as_str)
                    .is_some_and(|text| !text.is_empty())
            });
            let shared = value.get("shared").and_then(Value::as_bool);
            match (available, args_ok, project_args_ok, shared) {
                (false, _, _, _) => Check::new(
                    "project.mcp",
                    Status::Problem,
                    Reason::ProjectMcpUnavailable,
                    Remediation::ReviewProjectMcp,
                ),
                (true, false, _, _) | (true, _, false, _) => Check::new(
                    "project.mcp",
                    Status::Problem,
                    Reason::ProjectMcpMismatch,
                    Remediation::ReviewProjectMcp,
                ),
                (true, true, true, Some(_)) => Check::new(
                    "project.mcp",
                    Status::Ok,
                    Reason::ProjectMcpAvailable,
                    Remediation::None,
                )
                .with_facts(Facts {
                    available: Some(true),
                    shared,
                    ..Facts::default()
                }),
                _ => Check::new(
                    "project.mcp",
                    Status::Problem,
                    Reason::ResponseInvalid,
                    Remediation::ReinstallMatchingRelease,
                ),
            }
        }
    };
    vec![check]
}

fn agent_credential_check(
    location: &Location,
    root: Option<&Path>,
    project: Option<&str>,
    ready: bool,
    shared: bool,
    blocker: Blocker,
) -> Vec<Check> {
    let Some(name) = project else {
        return vec![];
    };
    if !valid_project(name) {
        return vec![Check::new(
            "project.credential",
            Status::Unknown,
            Reason::AgentCredentialUnavailable,
            Remediation::ReviewAgentCredential,
        )];
    }
    if !ready || !shared {
        return vec![Check::new(
            "project.credential",
            if shared && !ready {
                Status::Unknown
            } else {
                Status::Off
            },
            if shared && !ready {
                Reason::AgentCredentialUnavailable
            } else {
                Reason::AgentCredentialWithheld
            },
            if shared && !ready {
                Remediation::ReviewAgentCredential
            } else {
                Remediation::ReviewProjectSharing
            },
        )];
    }
    let Some(root) = root else {
        return vec![not_checked("project.credential", blocker)];
    };
    let path = match location.credential_path(Some(name)) {
        Ok(path) => path,
        Err(_) => {
            return vec![Check::new(
                "project.credential",
                Status::Problem,
                Reason::AgentCredentialInvalid,
                Remediation::ReviewAgentCredential,
            )];
        }
    };
    let _ = root;
    let exists = fs::symlink_metadata(&path).is_ok();
    let check = match load_credential(location, &path) {
        Ok(cap) => match cap.access {
            Access::Agent { project } if project == name => Check::new(
                "project.credential",
                Status::Ok,
                Reason::AgentCredentialOk,
                Remediation::None,
            ),
            _ => Check::new(
                "project.credential",
                Status::Problem,
                Reason::AgentCredentialWrongRole,
                Remediation::RestartDaemonExplicitly,
            ),
        },
        Err(Error::UnsafeRuntime) => Check::new(
            "project.credential",
            Status::Problem,
            Reason::AgentCredentialUnsafe,
            Remediation::RestartDaemonExplicitly,
        ),
        Err(_) if !exists => Check::new(
            "project.credential",
            Status::Unknown,
            Reason::AgentCredentialNotIssued,
            Remediation::RestartDaemonExplicitly,
        ),
        Err(_) => Check::new(
            "project.credential",
            Status::Problem,
            Reason::AgentCredentialInvalid,
            Remediation::RestartDaemonExplicitly,
        ),
    };
    vec![check]
}

fn integration_checks(
    mcp: &Option<Result<Value>>,
    hooks: &Option<Result<Value>>,
    pointer: &Option<Result<Value>>,
    project: Option<&str>,
    project_valid: bool,
    blocker: Blocker,
) -> Vec<Check> {
    let mut checks = mcp_checks(mcp, blocker);
    if project.is_some() && project_valid {
        checks.extend(hook_checks(hooks, blocker));
        checks.push(pointer_check(pointer, blocker));
    } else if project.is_some() {
        checks.push(Check::new(
            "integration.pointer",
            Status::Unknown,
            Reason::PointerStateUnknown,
            Remediation::ReviewPointer,
        ));
    }
    checks
}

fn availability_check(
    id: &'static str,
    result: &Option<Result<Value>>,
    blocker: Blocker,
    available: Reason,
    unavailable: Reason,
    unknown: Reason,
    remediation: Remediation,
) -> Check {
    let Some(result) = result else {
        return not_checked(id, blocker);
    };
    match result {
        Err(error) => Check::new(
            id,
            if matches!(error, Error::Unavailable | Error::Timeout | Error::NotFound) {
                Status::Unknown
            } else {
                Status::Problem
            },
            unknown,
            daemon_failure(*error).1,
        ),
        Ok(value) => match value.get("available").and_then(Value::as_bool) {
            Some(true) => {
                Check::new(id, Status::Ok, available, Remediation::None).with_facts(Facts {
                    available: Some(true),
                    ..Facts::default()
                })
            }
            Some(false) => {
                Check::new(id, Status::Problem, unavailable, remediation).with_facts(Facts {
                    available: Some(false),
                    ..Facts::default()
                })
            }
            None => Check::new(
                id,
                Status::Problem,
                Reason::ResponseInvalid,
                Remediation::ReinstallMatchingRelease,
            ),
        },
    }
}

fn mcp_checks(mcp: &Option<Result<Value>>, blocker: Blocker) -> Vec<Check> {
    let available = availability_check(
        "integration.mcp.availability",
        mcp,
        blocker,
        Reason::McpAvailable,
        Reason::McpUnavailable,
        Reason::McpStateUnknown,
        Remediation::ReviewMcpIntegration,
    );
    let mut checks = vec![available];
    for (id, client) in [
        ("integration.mcp.claude", "claude"),
        ("integration.mcp.codex", "codex"),
    ] {
        checks.push(integration_entry_check(
            id,
            mcp,
            blocker,
            "clients",
            client,
            Reason::McpUnavailable,
            Reason::McpCurrent,
            Reason::McpNotInstalled,
            Reason::McpOutdated,
            Reason::McpMalformed,
            Reason::McpStateUnknown,
            Remediation::ReviewMcpIntegration,
        ));
    }
    checks
}

fn hook_checks(hooks: &Option<Result<Value>>, blocker: Blocker) -> Vec<Check> {
    let mut checks = vec![availability_check(
        "integration.hooks.availability",
        hooks,
        blocker,
        Reason::HookAvailable,
        Reason::HookUnavailable,
        Reason::HookStateUnknown,
        Remediation::ReviewHookIntegration,
    )];
    for (key, claude_id, codex_id) in [
        (
            "hooks",
            "integration.hooks.claude",
            "integration.hooks.codex",
        ),
        (
            "gate_hooks",
            "integration.gate.claude",
            "integration.gate.codex",
        ),
    ] {
        for (id, vendor) in [(claude_id, "claude"), (codex_id, "codex")] {
            checks.push(integration_entry_check(
                id,
                hooks,
                blocker,
                key,
                vendor,
                Reason::HookUnavailable,
                Reason::HookCurrent,
                Reason::HookNotInstalled,
                Reason::HookOutdated,
                Reason::HookMalformed,
                Reason::HookStateUnknown,
                Remediation::ReviewHookIntegration,
            ));
        }
    }
    checks
}

#[allow(clippy::too_many_arguments)]
fn integration_entry_check(
    id: &'static str,
    result: &Option<Result<Value>>,
    blocker: Blocker,
    array_key: &'static str,
    entry_key: &'static str,
    unavailable: Reason,
    current: Reason,
    not_installed: Reason,
    outdated: Reason,
    malformed: Reason,
    unknown: Reason,
    remediation: Remediation,
) -> Check {
    let Some(result) = result else {
        return not_checked(id, blocker);
    };
    let value = match result {
        Ok(value) => value,
        Err(error) => {
            return Check::new(
                id,
                if matches!(error, Error::Unavailable | Error::Timeout | Error::NotFound) {
                    Status::Unknown
                } else {
                    Status::Problem
                },
                unknown,
                daemon_failure(*error).1,
            );
        }
    };
    if value.get("available").and_then(Value::as_bool) == Some(false) {
        return Check::new(id, Status::Unknown, unavailable, remediation).with_facts(Facts {
            available: Some(false),
            ..Facts::default()
        });
    }
    let Some(entries) = value.get(array_key).and_then(Value::as_array) else {
        return Check::new(
            id,
            Status::Problem,
            Reason::ResponseInvalid,
            Remediation::ReinstallMatchingRelease,
        );
    };
    let Some(entry) = entries.iter().find(|entry| {
        entry
            .get(if array_key == "clients" {
                "client"
            } else {
                "vendor"
            })
            .and_then(Value::as_str)
            == Some(entry_key)
    }) else {
        return Check::new(id, Status::Unknown, unknown, remediation);
    };
    let malformed_value = entry.get("malformed").and_then(Value::as_bool);
    let installed = entry.get("installed").and_then(Value::as_bool);
    let current_value = entry.get("current").and_then(Value::as_bool);
    let status = match (malformed_value, installed, current_value) {
        (Some(true), _, _) => (Status::Problem, malformed),
        (Some(false), Some(false), _) => (Status::Off, not_installed),
        (Some(false), Some(true), Some(false)) => (Status::Attention, outdated),
        (Some(false), Some(true), Some(true)) => (Status::Ok, current),
        _ => (Status::Problem, Reason::ResponseInvalid),
    };
    Check::new(
        id,
        status.0,
        status.1,
        if status.0 == Status::Ok {
            Remediation::None
        } else {
            remediation
        },
    )
    .with_facts(Facts {
        available: Some(true),
        malformed: malformed_value,
        installed,
        current: current_value,
        ..Facts::default()
    })
}

fn pointer_check(pointer: &Option<Result<Value>>, blocker: Blocker) -> Check {
    let Some(pointer) = pointer else {
        return not_checked("integration.pointer", blocker);
    };
    let value = match pointer {
        Ok(value) => value,
        Err(error) => {
            return Check::new(
                "integration.pointer",
                if matches!(error, Error::Unavailable | Error::Timeout | Error::NotFound) {
                    Status::Unknown
                } else {
                    Status::Problem
                },
                Reason::PointerStateUnknown,
                daemon_failure(*error).1,
            );
        }
    };
    match (
        value.get("malformed").and_then(Value::as_bool),
        value.get("state").and_then(Value::as_str),
    ) {
        (Some(true), _) => Check::new(
            "integration.pointer",
            Status::Problem,
            Reason::PointerMalformed,
            Remediation::ReviewPointer,
        ),
        (Some(false), Some("current")) => Check::new(
            "integration.pointer",
            Status::Ok,
            Reason::PointerCurrent,
            Remediation::None,
        ),
        (Some(false), Some("outdated")) => Check::new(
            "integration.pointer",
            Status::Attention,
            Reason::PointerOutdated,
            Remediation::ReviewPointer,
        ),
        (Some(false), Some("absent")) => Check::new(
            "integration.pointer",
            Status::Off,
            Reason::PointerAbsent,
            Remediation::ReviewPointer,
        ),
        (Some(false), Some(_)) | (None, Some(_)) => Check::new(
            "integration.pointer",
            Status::Unknown,
            Reason::PointerStateUnknown,
            Remediation::ReviewPointer,
        ),
        _ => Check::new(
            "integration.pointer",
            Status::Problem,
            Reason::ResponseInvalid,
            Remediation::ReinstallMatchingRelease,
        ),
    }
}

#[derive(Debug)]
enum ManifestRead {
    Missing,
    Unreadable,
    Invalid,
    Value(Value),
}

fn read_manifest(path: &Path) -> ManifestRead {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return ManifestRead::Missing,
        Err(_) => return ManifestRead::Unreadable,
    };
    if !meta.file_type().is_file()
        || meta.file_type().is_symlink()
        || meta.len() > MAX_MANIFEST_BYTES
    {
        return ManifestRead::Invalid;
    }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return ManifestRead::Unreadable,
    };
    let mut bytes = Vec::new();
    if file
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > MAX_MANIFEST_BYTES
    {
        return ManifestRead::Invalid;
    }
    match serde_json::from_slice(&bytes) {
        Ok(value) => ManifestRead::Value(value),
        Err(_) => ManifestRead::Invalid,
    }
}

fn manifest_format(value: &Value) -> Option<u32> {
    value
        .get("format")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok().filter(|value| *value <= 100))
}

fn manifest_valid(value: &Value) -> bool {
    manifest_format(value) == Some(2)
        && value.get("layout").and_then(Value::as_str) == Some("Precept/{bin,runtime,notices}")
}

fn package_checks_at(current: &Path) -> Vec<Check> {
    let manifest_path = current
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("runtime/manifest.json"));
    let manifest = manifest_path.as_deref().map(read_manifest);
    let manifest_check = match &manifest {
        None | Some(ManifestRead::Missing) => Check::new(
            "package.manifest",
            Status::Unknown,
            Reason::ManifestAbsent,
            Remediation::None,
        ),
        Some(ManifestRead::Unreadable) => Check::new(
            "package.manifest",
            Status::Problem,
            Reason::ManifestUnreadable,
            Remediation::ReextractPackage,
        ),
        Some(ManifestRead::Invalid) => Check::new(
            "package.manifest",
            Status::Problem,
            Reason::ManifestInvalid,
            Remediation::ReextractPackage,
        ),
        Some(ManifestRead::Value(value)) if manifest_valid(value) => Check::new(
            "package.manifest",
            Status::Ok,
            Reason::ManifestPresent,
            Remediation::None,
        )
        .with_facts(Facts {
            manifest_format: manifest_format(value),
            ..Facts::default()
        }),
        Some(ManifestRead::Value(value)) => Check::new(
            "package.manifest",
            Status::Problem,
            Reason::ManifestInvalid,
            Remediation::ReextractPackage,
        )
        .with_facts(Facts {
            manifest_format: manifest_format(value),
            ..Facts::default()
        }),
    };
    let version_check = match &manifest {
        Some(ManifestRead::Value(value)) if manifest_valid(value) => {
            match value.get("version").and_then(Value::as_str) {
                Some(version) if version == PRODUCT_VERSION => Check::new(
                    "package.version",
                    Status::Ok,
                    Reason::PackageVersionCompatible,
                    Remediation::None,
                )
                .with_facts(Facts {
                    product_version: Some(PRODUCT_VERSION),
                    expected_product_version: Some(PRODUCT_VERSION),
                    ..Facts::default()
                }),
                Some(_) => Check::new(
                    "package.version",
                    Status::Problem,
                    Reason::PackageVersionIncompatible,
                    Remediation::ReextractPackage,
                )
                .with_facts(Facts {
                    expected_product_version: Some(PRODUCT_VERSION),
                    ..Facts::default()
                }),
                None => Check::new(
                    "package.version",
                    Status::Problem,
                    Reason::PackageVersionUnknown,
                    Remediation::ReextractPackage,
                ),
            }
        }
        Some(ManifestRead::Missing) | None => Check::new(
            "package.version",
            Status::Unknown,
            Reason::ManifestAbsent,
            Remediation::None,
        ),
        _ => Check::new(
            "package.version",
            Status::Unknown,
            Reason::ManifestUnavailable,
            Remediation::ReextractPackage,
        ),
    };
    let inventory_check = match &manifest {
        Some(ManifestRead::Value(value)) if manifest_valid(value) => {
            package_inventory_check(value, current.parent().and_then(Path::parent))
        }
        Some(ManifestRead::Missing) | None => Check::new(
            "package.inventory",
            Status::Unknown,
            Reason::ManifestAbsent,
            Remediation::None,
        ),
        _ => Check::new(
            "package.inventory",
            Status::Unknown,
            Reason::ManifestUnavailable,
            Remediation::ReextractPackage,
        ),
    };
    vec![manifest_check, version_check, inventory_check]
}

fn expected_binaries(bridge_shipped: bool) -> Vec<String> {
    let suffix = std::env::consts::EXE_SUFFIX;
    let mut names = vec![
        format!("preceptd{suffix}"),
        format!("precept{suffix}"),
        format!("precept-mcp{suffix}"),
        format!("precept-hook{suffix}"),
        format!("precept-desktop{suffix}"),
    ];
    if cfg!(target_os = "linux") || bridge_shipped {
        names.push(format!("precept-bridge{suffix}"));
    }
    names
}

fn safe_package_file(file: &str) -> bool {
    if file == "README.txt" || file == "LICENSE.txt" {
        return true;
    }
    let mut parts = file.split('/');
    let first = parts.next().unwrap_or("");
    let second = parts.next().unwrap_or("");
    matches!(first, "bin" | "runtime" | "notices")
        && !second.is_empty()
        && parts.next().is_none()
        && !file.contains('\\')
        && file.len() <= 128
        && file.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        })
}

fn hash_metadata(value: &Value) -> bool {
    value
        .get("sha256")
        .and_then(Value::as_str)
        .is_some_and(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
}

fn package_inventory_check(manifest: &Value, package_root: Option<&Path>) -> Check {
    let Some(files) = manifest.get("files").and_then(Value::as_array) else {
        return Check::new(
            "package.inventory",
            Status::Problem,
            Reason::ResponseInvalid,
            Remediation::ReextractPackage,
        );
    };
    let Some(binaries) = manifest.get("binaries").and_then(Value::as_array) else {
        return Check::new(
            "package.inventory",
            Status::Problem,
            Reason::ResponseInvalid,
            Remediation::ReextractPackage,
        );
    };
    if files.len() > MAX_MANIFEST_FILES || binaries.len() > MAX_MANIFEST_BINARIES {
        return Check::new(
            "package.inventory",
            Status::Problem,
            Reason::PackageInventoryMismatch,
            Remediation::ReextractPackage,
        );
    }
    let bridge_name = format!("precept-bridge{}", std::env::consts::EXE_SUFFIX);
    let bridge_shipped = binaries.iter().any(|entry| {
        entry.get("file").and_then(Value::as_str) == Some(format!("bin/{bridge_name}").as_str())
    });
    let expected = expected_binaries(bridge_shipped);
    let mut actual = binaries
        .iter()
        .filter_map(|entry| entry.get("file").and_then(Value::as_str))
        .filter_map(|file| file.strip_prefix("bin/"))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected_sorted = expected.clone();
    expected_sorted.sort();
    let facts = Facts {
        binary_count: Some(binaries.len()),
        file_count: Some(files.len()),
        sizes_match: Some(true),
        hashes_recorded: Some(true),
        content_hashes_verified: Some(false),
        ..Facts::default()
    };
    if actual != expected_sorted
        || binaries.iter().any(|entry| {
            entry
                .get("file")
                .and_then(Value::as_str)
                .is_none_or(|file| !file.starts_with("bin/") || !safe_package_file(file))
        })
    {
        return Check::new(
            "package.inventory",
            Status::Problem,
            Reason::PackageInventoryMismatch,
            Remediation::ReextractPackage,
        )
        .with_facts(facts);
    }
    if files.iter().any(|entry| {
        entry
            .get("file")
            .and_then(Value::as_str)
            .is_none_or(|file| !safe_package_file(file))
            || !hash_metadata(entry)
            || entry.get("size").and_then(Value::as_u64).is_none()
    }) {
        return Check::new(
            "package.inventory",
            Status::Problem,
            Reason::PackageHashMetadataInvalid,
            Remediation::ReextractPackage,
        )
        .with_facts(facts);
    }
    let Some(root) = package_root else {
        return Check::new(
            "package.inventory",
            Status::Unknown,
            Reason::PackageMetadataUnknown,
            Remediation::ReviewInstallation,
        );
    };
    let mut seen = std::collections::BTreeSet::new();
    for entry in files {
        let Some(file) = entry.get("file").and_then(Value::as_str) else {
            continue;
        };
        if !seen.insert(file) {
            return Check::new(
                "package.inventory",
                Status::Problem,
                Reason::PackageInventoryMismatch,
                Remediation::ReextractPackage,
            )
            .with_facts(facts);
        }
        let path = root.join(file);
        let matches = fs::symlink_metadata(&path).is_ok_and(|meta| {
            meta.file_type().is_file()
                && !meta.file_type().is_symlink()
                && entry.get("size").and_then(Value::as_u64) == Some(meta.len())
        });
        if !matches {
            return Check::new(
                "package.inventory",
                Status::Problem,
                Reason::PackageSizeMismatch,
                Remediation::ReextractPackage,
            )
            .with_facts(facts);
        }
    }
    Check::new(
        "package.inventory",
        Status::Ok,
        Reason::PackageMetadataConsistent,
        Remediation::None,
    )
    .with_facts(facts)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let location = Location {
            runtime_dir: PathBuf::from(if cfg!(windows) {
                "C:\\private\\runtime"
            } else {
                "/private/runtime"
            }),
            endpoint: if cfg!(windows) {
                "doctor".into()
            } else {
                "/private/runtime/control.sock".into()
            },
        };
        assert!(endpoint_valid(&location, None));
        let invalid = Location {
            endpoint: if cfg!(windows) {
                "bad endpoint".into()
            } else {
                "relative.sock".into()
            },
            ..location.clone()
        };
        assert!(!endpoint_valid(&invalid, None));
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
                {"client":"claude","installed":installed,"current":current,"malformed":malformed},
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
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
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
}
