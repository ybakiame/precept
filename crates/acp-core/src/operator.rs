//! Data-only Profile operator DTOs. No authorization, storage or writer implementation.
pub use crate::typed_context::Origin;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Cli,
    Desktop,
    Tui,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionAction {
    Confirm,
    Reject,
    Deprecate,
    Demote,
    Delete,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDecisionRecord {
    pub preference_id: i64,
    pub action: DecisionAction,
    pub from_revision: i64,
    pub to_revision: i64,
    pub proposal_digest: String,
    pub receipt_commit_sequence: i64,
    pub channel: Channel,
    pub decided_at: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeerDemotion {
    pub id: i64,
    pub revision: i64,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewQueue {
    pub binding: Binding,
    pub commit_sequence: i64,
    pub records: Vec<PreferenceRecord>,
    pub next_after_id: Option<i64>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewInspect {
    pub binding: Binding,
    pub commit_sequence: i64,
    pub record: PreferenceRecord,
    pub decisions: Vec<ReviewDecisionRecord>,
    pub peer_demotions: Vec<PeerDemotion>,
    pub peer_demotion_total: i64,
}

/// SHA-256 over canonical compact JSON of the six immutable proposal fields.
/// Key order is sorted; no whitespace variance. Byte comparison only, not attestation.
pub fn proposal_digest(
    scope: &Scope,
    project: Option<&str>,
    key: &str,
    value: &str,
    origin: Origin,
    source: &str,
) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let scope_val = serde_json::to_value(scope).expect("scope serializes");
    let mut map = std::collections::BTreeMap::new();
    map.insert("key", serde_json::Value::String(key.to_owned()));
    map.insert(
        "origin",
        serde_json::to_value(origin).expect("origin serializes"),
    );
    map.insert(
        "project",
        match project {
            Some(p) => serde_json::Value::String(p.to_owned()),
            None => serde_json::Value::Null,
        },
    );
    map.insert("scope", scope_val);
    map.insert("source", serde_json::Value::String(source.to_owned()));
    map.insert("value", serde_json::Value::String(value.to_owned()));
    let bytes = serde_json::to_vec(&map).expect("canonical map serializes");
    Sha256::digest(&bytes).into()
}

pub fn digest_hex(digest: &[u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn digest_from_hex(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
    }
    Some(out)
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

    #[test]
    fn canonical_digest_is_deterministic_and_field_sensitive() {
        let d1 = proposal_digest(
            &Scope::Global,
            None,
            "editor",
            "vim",
            Origin::AgentReported,
            "test",
        );
        let d2 = proposal_digest(
            &Scope::Global,
            None,
            "editor",
            "vim",
            Origin::AgentReported,
            "test",
        );
        assert_eq!(d1, d2);
        let d3 = proposal_digest(
            &Scope::Global,
            None,
            "editor",
            "emacs",
            Origin::AgentReported,
            "test",
        );
        assert_ne!(d1, d3);
        let d4 = proposal_digest(
            &Scope::Global,
            None,
            "editor",
            "vim",
            Origin::DaemonObserved,
            "test",
        );
        assert_ne!(d1, d4);
        let d5 = proposal_digest(
            &Scope::Project {
                name: "alpha".into(),
            },
            Some("alpha"),
            "editor",
            "vim",
            Origin::AgentReported,
            "test",
        );
        assert_ne!(d1, d5);
    }

    #[test]
    fn digest_hex_roundtrip() {
        let d = proposal_digest(&Scope::Global, None, "k", "v", Origin::LegacyUnknown, "s");
        let hex = digest_hex(&d);
        assert_eq!(hex.len(), 64);
        assert_eq!(digest_from_hex(&hex), Some(d));
        assert_eq!(digest_from_hex(&hex.to_uppercase()), None);
        assert_eq!(digest_from_hex("short"), None);
        assert_eq!(digest_from_hex(&"g".repeat(64)), None);
    }

    #[test]
    fn channel_and_decision_action_closed_sets() {
        assert!(serde_json::from_str::<Channel>("\"cli\"").is_ok());
        assert!(serde_json::from_str::<Channel>("\"desktop\"").is_ok());
        assert!(serde_json::from_str::<Channel>("\"tui\"").is_ok());
        assert!(serde_json::from_str::<Channel>("\"web\"").is_err());
        assert!(serde_json::from_str::<DecisionAction>("\"confirm\"").is_ok());
        assert!(serde_json::from_str::<DecisionAction>("\"demote\"").is_ok());
        assert!(serde_json::from_str::<DecisionAction>("\"approve\"").is_err());
    }

    #[test]
    fn origin_default_is_agent_reported() {
        assert_eq!(Origin::default(), Origin::AgentReported);
    }

    #[test]
    fn review_dtos_refuse_unknown_fields() {
        let raw = r#"{"preference_id":1,"action":"confirm","from_revision":1,"to_revision":2,"proposal_digest":"aa","receipt_commit_sequence":1,"channel":"cli","decided_at":"t","extra":true}"#;
        assert!(serde_json::from_str::<ReviewDecisionRecord>(raw).is_err());
    }
}
