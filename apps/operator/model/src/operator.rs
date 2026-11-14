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
/// Review navigation shared by Desktop and TUI; rendering stays per-surface.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ReviewTab {
    #[default]
    Queue,
    Detail,
    History,
}
/// Destructive-tier arm. `retype` must equal the exact id text and the armed
/// identity must still match the reviewed record before a delete can prepare.
#[derive(Clone, PartialEq, Eq)]
pub struct DeleteArm {
    pub retype: String,
    pub id: i64,
    pub revision: i64,
    pub proposal_digest: String,
}
impl std::fmt::Debug for DeleteArm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeleteArm(redacted)")
    }
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
    pub channel: Channel,
    pub review_tab: ReviewTab,
    pub queue: Option<ReviewQueue>,
    pub inspect: Option<ReviewInspect>,
    /// Queue paging cursor; paging is not a stable snapshot.
    pub queue_after_id: i64,
    /// Baseline commit_sequence of the current queue paging session. The delta
    /// to `queue.commit_sequence` discloses drift; it is never hidden.
    pub queue_watermark: i64,
    /// Revision of the held exact inspect snapshot, bound into the guard.
    pub inspect_revision: i64,
    pub digest_expanded: bool,
    pub delete_arm: Option<DeleteArm>,
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
    queue_after_id: i64,
    queue_watermark: i64,
    inspect_revision: i64,
    inspect_digest: Option<String>,
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
    /// The exact selected record paired with the binding of the snapshot that
    /// sourced it. A held exact inspect snapshot outranks list/queue pages for
    /// the same id, so Detail never acts on a superseded page row.
    fn selected(&self) -> Option<(&PreferenceRecord, &Binding)> {
        let id = self.selected_id?;
        if let Some(i) = &self.inspect
            && i.record.id == id
        {
            return Some((&i.record, &i.binding));
        }
        if let Some(p) = &self.page
            && let Some(r) = p.records.iter().find(|r| r.id == id)
        {
            return Some((r, &p.binding));
        }
        self.queue.as_ref().and_then(|q| {
            q.records
                .iter()
                .find(|r| r.id == id)
                .map(|r| (r, &q.binding))
        })
    }
    pub fn record(&self) -> Option<&PreferenceRecord> {
        self.selected().map(|(r, _)| r)
    }
    /// Delete is armed only against one exact id+revision+digest with the id
    /// retyped verbatim; any drift disarms instead of widening the permission.
    fn delete_armed_for(&self, r: &PreferenceRecord) -> bool {
        self.delete_arm.as_ref().is_some_and(|a| {
            a.retype == r.id.to_string()
                && a.id == r.id
                && a.revision == r.revision
                && a.proposal_digest == r.proposal_digest
        })
    }
    /// The arm survives only while a current snapshot reconfirms the exact
    /// armed identity; a refreshed snapshot that no longer does disarms it.
    fn revalidate_delete_arm(&mut self) {
        let Some(a) = self.delete_arm.as_ref() else {
            return;
        };
        let exact = |r: &PreferenceRecord| {
            r.id == a.id && r.revision == a.revision && r.proposal_digest == a.proposal_digest
        };
        let confirmed = self.inspect.as_ref().is_some_and(|i| exact(&i.record))
            || self
                .queue
                .as_ref()
                .is_some_and(|q| q.records.iter().any(exact))
            || self
                .page
                .as_ref()
                .is_some_and(|p| p.records.iter().any(exact));
        if !confirmed {
            self.delete_arm = None;
        }
    }
    fn clear_review_snapshots(&mut self) {
        self.queue = None;
        self.inspect = None;
        self.queue_after_id = 0;
        self.queue_watermark = 0;
        self.inspect_revision = 0;
        self.digest_expanded = false;
        self.delete_arm = None;
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
        self.clear_review_snapshots();
        self.review_tab = ReviewTab::Queue;
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
            queue_after_id: self.operator.queue_after_id,
            queue_watermark: self.operator.queue_watermark,
            inspect_revision: self.operator.inspect_revision,
            inspect_digest: self
                .operator
                .inspect
                .as_ref()
                .map(|i| i.record.proposal_digest.clone()),
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
            "profile.review.queue" => {
                if self.operator.queue_after_id < 0 {
                    return None;
                }
                json!({"scope":self.operator_scope()?,"after_id":self.operator.queue_after_id})
            }
            "profile.review.inspect" => {
                let scope = self.operator_scope()?;
                let (r, _) = self.operator.selected()?;
                if r.scope != scope || r.id <= 0 || r.revision <= 0 {
                    return None;
                }
                json!({"scope":scope,"id":r.id,"revision":r.revision})
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
            "suggest" | "confirm" | "reject" | "deprecate" | "delete" if info.write_enabled => {
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
                    let Some((r, source)) = self.operator.selected() else {
                        return false;
                    };
                    if r.scope != scope || *source != binding {
                        return false;
                    }
                    // Deprecate only retires a confirmed row; confirm/reject
                    // only decide a suggested one. Delete is armed separately.
                    match action {
                        "confirm" | "reject" if r.status != "suggested" => return false,
                        "deprecate" if r.status != "confirmed" => return false,
                        "delete" if !self.operator.delete_armed_for(r) => return false,
                        _ => {}
                    }
                    let id = r.id;
                    let proposal_digest = r.proposal_digest.clone();
                    let channel = self.operator.channel;
                    let expected = Expected::Revision(r.revision);
                    match action {
                        "confirm" => Command::ConfirmPreference {
                            id,
                            scope,
                            proposal_digest,
                            channel,
                            expected,
                        },
                        "reject" => Command::RejectPreference {
                            id,
                            scope,
                            proposal_digest,
                            channel,
                            expected,
                        },
                        "deprecate" => Command::DeprecatePreference {
                            id,
                            scope,
                            proposal_digest,
                            channel,
                            expected,
                        },
                        _ => Command::DeletePreference {
                            id,
                            scope,
                            proposal_digest,
                            channel,
                            expected,
                        },
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
        let record = if matches!(action, "confirm" | "reject" | "deprecate" | "delete") {
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
        // A locally revoked write grant refuses a prepared mutation before any
        // submit. An absent info (uncertain outcome) stays daemon-authoritative.
        if review.mutation.is_some()
            && self
                .operator
                .info
                .as_ref()
                .is_some_and(|i| !i.write_enabled)
        {
            return None;
        }
        // A destructive apply rechecks the arm at submit time: a cleared or
        // drifted retype between prepare and apply refuses instead of writing.
        if matches!(
            review.mutation.as_ref().map(|m| &m.command),
            Some(Command::DeletePreference { .. })
        ) && !review
            .record
            .as_ref()
            .is_some_and(|r| self.operator.delete_armed_for(r))
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
                    self.operator.revalidate_delete_arm();
                }
                "profile.review.queue" => {
                    let queue: ReviewQueue =
                        serde_json::from_value(v).map_err(|_| "InvalidRequest")?;
                    let after_id = request.guard.queue_after_id;
                    if queue.commit_sequence < 0
                        || queue.records.len() > 12
                        || queue.records.iter().any(|r| {
                            r.scope != request.guard.scope
                                || r.id <= after_id
                                || r.status != "suggested"
                                || r.revision <= 0
                                || digest_from_hex(&r.proposal_digest).is_none()
                        })
                        || queue.records.windows(2).any(|w| w[0].id >= w[1].id)
                    {
                        return Err("InvalidRequest");
                    }
                    if let Some(next) = queue.next_after_id
                        && (queue.records.len() != 12
                            || queue.records.last().map(|r| r.id) != Some(next))
                    {
                        return Err("InvalidRequest");
                    }
                    // Paging is not a snapshot: a later page may never report a
                    // watermark below the disclosed baseline of this session.
                    if after_id > 0 && queue.commit_sequence < request.guard.queue_watermark {
                        return Err("InvalidRequest");
                    }
                    if self.operator.info.as_ref().and_then(|i| i.binding.as_ref())
                        != Some(&queue.binding)
                    {
                        return Err("StaleGeneration");
                    }
                    if after_id == 0 {
                        self.operator.queue_watermark = queue.commit_sequence;
                    }
                    self.operator.queue = Some(queue);
                    if self.operator.selected_id.is_some() && self.operator.record().is_none() {
                        self.operator.selected_id = None;
                    }
                    self.operator.revalidate_delete_arm();
                }
                "profile.review.inspect" => {
                    let inspect: ReviewInspect =
                        serde_json::from_value(v).map_err(|_| "InvalidRequest")?;
                    let id = request
                        .params
                        .get("id")
                        .and_then(Value::as_i64)
                        .ok_or("InvalidRequest")?;
                    let revision = request
                        .params
                        .get("revision")
                        .and_then(Value::as_i64)
                        .ok_or("InvalidRequest")?;
                    let r = &inspect.record;
                    if inspect.commit_sequence < 0
                        || (request.guard.queue_watermark > 0
                            && inspect.commit_sequence < request.guard.queue_watermark)
                        || r.id != id
                        || r.revision != revision
                        || r.revision <= 0
                        || r.scope != request.guard.scope
                        || digest_from_hex(&r.proposal_digest).is_none()
                        || inspect.decisions.len() > 8
                        || inspect.decisions.iter().any(|d| {
                            d.preference_id != id
                                || d.from_revision <= 0
                                || d.to_revision <= 0
                                || d.receipt_commit_sequence <= 0
                                || digest_from_hex(&d.proposal_digest).is_none()
                        })
                        || inspect.peer_demotions.len() > 12
                        || inspect.peer_demotions.len() as i64
                            != inspect.peer_demotion_total.min(12)
                        || inspect
                            .peer_demotions
                            .iter()
                            .any(|p| p.id <= 0 || p.id == id || p.revision <= 0)
                    {
                        return Err("InvalidRequest");
                    }
                    if self.operator.info.as_ref().and_then(|i| i.binding.as_ref())
                        != Some(&inspect.binding)
                    {
                        return Err("StaleGeneration");
                    }
                    self.operator.inspect = Some(inspect);
                    self.operator.inspect_revision = revision;
                    self.operator.digest_expanded = false;
                    self.operator.revalidate_delete_arm();
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
                    // A delete receipt must carry the identity-only tombstone of
                    // the exact deleted row; anything else is not a delete proof.
                    if let Command::DeletePreference { id, .. } = &mutation.command
                        && !receipt.affected.iter().any(|a| {
                            a.kind == "preference"
                                && a.record_id == id.to_string()
                                && a.deleted
                                && a.revision > 0
                        })
                    {
                        return Err("OutcomeUnknown");
                    }
                    self.operator.receipt = Some(receipt);
                    self.operator.pending = None;
                    self.operator.outcome_unknown = false;
                    self.operator.page = None;
                    self.operator.selected_id = None;
                    // Content and decision history snapshots never survive a
                    // commit; the receipt alone remains as identity-only proof.
                    self.operator.clear_review_snapshots();
                }
                "profile.write.disable" => {
                    if v.get("write_enabled").and_then(Value::as_bool) != Some(false) {
                        return Err("OutcomeUnknown");
                    }
                    if let Some(info) = &mut self.operator.info {
                        info.write_enabled = false;
                    }
                    self.operator.page = None;
                    self.operator.clear_review_snapshots();
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
                        self.operator.clear_review_snapshots();
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
                self.operator.clear_review_snapshots();
            }
            // An uncertain mutation keeps its guard identity (watermark,
            // inspected revision/digest) so the identical retry can settle it.
            self.operator.info = None;
            self.operator.page = None;
            self.operator.handoff = None;
            self.operator.selected_id = None;
        }
        // A settled request is fenced off: a duplicate or delayed completion can
        // never apply twice. Only an uncertain mutation keeps its guard so the
        // identical same-epoch retry can still settle it once.
        if !self.operator.outcome_unknown {
            self.operator.serial += 1;
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
    fn binding() -> Binding {
        Binding {
            profile_id: "a".repeat(32),
            generation: "b".repeat(32),
            owner_epoch: 1,
        }
    }
    fn synthetic_record(id: i64, scope: &Scope, status: &str, revision: i64) -> PreferenceRecord {
        let key = format!("synthetic-key-{id}");
        let project = match scope {
            Scope::Project { name } => Some(name.as_str()),
            Scope::Global => None,
        };
        PreferenceRecord {
            id,
            scope: scope.clone(),
            key: key.clone(),
            value: "synthetic-value".into(),
            source: "test".into(),
            status: status.into(),
            revision,
            origin: Origin::AgentReported,
            proposal_digest: digest_hex(&proposal_digest(
                scope,
                project,
                &key,
                "synthetic-value",
                Origin::AgentReported,
                "test",
            )),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }
    fn queue_of(
        records: Vec<PreferenceRecord>,
        commit_sequence: i64,
        next_after_id: Option<i64>,
    ) -> ReviewQueue {
        ReviewQueue {
            binding: binding(),
            commit_sequence,
            records,
            next_after_id,
        }
    }
    fn complete_queue(m: &mut Model, queue: ReviewQueue) -> bool {
        let request = m.operator_read("profile.review.queue").unwrap();
        m.complete_operator(&request, Ok(serde_json::to_value(queue).unwrap()))
    }
    fn decision(preference_id: i64, action: DecisionAction, seq: i64) -> ReviewDecisionRecord {
        ReviewDecisionRecord {
            preference_id,
            action,
            from_revision: 1,
            to_revision: 2,
            proposal_digest: "c".repeat(64),
            receipt_commit_sequence: seq,
            channel: Channel::Desktop,
            decided_at: "2026-01-01T00:00:00Z".into(),
        }
    }
    fn inspect_of(
        record: PreferenceRecord,
        decisions: Vec<ReviewDecisionRecord>,
        peer_demotions: Vec<PeerDemotion>,
        peer_demotion_total: i64,
    ) -> ReviewInspect {
        ReviewInspect {
            binding: binding(),
            commit_sequence: 6,
            record,
            decisions,
            peer_demotions,
            peer_demotion_total,
        }
    }
    fn complete_inspect(m: &mut Model, inspect: ReviewInspect) -> bool {
        let request = m.operator_read("profile.review.inspect").unwrap();
        m.complete_operator(&request, Ok(serde_json::to_value(inspect).unwrap()))
    }
    /// Queue-sourced selection with an exact inspect snapshot, no preferences page.
    fn inspect_model() -> Model {
        let mut m = model();
        m.operator.page = None;
        assert!(complete_queue(
            &mut m,
            queue_of(
                vec![synthetic_record(7, &Scope::Global, "suggested", 1)],
                5,
                None
            )
        ));
        m.operator.selected_id = Some(7);
        m
    }
    fn armed_delete_model() -> Model {
        let mut m = inspect_model();
        assert!(complete_inspect(
            &mut m,
            inspect_of(
                synthetic_record(7, &Scope::Global, "suggested", 1),
                vec![],
                vec![],
                0
            )
        ));
        m.operator.delete_arm = Some(DeleteArm {
            retype: "7".into(),
            id: 7,
            revision: 1,
            proposal_digest: m.operator.record().unwrap().proposal_digest.clone(),
        });
        m
    }
    fn receipt_json(operation_id: &str, commit_sequence: i64, affected: Value) -> Value {
        json!({"generation":"b".repeat(32),"operation_id":operation_id,"commit_sequence":commit_sequence,"affected":affected})
    }
    #[test]
    fn queue_empty_paging_watermark_drift_and_malformed_pages() {
        let mut m = model();
        // An empty queue is a valid pending-review state, never an error.
        assert!(complete_queue(&mut m, queue_of(vec![], 3, None)));
        assert!(m.operator.error.is_none());
        assert!(m.operator.queue.as_ref().unwrap().records.is_empty());
        assert_eq!(m.operator.queue_watermark, 3);
        // The first page establishes the drift baseline.
        let page1: Vec<_> = (1..=12)
            .map(|id| synthetic_record(id, &Scope::Global, "suggested", 1))
            .collect();
        assert!(complete_queue(&mut m, queue_of(page1.clone(), 5, Some(12))));
        assert_eq!(m.operator.queue_watermark, 5);
        // A later page discloses drift against the baseline, never rebases it.
        m.operator.queue_after_id = 12;
        assert!(complete_queue(
            &mut m,
            queue_of(
                vec![synthetic_record(13, &Scope::Global, "suggested", 1)],
                9,
                None
            )
        ));
        assert_eq!(m.operator.queue_watermark, 5);
        assert_eq!(m.operator.queue.as_ref().unwrap().commit_sequence, 9);
        // A later page below the disclosed baseline fails closed.
        m.operator.queue_after_id = 13;
        assert!(complete_queue(&mut m, queue_of(vec![], 4, None)));
        assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
        assert!(m.operator.queue.is_none());
        // A negative cursor never prepares a read.
        let mut m = model();
        m.operator.queue_after_id = -1;
        assert!(m.operator_read("profile.review.queue").is_none());
        let oversized: Vec<_> = (1..=13)
            .map(|id| synthetic_record(id, &Scope::Global, "suggested", 1))
            .collect();
        let mut descending = page1[..2].to_vec();
        descending.reverse();
        for bad in [
            queue_of(oversized, 5, Some(12)),
            queue_of(
                vec![synthetic_record(1, &Scope::Global, "confirmed", 1)],
                5,
                None,
            ),
            queue_of(descending, 5, None),
            queue_of(page1.clone(), 5, Some(11)),
            queue_of(
                vec![synthetic_record(
                    1,
                    &Scope::Project {
                        name: "alpha".into(),
                    },
                    "suggested",
                    1,
                )],
                5,
                None,
            ),
        ] {
            let mut m = model();
            assert!(complete_queue(&mut m, bad));
            assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
            assert!(m.operator.queue.is_none());
        }
        // Rows at or below the paging cursor are refused.
        let mut m = model();
        m.operator.queue_after_id = 5;
        assert!(complete_queue(
            &mut m,
            queue_of(
                vec![synthetic_record(3, &Scope::Global, "suggested", 1)],
                5,
                None
            )
        ));
        assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
        // A moved generation or owner epoch is stale, never applied.
        for moved in [
            Binding {
                generation: "c".repeat(32),
                ..binding()
            },
            Binding {
                owner_epoch: 2,
                ..binding()
            },
        ] {
            let mut m = model();
            let mut queue = queue_of(vec![], 5, None);
            queue.binding = moved;
            assert!(complete_queue(&mut m, queue));
            assert_eq!(m.operator.error.as_deref(), Some("StaleGeneration"));
            assert!(m.operator.queue.is_none());
        }
    }
    #[test]
    fn inspect_exact_revision_history_bound_and_peer_preview() {
        let mut m = inspect_model();
        let request = m.operator_read("profile.review.inspect").unwrap();
        assert_eq!(request.params["id"], json!(7));
        assert_eq!(request.params["revision"], json!(1));
        assert_eq!(request.params["scope"], json!({"kind":"global"}));
        let record = synthetic_record(7, &Scope::Global, "suggested", 1);
        let decisions: Vec<_> = (1..=8)
            .map(|seq| decision(7, DecisionAction::Confirm, seq))
            .collect();
        let peers: Vec<_> = (1..=12)
            .map(|i| PeerDemotion {
                id: 100 + i,
                revision: 1,
            })
            .collect();
        assert!(m.complete_operator(
            &request,
            Ok(serde_json::to_value(inspect_of(record, decisions, peers, 20)).unwrap())
        ));
        assert!(m.operator.error.is_none());
        assert_eq!(m.operator.inspect_revision, 1);
        assert!(!m.operator.digest_expanded);
        assert_eq!(m.operator.record().unwrap().id, 7);
        // A truncated preview must disclose the full total; exact totals match.
        let mut m = inspect_model();
        assert!(complete_inspect(
            &mut m,
            inspect_of(
                synthetic_record(7, &Scope::Global, "suggested", 1),
                vec![],
                vec![PeerDemotion {
                    id: 100,
                    revision: 1
                }],
                1
            )
        ));
        assert!(m.operator.error.is_none());
        let record = synthetic_record(7, &Scope::Global, "suggested", 1);
        let nine: Vec<_> = (1..=9)
            .map(|seq| decision(7, DecisionAction::Reject, seq))
            .collect();
        let mut wrong_id = decision(8, DecisionAction::Confirm, 1);
        wrong_id.preference_id = 8;
        for bad in [
            // A moved revision is a conflict, never a silently different preview.
            inspect_of(
                synthetic_record(7, &Scope::Global, "suggested", 2),
                vec![],
                vec![],
                0,
            ),
            inspect_of(record.clone(), nine, vec![], 0),
            inspect_of(record.clone(), vec![wrong_id], vec![], 0),
            inspect_of(
                record.clone(),
                vec![],
                vec![PeerDemotion {
                    id: 100,
                    revision: 1,
                }],
                20,
            ),
            inspect_of(
                record.clone(),
                vec![],
                vec![PeerDemotion { id: 7, revision: 1 }],
                1,
            ),
            inspect_of(record.clone(), vec![], vec![], -1),
            inspect_of(
                synthetic_record(8, &Scope::Global, "suggested", 1),
                vec![],
                vec![],
                0,
            ),
        ] {
            let mut m = inspect_model();
            assert!(complete_inspect(&mut m, bad));
            assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
            assert!(m.operator.inspect.is_none());
        }
    }
    #[test]
    fn project_and_global_scope_isolation() {
        let mut m = model();
        // A global queue never accepts project rows.
        assert!(complete_queue(
            &mut m,
            queue_of(
                vec![synthetic_record(
                    1,
                    &Scope::Project {
                        name: "alpha".into()
                    },
                    "suggested",
                    1
                )],
                5,
                None
            )
        ));
        assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
        // Project scope names the exact project in both read params.
        let mut m = model();
        m.operator.global = false;
        m.operator.page = None;
        let scope = Scope::Project {
            name: "alpha".into(),
        };
        let request = m.operator_read("profile.review.queue").unwrap();
        assert_eq!(
            request.params["scope"],
            json!({"kind":"project","name":"alpha"})
        );
        assert!(
            m.complete_operator(
                &request,
                Ok(serde_json::to_value(queue_of(
                    vec![synthetic_record(1, &Scope::Global, "suggested", 1)],
                    5,
                    None
                ))
                .unwrap())
            )
        );
        assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
        // With a fresh info read, project rows are accepted in project scope.
        let mut m = model();
        m.operator.global = false;
        m.operator.page = None;
        assert!(complete_queue(
            &mut m,
            queue_of(vec![synthetic_record(1, &scope, "suggested", 1)], 5, None)
        ));
        m.operator.selected_id = Some(1);
        let request = m.operator_read("profile.review.inspect").unwrap();
        assert_eq!(
            request.params["scope"],
            json!({"kind":"project","name":"alpha"})
        );
        assert!(
            m.complete_operator(
                &request,
                Ok(serde_json::to_value(inspect_of(
                    synthetic_record(1, &scope, "suggested", 1),
                    vec![],
                    vec![],
                    0
                ))
                .unwrap())
            )
        );
        assert!(m.operator.error.is_none());
        // A cross-scope record can never prepare a decision.
        m.operator.page = Some(PreferencePage {
            binding: binding(),
            records: vec![synthetic_record(9, &Scope::Global, "suggested", 1)],
            next_after_id: None,
        });
        m.operator.selected_id = Some(9);
        assert!(!m.operator_review("confirm", "cross"));
        assert!(!m.operator_review("delete", "cross"));
    }
    #[test]
    fn deprecate_requires_confirmed_exact_identity() {
        let mut m = model();
        assert!(!m.operator_review("deprecate", "op"));
        m.operator.page.as_mut().unwrap().records[0].status = "confirmed".into();
        m.operator.page.as_mut().unwrap().records[0].revision = 2;
        m.operator.channel = Channel::Tui;
        assert!(m.operator_review("deprecate", "dep-op"));
        let request = m.operator_confirm().unwrap();
        let digest = m.operator.page.as_ref().unwrap().records[0]
            .proposal_digest
            .clone();
        match &request.mutation.as_ref().unwrap().command {
            Command::DeprecatePreference {
                id,
                scope,
                proposal_digest,
                channel,
                expected,
            } => {
                assert_eq!(*id, 1);
                assert!(*scope == Scope::Global);
                assert_eq!(*proposal_digest, digest);
                assert_eq!(*channel, Channel::Tui);
                assert!(*expected == Expected::Revision(2));
            }
            _ => panic!("expected DeprecatePreference"),
        }
        assert!(m.complete_operator(
            &request,
            Ok(receipt_json("dep-op", 3, json!([{"kind":"preference","project":"","record_id":"1","revision":3,"deleted":false}])))
        ));
        assert!(m.operator.receipt.is_some());
        assert!(m.operator.page.is_none());
        // A suggested queue row can never be deprecated.
        let mut m = inspect_model();
        assert!(!m.operator_review("deprecate", "op"));
        // Digest or revision drift between prepare and apply refuses the submit.
        for drift in 0..2 {
            let mut m = model();
            m.operator.page.as_mut().unwrap().records[0].status = "confirmed".into();
            assert!(m.operator_review("deprecate", "drift"));
            let record = &mut m.operator.page.as_mut().unwrap().records[0];
            if drift == 0 {
                record.proposal_digest = "d".repeat(64);
            } else {
                record.revision += 1;
            }
            assert!(m.operator_confirm().is_none());
        }
    }
    #[test]
    fn delete_requires_exact_retype_and_armed_identity() {
        let mut m = armed_delete_model();
        let digest = m.operator.record().unwrap().proposal_digest.clone();
        // Unarmed delete never prepares.
        m.operator.delete_arm = None;
        assert!(!m.operator_review("delete", "op"));
        // Wrong, padded or stale retype text never arms the destructive tier.
        for retype in ["", "6", "07", " 7", "7 ", "seven"] {
            m.operator.delete_arm = Some(DeleteArm {
                retype: retype.into(),
                id: 7,
                revision: 1,
                proposal_digest: digest.clone(),
            });
            assert!(!m.operator_review("delete", "op"), "retype {retype:?}");
        }
        // A stale armed revision or digest refuses even with the exact text.
        m.operator.delete_arm = Some(DeleteArm {
            retype: "7".into(),
            id: 7,
            revision: 2,
            proposal_digest: digest.clone(),
        });
        assert!(!m.operator_review("delete", "op"));
        m.operator.delete_arm = Some(DeleteArm {
            retype: "7".into(),
            id: 7,
            revision: 1,
            proposal_digest: "d".repeat(64),
        });
        assert!(!m.operator_review("delete", "op"));
        // The exact arm prepares the destructive command on the reviewed tuple.
        m.operator.delete_arm = Some(DeleteArm {
            retype: "7".into(),
            id: 7,
            revision: 1,
            proposal_digest: digest.clone(),
        });
        assert!(m.operator_review("delete", "del-op"));
        let request = m.operator_confirm().unwrap();
        assert!(matches!(
            request.mutation.as_ref().unwrap().command,
            Command::DeletePreference {
                id: 7,
                expected: Expected::Revision(1),
                ..
            }
        ));
        // A cleared or edited arm between prepare and apply refuses the submit.
        for clear in 0..2 {
            let mut m = armed_delete_model();
            assert!(m.operator_review("delete", "del-op"));
            if clear == 0 {
                m.operator.delete_arm = None;
            } else {
                m.operator.delete_arm.as_mut().unwrap().retype = "8".into();
            }
            assert!(m.operator_confirm().is_none());
        }
    }
    #[test]
    fn delete_receipt_tombstone_clears_content_and_history() {
        let mut m = armed_delete_model();
        assert!(m.operator_review("delete", "del-op"));
        let request = m.operator_confirm().unwrap();
        let tombstone =
            json!([{"kind":"preference","project":"","record_id":"7","revision":2,"deleted":true}]);
        // A receipt without the identity-only tombstone is not a delete proof.
        assert!(m.complete_operator(&request, Ok(receipt_json("del-op", 7, json!([])))));
        assert!(m.operator.outcome_unknown);
        assert!(m.operator.receipt.is_none());
        // The identical same-epoch retry settles once with the real tombstone.
        let retry = m.operator_retry().unwrap();
        assert!(m.complete_operator(&retry, Ok(receipt_json("del-op", 7, tombstone))));
        assert!(!m.operator.outcome_unknown);
        assert_eq!(m.operator.receipt.as_ref().unwrap().operation_id, "del-op");
        // Content and decision-history snapshots never survive the deletion.
        assert!(m.operator.queue.is_none());
        assert!(m.operator.inspect.is_none());
        assert!(m.operator.page.is_none());
        assert!(m.operator.selected_id.is_none());
        assert!(m.operator.delete_arm.is_none());
        assert_eq!(m.operator.inspect_revision, 0);
        // A duplicate settlement of the same delete is refused.
        assert!(!m.complete_operator(&retry, Ok(receipt_json("del-op", 7, json!([])))));
    }
    #[test]
    fn stale_identity_changes_fence_reads_and_armed_actions() {
        // In-flight queue reads are refused after any identity change.
        for change in 0..10 {
            let mut m = model();
            let request = m.operator_read("profile.review.queue").unwrap();
            match change {
                0 => m.epoch += 1,
                1 => m.connection.endpoint = "different".into(),
                2 => m.selected_project = Some("beta".into()),
                3 => m.operator.task = "different-task".into(),
                4 => m.operator.serial += 1,
                5 => m.operator.queue_after_id = 9,
                6 => m.operator.queue_watermark = 9,
                7 => m.operator.inspect_revision = 9,
                8 => m.operator.global = false,
                _ => m.operator.invalidate(),
            }
            assert!(!m.complete_operator(
                &request,
                Ok(serde_json::to_value(queue_of(vec![], 5, None)).unwrap())
            ));
            assert!(m.operator.queue.is_none());
        }
        // An armed delete cannot apply after scope, watermark, revision or
        // digest drift; a revoked write grant refuses every decision action.
        for change in 0..6 {
            let mut m = armed_delete_model();
            assert!(m.operator_review("delete", "fenced"));
            match change {
                0 => m.epoch += 1,
                1 => m.operator.global = false,
                2 => m.operator.queue_watermark = 99,
                3 => m.operator.inspect_revision = 99,
                4 => {
                    let mut drifted = m.operator.inspect.clone().unwrap();
                    drifted.record.proposal_digest = "d".repeat(64);
                    m.operator.inspect = Some(drifted);
                }
                _ => m.operator.info.as_mut().unwrap().write_enabled = false,
            }
            assert!(m.operator_confirm().is_none(), "change {change}");
        }
        // A revoked grant refuses new review preparations outright.
        let mut m = armed_delete_model();
        m.operator.info.as_mut().unwrap().write_enabled = false;
        assert!(!m.operator_review("delete", "revoked"));
        assert!(!m.operator_review("confirm", "revoked"));
        assert!(!m.operator_review("deprecate", "revoked"));
    }
    #[test]
    fn delayed_switch_duplicate_apply_and_cancel_refused() {
        // A project switch invalidates an in-flight queue read.
        let mut m = model();
        let request = m.operator_read("profile.review.queue").unwrap();
        m.select(1);
        assert!(!m.complete_operator(
            &request,
            Ok(serde_json::to_value(queue_of(vec![], 5, None)).unwrap())
        ));
        assert!(m.operator.queue.is_none());
        // An explicit cancel invalidates an in-flight inspect read.
        let mut m = inspect_model();
        let request = m.operator_read("profile.review.inspect").unwrap();
        m.operator.invalidate();
        assert!(
            !m.complete_operator(
                &request,
                Ok(serde_json::to_value(inspect_of(
                    synthetic_record(7, &Scope::Global, "suggested", 1),
                    vec![],
                    vec![],
                    0
                ))
                .unwrap())
            )
        );
        assert!(m.operator.inspect.is_none());
        // A settled read can never apply twice.
        let mut m = model();
        let request = m.operator_read("profile.review.queue").unwrap();
        let value = serde_json::to_value(queue_of(vec![], 5, None)).unwrap();
        assert!(m.complete_operator(&request, Ok(value.clone())));
        assert!(!m.complete_operator(&request, Ok(value)));
        // A settled mutation can never apply twice; the uncertain retry keeps
        // its identity and settles exactly once.
        let mut m = model();
        assert!(m.operator_review("confirm", "once"));
        let request = m.operator_confirm().unwrap();
        assert!(m.complete_operator(&request, Err("OutcomeUnknown")));
        let retry = m.operator_retry().unwrap();
        let receipt = receipt_json("once", 2, json!([]));
        assert!(m.complete_operator(&retry, Ok(receipt.clone())));
        assert!(!m.complete_operator(&retry, Ok(receipt.clone())));
        assert!(!m.complete_operator(&request, Ok(receipt)));
    }
    #[test]
    fn queue_sourced_selection_drives_detail_and_confirm() {
        let mut m = model();
        m.operator.page = None;
        m.operator.selected_id = Some(1);
        assert!(m.operator.record().is_none());
        assert!(m.operator_read("profile.review.inspect").is_none());
        assert!(complete_queue(
            &mut m,
            queue_of(
                vec![synthetic_record(1, &Scope::Global, "suggested", 1)],
                5,
                None
            )
        ));
        // Detail resolves the queue-sourced record without a preferences page.
        assert_eq!(m.operator.record().unwrap().id, 1);
        let request = m.operator_read("profile.review.inspect").unwrap();
        assert_eq!(request.params["id"], json!(1));
        assert_eq!(request.params["revision"], json!(1));
        assert!(
            m.complete_operator(
                &request,
                Ok(serde_json::to_value(inspect_of(
                    synthetic_record(1, &Scope::Global, "suggested", 1),
                    vec![],
                    vec![],
                    0
                ))
                .unwrap())
            )
        );
        // Confirm binds the exact inspected snapshot and its digest.
        assert!(m.operator_review("confirm", "q-op"));
        let request = m.operator_confirm().unwrap();
        assert!(matches!(
            request.mutation.as_ref().unwrap().command,
            Command::ConfirmPreference {
                id: 1,
                expected: Expected::Revision(1),
                ..
            }
        ));
        assert!(m.complete_operator(&request, Ok(receipt_json("q-op", 6, json!([])))));
        assert!(m.operator.queue.is_none());
        // A queue refresh that drops the selected record clears the selection
        // when no other snapshot still sources it.
        let mut m = model();
        m.operator.page = None;
        assert!(complete_queue(
            &mut m,
            queue_of(
                vec![synthetic_record(7, &Scope::Global, "suggested", 1)],
                5,
                None
            )
        ));
        m.operator.selected_id = Some(7);
        assert!(complete_queue(&mut m, queue_of(vec![], 6, None)));
        assert!(m.operator.selected_id.is_none());
        assert!(m.operator.record().is_none());
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
