use super::{Binding, Origin, RuleOptions, Scope, Snapshot};
use serde::{Deserialize, Serialize};

/// One exact persisted preference, never inferred from effective context.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreferenceRecord {
    pub id: i64,
    pub scope: Scope,
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub status: String,
    pub revision: i64,
    pub origin: Origin,
    /// Where it applies, its priority and declared enforcement (reviewed).
    #[serde(default)]
    pub rule: RuleOptions,
    /// Whether a confirmed rule is delivered (the operator's switch).
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    pub proposal_digest: String,
    pub created_at: String,
    pub updated_at: String,
}
fn enabled_default() -> bool {
    true
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreferencePage {
    pub binding: Binding,
    pub records: Vec<PreferenceRecord>,
    pub next_after_id: Option<i64>,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreferencePreview {
    pub binding: Binding,
    pub record: PreferenceRecord,
}
/// Saved history only. Does not assert fresh repository observations.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffRecord {
    pub project: String,
    pub task_id: String,
    pub revision: i64,
    pub objective: String,
    pub completed: String,
    pub next_action: String,
    pub validation_json: String,
    pub snapshot: Snapshot,
}
/// One saved Handoff of a project, for a list: its task, objective's first
/// line, branch and when it was saved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffSummary {
    pub task_id: String,
    pub objective: String,
    pub branch: String,
    pub revision: i64,
    pub updated_at: String,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffList {
    pub binding: Binding,
    pub handoffs: Vec<HandoffSummary>,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffPreview {
    pub binding: Binding,
    pub record: Option<HandoffRecord>,
}
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileInfo {
    pub selected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<Binding>,
    pub write_enabled: bool,
    /// The selected Profile is the one selected at every daemon start.
    #[serde(default)]
    pub remembered: bool,
    /// A Profile is remembered for start (selected now or not).
    #[serde(default)]
    pub startup_profile: bool,
    /// Why the remembered Profile was not selected at start, as a fixed code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup_failure: Option<String>,
    /// Why an explicitly remembered write grant could not be restored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write_restore_failure: Option<String>,
    /// While none is selected: where the daemon suggests a first Profile (its
    /// private state directory) and whether a file is already there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_path: Option<String>,
    #[serde(default)]
    pub suggested_exists: bool,
}
