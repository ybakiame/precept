//! Bounded, read-only installation diagnostics. Every emitted string is either
//! a fixed report token or a known-safe version/schema value; paths, operating
//! system errors, credentials, commands and Profile contents never enter a report.

mod daemon;
mod integration;
mod package;
mod project;
mod runtime;
#[cfg(test)]
mod tests;

use daemon::{daemon_checks, profile_checks};
use integration::integration_checks;
use package::package_checks_at;
use precept_ipc::{Access, Location, load_credential, valid_project};
use project::{agent_credential_check, project_checks, project_mcp_check};
use runtime::{
    Blocker, endpoint_check, endpoint_valid, executable_checks, operator_credential_check, request,
    runtime_check,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

const FORMAT: &str = "precept-doctor";
const VERSION: u32 = 1;
const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");
const PLATFORM: &str = if cfg!(windows) { "windows" } else { "linux" };
const CURRENT_SCHEMA: &str = "Version1";
const TOTAL_IPC_TIMEOUT: Duration = Duration::from_secs(12);

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
    McpBroken,
    McpMalformed,
    McpStateUnknown,
    HookAvailable,
    HookUnavailable,
    HookCurrent,
    HookNotInstalled,
    HookOutdated,
    HookBroken,
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
    UpdateAgentEntries,
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
    /// An installed entry names a program that no longer exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    broken: Option<bool>,
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
            Some(Err(error)) => Blocker::Daemon(error.clone()),
            _ => Blocker::None,
        }
    };
    checks.extend(daemon_checks(&health, blocker.clone()));
    let writes = profile
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|value| value.get("write_enabled"))
        .and_then(Value::as_bool);
    checks.extend(profile_checks(&profile, &health, blocker.clone()));
    let project_checks = project_checks(&projects, project, project_valid, writes, blocker.clone());
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
        blocker.clone(),
    ));
    checks.extend(agent_credential_check(
        location,
        root.as_deref().ok(),
        project,
        selected_project,
        sharing,
        blocker.clone(),
    ));
    checks.extend(integration_checks(
        &mcp,
        &hooks,
        &pointer,
        project,
        project_valid,
        blocker.clone(),
    ));
    if let Ok(exe) = std::env::current_exe() {
        checks.extend(package_checks_at(&exe));
    } else {
        checks.extend(package_checks_at(Path::new("")));
    }
    Report::new(checks).render(json)
}
