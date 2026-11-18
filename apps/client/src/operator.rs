//! Operator read/control and OperatorWrite requests: local IPC or the WSL bridge.
use acp_core::operator::Mutation;
use acp_operator_core::operator::Review;
use serde_json::Value;

/// Stable UI error vocabulary for daemon results; shared by the local client
/// and the WSL bridge helper so both targets report identically.
pub fn operator_error(e: acp_ipc::Error) -> &'static str {
    match e {
        acp_ipc::Error::Busy => "Busy",
        acp_ipc::Error::Capacity => "Capacity",
        acp_ipc::Error::Conflict => "Conflict",
        acp_ipc::Error::OutcomeUnknown => "OutcomeUnknown",
        acp_ipc::Error::Unauthorized | acp_ipc::Error::UnsafeRuntime => "Unauthorized",
        acp_ipc::Error::RestartRequired => "RestartRequired",
        acp_ipc::Error::StaleGeneration => "StaleGeneration",
        acp_ipc::Error::OwnerMismatch => "OwnerMismatch",
        acp_ipc::Error::IdempotencyConflict => "IdempotencyConflict",
        acp_ipc::Error::InvalidRequest => "InvalidRequest",
        acp_ipc::Error::InvalidTransition => "InvalidTransition",
        acp_ipc::Error::ReviewMismatch => "ReviewMismatch",
        _ => "Unavailable",
    }
}

/// One Operator read/control request or one OperatorWrite mutation against the
/// daemon at `location`. Authorization stays entirely daemon-side.
pub async fn operator_at(
    location: &acp_ipc::Location,
    method: &str,
    params: Value,
    mutation: Option<&Mutation>,
) -> Result<Value, &'static str> {
    use acp_ipc::operator;
    match mutation {
        Some(mutation) => operator::mutate(location, mutation).await,
        None => operator::control(location, method, params).await,
    }
    .map_err(operator_error)
}

/// `project.add` / `project.remove` against the daemon at `location`, with the
/// write credential. Authorization stays entirely daemon-side.
pub async fn project_write_at(
    location: &acp_ipc::Location,
    method: &str,
    params: Value,
) -> Result<Value, &'static str> {
    acp_ipc::operator::project_write(location, method, params)
        .await
        .map_err(operator_error)
}

impl crate::native::Inputs {
    /// A project-set request: `project.check` / `profile.startup.forget`
    /// (Operator) or `project.add` / `project.remove` (OperatorWrite), locally
    /// or through the WSL bridge. Explicit user actions, so a lost bridge may be
    /// restarted; a write whose reply is lost is `OutcomeUnknown`.
    pub fn project_call(&self, method: &str, params: Value) -> Result<Value, &'static str> {
        use crate::bridge::{CallError, Op, PROJECT_WRITES};
        let write = PROJECT_WRITES.contains(&method);
        if !write
            && !matches!(
                method,
                "project.check" | "profile.startup.forget" | "profile.info"
            )
        {
            return Err("Unsupported");
        }
        if self.wsl_distribution.is_some() {
            let Ok(bridge) = self.wsl_bridge(true) else {
                return Err("Unavailable");
            };
            let op = if write {
                Op::ProjectWrite {
                    method: method.into(),
                    params,
                }
            } else {
                Op::Control {
                    method: method.into(),
                    params,
                }
            };
            return bridge
                .call(op, std::time::Duration::from_secs(30))
                .map_err(|e| match e {
                    CallError::Remote(code) => code,
                    CallError::Lost | CallError::Timeout if write => "OutcomeUnknown",
                    _ => "Unavailable",
                });
        }
        let Some(runtime_dir) = self.runtime_dir.clone() else {
            return Err("Unauthorized");
        };
        let location = acp_ipc::Location {
            runtime_dir,
            endpoint: self.endpoint.clone().ok_or("Unauthorized")?,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Unavailable")?;
        if write {
            runtime.block_on(project_write_at(&location, method, params))
        } else {
            runtime.block_on(operator_at(&location, method, params, None))
        }
    }
    pub fn operator_call(&self, request: &Review) -> Result<Value, &'static str> {
        use crate::bridge::{CallError, Op};
        if self.wsl_distribution.is_some() {
            // Operator requests are explicit user actions, so they may restart a
            // lost bridge (e.g. the identical retry after OutcomeUnknown). Session
            // polling never does. Nothing is submitted if the bridge cannot start.
            let Ok(bridge) = self.wsl_bridge(true) else {
                return Err("Unavailable");
            };
            let (op, timeout) = match &request.mutation {
                Some(mutation) => (
                    Op::Mutate {
                        mutation: Box::new(mutation.clone()),
                    },
                    std::time::Duration::from_secs(30),
                ),
                None => (
                    Op::Control {
                        method: request.method.clone(),
                        params: request.params.clone(),
                    },
                    std::time::Duration::from_secs(15),
                ),
            };
            return bridge.call(op, timeout).map_err(|e| match e {
                CallError::Remote(code) => code,
                // Delivered (or possibly delivered) but unanswered: a mutation's
                // effect is unknown. Only an explicit identical retry may settle it.
                CallError::Lost | CallError::Timeout if request.mutation.is_some() => {
                    "OutcomeUnknown"
                }
                _ => "Unavailable",
            });
        }
        let Some(runtime_dir) = self.runtime_dir.clone() else {
            return Err("Unauthorized");
        };
        let location = acp_ipc::Location {
            runtime_dir,
            endpoint: self.endpoint.clone().ok_or("Unauthorized")?,
        };
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Unavailable")?
            .block_on(operator_at(
                &location,
                &request.method,
                request.params.clone(),
                request.mutation.as_ref(),
            ))
    }
}

pub fn operation_id() -> acp_ipc::Result<String> {
    acp_ipc::operator::operation_id()
}

#[cfg(test)]
mod native_boundary_tests {
    use acp_operator_core::{Model, connection::Target};
    #[test]
    fn offline_never_and_wsl_only_over_a_live_bridge_attempt_operator_ipc() {
        let mut m = Model::fixture();
        m.connection.target = Target::Local;
        m.connection.runtime = std::env::temp_dir().to_str().unwrap().into();
        m.connection.endpoint = "synthetic".into();
        m.operator.open = true;
        let request = m.operator_read("profile.info").unwrap();
        assert_eq!(
            crate::native::Inputs::default().operator_call(&request),
            Err("Unauthorized")
        );
        // Off Windows the WSL transport is unavailable: nothing is submitted.
        let inputs = crate::native::Inputs {
            wsl_distribution: Some("synthetic".into()),
            wsl_user: Some("operator".into()),
            wsl_bridge: Some("/opt/acp-wsl-bridge".into()),
            wsl_runtime_dir: Some("/tmp/private".into()),
            wsl_endpoint: Some("/tmp/private/control.sock".into()),
            ..Default::default()
        };
        assert_eq!(inputs.operator_call(&request), Err("Unavailable"));
    }
}
