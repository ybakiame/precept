//! Data-only Profile operator DTOs. No authorization, storage or writer implementation.
use serde::{Deserialize, Serialize};

/// Who entered a preference proposal: the operator typed it, or the operator
/// recorded an agent's suggestion. A claim the operator makes, not attestation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    #[default]
    Operator,
    Agent,
}
impl Origin {
    pub const ALL: [Self; 2] = [Self::Operator, Self::Agent];
    /// The stored and wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Operator => "operator",
            Self::Agent => "agent",
        }
    }
    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|o| o.as_str() == raw)
    }
}

/// The most tags one preference carries, and the longest tag in characters.
pub const MAX_TAGS: usize = 5;
pub const MAX_TAG_CHARS: usize = 32;

/// One tag in its canonical form: lowercase, a leading `#` dropped, spaces and
/// underscores as `-`, letters (any script), digits and inner `-` only.
pub fn normalize_tag(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let raw = raw.strip_prefix('#').unwrap_or(raw);
    let mut out = String::new();
    for c in raw.chars().flat_map(char::to_lowercase) {
        let c = if c.is_whitespace() || c == '_' {
            '-'
        } else {
            c
        };
        if c == '-' && (out.is_empty() || out.ends_with('-')) {
            continue;
        }
        if c != '-' && !c.is_alphanumeric() {
            return None;
        }
        out.push(c);
    }
    while out.ends_with('-') {
        out.pop();
    }
    (!out.is_empty() && out.chars().count() <= MAX_TAG_CHARS).then_some(out)
}
/// A preference's tags in canonical order: each normalized, no repeats, sorted,
/// one to [`MAX_TAGS`]. `None` if any tag is invalid or the count is out of range.
pub fn canonical_tags<S: AsRef<str>>(raw: &[S]) -> Option<Vec<String>> {
    let mut tags = raw
        .iter()
        .map(|t| normalize_tag(t.as_ref()))
        .collect::<Option<Vec<_>>>()?;
    tags.sort();
    tags.dedup();
    (1..=MAX_TAGS).contains(&tags.len()).then_some(tags)
}
/// Whether `tags` is already canonical (what the writer and verifier require).
pub fn tags_canonical(tags: &[String]) -> bool {
    canonical_tags(tags).as_deref() == Some(tags)
}
/// The stored form: a compact JSON array.
pub fn tags_json(tags: &[String]) -> String {
    serde_json::to_string(tags).expect("tags serialize")
}
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
    Project {
        name: String,
    },
    /// Descriptive notes about this machine for one project (never committed,
    /// never competing with preferences or contracts, always advisory).
    Environment {
        name: String,
    },
}
impl Scope {
    /// The project a scoped record belongs to.
    pub fn project(&self) -> Option<&str> {
        match self {
            Scope::Global => None,
            Scope::Project { name } | Scope::Environment { name } => Some(name),
        }
    }
    /// The stored scope word.
    pub fn kind(&self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::Project { .. } => "project",
            Scope::Environment { .. } => "environment",
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
            Environment { name: String },
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Global {} => Self::Global,
            Wire::Project { name } => Self::Project { name },
            Wire::Environment { name } => Self::Environment { name },
        })
    }
}
/// Where a rule applies. An empty list applies everywhere; a populated one
/// narrows the rule to sessions whose request names a matching value (a
/// request that names nothing for that dimension still receives the rule,
/// labelled with where it applies). Canonical: lists sorted, no repeats,
/// labels lowercase.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliesTo {
    #[serde(default)]
    pub agents: Vec<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    /// Repository-relative paths; one ending in `/` covers everything under it.
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub task_tags: Vec<String>,
}
/// The most entries in one `applies_to` list.
pub const MAX_APPLIES: usize = 16;
fn applies_label(s: &str) -> bool {
    !s.is_empty() && s.len() <= 120 && !s.chars().any(|c| c.is_control() || c.is_whitespace())
}
fn applies_path(s: &str) -> bool {
    let body = s.strip_suffix('/').unwrap_or(s);
    !body.is_empty()
        && s.len() <= 256
        && !s.contains(['\\', ':', '*', '?', '[', ']', '{', '}'])
        && !s.chars().any(char::is_control)
        && body.split('/').all(|p| !matches!(p, "" | "." | ".."))
}
impl AppliesTo {
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
            && self.languages.is_empty()
            && self.paths.is_empty()
            && self.task_tags.is_empty()
    }
    /// The canonical form, or `None` when an entry is invalid or a list too long.
    pub fn canonical(&self) -> Option<Self> {
        fn list(v: &[String], lower: bool, ok: fn(&str) -> bool) -> Option<Vec<String>> {
            let mut out: Vec<String> = v
                .iter()
                .map(|s| {
                    let s = s.trim();
                    if lower {
                        s.to_lowercase()
                    } else {
                        s.to_owned()
                    }
                })
                .collect();
            if !out.iter().all(|s| ok(s)) {
                return None;
            }
            out.sort();
            out.dedup();
            (out.len() <= MAX_APPLIES).then_some(out)
        }
        Some(Self {
            agents: list(&self.agents, true, applies_label)?,
            languages: list(&self.languages, true, applies_label)?,
            paths: list(&self.paths, false, applies_path)?,
            task_tags: list(&self.task_tags, true, applies_label)?,
        })
    }
    pub fn is_canonical(&self) -> bool {
        self.canonical().as_ref() == Some(self)
    }
    /// Compact JSON (the stored form; field order is fixed).
    pub fn json(&self) -> String {
        serde_json::to_string(self).expect("applies_to serializes")
    }
}

