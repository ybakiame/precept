use super::runtime::{Blocker, not_checked};
use super::{CURRENT_SCHEMA, Check, Facts, PRODUCT_VERSION, Reason, Remediation, Status};
use precept_ipc::{API_VERSION, Error, Result};
use serde_json::Value;
pub(super) fn daemon_failure(error: Error) -> (Reason, Remediation) {
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

pub(super) fn daemon_health_check(health: &Option<Result<Value>>, blocker: Blocker) -> Check {
    let Some(health) = health else {
        return not_checked("daemon.health", blocker.clone());
    };
    match health {
        Err(error) => {
            let (reason, remediation) = daemon_failure(error.clone());
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

pub(super) fn daemon_api_check(health: &Option<Result<Value>>, blocker: Blocker) -> Check {
    let Some(health) = health else {
        return not_checked("daemon.api", blocker.clone());
    };
    match health {
        Err(error) => Check::new(
            "daemon.api",
            Status::Unknown,
            Reason::ApiUnknown,
            daemon_failure(error.clone()).1,
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

pub(super) fn daemon_product_check(health: &Option<Result<Value>>, blocker: Blocker) -> Check {
    let Some(health) = health else {
        return not_checked("daemon.product", blocker.clone());
    };
    match health {
        Err(error) => Check::new(
            "daemon.product",
            Status::Unknown,
            Reason::ProductUnknown,
            daemon_failure(error.clone()).1,
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

pub(super) fn profile_schema_check(health: &Option<Result<Value>>, blocker: Blocker) -> Check {
    let Some(health) = health else {
        return not_checked("profile.schema", blocker.clone());
    };
    let value = match health {
        Ok(value) => value,
        Err(error) => {
            return Check::new(
                "profile.schema",
                Status::Unknown,
                Reason::SchemaUnknown,
                daemon_failure(error.clone()).1,
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

pub(super) fn daemon_checks(health: &Option<Result<Value>>, blocker: Blocker) -> Vec<Check> {
    vec![
        daemon_health_check(health, blocker.clone()),
        daemon_api_check(health, blocker.clone()),
        daemon_product_check(health, blocker.clone()),
        profile_schema_check(health, blocker.clone()),
    ]
}

pub(super) fn profile_checks(
    profile: &Option<Result<Value>>,
    health: &Option<Result<Value>>,
    blocker: Blocker,
) -> Vec<Check> {
    let Some(profile) = profile else {
        return vec![
            not_checked("profile.selected", blocker.clone()),
            not_checked("profile.writes", blocker.clone()),
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
                    daemon_failure(error.clone()).1,
                ),
                Check::new(
                    "profile.writes",
                    Status::Unknown,
                    Reason::WritesUnknown,
                    daemon_failure(error.clone()).1,
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
