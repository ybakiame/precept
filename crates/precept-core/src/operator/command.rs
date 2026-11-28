use super::{Channel, Origin, PersonalSyncChange, RuleOptions};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub profile_id: String,
    pub generation: String,
    pub owner_epoch: i64,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordVersion {
    pub kind: String,
    pub project: String,
    pub record_id: String,
    pub revision: i64,
    pub deleted: bool,
}
impl RecordVersion {
    pub fn key(&self) -> (String, String, String) {
        (
            self.kind.clone(),
            self.project.clone(),
            self.record_id.clone(),
        )
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub generation: String,
    pub operation_id: String,
    pub commit_sequence: i64,
    pub affected: Vec<RecordVersion>,
}
impl std::fmt::Debug for Receipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Receipt(redacted)")
    }
}
impl std::fmt::Debug for Binding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Binding(redacted)")
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "revision",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Expected {
    Absent,
    Revision(i64),
}
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Scope {
    Global,
    Project { name: String },
}
impl Scope {
    /// The project a scoped record belongs to.
    pub fn project(&self) -> Option<&str> {
        match self {
            Scope::Global => None,
            Scope::Project { name } => Some(name),
        }
    }
    /// The stored scope word.
    pub fn kind(&self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::Project { .. } => "project",
        }
    }
}
// Struct variants enforce unknown-field refusal, including explicit null fields.
impl<'de> Deserialize<'de> for Scope {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Wire {
            Global {},
            Project { name: String },
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Global {} => Self::Global,
            Wire::Project { name } => Self::Project { name },
        })
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub base_sha: String,
    pub branch: String,
    pub state: String,
    pub fingerprint: Option<String>,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    RegisterProject {
        name: String,
        root: String,
        expected: Expected,
    },
    /// Moves a registered project to another canonical root, keeping its
    /// name and every record scoped to it (operator only, `project.relocate`).
    RelocateProject {
        name: String,
        root: String,
        expected: Expected,
    },
    SuggestPreference {
        scope: Scope,
        key: String,
        value: String,
        tags: Vec<String>,
        origin: Origin,
        /// Where it applies, its priority and its declared enforcement.
        #[serde(default)]
        rule: RuleOptions,
        expected: Expected,
    },
    /// Turns a confirmed rule on or off without a new review (operator only).
    SetPreferenceEnabled {
        id: i64,
        scope: Scope,
        enabled: bool,
        channel: Channel,
        expected: Expected,
    },
    ConfirmPreference {
        id: i64,
        scope: Scope,
        proposal_digest: String,
        channel: Channel,
        expected: Expected,
        /// The operator's note, kept with the decision.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    RejectPreference {
        id: i64,
        scope: Scope,
        proposal_digest: String,
        channel: Channel,
        expected: Expected,
        /// The operator's note, kept with the decision.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    DeprecatePreference {
        id: i64,
        scope: Scope,
        proposal_digest: String,
        channel: Channel,
        expected: Expected,
        /// The operator's note, kept with the decision.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// Replaces a suggestion with the operator's edited version in one
    /// write: the new record is suggested (still to be confirmed) and the old
    /// one is rejected, its decision naming the new id.
    RevisePreference {
        id: i64,
        scope: Scope,
        proposal_digest: String,
        key: String,
        value: String,
        tags: Vec<String>,
        #[serde(default)]
        rule: RuleOptions,
        channel: Channel,
        expected: Expected,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// Files several suggestions in one write (an import the operator
    /// picked): each is an ordinary `suggested` record, none is confirmed.
    SuggestBatch { items: Vec<SuggestItem> },
    /// Confirms or rejects several exact suggestions in one write, each as
    /// its own decision: every item must still be the reviewed revision and
    /// proposal, or nothing is written.
    DecideBatch {
        decision: BatchDecision,
        items: Vec<BatchItem>,
        channel: Channel,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    DeletePreference {
        id: i64,
        scope: Scope,
        proposal_digest: String,
        channel: Channel,
        expected: Expected,
    },
    ApplyPersonalSync {
        bundle_id: String,
        bundle_digest: String,
        device_id: String,
        expected_commit: i64,
        channel: Channel,
        changes: Vec<PersonalSyncChange>,
    },
    PutHandoff {
        project: String,
        task_id: String,
        objective: String,
        completed: String,
        next_action: String,
        validation_json: String,
        snapshot: Snapshot,
        expected: Expected,
    },
}

/// What a batch decides for each of its items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchDecision {
    Confirm,
    Reject,
}
/// One exact suggestion of a batch.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchItem {
    pub id: i64,
    pub scope: Scope,
    pub proposal_digest: String,
    pub expected: Expected,
}
/// One suggestion of a [`Command::SuggestBatch`].
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestItem {
    pub scope: Scope,
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub origin: Origin,
    #[serde(default)]
    pub rule: RuleOptions,
}
/// The most suggestions one batch decides.
pub const MAX_BATCH: usize = 50;

impl Command {
    /// The operator's note on a decision, if the command carries one.
    pub fn note(&self) -> Option<&str> {
        match self {
            Command::ConfirmPreference { note, .. }
            | Command::RejectPreference { note, .. }
            | Command::DeprecatePreference { note, .. }
            | Command::RevisePreference { note, .. }
            | Command::DecideBatch { note, .. } => note.as_deref(),
            _ => None,
        }
    }
}

/// Client serialization only; daemon-owned Request validation is authoritative.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mutation {
    pub version: u32,
    pub binding: Binding,
    pub operation_id: String,
    pub command: Command,
}
