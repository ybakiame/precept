//! One ephemeral operator workflow for Desktop and TUI, never a second Writer.
use crate::{
    Model,
    connection::{Connection, Target},
};
pub use precept_core::operator::*;
use serde_json::{Value, json};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Pane {
    /// Profile actions moved to the Connection page and Workspaces (direct,
    /// explicit calls); the review workspace starts on its records.
    Profile,
    #[default]
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
/// Which tab is active in the unified Review & preferences workspace.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ReviewMode {
    /// To review: pending suggestions across personal or current-project scope.
    #[default]
    Inbox,
    /// Personal preferences: global scope records across all projects.
    Personal,
    /// Project preferences: records scoped to the currently selected project.
    Project,
    /// Environment notes of the selected project: this machine's facts,
    /// never committed, never competing, always advisory.
    Environment,
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Operator {
    pub mode: ReviewMode,
    /// A preference just saved (suggested): open it for confirmation once the
    /// queue shows it.
    pub follow: Option<i64>,
    /// The read the UI sends next to continue that path (never a write).
    pub follow_up: Option<&'static str>,
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
    /// Create/select also remembers the Profile for every daemon start.
    pub remember: bool,
    pub key: String,
    pub value: String,
    /// The draft's tags (canonical, one to five) and the tag being typed.
    pub tags: Vec<String>,
    pub tag_draft: String,
    /// Who the draft comes from: the operator, or an agent's suggestion.
    pub origin: Origin,
    /// Where the draft applies (comma-separated, empty = everywhere).
    pub applies_agents: String,
    pub applies_languages: String,
    pub applies_paths: String,
    pub applies_tags: String,
    /// The draft's priority as typed (empty = 0).
    pub priority: String,
    pub enforcement: precept_core::operator::Enforcement,
    /// The note for the next decision (confirm, reject, retire or edit).
    pub note: String,
    /// The suggestion whose edited version the draft holds.
    pub revising: Option<i64>,
    /// Suggestions picked for one batch decision (ids, ascending).
    pub batch: Vec<i64>,
    /// What an import scan found in the selected project's files.
    pub import: Option<ImportScan>,
    /// The scan's candidates picked to become suggestions (indices).
    pub import_picked: Vec<usize>,
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
    /// Authority and live sources of the last completed exact inspection.
    inspection: Option<Guard>,
}
/// The most rules one import files (the writer's batch bound).
pub const MAX_BATCH_UI: usize = precept_core::operator::MAX_BATCH;
/// One rule an import scan found (`import.scan`).
#[derive(Clone, PartialEq, Eq, serde::Deserialize)]
pub struct ImportCandidate {
    pub source: String,
    pub title: String,
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub rule: precept_core::operator::RuleOptions,
    pub scope: Scope,
    /// Already in the Profile, suggested or confirmed, exactly as found.
    pub exists: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct ImportSkipped {
    pub source: String,
    pub title: String,
    pub reason: String,
}
/// A scan of one project's files.
#[derive(Clone, PartialEq, Eq, serde::Deserialize)]
pub struct ImportScan {
    #[serde(skip)]
    pub project: String,
    pub candidates: Vec<ImportCandidate>,
    pub skipped: Vec<ImportSkipped>,
}
impl ImportScan {
    /// Parses a daemon reply for `project`; new rules start picked.
    pub fn parse(project: &str, v: serde_json::Value) -> Option<(Self, Vec<usize>)> {
        let mut scan: Self = serde_json::from_value(v).ok()?;
        if scan.candidates.len() > precept_core::import::MAX_CANDIDATES {
            return None;
        }
        scan.project = project.to_owned();
        let picked = scan
            .candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.exists)
            .map(|(i, _)| i)
            .take(precept_core::operator::MAX_BATCH)
            .collect();
        Some((scan, picked))
    }
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
    info: Option<ProfileInfo>,
    selected_id: Option<i64>,
    source_snapshot: Option<std::sync::Arc<crate::Snapshot>>,
    source_available: bool,
    source_live: bool,
    permission_denied: bool,
    open: bool,
    queue_sequence: Option<i64>,
    task: String,
    context_task: String,
    queried_task: String,
    scope: Scope,
    after_id: i64,
    queue_after_id: i64,
    queue_watermark: i64,
    inspect_revision: i64,
    inspect_record: Option<PreferenceRecord>,
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
    /// Picks or unpicks a suggestion for the batch.
    pub fn toggle_batch(&mut self, id: i64) {
        match self.batch.binary_search(&id) {
            Ok(i) => {
                self.batch.remove(i);
            }
            Err(i) if self.batch.len() < precept_core::operator::MAX_BATCH => {
                self.batch.insert(i, id)
            }
            Err(_) => {}
        }
        self.review = None;
    }
    /// The picked suggestions as the current list shows them (the queue on
    /// To review, else the scope's records), with that list's binding.
    pub fn batch_records(&self) -> (Vec<&PreferenceRecord>, Option<&Binding>) {
        let (records, binding) = match (&self.queue, &self.page) {
            (Some(q), _) if self.mode == ReviewMode::Inbox => (&q.records, Some(&q.binding)),
            (_, Some(p)) => (&p.records, Some(&p.binding)),
            _ => return (Vec::new(), None),
        };
        (
            records
                .iter()
                .filter(|r| r.status == "suggested" && self.batch.binary_search(&r.id).is_ok())
                .collect(),
            binding,
        )
    }
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
    /// Adds `raw` to the draft's tags. False (nothing changes) when it is not
    /// a valid tag or the draft already has [`MAX_TAGS`]; a repeat is a no-op.
    pub fn add_tag(&mut self, raw: &str) -> bool {
        let Some(tag) = normalize_tag(raw) else {
            return false;
        };
        if self.tags.contains(&tag) {
            return true;
        }
        if self.tags.len() >= MAX_TAGS {
            return false;
        }
        self.tags.push(tag);
        self.tags.sort();
        true
    }
    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.retain(|t| t != tag);
    }
    /// A draft holding the selected suggestion, to edit and save as its
    /// replacement. Only a suggestion can be edited.
    pub fn start_revision(&mut self) -> bool {
        let Some(r) = self.record().filter(|r| r.status == "suggested").cloned() else {
            return false;
        };
        self.start_draft();
        self.key = r.key;
        self.value = r.value;
        self.tags = r.tags;
        let join = |v: &[String]| v.join(", ");
        self.applies_agents = join(&r.rule.applies_to.agents);
        self.applies_languages = join(&r.rule.applies_to.languages);
        self.applies_paths = join(&r.rule.applies_to.paths);
        self.applies_tags = join(&r.rule.applies_to.task_tags);
        if r.rule.priority != 0 {
            self.priority = r.rule.priority.to_string();
        }
        self.enforcement = r.rule.enforcement;
        self.revising = Some(r.id);
        true
    }
    /// A fresh draft for the add dialog.
    pub fn start_draft(&mut self) {
        self.revising = None;
        self.key.clear();
        self.value.clear();
        self.tags.clear();
        self.tag_draft.clear();
        self.origin = Origin::Operator;
        self.applies_agents.clear();
        self.applies_languages.clear();
        self.applies_paths.clear();
        self.applies_tags.clear();
        self.priority.clear();
        self.enforcement = Default::default();
    }
    /// The draft's rule options, or `None` when a value is invalid.
    pub fn draft_rule(&self, scope: &Scope) -> Option<precept_core::operator::RuleOptions> {
        let split = |s: &str| -> Vec<String> {
            s.split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .collect()
        };
        let priority = match self.priority.trim() {
            "" => 0,
            p => p.replace('−', "-").parse().ok()?,
        };
        let rule = precept_core::operator::RuleOptions {
            applies_to: precept_core::operator::AppliesTo {
                agents: split(&self.applies_agents),
                languages: split(&self.applies_languages),
                paths: split(&self.applies_paths),
                task_tags: split(&self.applies_tags),
            }
            .canonical()?,
            priority,
            enforcement: if matches!(scope, Scope::Environment { .. }) {
                Default::default()
            } else {
                self.enforcement
            },
        };
        rule.valid_for(scope).then_some(rule)
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
    fn clear_review_snapshots(&mut self) {
        self.queue = None;
        self.inspect = None;
        self.inspection = None;
        self.queue_after_id = 0;
        self.queue_watermark = 0;
        self.inspect_revision = 0;
        self.digest_expanded = false;
        self.delete_arm = None;
    }
    pub fn invalidate(&mut self) {
        self.serial += 1;
        self.follow = None;
        self.follow_up = None;
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
        } else if self.operator.mode == ReviewMode::Environment {
            Some(Scope::Environment {
                name: self.selected_project.clone()?,
            })
        } else {
            Some(Scope::Project {
                name: self.selected_project.clone()?,
            })
        }
    }
    pub fn operator_guard(&self) -> Option<Guard> {
        // Local and WSL targets get identical epoch/connection/source fences.
        (matches!(self.connection.target, Target::Local | Target::Wsl)
            && self.connection.validate())
        .then(|| Guard {
            epoch: self.epoch,
            connection: self.connection.clone(),
            project: self.selected_project.clone(),
            info: self.operator.info.clone(),
            selected_id: self.operator.selected_id,
            source_snapshot: self.data.clone(),
            source_available: matches!(self.phase, crate::Phase::Ready | crate::Phase::Empty),
            source_live: self.live,
            permission_denied: self.permission_denied,
            open: self.operator.open,
            queue_sequence: self.operator.queue.as_ref().map(|q| q.commit_sequence),
            task: self.operator.task.clone(),
            context_task: self.task_id.clone(),
            queried_task: self.queried_task.clone(),
            scope: self.operator_scope().unwrap_or(Scope::Global),
            after_id: self.operator.after_id,
            queue_after_id: self.operator.queue_after_id,
            queue_watermark: self.operator.queue_watermark,
            inspect_revision: self.operator.inspect_revision,
            inspect_record: self.operator.inspect.as_ref().map(|i| i.record.clone()),
            serial: self.operator.serial,
        })
    }
    /// Whether the exact inspection still belongs to the current selection,
    /// authority and observed sources. A false value requires a fresh inspect.
    /// The draft's serial is not part of it: editing a draft (an edit of this
    /// very record) does not change what was inspected.
    pub fn operator_inspected(&self) -> bool {
        self.operator.inspection.as_ref().is_some_and(|inspected| {
            self.operator_guard().is_some_and(|mut guard| {
                guard.serial = inspected.serial;
                guard == *inspected
            })
        })
    }
    pub fn operator_current(&self, review: &Review) -> bool {
        let Some(mut guard) = self.operator_guard() else {
            return false;
        };
        // OutcomeUnknown hides live grant controls, but keeps the captured
        // authority for exact manual replay only. A newly observed grant or
        // binding always wins; epoch/serial/connection/source fences still apply.
        if self.operator.outcome_unknown
            && self.operator.info.is_none()
            && let Some(pending) = &self.operator.pending
        {
            guard.info = pending.guard.info.clone();
        }
        guard == review.guard
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
        self.operator.delete_arm = None;
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
        self.operator.review = None;
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
                json!({"path":self.operator.path,"remember":self.operator.remember}),
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
            "import" if info.write_enabled => {
                let Some(binding) = info.binding.clone() else {
                    return false;
                };
                let Some(scan) = &self.operator.import else {
                    return false;
                };
                if self.selected_project.as_deref() != Some(scan.project.as_str()) {
                    return false;
                }
                let items: Vec<precept_core::operator::SuggestItem> = self
                    .operator
                    .import_picked
                    .iter()
                    .filter_map(|i| scan.candidates.get(*i))
                    .filter(|c| !c.exists)
                    .map(|c| precept_core::operator::SuggestItem {
                        scope: c.scope.clone(),
                        key: c.key.clone(),
                        value: c.value.clone(),
                        tags: c.tags.clone(),
                        origin: Origin::Operator,
                        rule: c.rule.clone(),
                    })
                    .collect();
                if items.is_empty() || items.len() > precept_core::operator::MAX_BATCH {
                    return false;
                }
                let mutation = Mutation {
                    version: 2,
                    binding,
                    operation_id: operation_id.into(),
                    command: Command::SuggestBatch { items },
                };
                ("profile.mutate", json!({}), Some(mutation))
            }
            "batch_confirm" | "batch_reject" if info.write_enabled => {
                let Some(binding) = info.binding.clone() else {
                    return false;
                };
                let (records, source) = self.operator.batch_records();
                // Every picked row must still be listed, from this Profile.
                if records.is_empty()
                    || records.len() != self.operator.batch.len()
                    || source != Some(&binding)
                {
                    return false;
                }
                let Some(note) =
                    precept_core::operator::canonical_note(Some(self.operator.note.as_str()))
                else {
                    self.operator.error = Some("InvalidNote".into());
                    return false;
                };
                let items = records
                    .iter()
                    .map(|r| precept_core::operator::BatchItem {
                        id: r.id,
                        scope: r.scope.clone(),
                        proposal_digest: r.proposal_digest.clone(),
                        expected: Expected::Revision(r.revision),
                    })
                    .collect();
                let mutation = Mutation {
                    version: 2,
                    binding,
                    operation_id: operation_id.into(),
                    command: Command::DecideBatch {
                        decision: if action == "batch_confirm" {
                            precept_core::operator::BatchDecision::Confirm
                        } else {
                            precept_core::operator::BatchDecision::Reject
                        },
                        items,
                        channel: self.operator.channel,
                        note,
                    },
                };
                ("profile.mutate", json!({}), Some(mutation))
            }
            "suggest" | "confirm" | "reject" | "deprecate" | "delete" | "switch_on"
            | "switch_off" | "revise"
                if info.write_enabled =>
            {
                let Some(binding) = info.binding.clone() else {
                    return false;
                };
                let Some(scope) = self.operator_scope() else {
                    return false;
                };
                let command = if action == "suggest" {
                    let Some(tags) = canonical_tags(&self.operator.tags) else {
                        return false;
                    };
                    if self.operator.key.trim().is_empty() || self.operator.value.trim().is_empty()
                    {
                        return false;
                    }
                    let Some(rule) = self.operator.draft_rule(&scope) else {
                        self.operator.error = Some("InvalidRule".into());
                        return false;
                    };
                    // A suggestion's new tags keep a generated oklch() color.
                    self.pin_tag_colors(&tags);
                    Command::SuggestPreference {
                        scope,
                        key: self.operator.key.clone(),
                        value: self.operator.value.clone(),
                        tags,
                        origin: self.operator.origin,
                        rule,
                        expected: Expected::Absent,
                    }
                } else {
                    let Some((r, source)) = self.operator.selected() else {
                        return false;
                    };
                    // A list/queue row is not an inspection: decisions require
                    // the exact snapshot that disclosed peers and history.
                    let Some(inspect) = &self.operator.inspect else {
                        return false;
                    };
                    // The record, authority and sources must still be the ones
                    // inspected; the draft (its serial) is not part of that.
                    if !self.operator_inspected() {
                        return false;
                    }
                    if r.scope != scope
                        || *source != binding
                        || inspect.record != *r
                        || inspect.binding != binding
                        || self.operator.inspect_revision != r.revision
                    {
                        return false;
                    }
                    // Deprecate only retires a confirmed row; confirm/reject
                    // only decide a suggested one. Delete is armed separately.
                    match action {
                        "confirm" | "reject" if r.status != "suggested" => return false,
                        "revise"
                            if r.status != "suggested" || self.operator.revising != Some(r.id) =>
                        {
                            return false;
                        }
                        "deprecate" if r.status != "confirmed" => return false,
                        "switch_on" | "switch_off" if r.status != "confirmed" => return false,
                        "switch_on" if r.enabled => return false,
                        "switch_off" if !r.enabled => return false,
                        "delete" if !self.operator.delete_armed_for(r) => return false,
                        _ => {}
                    }
                    let id = r.id;
                    let proposal_digest = r.proposal_digest.clone();
                    let channel = self.operator.channel;
                    let expected = Expected::Revision(r.revision);
                    // The operator's note, only on decisions that keep one.
                    let note = if matches!(action, "confirm" | "reject" | "deprecate" | "revise") {
                        let Some(note) = precept_core::operator::canonical_note(Some(
                            self.operator.note.as_str(),
                        )) else {
                            self.operator.error = Some("InvalidNote".into());
                            return false;
                        };
                        note
                    } else {
                        None
                    };
                    let revision = if action == "revise" {
                        let Some(tags) = canonical_tags(&self.operator.tags) else {
                            return false;
                        };
                        if self.operator.key.trim().is_empty()
                            || self.operator.value.trim().is_empty()
                        {
                            return false;
                        }
                        let Some(rule) = self.operator.draft_rule(&scope) else {
                            self.operator.error = Some("InvalidRule".into());
                            return false;
                        };
                        Some((tags, rule))
                    } else {
                        None
                    };
                    match action {
                        "revise" => {
                            let (tags, rule) = revision.expect("built above");
                            self.pin_tag_colors(&tags);
                            Command::ReviseSuggestion {
                                id,
                                scope,
                                proposal_digest,
                                key: self.operator.key.clone(),
                                value: self.operator.value.clone(),
                                tags,
                                rule,
                                channel,
                                expected,
                                note,
                            }
                        }
                        "switch_on" | "switch_off" => Command::SetPreferenceEnabled {
                            id,
                            scope,
                            enabled: action == "switch_on",
                            channel,
                            expected,
                        },
                        "confirm" => Command::ConfirmPreference {
                            id,
                            scope,
                            proposal_digest,
                            channel,
                            expected,
                            note,
                        },
                        "reject" => Command::RejectPreference {
                            id,
                            scope,
                            proposal_digest,
                            channel,
                            expected,
                            note,
                        },
                        "deprecate" => Command::DeprecatePreference {
                            id,
                            scope,
                            proposal_digest,
                            channel,
                            expected,
                            note,
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
        let record = if matches!(
            action,
            "confirm" | "reject" | "deprecate" | "delete" | "switch_on" | "switch_off" | "revise"
        ) {
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
                    // Loading more appends to the rows shown when it continues
                    // exactly where they end; any other read replaces them.
                    // (A copy: the selection below is checked against the old rows.)
                    let page = append_page(
                        self.operator.page.clone(),
                        page,
                        request.guard.after_id,
                        |p| (&p.binding, p.next_after_id),
                        |old, new| {
                            old.records.extend(new.records);
                            old.next_after_id = new.next_after_id;
                        },
                    );
                    self.operator.after_id = 0;
                    // A refresh keeps the selection (and its exact inspection)
                    // only while the page still holds that exact record: same
                    // id, revision and digest. Anything else is cleared.
                    let same = self.operator.record().is_some_and(|old| {
                        page.records.iter().any(|r| {
                            r.id == old.id
                                && r.revision == old.revision
                                && r.proposal_digest == old.proposal_digest
                        })
                    });
                    if !same {
                        self.operator.selected_id = None;
                        self.operator.inspect = None;
                        self.operator.inspect_revision = 0;
                        self.operator.digest_expanded = false;
                    }
                    // A just-saved preference may arrive through the scope's
                    // record list as well as the queue: open it the same way.
                    if let Some(id) = self.operator.follow
                        && page.records.iter().any(|r| r.id == id)
                    {
                        self.operator.follow = None;
                        self.operator.selected_id = Some(id);
                        self.operator.review_tab = ReviewTab::Detail;
                        self.operator.follow_up = Some("profile.review.inspect");
                    }
                    self.operator.page = Some(page);
                }
                "profile.review.queue" => {
                    let queue: ReviewQueue =
                        serde_json::from_value(v).map_err(|_| "InvalidRequest")?;
                    let after_id = request.guard.queue_after_id;
                    if queue.commit_sequence < 0
                        || request
                            .guard
                            .queue_sequence
                            .is_some_and(|sequence| queue.commit_sequence < sequence)
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
                    let queue = append_page(
                        self.operator.queue.take(),
                        queue,
                        after_id,
                        |q| (&q.binding, q.next_after_id),
                        |old, new| {
                            old.records.extend(new.records);
                            old.next_after_id = new.next_after_id;
                            old.commit_sequence = new.commit_sequence;
                        },
                    );
                    self.operator.queue_after_id = 0;
                    // A queue refresh replaces the source of review. An older
                    // exact inspect may not hide a moved or removed candidate.
                    self.operator.inspect = None;
                    self.operator.inspect_revision = 0;
                    self.operator.digest_expanded = false;
                    self.operator.page = None;
                    self.operator.queue = Some(queue);
                    if let Some(id) = self.operator.follow.take()
                        && self
                            .operator
                            .queue
                            .as_ref()
                            .is_some_and(|q| q.records.iter().any(|r| r.id == id))
                    {
                        self.operator.selected_id = Some(id);
                        self.operator.review_tab = ReviewTab::Detail;
                        self.operator.follow_up = Some("profile.review.inspect");
                    }
                    if self.operator.selected_id.is_some() && self.operator.record().is_none() {
                        self.operator.selected_id = None;
                    }
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
                        || request
                            .guard
                            .queue_sequence
                            .is_some_and(|sequence| inspect.commit_sequence < sequence)
                        || r.id != id
                        || r.revision != revision
                        || r.revision <= 0
                        || r.scope != request.guard.scope
                        || digest_from_hex(&r.proposal_digest).is_none()
                        || inspect.decisions.len() > 8
                        || inspect
                            .decisions
                            .windows(2)
                            .any(|w| w[0].receipt_commit_sequence <= w[1].receipt_commit_sequence)
                        || inspect.decisions.iter().any(|d| {
                            d.preference_id != id
                                || d.from_revision <= 0
                                || d.to_revision.checked_sub(1) != Some(d.from_revision)
                                || d.receipt_commit_sequence <= 0
                                || d.receipt_commit_sequence > inspect.commit_sequence
                                || digest_from_hex(&d.proposal_digest).is_none()
                                || d.proposal_digest != r.proposal_digest
                        })
                        || inspect.peer_demotions.len() > 12
                        || inspect
                            .peer_demotions
                            .windows(2)
                            .any(|w| w[0].id >= w[1].id)
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
                    // Preference IDs are profile-global: Writer's identity-only
                    // receipt leaves project empty even for project-scoped rows.
                    if let Command::DeletePreference { id, .. } = &mutation.command
                        && !receipt.affected.iter().any(|a| {
                            a.kind == "preference"
                                && a.record_id == id.to_string()
                                && a.deleted
                                && a.project.is_empty()
                                && request
                                    .record
                                    .as_ref()
                                    .and_then(|r| r.revision.checked_add(1))
                                    == Some(a.revision)
                        })
                    {
                        return Err("OutcomeUnknown");
                    }
                    // Save, then confirm: a preference the operator just saved
                    // opens as the exact record to confirm (two explicit steps).
                    // An edit opens its replacement the same way.
                    if matches!(
                        &mutation.command,
                        Command::SuggestPreference { .. } | Command::ReviseSuggestion { .. }
                    ) {
                        self.operator.follow = receipt
                            .affected
                            .iter()
                            .find(|a| a.kind == "preference" && !a.deleted && a.revision == 1)
                            .and_then(|a| a.record_id.parse().ok());
                        if self.operator.follow.is_some() {
                            // The scoped pages find it in their record list;
                            // the inbox in the queue.
                            if self.operator.mode == ReviewMode::Inbox {
                                self.operator.queue_after_id = 0;
                                self.operator.follow_up = Some("profile.review.queue");
                            } else {
                                self.operator.after_id = 0;
                                self.operator.follow_up = Some("profile.preferences.list");
                            }
                        }
                    }
                    self.operator.receipt = Some(receipt);
                    self.operator.note.clear();
                    self.operator.revising = None;
                    self.operator.batch.clear();
                    if matches!(&mutation.command, Command::SuggestBatch { .. }) {
                        self.operator.import = None;
                        self.operator.import_picked.clear();
                    }
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
        // A retry of the pending uncertain mutation that produced no daemon
        // decision ("Unavailable": nothing was submitted, e.g. a lost WSL bridge
        // or a stopped daemon) settles nothing: the uncertainty and the exact
        // request are kept for a later identical retry.
        let unsubmitted_retry = request.mutation.is_some()
            && self.operator.outcome_unknown
            && self.operator.pending.as_ref() == Some(request)
            && result.as_ref().err() == Some(&"Unavailable");
        self.operator.error = result.err().map(str::to_owned);
        if self.operator.error.is_some() {
            self.operator.outcome_unknown = request.mutation.is_some()
                && (self.operator.error.as_deref() == Some("OutcomeUnknown") || unsubmitted_retry);
            if !self.operator.outcome_unknown {
                self.operator.pending = None;
                self.operator.clear_review_snapshots();
            }
            // An uncertain mutation keeps its guard identity (watermark,
            // inspected revision/digest) so the identical retry can settle it.
            self.operator.info = None;
            self.operator.page = None;
            self.operator.handoff = None;
            if !self.operator.outcome_unknown {
                self.operator.selected_id = None;
            }
        }
        // A settled request is fenced off: a duplicate or delayed completion can
        // never apply twice. Only an uncertain mutation keeps its guard so the
        // identical same-epoch retry can still settle it once.
        if !self.operator.outcome_unknown {
            self.operator.serial += 1;
        }
        if request.method == "profile.review.inspect" && self.operator.error.is_none() {
            self.operator.inspection = self.operator_guard();
        }
        true
    }
}

/// `new` appended to `old` when it was read from exactly where `old` ends
/// (same binding, `after_id` its next cursor); otherwise `new` alone.
fn append_page<P>(
    old: Option<P>,
    new: P,
    after_id: i64,
    cursor: impl Fn(&P) -> (&Binding, Option<i64>),
    extend: impl FnOnce(&mut P, P),
) -> P {
    match old {
        Some(mut old)
            if after_id > 0 && {
                let (binding, next) = cursor(&old);
                binding == cursor(&new).0 && next == Some(after_id)
            } =>
        {
            extend(&mut old, new);
            old
        }
        _ => new,
    }
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
            ..Default::default()
        });
        let binding = m.operator.info.as_ref().unwrap().binding.clone().unwrap();
        m.operator.page = Some(PreferencePage {
            binding,
            records: vec![PreferenceRecord {
                rule: Default::default(),
                enabled: true,
                id: 1,
                scope: Scope::Global,
                key: "synthetic-key".into(),
                value: "synthetic-value".into(),
                tags: vec!["test".into()],
                status: "suggested".into(),
                revision: 1,
                origin: Origin::Agent,
                proposal_digest: digest_hex(&proposal_digest(
                    &Scope::Global,
                    None,
                    "synthetic-key",
                    "synthetic-value",
                    Origin::Agent,
                    &["test".to_owned()],
                    &Default::default(),
                )),
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            }],
            next_after_id: None,
        });
        m.operator.selected_id = Some(1);
        let record = m.operator.page.as_ref().unwrap().records[0].clone();
        assert!(complete_inspect(
            &mut m,
            inspect_of(record, vec![], vec![], 0)
        ));
        m
    }
    #[test]
    fn no_implicit_grant_or_cross_target_writes() {
        let mut m = model();
        let local = m.connection.clone();
        // Synthetic data and an incomplete WSL connection never get Operator access.
        for target in [Target::Synthetic, Target::Wsl] {
            m.connection.target = target;
            assert!(!m.operator_review("confirm", "synthetic-op"));
            assert!(m.operator_read("profile.info").is_none());
        }
        // An explicit, valid WSL connection gets the same guarded Operator surface.
        m.connection = crate::connection::Connection {
            target: Target::Wsl,
            distribution: "Synthetic Linux".into(),
            user: "operator".into(),
            helper: "/synthetic/precept-bridge".into(),
            wsl_runtime: "/synthetic/runtime".into(),
            wsl_endpoint: "/synthetic/runtime/control.sock".into(),
            ..Default::default()
        };
        assert!(m.operator_guard().is_some());
        assert!(m.operator_read("profile.info").is_some());
        m.operator.busy = false;
        m.connection = local;
        m.operator.info.as_mut().unwrap().write_enabled = false;
        assert!(!m.operator_review("confirm", "synthetic-op"));
        assert!(m.operator_review("enable", "synthetic-op"));
        assert!(!m.operator.info.as_ref().unwrap().write_enabled);
        let request = m.operator_confirm().unwrap();
        assert_eq!(request.method, "profile.write.enable");
        assert!(request.mutation.is_none());
    }
    #[test]
    fn an_unsubmitted_retry_keeps_the_uncertainty_and_the_exact_request() {
        let mut m = model();
        assert!(m.operator_review("confirm", "kept-op"));
        let request = m.operator_confirm().unwrap();
        assert!(m.complete_operator(&request, Err("OutcomeUnknown")));
        // The retry could not be submitted (lost WSL bridge, stopped daemon).
        let retry = m.operator_retry().unwrap();
        assert!(m.complete_operator(&retry, Err("Unavailable")));
        assert!(m.operator.outcome_unknown);
        assert!(m.operator.pending.as_ref() == Some(&request));
        // A later identical retry is still possible and settles it once.
        let again = m.operator_retry().unwrap();
        assert_eq!(
            serde_json::to_vec(&again.mutation).unwrap(),
            serde_json::to_vec(&request.mutation).unwrap()
        );
        let receipt = json!({"generation":"b".repeat(32),"operation_id":"kept-op","commit_sequence":2,"affected":[]});
        assert!(m.complete_operator(&again, Ok(receipt)));
        assert!(!m.operator.outcome_unknown && m.operator.pending.is_none());
        // A fresh mutation that was never submitted is not uncertain.
        let mut fresh = model();
        assert!(fresh.operator_review("confirm", "fresh-op"));
        let first = fresh.operator_confirm().unwrap();
        assert!(fresh.complete_operator(&first, Err("Unavailable")));
        assert!(!fresh.operator.outcome_unknown && fresh.operator.pending.is_none());
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
        m.operator.inspect.as_mut().unwrap().binding.owner_epoch = 2;
        assert!(!m.operator_review("confirm", "once"));
        m.operator.inspect.as_mut().unwrap().binding.owner_epoch = 1;
        m.operator.inspect.as_mut().unwrap().record.status = "confirmed".into();
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
        let project = scope.project();
        PreferenceRecord {
            rule: Default::default(),
            enabled: true,
            id,
            scope: scope.clone(),
            key: key.clone(),
            value: "synthetic-value".into(),
            tags: vec!["test".into()],
            status: status.into(),
            revision,
            origin: Origin::Agent,
            proposal_digest: digest_hex(&proposal_digest(
                scope,
                project,
                &key,
                "synthetic-value",
                Origin::Agent,
                &["test".to_owned()],
                &Default::default(),
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
            proposal_digest: synthetic_record(preference_id, &Scope::Global, "suggested", 1)
                .proposal_digest,
            receipt_commit_sequence: seq,
            channel: Channel::Desktop,
            decided_at: "2026-01-01T00:00:00Z".into(),
            note: None,
            replaced_by: None,
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
            commit_sequence: 8,
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
            .rev()
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
        m.operator.inspect.as_mut().unwrap().record.status = "confirmed".into();
        m.operator.inspect.as_mut().unwrap().record.revision = 2;
        m.operator.inspect_revision = 2;
        m.operator.channel = Channel::Tui;
        let record = m.operator.record().unwrap().clone();
        assert!(complete_inspect(
            &mut m,
            inspect_of(record, vec![], vec![], 0)
        ));
        assert!(m.operator_review("deprecate", "dep-op"));
        let request = m.operator_confirm().unwrap();
        let digest = m.operator.record().unwrap().proposal_digest.clone();
        match &request.mutation.as_ref().unwrap().command {
            Command::DeprecatePreference {
                id,
                scope,
                proposal_digest,
                channel,
                expected,
                note,
            } => {
                assert_eq!(*note, None);
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
            m.operator.inspect.as_mut().unwrap().record.status = "confirmed".into();
            let record = m.operator.record().unwrap().clone();
            assert!(complete_inspect(
                &mut m,
                inspect_of(record, vec![], vec![], 0)
            ));
            assert!(m.operator_review("deprecate", "drift"));
            let record = &mut m.operator.inspect.as_mut().unwrap().record;
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
    fn set_up_profile_reads_never_fence_an_in_flight_workspace_request() {
        let mut m = model();
        m.operator.busy = false;
        let read = m.operator_read("profile.info").unwrap();
        // The Set-up pages record their own Profile state while it is in flight.
        *m.profile_info = m.operator.info.clone();
        m.profile_info.as_mut().unwrap().write_enabled =
            !m.operator.info.as_ref().unwrap().write_enabled;
        assert!(
            m.operator_current(&read),
            "the workspace reply still settles"
        );
        // Whereas changing the workspace's own observation does fence it.
        m.operator.info.as_mut().unwrap().write_enabled ^= true;
        assert!(!m.operator_current(&read));
    }
    #[test]
    fn a_saved_preference_opens_as_the_exact_record_to_confirm() {
        let mut m = model();
        m.operator.key = "editor".into();
        m.operator.value = "synthetic".into();
        m.operator.tags = vec!["style".into()];
        m.operator.origin = Origin::Agent;
        assert!(m.operator_review("suggest", "s-op"));
        // The suggestion carries its tags and origin; a new tag keeps an oklch() color.
        assert!(matches!(
            &m.operator.review.as_ref().unwrap().mutation.as_ref().unwrap().command,
            Command::SuggestPreference { tags, origin: Origin::Agent, .. } if tags == &["style"]
        ));
        assert_eq!(m.tag_colors["style"], crate::tag_colors::generated("style"));
        let request = m.operator_confirm().unwrap();
        let affected = json!([{"kind":"preference","project":"","record_id":"9","revision":1,"deleted":false}]);
        assert!(m.complete_operator(&request, Ok(receipt_json("s-op", 4, affected))));
        // Step one is done (saved as suggested); nothing is confirmed.
        assert_eq!(m.operator.follow, Some(9));
        // The inbox (default) finds it in the queue; a scoped page in its list.
        assert_eq!(m.operator.follow_up, Some("profile.review.queue"));
        let next = m.operator.follow_up.take().unwrap();
        let read = m.operator_read(next).unwrap();
        assert_eq!(read.method, "profile.review.queue");
        let queue = queue_of(
            vec![
                synthetic_record(3, &Scope::Global, "suggested", 1),
                synthetic_record(9, &Scope::Global, "suggested", 1),
            ],
            5,
            None,
        );
        assert!(m.complete_operator(&read, Ok(serde_json::to_value(queue).unwrap())));
        // Step two is offered: the exact record, in Detail, to inspect and confirm.
        assert_eq!(m.operator.selected_id, Some(9));
        assert_eq!(m.operator.review_tab, ReviewTab::Detail);
        assert_eq!(m.operator.follow_up, Some("profile.review.inspect"));
        assert!(m.operator.follow.is_none());
        // The scope's record list opens it the same way (a scoped page reloads
        // its list after the receipt, possibly before the queue read).
        let mut m = model();
        m.operator.page = None;
        m.operator.follow = Some(7);
        let read = m.operator_read("profile.preferences.list").unwrap();
        let page = PreferencePage {
            binding: binding(),
            records: vec![synthetic_record(7, &Scope::Global, "suggested", 1)],
            next_after_id: None,
        };
        assert!(m.complete_operator(&read, Ok(serde_json::to_value(page).unwrap())));
        assert_eq!(m.operator.selected_id, Some(7));
        assert_eq!(m.operator.follow_up, Some("profile.review.inspect"));
        // A refresh of the same exact record keeps the selection and its
        // inspection; a changed revision clears both.
        m.operator.inspect = None;
        let read = m.operator_read("profile.preferences.list").unwrap();
        let same = PreferencePage {
            binding: binding(),
            records: vec![synthetic_record(7, &Scope::Global, "suggested", 1)],
            next_after_id: None,
        };
        assert!(m.complete_operator(&read, Ok(serde_json::to_value(same).unwrap())));
        assert_eq!(m.operator.selected_id, Some(7));
        let read = m.operator_read("profile.preferences.list").unwrap();
        let changed = PreferencePage {
            binding: binding(),
            records: vec![synthetic_record(7, &Scope::Global, "confirmed", 2)],
            next_after_id: None,
        };
        assert!(m.complete_operator(&read, Ok(serde_json::to_value(changed).unwrap())));
        assert_eq!(m.operator.selected_id, None);
        // A refresh that does not show it never selects something else.
        let mut m = model();
        m.operator.follow = Some(42);
        assert!(complete_queue(&mut m, queue_of(vec![], 5, None)));
        assert!(m.operator.follow.is_none() && m.operator.follow_up.is_none());
    }
    #[test]
    fn queue_sourced_selection_drives_detail_and_confirm() {
        let mut m = model();
        m.operator.page = None;
        m.operator.inspect = None;
        m.operator.inspect_revision = 0;
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
    fn change_review_context(m: &mut Model, change: usize) {
        match change {
            0 => {
                m.operator
                    .info
                    .as_mut()
                    .unwrap()
                    .binding
                    .as_mut()
                    .unwrap()
                    .profile_id = "d".repeat(32)
            }
            1 => {
                m.operator
                    .info
                    .as_mut()
                    .unwrap()
                    .binding
                    .as_mut()
                    .unwrap()
                    .generation = "d".repeat(32)
            }
            2 => {
                m.operator
                    .info
                    .as_mut()
                    .unwrap()
                    .binding
                    .as_mut()
                    .unwrap()
                    .owner_epoch += 1
            }
            3 => m.operator.info.as_mut().unwrap().write_enabled = false,
            4 => m.operator.selected_id = Some(8),
            5 => m.task_id = "new-context-task".into(),
            6 => m.queried_task = "new-queried-task".into(),
            7 => m.operator.after_id += 1,
            8 => m.operator.queue.as_mut().unwrap().commit_sequence += 1,
            9 => m.phase = crate::Phase::Unavailable,
            10 => {
                let snapshot = std::sync::Arc::make_mut(m.data.as_mut().unwrap());
                snapshot.projects[0].context.documents[0]["status"] = json!("unavailable");
            }
            11 => {
                let snapshot = std::sync::Arc::make_mut(m.data.as_mut().unwrap());
                snapshot.projects[0].context.documents[0]["sha256"] = json!("d".repeat(64));
            }
            12 => {
                let snapshot = std::sync::Arc::make_mut(m.data.as_mut().unwrap());
                snapshot.projects[0]
                    .context
                    .warnings
                    .push("Source changed".into());
            }
            13 => m.operator.open = false,
            14 => m.permission_denied = true,
            15 => m.live = !m.live,
            _ => m.data = None,
        }
    }
    #[test]
    fn profile_grant_selection_and_live_sources_fence_prepared_and_delayed_actions() {
        for change in 0..17 {
            let mut m = armed_delete_model();
            assert!(m.operator_review("delete", "stale"));
            change_review_context(&mut m, change);
            assert!(m.operator_confirm().is_none(), "prepared change {change}");
            assert!(m.operator.pending.is_none());

            let mut m = armed_delete_model();
            let request = m.operator_read("profile.review.inspect").unwrap();
            change_review_context(&mut m, change);
            assert!(
                !m.complete_operator(&request, Err("Conflict")),
                "late change {change}"
            );
            assert!(m.operator.error.is_none());
        }
        // Losing observed authority is not equivalent to a current write grant.
        let mut m = armed_delete_model();
        assert!(m.operator_review("delete", "no-info"));
        m.operator.info = None;
        assert!(m.operator_confirm().is_none());
    }
    #[test]
    fn unknown_replay_never_crosses_observed_authority_or_source_changes() {
        for change in 0..17 {
            let mut m = armed_delete_model();
            assert!(m.operator_retry().is_none());
            assert!(m.operator_review("delete", "immutable-delete"));
            let request = m.operator_confirm().unwrap();
            let info = m.operator.info.clone();
            let bytes = serde_json::to_vec(&request.mutation).unwrap();
            assert!(m.complete_operator(&request, Err("OutcomeUnknown")));
            assert!(!m.operator.busy, "no automatic retry");
            assert_eq!(m.operator.selected_id, Some(7));
            m.operator.info = info;
            change_review_context(&mut m, change);
            assert!(m.operator_retry().is_none(), "replay change {change}");
            assert!(
                !m.complete_operator(&request, Ok(receipt_json("immutable-delete", 9, json!([]))))
            );
            assert_eq!(
                serde_json::to_vec(&m.operator.pending.as_ref().unwrap().mutation).unwrap(),
                bytes,
                "request identity is retained, never rebound"
            );
        }
    }
    #[test]
    fn refreshed_queue_drops_stale_inspect_content_and_delete_consent() {
        let mut m = armed_delete_model();
        assert!(m.operator_review("delete", "old-delete"));
        let request = m.operator_read("profile.review.queue").unwrap();
        assert!(m.operator.review.is_none());
        assert!(m.operator.delete_arm.is_none());
        let mut record = synthetic_record(7, &Scope::Global, "suggested", 2);
        record.tags = vec!["changed".into()];
        record.value = "new proposal".into();
        record.proposal_digest = "d".repeat(64);
        assert!(m.complete_operator(
            &request,
            Ok(serde_json::to_value(queue_of(vec![record.clone()], 9, None)).unwrap())
        ));
        assert!(m.operator.inspect.is_none());
        assert_eq!(m.operator.inspect_revision, 0);
        assert!(m.operator.record() == Some(&record));
        assert!(!m.operator_review("delete", "stale-consent"));
        assert!(complete_queue(&mut m, queue_of(vec![], 10, None)));
        assert!(m.operator.record().is_none());
        assert!(m.operator.selected_id.is_none());
    }
    #[test]
    fn changed_immutable_content_tags_or_origin_refuses_prepared_decision() {
        for change in 0..5 {
            let mut m = armed_delete_model();
            assert!(m.operator_review("confirm", "exact-content"));
            let record = &mut m.operator.inspect.as_mut().unwrap().record;
            match change {
                0 => record.key = "different-key".into(),
                1 => record.value = "different-value".into(),
                2 => record.tags = vec!["different".into()],
                3 => record.origin = Origin::Operator,
                _ => record.status = "rejected".into(),
            }
            assert!(m.operator_confirm().is_none(), "content change {change}");
        }
    }
    #[test]
    fn history_order_future_receipts_and_nonconsecutive_revisions_fail_closed() {
        for change in 0..4 {
            let mut m = inspect_model();
            let mut inspect = inspect_of(
                synthetic_record(7, &Scope::Global, "suggested", 1),
                vec![
                    decision(7, DecisionAction::Demote, 8),
                    decision(7, DecisionAction::Confirm, 6),
                ],
                vec![],
                0,
            );
            match change {
                0 => inspect.decisions.reverse(),
                1 => inspect.decisions[0].receipt_commit_sequence = 9,
                2 => inspect.decisions[0].to_revision = 4,
                _ => inspect.decisions[1].receipt_commit_sequence = 8,
            }
            assert!(complete_inspect(&mut m, inspect));
            assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
            assert!(m.operator.inspect.is_none());
        }
    }
    #[test]
    fn delete_tombstone_must_match_preference_identity_and_resulting_revision() {
        for (project, revision) in [("other-project", 2), ("", 1), ("", 3)] {
            let mut m = armed_delete_model();
            assert!(m.operator_review("delete", "exact-delete"));
            let request = m.operator_confirm().unwrap();
            assert!(m.complete_operator(
                &request,
                Ok(receipt_json(
                    "exact-delete",
                    9,
                    json!([{
                        "kind":"preference", "project":project, "record_id":"7",
                        "revision":revision, "deleted":true
                    }])
                ))
            ));
            assert!(m.operator.outcome_unknown);
            assert!(m.operator.receipt.is_none());
            let replay = m.operator_retry().unwrap();
            assert_eq!(
                serde_json::to_vec(&request.mutation).unwrap(),
                serde_json::to_vec(&replay.mutation).unwrap()
            );
            assert!(m.complete_operator(
                &replay,
                Ok(receipt_json(
                    "exact-delete",
                    9,
                    json!([{
                        "kind":"preference", "project":"", "record_id":"7",
                        "revision":2, "deleted":true
                    }])
                ))
            ));
            assert!(m.operator.receipt.is_some());
        }
    }
    #[test]
    fn project_delete_accepts_writer_global_id_receipt_without_losing_request_scope() {
        let mut m = model();
        let scope = Scope::Project {
            name: "alpha".into(),
        };
        let record = synthetic_record(7, &scope, "suggested", 1);
        m.operator.global = false;
        m.operator.page = Some(PreferencePage {
            binding: binding(),
            records: vec![record.clone()],
            next_after_id: None,
        });
        m.operator.selected_id = Some(7);
        m.operator.inspect = None;
        m.operator.inspect_revision = 0;
        assert!(complete_inspect(
            &mut m,
            inspect_of(record.clone(), vec![], vec![], 0)
        ));
        m.operator.delete_arm = Some(DeleteArm {
            retype: "7".into(),
            id: 7,
            revision: 1,
            proposal_digest: record.proposal_digest.clone(),
        });
        assert!(m.operator_review("delete", "project-delete"));
        let request = m.operator_confirm().unwrap();
        match &request.mutation.as_ref().unwrap().command {
            Command::DeletePreference {
                id,
                scope: actual_scope,
                expected,
                proposal_digest,
                ..
            } => {
                assert_eq!(*id, 7);
                assert!(*actual_scope == scope);
                assert!(*expected == Expected::Revision(1));
                assert_eq!(proposal_digest, &record.proposal_digest);
            }
            _ => panic!("expected exact project delete"),
        }
        assert!(m.complete_operator(
            &request,
            Ok(receipt_json(
                "project-delete",
                9,
                json!([{
                    "kind":"preference", "project":"", "record_id":"7",
                    "revision":2, "deleted":true
                }])
            ))
        ));
        assert!(!m.operator.outcome_unknown);
        assert!(m.operator.receipt.is_some());
        assert!(m.operator.pending.is_none());
        assert!(m.operator.record().is_none());
        assert!(m.operator.inspect.is_none());
        assert!(m.operator.delete_arm.is_none());
        assert!(m.operator_retry().is_none());
    }
    #[test]
    fn precise_review_refusals_are_settled_without_replay_or_success() {
        for error in [
            "ReviewMismatch",
            "Conflict",
            "InvalidTransition",
            "Unauthorized",
            "Busy",
            "Capacity",
            "Unavailable",
            "IdempotencyConflict",
        ] {
            let mut m = armed_delete_model();
            assert!(m.operator_review("confirm", "refused"));
            let request = m.operator_confirm().unwrap();
            assert!(m.complete_operator(&request, Err(error)));
            assert_eq!(m.operator.error.as_deref(), Some(error));
            assert!(m.operator.receipt.is_none());
            assert!(m.operator.pending.is_none());
            assert!(m.operator.review.is_none());
            assert!(m.operator.inspect.is_none());
            assert!(m.operator_retry().is_none());
        }
    }
    #[test]
    fn decisions_require_current_exact_inspection_and_failed_prepare_cancels() {
        for action in ["confirm", "reject", "deprecate", "delete"] {
            let mut m = model();
            m.operator.inspect = None;
            m.operator.inspect_revision = 0;
            if action == "deprecate" {
                m.operator.page.as_mut().unwrap().records[0].status = "confirmed".into();
            }
            assert!(!m.operator_review(action, "uninspected"), "{action}");
            assert!(m.operator_confirm().is_none());
            let record = m.operator.record().unwrap().clone();
            assert!(complete_inspect(
                &mut m,
                inspect_of(record.clone(), vec![], vec![], 0)
            ));
            if action == "delete" {
                m.operator.delete_arm = Some(DeleteArm {
                    retype: record.id.to_string(),
                    id: record.id,
                    revision: record.revision,
                    proposal_digest: record.proposal_digest,
                });
            }
            assert!(m.operator_review(action, "inspected"), "{action}");
            assert!(m.operator_confirm().is_some(), "{action}");
        }
        let mut m = model();
        assert!(m.operator_review("confirm", "old-confirm"));
        assert!(!m.operator_review("deprecate", "invalid-action"));
        assert!(m.operator.review.is_none());
        assert!(m.operator_confirm().is_none());
        for change in 0..17 {
            let mut m = armed_delete_model();
            change_review_context(&mut m, change);
            assert!(
                !m.operator_review("delete", "obsolete-inspect"),
                "change {change}"
            );
            assert!(
                !m.operator_review("confirm", "obsolete-inspect"),
                "change {change}"
            );
        }
    }
    #[test]
    fn queue_and_inspect_watermarks_never_regress_behind_latest_observed_page() {
        let mut m = inspect_model();
        m.operator.queue.as_mut().unwrap().commit_sequence = 9;
        m.operator.queue_after_id = 7;
        assert!(complete_queue(&mut m, queue_of(vec![], 8, None)));
        assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
        let mut m = inspect_model();
        m.operator.queue.as_mut().unwrap().commit_sequence = 9;
        assert!(complete_inspect(
            &mut m,
            inspect_of(
                synthetic_record(7, &Scope::Global, "suggested", 1),
                vec![],
                vec![],
                0
            )
        ));
        assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
        assert!(m.operator.inspect.is_none());
    }
    #[test]
    fn changed_history_digest_and_duplicate_peer_preview_are_not_displayed_as_exact() {
        for change in 0..2 {
            let mut m = inspect_model();
            let mut inspect = inspect_of(
                synthetic_record(7, &Scope::Global, "suggested", 1),
                vec![decision(7, DecisionAction::Confirm, 6)],
                vec![PeerDemotion {
                    id: 100,
                    revision: 1,
                }],
                1,
            );
            if change == 0 {
                inspect.decisions[0].proposal_digest = "d".repeat(64);
            } else {
                inspect.peer_demotions.push(PeerDemotion {
                    id: 100,
                    revision: 1,
                });
                inspect.peer_demotion_total = 2;
            }
            assert!(complete_inspect(&mut m, inspect));
            assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
            assert!(m.operator.inspect.is_none());
        }
    }
    #[test]
    fn unsafe_root_refusal_discards_inspection_and_requires_fresh_consent_after_restore() {
        // Native IPC sanitizes UnsafeRuntime to Unauthorized. Either spelling
        // must settle as a refusal, never preserve an arm for a restored root.
        for error in ["UnsafeRuntime", "Unauthorized"] {
            for action in ["confirm", "reject", "deprecate", "delete"] {
                let mut m = armed_delete_model();
                if action == "deprecate" {
                    m.operator.inspect.as_mut().unwrap().record.status = "confirmed".into();
                    let record = m.operator.record().unwrap().clone();
                    assert!(complete_inspect(
                        &mut m,
                        inspect_of(record, vec![], vec![], 0)
                    ));
                }
                let info = m.operator.info.clone();
                let record = m.operator.record().unwrap().clone();
                let arm = m.operator.delete_arm.clone();
                assert!(m.operator_review(action, "unsafe-root"));
                let request = m.operator_confirm().unwrap();
                assert!(m.complete_operator(&request, Err(error)));
                assert_eq!(m.operator.error.as_deref(), Some(error));
                assert!(m.operator.inspection.is_none());
                assert!(m.operator.inspect.is_none());
                assert!(m.operator.delete_arm.is_none());
                assert!(m.operator.review.is_none());
                assert!(m.operator.pending.is_none());
                assert!(m.operator.receipt.is_none());
                assert!(m.operator_retry().is_none());

                // Restoring availability or even old content/typed consent does
                // not restore the discarded inspection authority.
                m.operator.info = info;
                m.operator.page = Some(PreferencePage {
                    binding: binding(),
                    records: vec![record.clone()],
                    next_after_id: None,
                });
                m.operator.selected_id = Some(record.id);
                m.operator.delete_arm = arm;
                assert!(!m.operator_inspected());
                assert!(!m.operator_review(action, "restored-root"));
                assert!(m.operator_confirm().is_none());
                assert!(complete_inspect(
                    &mut m,
                    inspect_of(record.clone(), vec![], vec![], 0)
                ));
                assert!(m.operator_inspected());
                assert!(
                    m.operator.delete_arm.is_none(),
                    "inspection never restores typed consent"
                );
                if action == "delete" {
                    m.operator.delete_arm = Some(DeleteArm {
                        retype: record.id.to_string(),
                        id: record.id,
                        revision: record.revision,
                        proposal_digest: record.proposal_digest,
                    });
                }
                assert!(m.operator_review(action, "fresh-explicit-review"));
                assert!(m.operator.pending.is_none(), "preparation does not submit");
            }
        }
    }
}