/// What a rule declares about checking it: `advisory` (delivered only),
/// `verify` (checkable against the repository), `hard` (a pre-tool gate may
/// deny calls that break it). A declaration, never an achieved level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Enforcement {
    #[default]
    Advisory,
    Verify,
    Hard,
}
impl Enforcement {
    pub const ALL: [Self; 3] = [Self::Advisory, Self::Verify, Self::Hard];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Advisory => "advisory",
            Self::Verify => "verify",
            Self::Hard => "hard",
        }
    }
    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.as_str() == raw)
    }
}

/// Priority bounds: a higher priority wins among rules of one key and scope.
pub const PRIORITY_MIN: i64 = -100;
pub const PRIORITY_MAX: i64 = 100;

/// The reviewed rule options of a proposal beside its key and value.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleOptions {
    #[serde(default)]
    pub applies_to: AppliesTo,
    #[serde(default)]
    pub priority: i64,
    #[serde(default)]
    pub enforcement: Enforcement,
}
impl RuleOptions {
    /// Canonical and in bounds; environment notes are always advisory.
    pub fn valid_for(&self, scope: &Scope) -> bool {
        self.applies_to.is_canonical()
            && (PRIORITY_MIN..=PRIORITY_MAX).contains(&self.priority)
            && (!matches!(scope, Scope::Environment { .. })
                || self.enforcement == Enforcement::Advisory)
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
    /// Confirms or rejects several exact suggestions, or deprecates several
    /// confirmed records, in one write, each as its own decision: every item
    /// must still be the reviewed revision and proposal, or nothing is
    /// written.
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
    /// Retires confirmed records (moving environment notes to memory).
    Deprecate,
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
    /// While none is selected: where the daemon suggests a first Profile (its
    /// private state directory) and whether a file is already there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_path: Option<String>,
    #[serde(default)]
    pub suggested_exists: bool,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    #[default]
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
    fn rule_options_are_canonical_bounded_and_digested() {
        let raw = AppliesTo {
            agents: vec!["Claude-Code".into(), "claude-code".into()],
            languages: vec![" Rust ".into()],
            paths: vec!["apps/ui/".into(), "Cargo.toml".into()],
            task_tags: vec![],
        };
        let c = raw.canonical().unwrap();
        assert_eq!(c.agents, ["claude-code"]);
        assert_eq!(c.languages, ["rust"]);
        assert_eq!(c.paths, ["Cargo.toml", "apps/ui/"]);
        assert!(c.is_canonical() && !raw.is_canonical());
        assert_eq!(
            c.json(),
            r#"{"agents":["claude-code"],"languages":["rust"],"paths":["Cargo.toml","apps/ui/"],"task_tags":[]}"#
        );
        for bad in ["../x", "/abs", "a//b", "c:\\x", "src/*.rs", ""] {
            let a = AppliesTo {
                paths: vec![bad.into()],
                ..Default::default()
            };
            assert!(a.canonical().is_none(), "{bad}");
        }
        let many = AppliesTo {
            agents: (0..17).map(|i| format!("a{i}")).collect(),
            ..Default::default()
        };
        assert!(many.canonical().is_none());
        let env = Scope::Environment {
            name: "alpha".into(),
        };
        let hard = RuleOptions {
            enforcement: Enforcement::Hard,
            ..Default::default()
        };
        assert!(hard.valid_for(&Scope::Global) && !hard.valid_for(&env));
        assert!(
            !RuleOptions {
                priority: 101,
                ..Default::default()
            }
            .valid_for(&Scope::Global)
        );
        let tags = vec!["style".to_owned()];
        let base = proposal_digest(
            &Scope::Global,
            None,
            "k",
            "v",
            Origin::Operator,
            &tags,
            &RuleOptions::default(),
        );
        let scoped = RuleOptions {
            applies_to: c,
            ..Default::default()
        };
        for rule in [
            scoped,
            RuleOptions {
                priority: 5,
                ..Default::default()
            },
            hard,
        ] {
            assert_ne!(
                base,
                proposal_digest(
                    &Scope::Global,
                    None,
                    "k",
                    "v",
                    Origin::Operator,
                    &tags,
                    &rule
                )
            );
        }
        assert_ne!(
            base,
            proposal_digest(
                &env,
                Some("alpha"),
                "k",
                "v",
                Origin::Operator,
                &tags,
                &RuleOptions::default()
            )
        );
    }

    #[test]
    fn canonical_digest_is_deterministic_and_field_sensitive() {
        let tags = vec!["style".to_owned()];
        let d1 = proposal_digest(
            &Scope::Global,
            None,
            "editor",
            "vim",
            Origin::Agent,
            &tags,
            &RuleOptions::default(),
        );
        let d2 = proposal_digest(
            &Scope::Global,
            None,
            "editor",
            "vim",
            Origin::Agent,
            &tags,
            &RuleOptions::default(),
        );
        assert_eq!(d1, d2);
        let d3 = proposal_digest(
            &Scope::Global,
            None,
            "editor",
            "emacs",
            Origin::Agent,
            &tags,
            &RuleOptions::default(),
        );
        assert_ne!(d1, d3);
        let d4 = proposal_digest(
            &Scope::Global,
            None,
            "editor",
            "vim",
            Origin::Operator,
            &tags,
            &RuleOptions::default(),
        );
        assert_ne!(d1, d4);
        let d5 = proposal_digest(
            &Scope::Project {
                name: "alpha".into(),
            },
            Some("alpha"),
            "editor",
            "vim",
            Origin::Agent,
            &tags,
            &RuleOptions::default(),
        );
        assert_ne!(d1, d5);
        let d6 = proposal_digest(
            &Scope::Global,
            None,
            "editor",
            "vim",
            Origin::Agent,
            &["git".to_owned()],
            &RuleOptions::default(),
        );
        assert_ne!(d1, d6);
    }

    #[test]
    fn digest_hex_roundtrip() {
        let d = proposal_digest(
            &Scope::Global,
            None,
            "k",
            "v",
            Origin::Operator,
            &["t".into()],
            &RuleOptions::default(),
        );
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
    fn origin_is_a_closed_pair_defaulting_to_operator() {
        assert_eq!(Origin::default(), Origin::Operator);
        for o in Origin::ALL {
            assert_eq!(Origin::parse(o.as_str()), Some(o));
            let wire = serde_json::to_string(&o).unwrap();
            assert_eq!(wire, format!("\"{}\"", o.as_str()));
        }
        assert!(serde_json::from_str::<Origin>("\"agent_reported\"").is_err());
        assert_eq!(Origin::parse("legacy_unknown"), None);
    }

    #[test]
    fn tags_normalize_dedupe_sort_and_bound() {
        assert_eq!(
            normalize_tag(" #Code Style ").as_deref(),
            Some("code-style")
        );
        assert_eq!(
            normalize_tag("snake_case__x").as_deref(),
            Some("snake-case-x")
        );
        assert_eq!(normalize_tag("测试").as_deref(), Some("测试"));
        assert_eq!(normalize_tag("--"), None);
        assert_eq!(normalize_tag("a/b"), None);
        assert_eq!(normalize_tag(&"x".repeat(33)), None);
        assert_eq!(
            canonical_tags(&["Git", "style", "git"]),
            Some(vec!["git".to_owned(), "style".to_owned()])
        );
        assert_eq!(canonical_tags::<&str>(&[]), None);
        assert_eq!(canonical_tags(&["a", "b", "c", "d", "e", "f"]), None);
        assert!(tags_canonical(&["a".into(), "b".into()]));
        assert!(!tags_canonical(&["b".into(), "a".into()]));
        assert!(!tags_canonical(&["A".into()]));
        assert_eq!(tags_json(&["a".into(), "b".into()]), r#"["a","b"]"#);
    }

    #[test]
    fn review_dtos_refuse_unknown_fields() {
        let raw = r#"{"preference_id":1,"action":"confirm","from_revision":1,"to_revision":2,"proposal_digest":"aa","receipt_commit_sequence":1,"channel":"cli","decided_at":"t","extra":true}"#;
        assert!(serde_json::from_str::<ReviewDecisionRecord>(raw).is_err());
    }
}
