//! One ephemeral operator workflow for Desktop and TUI, never a second Writer.
use crate::{
    Model,
    connection::{Connection, Target},
};
pub use acp_core::operator::*;
use serde_json::{Value, json};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Pane {
    #[default]
    Profile,
    Preferences,
    Handoff,
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Operator {
    pub pane: Pane,
    pub open: bool,
    pub busy: bool,
    pub serial: u64,
    pub info: Option<ProfileInfo>,
    pub page: Option<PreferencePage>,
    pub handoff: Option<HandoffPreview>,
    pub handoff_page: usize,
    pub selected_id: Option<i64>,
    pub global: bool,
    pub after_id: i64,
    pub path: String,
    pub key: String,
    pub value: String,
    pub source: String,
    pub task: String,
    pub error: Option<String>,
    pub receipt: Option<Receipt>,
    pub review: Option<Review>,
    pub pending: Option<Review>,
    pub outcome_unknown: bool,
}
impl std::fmt::Debug for Operator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Operator(redacted)")
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Guard {
    epoch: u64,
    connection: Connection,
    project: Option<String>,
    task: String,
    context_task: String,
    queried_task: String,
    scope: Scope,
    after_id: i64,
    serial: u64,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Review {
    guard: Guard,
    pub method: String,
    pub params: Value,
    pub mutation: Option<Mutation>,
    pub record: Option<PreferenceRecord>,
}
impl Operator {
    pub fn record(&self) -> Option<&PreferenceRecord> {
        self.page
            .as_ref()?
            .records
            .iter()
            .find(|r| Some(r.id) == self.selected_id)
    }
    pub fn invalidate(&mut self) {
        self.serial += 1;
        self.review = None;
        self.info = None;
        self.page = None;
        self.handoff = None;
        self.selected_id = None;
        self.busy = false;
        self.receipt = None;
        self.error = None;
        // An uncertain operation is retained for identification, never rebound.
    }
}
impl Model {
    pub fn operator_scope(&self) -> Option<Scope> {
        if self.operator.global {
            Some(Scope::Global)
        } else {
            Some(Scope::Project {
                name: self.selected_project.clone()?,
            })
        }
    }
    pub fn operator_guard(&self) -> Option<Guard> {
        (self.connection.target == Target::Local && self.connection.validate()).then(|| Guard {
            epoch: self.epoch,
            connection: self.connection.clone(),
            project: self.selected_project.clone(),
            task: self.operator.task.clone(),
            context_task: self.task_id.clone(),
            queried_task: self.queried_task.clone(),
            scope: self.operator_scope().unwrap_or(Scope::Global),
            after_id: self.operator.after_id,
            serial: self.operator.serial,
        })
    }
    pub fn operator_current(&self, review: &Review) -> bool {
        self.operator_guard().as_ref() == Some(&review.guard)
    }
    pub fn operator_read(&mut self, method: &str) -> Option<Review> {
        if self.operator.busy || self.operator.outcome_unknown || !self.operator.open {
            return None;
        }
        let params = match method {
            "profile.info" => json!({}),
            "profile.preferences.list" => {
                json!({"scope":self.operator_scope()?,"after_id":self.operator.after_id})
            }
            "profile.handoff.get" if crate::valid_task(&self.operator.task) => {
                json!({"project":self.selected_project.as_ref()?,"task_id":self.operator.task})
            }
            _ => return None,
        };
        self.operator.serial += 1;
        let guard = self.operator_guard()?;
        self.operator.review = None;
        self.operator.busy = true;
        Some(Review {
            guard,
            method: method.into(),
            params,
            mutation: None,
            record: None,
        })
    }
    pub fn operator_review(&mut self, action: &str, operation_id: &str) -> bool {
        if self.operator.busy || self.operator.outcome_unknown || !self.operator.open {
            return false;
        }
        let Some(guard) = self.operator_guard() else {
            return false;
        };
        let Some(info) = self.operator.info.as_ref() else {
            return false;
        };
        let (method, params, mutation) = match action {
            "create" | "select" if !info.selected => (
                if action == "create" {
                    "profile.create"
                } else {
                    "profile.select"
                },
                json!({"path":self.operator.path}),
                None,
            ),
            "enable" if info.selected && !info.write_enabled => {
                let Some(b) = &info.binding else {
                    return false;
                };
                (
                    "profile.write.enable",
                    json!({"profile_id":b.profile_id,"generation":b.generation,"confirm":true}),
                    None,
                )
            }
            "disable" if info.write_enabled => ("profile.write.disable", json!({}), None),
            "suggest" | "confirm" | "reject" if info.write_enabled => {
                let Some(binding) = info.binding.clone() else {
                    return false;
                };
                let Some(scope) = self.operator_scope() else {
                    return false;
                };
                let command = if action == "suggest" {
                    if self.operator.key.trim().is_empty()
                        || self.operator.value.trim().is_empty()
                        || self.operator.source.trim().is_empty()
                    {
                        return false;
                    }
                    Command::SuggestPreference {
                        scope,
                        key: self.operator.key.clone(),
                        value: self.operator.value.clone(),
                        source: self.operator.source.clone(),
                        origin: Origin::AgentReported,
                        expected: Expected::Absent,
                    }
                } else {
                    let Some(page) = self.operator.page.as_ref() else {
                        return false;
                    };
                    let Some(r) = self.operator.record() else {
                        return false;
                    };
                    if r.status != "suggested" || r.scope != scope || page.binding != binding {
                        return false;
                    }
                    if action == "confirm" {
                        Command::ConfirmPreference {
                            id: r.id,
                            scope,
                            proposal_digest: r.proposal_digest.clone(),
                            channel: Channel::Cli,
                            expected: Expected::Revision(r.revision),
                        }
                    } else {
                        Command::RejectPreference {
                            id: r.id,
                            scope,
                            proposal_digest: r.proposal_digest.clone(),
                            channel: Channel::Cli,
                            expected: Expected::Revision(r.revision),
                        }
                    }
                };
                let mutation = Mutation {
                    version: 2,
                    binding,
                    operation_id: operation_id.into(),
                    command,
                };
                ("profile.mutate", json!({}), Some(mutation))
            }
            _ => return false,
        };
        let record = if action == "confirm" || action == "reject" {
            self.operator.record().cloned()
        } else {
            None
        };
        self.operator.review = Some(Review {
            record,
            guard,
            method: method.into(),
            params,
            mutation,
        });
        true
    }
    pub fn operator_confirm(&mut self) -> Option<Review> {
        if self.operator.busy {
            return None;
        }
        let review = self.operator.review.take()?;
        if !self.operator_current(&review)
            || review
                .record
                .as_ref()
                .is_some_and(|r| self.operator.record() != Some(r))
        {
            return None;
        }
        self.operator.busy = true;
        if review.mutation.is_some() {
            self.operator.receipt = None;
            self.operator.pending = Some(review.clone());
        }
        Some(review)
    }
    pub fn operator_retry(&mut self) -> Option<Review> {
        if self.operator.busy || !self.operator.outcome_unknown {
            return None;
        }
        let review = self.operator.pending.clone()?;
        if !self.operator_current(&review) {
            return None;
        }
        self.operator.busy = true;
        Some(review)
    }
    pub fn complete_operator(&mut self, request: &Review, result: Result<Value, &str>) -> bool {
        if !self.operator_current(request) {
            return false;
        }
        self.operator.busy = false;
        self.operator.review = None;
        let result = result.and_then(|v| {
            match request.method.as_str() {
                "profile.preferences.list" => {
                    let page: PreferencePage =
                        serde_json::from_value(v).map_err(|_| "InvalidRequest")?;
                    if page.records.len() > 12
                        || page.records.iter().any(|r| {
                            r.scope != request.guard.scope
                                || r.id <= request.guard.after_id
                                || r.revision <= 0
                        })
                    {
                        return Err("InvalidRequest");
                    }
                    if self.operator.info.as_ref().and_then(|i| i.binding.as_ref())
                        != Some(&page.binding)
                    {
                        return Err("StaleGeneration");
                    }
                    self.operator.selected_id = None;
                    self.operator.page = Some(page);
                }
                "profile.handoff.get" => {
                    let preview: HandoffPreview =
                        serde_json::from_value(v).map_err(|_| "InvalidRequest")?;
                    if preview.record.as_ref().is_some_and(|r| {
                        Some(&r.project) != request.guard.project.as_ref()
                            || r.task_id != request.guard.task
                            || r.revision <= 0
                    }) {
                        return Err("InvalidRequest");
                    }
                    if self.operator.info.as_ref().and_then(|i| i.binding.as_ref())
                        != Some(&preview.binding)
                    {
                        return Err("StaleGeneration");
                    }
                    self.operator.handoff = Some(preview);
                    self.operator.handoff_page = 0;
                }
                "profile.mutate" => {
                    let receipt: Receipt =
                        serde_json::from_value(v).map_err(|_| "OutcomeUnknown")?;
                    let mutation = request.mutation.as_ref().ok_or("OutcomeUnknown")?;
                    if receipt.operation_id != mutation.operation_id
                        || receipt.generation != mutation.binding.generation
                        || receipt.commit_sequence <= 0
                    {
                        return Err("OutcomeUnknown");
                    }
                    self.operator.receipt = Some(receipt);
                    self.operator.pending = None;
                    self.operator.outcome_unknown = false;
                    self.operator.page = None;
                    self.operator.selected_id = None;
                }
                "profile.write.disable" => {
                    if v.get("write_enabled").and_then(Value::as_bool) != Some(false) {
                        return Err("OutcomeUnknown");
                    }
                    if let Some(info) = &mut self.operator.info {
                        info.write_enabled = false;
                    }
                    self.operator.page = None;
                }
                _ => {
                    let info: ProfileInfo =
                        serde_json::from_value(v).map_err(|_| "InvalidRequest")?;
                    if info.selected != info.binding.is_some()
                        || (!info.selected && info.write_enabled)
                    {
                        return Err("InvalidRequest");
                    }
                    if self.operator.info.as_ref().and_then(|i| i.binding.as_ref())
                        != info.binding.as_ref()
                    {
                        self.operator.page = None;
                        self.operator.selected_id = None;
                    }
                    self.operator.info = Some(info);
                }
            }
            Ok(())
        });
        self.operator.error = result.err().map(str::to_owned);
        if self.operator.error.is_some() {
            self.operator.outcome_unknown = request.mutation.is_some()
                && self.operator.error.as_deref() == Some("OutcomeUnknown");
            if !self.operator.outcome_unknown {
                self.operator.pending = None;
            }
            self.operator.info = None;
            self.operator.page = None;
            self.operator.handoff = None;
            self.operator.selected_id = None;
        }
        true
    }
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
impl crate::native::Inputs {
    pub fn operator_call(&self, request: &Review) -> Result<Value, &'static str> {
        use acp_ipc::{Location, operator};
        if self.wsl_distribution.is_some() || self.runtime_dir.is_none() {
            return Err("Unauthorized");
        }
        let location = Location {
            runtime_dir: self.runtime_dir.clone().ok_or("Unauthorized")?,
            endpoint: self.endpoint.clone().ok_or("Unauthorized")?,
        };
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Unavailable")?
            .block_on(async {
                if let Some(mutation) = &request.mutation {
                    operator::mutate(&location, mutation).await
                } else {
                    operator::control(&location, &request.method, request.params.clone()).await
                }
            });
        result.map_err(|e| match e {
            acp_ipc::Error::Busy => "Busy",
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
        })
    }
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub fn operation_id() -> acp_ipc::Result<String> {
    acp_ipc::operator::operation_id()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model() -> Model {
        let mut m = Model::fixture();
        m.select(0);
        m.connection.target = Target::Local;
        m.connection.runtime = std::env::temp_dir().to_str().unwrap().into();
        m.connection.endpoint = "synthetic".into();
        m.operator.open = true;
        m.operator.global = true;
        m.operator.info = Some(ProfileInfo {
            selected: true,
            binding: Some(Binding {
                profile_id: "a".repeat(32),
                generation: "b".repeat(32),
                owner_epoch: 1,
            }),
            write_enabled: true,
        });
        let binding = m.operator.info.as_ref().unwrap().binding.clone().unwrap();
        m.operator.page = Some(PreferencePage {
            binding,
            records: vec![PreferenceRecord {
                id: 1,
                scope: Scope::Global,
                key: "synthetic-key".into(),
                value: "synthetic-value".into(),
                source: "test".into(),
                status: "suggested".into(),
                revision: 1,
                origin: Origin::AgentReported,
                proposal_digest: digest_hex(&proposal_digest(
                    &Scope::Global,
                    None,
                    "synthetic-key",
                    "synthetic-value",
                    Origin::AgentReported,
                    "test",
                )),
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            }],
            next_after_id: None,
        });
        m.operator.selected_id = Some(1);
        m
    }
    #[test]
    fn no_implicit_grant_or_cross_target_writes() {
        let mut m = model();
        for target in [Target::Synthetic, Target::Wsl] {
            m.connection.target = target;
            assert!(!m.operator_review("confirm", "synthetic-op"));
            assert!(m.operator_read("profile.info").is_none());
        }
        m.connection.target = Target::Local;
        m.operator.info.as_mut().unwrap().write_enabled = false;
        assert!(!m.operator_review("confirm", "synthetic-op"));
        assert!(m.operator_review("enable", "synthetic-op"));
        assert!(!m.operator.info.as_ref().unwrap().write_enabled);
        let request = m.operator_confirm().unwrap();
        assert_eq!(request.method, "profile.write.enable");
        assert!(request.mutation.is_none());
    }
    #[test]
    fn exact_review_revision_and_same_request_only_uncertain_retry() {
        let mut m = model();
        assert!(m.operator_review("confirm", "keep-this-id"));
        let request = m.operator_confirm().unwrap();
        assert!(matches!(
            request.mutation.as_ref().unwrap().command,
            Command::ConfirmPreference {
                id: 1,
                expected: Expected::Revision(1),
                ..
            }
        ));
        assert!(m.operator_confirm().is_none());
        assert!(m.complete_operator(&request, Err("OutcomeUnknown")));
        assert!(m.operator.info.is_none());
        assert!(!m.operator_review("confirm", "new-id"));
        assert!(m.operator_read("profile.info").is_none());
        let retry = m.operator_retry().unwrap();
        assert_eq!(
            serde_json::to_vec(&request.mutation).unwrap(),
            serde_json::to_vec(&retry.mutation).unwrap()
        );
        let receipt = json!({"generation":"b".repeat(32),"operation_id":"keep-this-id","commit_sequence":2,"affected":[]});
        assert!(m.complete_operator(&retry, Ok(receipt)));
        assert!(!m.operator.outcome_unknown);
        assert!(m.operator.pending.is_none());
        assert_eq!(
            m.operator.receipt.as_ref().unwrap().operation_id,
            "keep-this-id"
        );
    }
    #[test]
    fn scope_changes_reject_stale_success_failure_review_and_retry() {
        for change in 0..5 {
            let mut m = model();
            assert!(m.operator_review("confirm", "stable"));
            let request = m.operator_confirm().unwrap();
            assert!(m.complete_operator(&request, Err("OutcomeUnknown")));
            match change {
                0 => m.epoch += 1,
                1 => m.connection.endpoint = "different".into(),
                2 => m.selected_project = Some("beta".into()),
                3 => m.operator.task = "different-task".into(),
                _ => {
                    m.operator.serial += 1;
                    m.operator.selected_id = None;
                }
            }
            assert!(m.operator_retry().is_none());
            assert!(!m.complete_operator(&request, Err("Unauthorized")));
            assert!(!m.complete_operator(&request, Ok(json!({}))));
            assert_eq!(
                m.operator
                    .pending
                    .as_ref()
                    .unwrap()
                    .mutation
                    .as_ref()
                    .unwrap()
                    .operation_id,
                "stable"
            );
        }
    }
    #[test]
    fn malformed_preview_and_wrong_scope_fail_closed() {
        let mut m = model();
        let r = m.operator_read("profile.preferences.list").unwrap();
        let mut page = m.operator.page.clone().unwrap();
        page.records[0].scope = Scope::Project {
            name: "beta".into(),
        };
        assert!(m.complete_operator(&r, Ok(serde_json::to_value(page).unwrap())));
        assert!(m.operator.page.is_none());
        assert!(m.operator.info.is_none());
        assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
    }
    #[test]
    fn disable_ack_and_explicit_denial_never_report_success() {
        let mut m = model();
        assert!(m.operator_review("disable", "unused"));
        let r = m.operator_confirm().unwrap();
        assert!(
            m.operator.info.as_ref().unwrap().write_enabled,
            "not disabled until ack"
        );
        assert!(m.complete_operator(&r, Ok(json!({"write_enabled":false}))));
        assert!(!m.operator.info.as_ref().unwrap().write_enabled);
        for error in [
            "Unauthorized",
            "Busy",
            "Conflict",
            "StaleGeneration",
            "RestartRequired",
            "Unavailable",
        ] {
            let mut m = model();
            assert!(m.operator_review("confirm", "once"));
            let r = m.operator_confirm().unwrap();
            assert!(m.complete_operator(&r, Err(error)));
            assert!(m.operator.receipt.is_none());
            assert!(m.operator.info.is_none());
            assert!(!m.operator.busy);
            assert_eq!(m.operator.error.as_deref(), Some(error));
            assert!(!m.operator.outcome_unknown);
        }
    }
    #[test]
    fn changed_generation_or_review_state_cannot_be_confirmed() {
        let mut m = model();
        m.operator.page.as_mut().unwrap().binding.owner_epoch = 2;
        assert!(!m.operator_review("confirm", "once"));
        m.operator.page.as_mut().unwrap().binding.owner_epoch = 1;
        m.operator.page.as_mut().unwrap().records[0].status = "confirmed".into();
        assert!(!m.operator_review("confirm", "once"));
        assert!(!m.operator_review("reject", "once"));
    }
}

#[cfg(all(test, feature = "native", not(target_arch = "wasm32")))]
mod native_boundary_tests {
    use super::*;
    #[test]
    fn wsl_and_offline_never_attempt_operator_ipc_even_with_a_prepared_request() {
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
        let inputs = crate::native::Inputs {
            runtime_dir: Some(std::env::temp_dir()),
            endpoint: Some("synthetic".into()),
            wsl_distribution: Some("synthetic".into()),
            ..Default::default()
        };
        assert_eq!(inputs.operator_call(&request), Err("Unauthorized"));
    }
}
