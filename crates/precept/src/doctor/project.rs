use super::daemon::daemon_failure;
use super::runtime::{Blocker, not_checked};
use super::{Check, Facts, Reason, Remediation, Status};
use precept_ipc::{Access, Error, Location, Result, load_credential, valid_project};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};
pub(super) fn project_refused(count: Check, binding: Check) -> Vec<Check> {
    let mut checks = vec![count, binding];
    checks.extend(unknown_project_capabilities());
    checks
}

pub(super) fn project_checks(
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
            daemon_failure(error.clone()).1,
        ),
        None => not_checked("project.count", blocker.clone()),
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
                    daemon_failure(error.clone()).1,
                ),
            );
        }
        None => {
            return project_refused(count_check, not_checked("project.binding", blocker.clone()));
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

pub(super) fn unknown_project_capabilities() -> Vec<Check> {
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

pub(super) fn boolean_capability(
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
pub(super) fn normalized_windows_path(path: &Path) -> PathBuf {
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

pub(super) fn same_path_argument(expected: &Path, actual: &str) -> bool {
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

pub(super) fn project_mcp_check(
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
        return vec![not_checked("project.mcp", blocker.clone())];
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
            daemon_failure(error.clone()).1,
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

pub(super) fn agent_credential_check(
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
        return vec![not_checked("project.credential", blocker.clone())];
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
        Err(_) if !exists => Check::new(
            "project.credential",
            Status::Unknown,
            Reason::AgentCredentialNotIssued,
            Remediation::RestartDaemonExplicitly,
        ),
        Err(Error::UnsafeRuntime) => Check::new(
            "project.credential",
            Status::Problem,
            Reason::AgentCredentialUnsafe,
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
