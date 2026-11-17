//! Session view state is ephemeral and isolated from context snapshots/Handoff.
pub use acp_session::{AuthMethodKind, AuthStatus, Capabilities, SESSION_API, State, View};
use serde_json::Value;
#[derive(Clone, Debug, Default)]
pub struct PollGuard(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl PartialEq for PollGuard {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for PollGuard {}
impl PollGuard {
    pub fn acquire(&self) -> bool {
        !self.0.swap(true, std::sync::atomic::Ordering::SeqCst)
    }
    pub fn release(&self) {
        self.0.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionModel {
    pub capabilities: Option<Capabilities>,
    pub view: Option<View>,
    pub executable: String,
    pub prompt: String,
    pub confirm_start: bool,
    pub busy: bool,
    pub polling: PollGuard,
    pub command_epoch: u64,
    pub error: Option<String>,
    pub event_page: usize,
    pub permission_review: Option<String>,
}
/// Authority captured when a session control is rendered, never rebound on click.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionScope {
    epoch: u64,
    project: String,
    connection: crate::connection::Connection,
    session_id: String,
}
#[derive(Clone, Debug)]
pub struct Observation {
    pub capabilities: Capabilities,
    pub view: Option<View>,
}
impl SessionModel {
    pub fn receive(&mut self, project: &str, result: Result<Observation, &str>) {
        match result {
            Ok(o)
                if o.capabilities.session_api_version == SESSION_API
                    && o.capabilities.protocol_version == 1
                    && o.capabilities.executables.len() <= 8
                    && o.view.as_ref().is_none_or(|v| {
                        v.project == project
                            && v.session_api_version == SESSION_API
                            && v.events.len() <= 64
                            && v.events.iter().all(|s| s.chars().count() <= 512)
                            && v.authentication.methods.len() <= 8
                            && v.activity.response_chunks <= 4096
                            && v.activity.tool_events <= 4096
                            && v.permissions.len() <= 4
                            && v.permissions.iter().all(|p| p.operation.len() <= 2048)
                    }) =>
            {
                if o.view
                    .as_ref()
                    .zip(self.view.as_ref())
                    .is_some_and(|(new, old)| {
                        new.session_id == old.session_id && new.revision < old.revision
                    })
                {
                    return;
                }
                if self.view.as_ref().map(|v| &v.session_id)
                    != o.view.as_ref().map(|v| &v.session_id)
                    || (o
                        .view
                        .as_ref()
                        .is_some_and(|v| v.authentication.status == AuthStatus::Required)
                        && self
                            .view
                            .as_ref()
                            .is_none_or(|v| v.authentication.status != AuthStatus::Required))
                {
                    self.prompt.clear();
                    self.confirm_start = false;
                    self.event_page = 0;
                    self.permission_review = None;
                }
                if !o.capabilities.executables.contains(&self.executable) {
                    self.confirm_start = false;
                    self.executable = o
                        .view
                        .as_ref()
                        .map(|v| &v.executable)
                        .filter(|id| o.capabilities.executables.contains(id))
                        .or_else(|| o.capabilities.executables.first())
                        .cloned()
                        .unwrap_or_default();
                }
                if self.permission_review.as_ref().is_some_and(|id| {
                    o.view
                        .as_ref()
                        .is_none_or(|v| !v.permissions.iter().any(|p| &p.request_id == id))
                }) {
                    self.permission_review = None;
                }
                self.capabilities = Some(o.capabilities);
                self.view = o.view;
                self.error = None;
            }
            _ => {
                self.view = None;
                self.capabilities = None;
                self.prompt.clear();
                self.confirm_start = false;
                self.permission_review = None;
                self.error = Some("Session unavailable or access refused".into());
            }
        }
    }
    /// Review a fresh generation of the observed agent; never launch or authenticate.
    pub fn review_auth_retry(&mut self) -> bool {
        let Some(v) = &self.view else {
            return false;
        };
        if self.busy
            || v.state != State::AuthRequired
            || v.process_alive
            || !self
                .capabilities
                .as_ref()
                .is_some_and(|c| c.executables.contains(&v.executable))
        {
            return false;
        }
        self.executable = v.executable.clone();
        self.confirm_start = true;
        true
    }
    pub fn can_prompt(&self) -> bool {
        self.view.as_ref().is_some_and(|v| {
            v.process_alive
                && v.authentication.status == AuthStatus::SessionAccepted
                && matches!(v.state, State::Ready | State::Completed)
        }) && !self.busy
            && !self.prompt.trim().is_empty()
            && self.prompt.len() <= 4096
    }
}
/// Builds an Observation from daemon JSON. Applied on the Windows side to every
/// WSL bridge reply, so a helper can never bypass the version checks.
pub fn observation(capabilities: Value, view: Value) -> Result<Observation, &'static str> {
    let capabilities: Capabilities =
        serde_json::from_value(capabilities).map_err(|_| "Invalid session capabilities")?;
    if capabilities.session_api_version != SESSION_API || capabilities.protocol_version != 1 {
        return Err("Unsupported session version");
    }
    let view = serde_json::from_value(view).map_err(|_| "Invalid session state")?;
    Ok(Observation { capabilities, view })
}

/// One session request against the daemon at `location`, returning the raw
/// `{"capabilities","view"}` pair. Session policy stays entirely daemon-side.
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub(crate) async fn session_raw(
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
    Ok(serde_json::json!({"capabilities": capabilities, "view": view}))
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
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
        observation(capabilities, view)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "native")]
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
    #[test]
    fn cross_scope_malformed_denial_and_old_revision_clear_or_ignore() {
        let cap = Capabilities {
            session_api_version: SESSION_API,
            protocol_version: 1,
            executables: vec!["fixture".into()],
            resume_supported: false,
        };
        let view = View {
            session_api_version: SESSION_API,
            project: "alpha".into(),
            session_id: "session-1".into(),
            state: State::WaitingPermission,
            revision: 3,
            ..Default::default()
        };
        let mut m = SessionModel::default();
        m.receive(
            "alpha",
            Ok(Observation {
                capabilities: cap.clone(),
                view: Some(view.clone()),
            }),
        );
        assert_eq!(m.view, Some(view.clone()));
        let mut old = view.clone();
        old.revision = 1;
        m.receive(
            "alpha",
            Ok(Observation {
                capabilities: cap.clone(),
                view: Some(old),
            }),
        );
        assert_eq!(m.view, Some(view.clone()));
        let mut next = view.clone();
        next.permissions = vec![acp_session::Permission {
            request_id: "permission-1".into(),
            operation: "synthetic operation".into(),
            allow_once: Some("once".into()),
            expires_ms: 123,
        }];
        m.receive(
            "alpha",
            Ok(Observation {
                capabilities: cap.clone(),
                view: Some(next.clone()),
            }),
        );
        m.permission_review = Some("permission-1".into());
        m.prompt = "old prompt draft".into();
        next.session_id = "different-process-session".into();
        m.receive(
            "alpha",
            Ok(Observation {
                capabilities: cap.clone(),
                view: Some(next),
            }),
        );
        assert!(m.permission_review.is_none());
        assert!(m.prompt.is_empty());
        m.prompt = "private pending input".into();
        m.receive(
            "beta",
            Ok(Observation {
                capabilities: cap,
                view: Some(view),
            }),
        );
        assert!(m.view.is_none());
        assert!(m.prompt.is_empty());
        assert!(!m.can_prompt());
        m.receive("alpha", Err("auth"));
        assert!(m.capabilities.is_none());
    }
}

