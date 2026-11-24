use super::daemon::daemon_failure;
use super::{Check, Reason, Remediation, Status};
use precept_ipc::{Access, Client, Error, Location, Result, load_credential};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
#[derive(Clone)]
pub(super) enum Blocker {
    None,
    Runtime,
    RuntimeAbsent,
    Endpoint,
    Credential,
    Daemon(Error),
}

pub(super) fn not_checked(id: &'static str, blocker: Blocker) -> Check {
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

pub(super) async fn request(
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

pub(super) fn runtime_check(root: &Result<PathBuf>, runtime_dir: &Path) -> Check {
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

pub(super) fn endpoint_valid(location: &Location, root: Option<&Path>) -> bool {
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

pub(super) fn endpoint_check(valid: bool, runtime_observed: bool) -> Check {
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

pub(super) fn executable_checks() -> Vec<Check> {
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

pub(super) fn executable_checks_at(current: Result<PathBuf>) -> Vec<Check> {
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

pub(super) fn executable_check(id: &'static str, path: &Path) -> Check {
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

pub(super) type CredentialObservation =
    std::result::Result<(precept_ipc::Credential, bool), (Error, bool)>;

pub(super) fn operator_credential_check(credential: Option<&CredentialObservation>) -> Check {
    let Some(credential) = credential else {
        return operator_credential_outcome(None, None, false);
    };
    match credential {
        Ok((cap, exists)) => operator_credential_outcome(Some(&cap.access), None, *exists),
        Err((error, exists)) => operator_credential_outcome(None, Some(error.clone()), *exists),
    }
}

pub(super) fn operator_credential_outcome(
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
        (None, Some(_)) if !exists => Check::new(
            "credential.operator",
            Status::Unknown,
            Reason::CredentialNotIssued,
            Remediation::StartDaemonExplicitly,
        ),
        (None, Some(Error::UnsafeRuntime)) => Check::new(
            "credential.operator",
            Status::Problem,
            Reason::CredentialUnsafe,
            Remediation::RestartDaemonExplicitly,
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
