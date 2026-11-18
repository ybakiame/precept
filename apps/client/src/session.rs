//! Session requests against the daemon: local IPC or the WSL bridge.
use acp_operator_core::session::{Observation, SESSION_API, observation};
use serde_json::Value;

/// One session request against the daemon at `location`, returning the raw
/// `{"capabilities","view"}` pair. Session policy stays entirely daemon-side.
pub async fn session_raw(
    location: &acp_ipc::Location,
    project: &str,
    method: &str,
    mut fields: Value,
) -> Result<Value, &'static str> {
    use acp_ipc::{Access, Client, load_credential};
    let c = load_credential(
        location,
        &location
            .credential_path(None)
            .map_err(|_| "Session unavailable")?,
    )
    .map_err(|_| "Session access refused")?;
    if c.access != Access::Operator {
        return Err("Session access refused");
    }
    let mut client = Client::connect(location, &c)
        .await
        .map_err(|_| "Session unavailable")?;
    let capabilities = client
        .request(
            "session.capabilities",
            serde_json::json!({"session_api_version":SESSION_API,"project":project}),
        )
        .await
        .map_err(|_| "Session unavailable")?;
    // Fail closed on version before sending any session command.
    observation(capabilities.clone(), Value::Null)?;
    fields["session_api_version"] = SESSION_API.into();
    fields["project"] = project.into();
    let result = client
        .request(
            if method == "session.refresh" {
                "session.get"
            } else {
                method
            },
            fields,
        )
        .await
        .map_err(|_| "Session unavailable or request denied")?;
    let view = if method == "session.refresh" {
        result
            .get("session")
            .cloned()
            .ok_or("Invalid session state")?
    } else {
        result
    };
    // A refresh also reports the daemon's context lease (read-only). Without an
    // observed session there is no lease (the daemon answers null, which the IPC
    // client treats as an error), so it is only read for an existing session.
    let context = if method == "session.refresh" && !view.is_null() {
        client
            .request(
                "session.context",
                serde_json::json!({"session_api_version":SESSION_API,"project":project}),
            )
            .await
            .map_err(|_| "Session unavailable")?
    } else {
        Value::Null
    };
    Ok(serde_json::json!({"capabilities": capabilities, "view": view, "context": context}))
}

impl crate::native::Inputs {
    pub fn session_call(
        &self,
        project: &str,
        method: &str,
        fields: Value,
    ) -> Result<Observation, &'static str> {
        let raw = if self.wsl_distribution.is_some() {
            use crate::bridge::{CallError, Op};
            // Reuses the live bridge only: session polling never starts WSL.
            let bridge = self.wsl_bridge(false).map_err(|_| "Session unavailable")?;
            bridge
                .call(
                    Op::Session {
                        project: project.into(),
                        method: method.into(),
                        fields,
                    },
                    std::time::Duration::from_secs(30),
                )
                .map_err(|e| match e {
                    CallError::Remote(code) => code,
                    _ => "Session unavailable",
                })?
        } else {
            let location = acp_ipc::Location {
                runtime_dir: self
                    .runtime_dir
                    .clone()
                    .ok_or("Native local target required")?,
                endpoint: self
                    .endpoint
                    .clone()
                    .ok_or("Native local target required")?,
            };
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| "Session unavailable")?
                .block_on(session_raw(&location, project, method, fields))?
        };
        let capabilities = raw
            .get("capabilities")
            .cloned()
            .ok_or("Invalid session state")?;
        let view = raw.get("view").cloned().ok_or("Invalid session state")?;
        let mut observed = observation(capabilities, view)?;
        observed.context = match raw.get("context") {
            None | Some(Value::Null) => None,
            Some(lease) => {
                Some(serde_json::from_value(lease.clone()).map_err(|_| "Invalid session state")?)
            }
        };
        Ok(observed)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn wsl_and_synthetic_inputs_cannot_invoke_session_mutation() {
        let mut input = crate::native::Inputs::default();
        assert!(
            input
                .session_call("alpha", "session.start", serde_json::json!({}))
                .is_err()
        );
        input.wsl_distribution = Some("synthetic".into());
        assert!(
            input
                .session_call("alpha", "session.start", serde_json::json!({}))
                .is_err()
        );
    }
}