impl crate::Model {
    pub fn session_scope(&self) -> Option<SessionScope> {
        let project = self.selected_project.as_ref()?;
        let view = self.session.view.as_ref()?;
        (view.project == *project).then(|| SessionScope {
            epoch: self.epoch,
            project: project.clone(),
            connection: self.connection.clone(),
            session_id: view.session_id.clone(),
        })
    }
    pub fn permission_decision(
        &self,
        scope: &SessionScope,
        request_id: &str,
        allow: bool,
    ) -> Option<serde_json::Value> {
        if self.session_scope().as_ref() != Some(scope) {
            return None;
        }
        let permission = self
            .session
            .view
            .as_ref()?
            .permissions
            .iter()
            .find(|p| p.request_id == request_id)?;
        let option = if allow {
            Some(permission.allow_once.clone()?)
        } else {
            None
        };
        Some(serde_json::json!({
            "session_id": scope.session_id,
            "request_id": request_id,
            "option_id": option,
        }))
    }
    pub fn complete_session(
        &mut self,
        epoch: u64,
        project: &str,
        connection: &crate::connection::Connection,
        command_epoch: u64,
        refresh: bool,
        result: Result<Observation, &str>,
    ) -> bool {
        if self.epoch != epoch
            || self.selected_project.as_deref() != Some(project)
            || &self.connection != connection
        {
            return false;
        }
        if refresh {
            self.session.polling.release();
        }
        if command_epoch != self.session.command_epoch {
            return false;
        }
        if !refresh {
            self.session.busy = false;
        }
        self.session.receive(project, result);
        true
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;
    #[test]
    fn observed_agent_is_default_and_auth_retry_never_rebinds_to_another_agent() {
        let mut m = SessionModel::default();
        let mut capabilities = Capabilities {
            session_api_version: SESSION_API,
            protocol_version: 1,
            executables: vec!["first".into(), "auth-agent".into()],
            resume_supported: false,
        };
        let mut view = View {
            session_api_version: SESSION_API,
            project: "alpha".into(),
            session_id: "auth-generation".into(),
            executable: "auth-agent".into(),
            state: State::AuthRequired,
            ..Default::default()
        };
        let observe = |m: &mut SessionModel, c: &Capabilities, v: &View| {
            m.receive(
                "alpha",
                Ok(Observation {
                    capabilities: c.clone(),
                    view: Some(v.clone()),
                }),
            )
        };
        observe(&mut m, &capabilities, &view);
        assert_eq!(m.executable, "auth-agent");
        m.executable = "first".into();
        observe(&mut m, &capabilities, &view);
        assert_eq!(
            m.executable, "first",
            "polling retains deliberate valid selection"
        );
        assert!(m.review_auth_retry());
        assert_eq!(m.executable, "auth-agent");
        assert!(m.confirm_start);
        m.confirm_start = false;
        view.process_alive = true;
        observe(&mut m, &capabilities, &view);
        assert!(!m.review_auth_retry());
        view.process_alive = false;
        m.confirm_start = true;
        capabilities.executables.retain(|id| id == "first");
        observe(&mut m, &capabilities, &view);
        assert_eq!(
            m.executable, "first",
            "revoked selection must fall back safely"
        );
        assert!(!m.review_auth_retry(), "never retry a removed executable");
        assert!(!m.confirm_start);
    }
    #[test]
    fn auth_requirement_clears_drafts_once_but_passive_poll_preserves_retry_review() {
        let mut m = SessionModel::default();
        let capabilities = Capabilities {
            session_api_version: SESSION_API,
            protocol_version: 1,
            executables: vec!["fixture".into()],
            resume_supported: false,
        };
        let mut view = View {
            session_api_version: SESSION_API,
            project: "alpha".into(),
            session_id: "same-generation".into(),
            state: State::Ready,
            process_alive: true,
            revision: 1,
            ..Default::default()
        };
        view.authentication.status = AuthStatus::SessionAccepted;
        let observation = |view: &View| {
            Ok(Observation {
                capabilities: capabilities.clone(),
                view: Some(view.clone()),
            })
        };
        m.receive("alpha", observation(&view));
        m.prompt = "pending human draft".into();
        m.confirm_start = true;
        m.permission_review = Some("old-permission".into());
        view.state = State::AuthRequired;
        view.authentication.status = AuthStatus::Required;
        view.process_alive = false;
        view.revision += 1;
        m.receive("alpha", observation(&view));
        assert!(m.prompt.is_empty());
        assert!(!m.confirm_start);
        assert!(m.permission_review.is_none());
        m.prompt = "must not send".into();
        assert!(!m.can_prompt());
        m.confirm_start = true;
        m.receive("alpha", observation(&view));
        assert!(
            m.confirm_start,
            "unchanged auth observation must not dismiss explicit retry review"
        );
        let mut old = capabilities;
        old.session_api_version = 1;
        m.receive(
            "alpha",
            Ok(Observation {
                capabilities: old,
                view: Some(view),
            }),
        );
        assert!(m.view.is_none());
        assert!(!m.confirm_start);
        assert!(m.prompt.is_empty());
    }
    #[test]
    fn rendered_permission_never_rebinds_to_a_new_session_or_target() {
        let mut m = crate::Model::fixture();
        m.select(0);
        m.session.view = Some(View {
            project: "alpha".into(),
            session_id: "original".into(),
            permissions: vec![acp_session::Permission {
                request_id: "permission-1".into(),
                operation: "synthetic operation".into(),
                allow_once: Some("once".into()),
                expires_ms: 123,
            }],
            ..Default::default()
        });
        let scope = m.session_scope().unwrap();
        assert_eq!(
            m.permission_decision(&scope, "permission-1", true).unwrap()["session_id"],
            "original"
        );
        assert!(m.permission_decision(&scope, "missing", false).is_none());
        m.session.view.as_mut().unwrap().session_id = "replacement".into();
        // The new agent reused the same local request/option IDs.
        assert!(
            m.permission_decision(&scope, "permission-1", true)
                .is_none()
        );
        assert!(
            m.permission_decision(&scope, "permission-1", false)
                .is_none()
        );
        m.session.view.as_mut().unwrap().session_id = "original".into();
        m.connection.endpoint = "different-target".into();
        assert!(
            m.permission_decision(&scope, "permission-1", true)
                .is_none()
        );
        m.connection = scope.connection.clone();
        m.epoch += 1;
        assert!(
            m.permission_decision(&scope, "permission-1", true)
                .is_none()
        );
        m.epoch = scope.epoch;
        m.selected_project = Some("beta".into());
        assert!(
            m.permission_decision(&scope, "permission-1", true)
                .is_none()
        );
        m.selected_project = Some("alpha".into());
        m.session.view.as_mut().unwrap().permissions.clear();
        assert!(
            m.permission_decision(&scope, "permission-1", true)
                .is_none()
        );
    }
    #[test]
    fn project_and_target_changes_reject_in_flight_session_observations() {
        let mut m = crate::Model::fixture();
        m.select(0);
        m.connection.target = crate::connection::Target::Local;
        let connection = m.connection.clone();
        let epoch = m.epoch;
        let cap = Capabilities {
            session_api_version: SESSION_API,
            protocol_version: 1,
            executables: vec!["fixture".into()],
            resume_supported: false,
        };
        let result = || {
            Ok(Observation {
                capabilities: cap.clone(),
                view: Some(View {
                    session_api_version: SESSION_API,
                    project: "alpha".into(),
                    session_id: "observed".into(),
                    ..Default::default()
                }),
            })
        };
        assert!(m.complete_session(epoch, "alpha", &connection, 0, true, result()));
        m.session.prompt = "pending private prompt".into();
        m.session.confirm_start = true;
        m.select(1);
        assert!(m.session.view.is_none());
        assert!(m.session.prompt.is_empty());
        assert!(!m.session.confirm_start);
        assert!(!m.complete_session(epoch, "alpha", &connection, 0, true, result()));
        m.connection_draft = Default::default();
        assert!(m.apply_connection());
        assert!(!m.complete_session(epoch, "alpha", &connection, 0, true, result()));
        assert!(m.session.view.is_none());
    }
}
