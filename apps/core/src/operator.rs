//! One ephemeral operator workflow for the desktop, never a second Writer.

mod import;
mod state;
#[cfg(test)]
mod tests;
mod workflow;

use crate::connection::Connection;
pub use import::{ImportCandidate, ImportScan, ImportSkipped, MAX_BATCH_UI};
pub use precept_core::operator::*;
use serde_json::Value;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Pane {
    /// Profile actions moved to the Connection page and Workspaces (direct,
    /// explicit calls); the review workspace starts on its records.
    Profile,
    #[default]
    Preferences,
    Handoff,
}
/// Review navigation of the operator workspace.
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
/// Which pending-review or reviewed-record scope is active in Context.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum ReviewMode {
    /// To review: pending suggestions across personal or current-project scope.
    #[default]
    Inbox,
    /// Personal preferences: global scope records across all projects.
    Personal,
    /// Project preferences: records scoped to the currently selected project.
    Project,
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
    /// Deprecated records are hidden in scoped lists unless explicitly shown.
    pub show_deprecated: bool,
    pub path: String,
    /// Create/select also remembers the Profile for every daemon start.
    pub remember: bool,
    /// The add dialog is open (a new draft; an edit opens with `revising`).
    pub adding: bool,
    pub key: String,
    pub value: String,
    /// The draft's tags (canonical, one to ten) and the tag being typed.
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
    /// The record whose edited version the draft holds.
    pub revising: Option<i64>,
    /// That record is a confirmed rule: saving the edit takes effect at once.
    pub revising_confirmed: bool,
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
    /// The confirmed rule the inspected suggestion would replace (same key
    /// and scope), read to show the change.
    pub update_of: Option<precept_core::operator::PreferenceRecord>,
    /// The rule file preview of the confirmed convention being written to
    /// the repository (`rules.export`), with the record it is for.
    pub export: Option<(i64, RuleExport)>,
    pub export_busy: bool,
    pub delete_arm: Option<DeleteArm>,
    /// Authority and live sources of the last completed exact inspection.
    inspection: Option<Guard>,
}
/// What writing a confirmed convention as a repository rule file would do.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct RuleExport {
    /// `.precept/rules/<id>.md`.
    pub path: String,
    /// The file text Precept writes.
    pub text: String,
    /// The file's current text, when it exists.
    #[serde(default)]
    pub current: Option<String>,
    /// `absent`, `same` (already written), `different` (an update) or
    /// `invalid` (not a rule file Precept can update).
    pub state: String,
    pub valid: bool,
    /// The current file's digest (none when absent): the write's guard.
    pub sha256: Option<String>,
}
/// Whether a record can be edited: a suggestion, or a confirmed rule that is
/// on (switch a rule on before editing it).
pub fn editable(r: &precept_core::operator::PreferenceRecord) -> bool {
    r.status == "suggested" || (r.status == "confirmed" && r.enabled)
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
