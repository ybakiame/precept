//! Data-only Profile operator DTOs. No authorization, storage or writer implementation.
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
    SuggestPreference {
        scope: Scope,
        key: String,
        value: String,
        source: String,
        expected: Expected,
    },
    ConfirmPreference {
        id: i64,
        scope: Scope,
        expected: Expected,
    },
    RejectPreference {
        id: i64,
        scope: Scope,
        expected: Expected,
    },
    DeprecatePreference {
        id: i64,
        scope: Scope,
        expected: Expected,
    },
    DeletePreference {
        id: i64,
        scope: Scope,
        expected: Expected,
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

/// One exact persisted preference, never inferred from effective context.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreferenceRecord {
    pub id: i64,
    pub scope: Scope,
    pub key: String,
    pub value: String,
    pub source: String,
    pub status: String,
    pub revision: i64,
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
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffPreview {
    pub binding: Binding,
    pub record: Option<HandoffRecord>,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileInfo {
    pub selected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<Binding>,
    pub write_enabled: bool,
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scope_refuses_extra_fields_including_unit_variant_null() {
        for raw in [
            r#"{"kind":"global","name":"alpha"}"#,
            r#"{"kind":"global","name":null}"#,
            r#"{"kind":"global","extra":false}"#,
            r#"{"kind":"project","name":"alpha","extra":true}"#,
        ] {
            assert!(serde_json::from_str::<Scope>(raw).is_err());
        }
        for scope in [
            Scope::Global,
            Scope::Project {
                name: "alpha".into(),
            },
        ] {
            let raw = serde_json::to_string(&scope).unwrap();
            assert!(serde_json::from_str::<Scope>(&raw).unwrap() == scope);
        }
    }
}
