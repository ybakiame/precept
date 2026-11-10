//! Session view state is ephemeral and isolated from context snapshots/Handoff.
pub use acp_session::{Capabilities, State, View};
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
#[derive(Clone, Debug)]
pub struct Observation {
    pub capabilities: Capabilities,
    pub view: Option<View>,
}
impl SessionModel {
    pub fn receive(&mut self, project: &str, result: Result<Observation, &str>) {
        match result {
            Ok(o)
                if o.capabilities.session_api_version == 1
                    && o.capabilities.protocol_version == 1
                    && o.capabilities.executables.len() <= 8
                    && o.view.as_ref().is_none_or(|v| {
                        v.project == project
                            && v.session_api_version == 1
                            && v.events.len() <= 64
                            && v.events.iter().all(|s| s.chars().count() <= 512)
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
                {
                    self.prompt.clear();
                    self.confirm_start = false;
                    self.event_page = 0;
                    self.permission_review = None;
                }
                if self.executable.is_empty() {
                    self.executable = o
                        .capabilities
                        .executables
                        .first()
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
    pub fn can_prompt(&self) -> bool {
        self.view
            .as_ref()
            .is_some_and(|v| v.process_alive && matches!(v.state, State::Ready | State::Completed))
            && !self.busy
            && !self.prompt.trim().is_empty()
            && self.prompt.len() <= 4096
    }
}
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
impl crate::native::Inputs {
    pub fn session_call(
        &self,
        project: &str,
        method: &str,
        mut fields: serde_json::Value,
    ) -> Result<Observation, &'static str> {
        use acp_ipc::{Access, Client, Location, load_credential};
        // Never extend the Windows->WSL read-only helper into an execution bridge.
        if self.wsl_distribution.is_some() || self.runtime_dir.is_none() {
            return Err("Native local target required");
        }
        let location = Location {
            runtime_dir: self.runtime_dir.clone().ok_or("Local target required")?,
            endpoint: self.endpoint.clone().ok_or("Local target required")?,
        };
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Session unavailable")?
            .block_on(async {
                let c = load_credential(
                    &location,
                    &location
                        .credential_path(None)
                        .map_err(|_| "Session unavailable")?,
                )
                .map_err(|_| "Session access refused")?;
                if c.access != Access::Operator {
                    return Err("Session access refused");
                }
                let mut client = Client::connect(&location, &c)
                    .await
                    .map_err(|_| "Session unavailable")?;
                let cap = client
                    .request(
                        "session.capabilities",
                        serde_json::json!({"session_api_version":1,"project":project}),
                    )
                    .await
                    .map_err(|_| "Session unavailable")?;
                let capabilities: Capabilities =
                    serde_json::from_value(cap).map_err(|_| "Invalid session capabilities")?;
                if capabilities.session_api_version != 1 || capabilities.protocol_version != 1 {
                    return Err("Unsupported session version");
                }
                fields["session_api_version"] = 1.into();
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
                let view = serde_json::from_value(if method == "session.refresh" {
                    result
                        .get("session")
                        .cloned()
                        .ok_or("Invalid session state")?
                } else {
                    result
                })
                .map_err(|_| "Invalid session state")?;
                Ok(Observation { capabilities, view })
            })
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
            session_api_version: 1,
            protocol_version: 1,
            executables: vec!["fixture".into()],
            resume_supported: false,
        };
        let view = View {
            session_api_version: 1,
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
    fn project_and_target_changes_reject_in_flight_session_observations() {
        let mut m = crate::Model::fixture();
        m.select(0);
        m.connection.target = crate::connection::Target::Local;
        let connection = m.connection.clone();
        let epoch = m.epoch;
        let cap = Capabilities {
            session_api_version: 1,
            protocol_version: 1,
            executables: vec!["fixture".into()],
            resume_supported: false,
        };
        let result = || {
            Ok(Observation {
                capabilities: cap.clone(),
                view: Some(View {
                    session_api_version: 1,
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
