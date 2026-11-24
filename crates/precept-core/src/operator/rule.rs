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
pub const MAX_TAGS: usize = 10;
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
    /// Canonical and in bounds.
    pub fn valid(&self) -> bool {
        self.applies_to.is_canonical() && (PRIORITY_MIN..=PRIORITY_MAX).contains(&self.priority)
    }
}
