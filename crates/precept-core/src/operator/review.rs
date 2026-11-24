use super::{Binding, Origin, PreferenceRecord, RuleOptions, Scope};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    #[default]
    Cli,
    Desktop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionAction {
    Confirm,
    Reject,
    Deprecate,
    Demote,
    Delete,
    /// The operator switched a confirmed rule on or off (no new review).
    Enable,
    Disable,
    /// A suggestion replaced by an edited one (`replaced_by`).
    Replace,
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
    /// The operator's note on the decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// For `replace`: the suggestion that took this one's place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_by: Option<i64>,
}

/// The longest decision note, in characters.
pub const MAX_NOTE: usize = 500;
/// A note as stored: trimmed, one to [`MAX_NOTE`] characters, no control
/// characters but line breaks. `Some(None)` for no note, `None` if invalid.
pub fn canonical_note(note: Option<&str>) -> Option<Option<String>> {
    let Some(raw) = note else {
        return Some(None);
    };
    let t = raw.trim();
    if t.is_empty() {
        return Some(None);
    }
    (t.chars().count() <= MAX_NOTE && !t.chars().any(|c| c.is_control() && c != '\n'))
        .then(|| Some(t.to_owned()))
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

/// How many suggestions in one scope wait for a decision, at a watermark.
/// A count of the scope as read, not a promise the queue is unchanged since.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewCount {
    pub binding: Binding,
    pub commit_sequence: i64,
    pub suggested: i64,
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
    #[serde(default)]
    pub related: Vec<ReviewRelated>,
    #[serde(default)]
    pub related_total: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<ReviewEvidence>,
}

/// Why another scoped record is relevant to this exact review.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewRelation {
    Duplicate,
    SameKey,
    SameValue,
}

/// One bounded related-record pointer. Opening it still requires a fresh exact
/// revision inspection; this is evidence, never decision authority.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewRelated {
    pub id: i64,
    pub revision: i64,
    pub key: String,
    pub relation: ReviewRelation,
}

/// Project-scoped evidence retained by the daemon. This is not a complete
/// history and says nothing about global records outside one project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewEvidence {
    pub gate_denials: u64,
    pub last_gate_denial_ms: Option<u64>,
}

/// SHA-256 over canonical compact JSON of the immutable proposal fields
/// (tags in canonical order; rule options canonical).
/// Key order is sorted; no whitespace variance. Byte comparison only, not attestation.
pub fn proposal_digest(
    scope: &Scope,
    project: Option<&str>,
    key: &str,
    value: &str,
    origin: Origin,
    tags: &[String],
    rule: &RuleOptions,
) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let scope_val = serde_json::to_value(scope).expect("scope serializes");
    let mut map = std::collections::BTreeMap::new();
    map.insert("key", serde_json::Value::String(key.to_owned()));
    map.insert(
        "applies_to",
        serde_json::to_value(&rule.applies_to).expect("applies_to serializes"),
    );
    map.insert(
        "enforcement",
        serde_json::Value::String(rule.enforcement.as_str().into()),
    );
    map.insert("priority", serde_json::Value::from(rule.priority));
    map.insert("origin", serde_json::Value::String(origin.as_str().into()));
    map.insert(
        "project",
        match project {
            Some(p) => serde_json::Value::String(p.to_owned()),
            None => serde_json::Value::Null,
        },
    );
    map.insert("scope", scope_val);
    map.insert("tags", serde_json::to_value(tags).expect("tags serialize"));
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
