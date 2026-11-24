use super::daemon::daemon_failure;
use super::runtime::{Blocker, not_checked};
use super::{Check, Facts, Reason, Remediation, Status};
use precept_ipc::{Error, Result};
use serde_json::Value;
pub(super) fn integration_checks(
    mcp: &Option<Result<Value>>,
    hooks: &Option<Result<Value>>,
    pointer: &Option<Result<Value>>,
    project: Option<&str>,
    project_valid: bool,
    blocker: Blocker,
) -> Vec<Check> {
    let mut checks = mcp_checks(mcp, blocker.clone());
    if project.is_some() && project_valid {
        checks.extend(hook_checks(hooks, blocker.clone()));
        checks.push(pointer_check(pointer, blocker.clone()));
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

pub(super) fn availability_check(
    id: &'static str,
    result: &Option<Result<Value>>,
    blocker: Blocker,
    available: Reason,
    unavailable: Reason,
    unknown: Reason,
    remediation: Remediation,
) -> Check {
    let Some(result) = result else {
        return not_checked(id, blocker.clone());
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
            daemon_failure(error.clone()).1,
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

pub(super) fn mcp_checks(mcp: &Option<Result<Value>>, blocker: Blocker) -> Vec<Check> {
    let available = availability_check(
        "integration.mcp.availability",
        mcp,
        blocker.clone(),
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
            blocker.clone(),
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

pub(super) fn hook_checks(hooks: &Option<Result<Value>>, blocker: Blocker) -> Vec<Check> {
    let mut checks = vec![availability_check(
        "integration.hooks.availability",
        hooks,
        blocker.clone(),
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
                blocker.clone(),
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
pub(super) fn integration_entry_check(
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
        return not_checked(id, blocker.clone());
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
                daemon_failure(error.clone()).1,
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

pub(super) fn pointer_check(pointer: &Option<Result<Value>>, blocker: Blocker) -> Check {
    let Some(pointer) = pointer else {
        return not_checked("integration.pointer", blocker.clone());
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
                daemon_failure(error.clone()).1,
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
