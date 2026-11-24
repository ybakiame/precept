use serde::{Deserialize, Serialize};

/// The most entries kept per project.
pub const MAX_ENTRIES: usize = 300;
pub const MAX_TITLE: usize = 120;
pub const MAX_BODY: usize = 2000;
/// A reaffirmation's stated basis (what verified the note).
pub const MAX_BASIS: usize = 300;
/// Earlier bodies kept per entry.
pub const MAX_HISTORY: usize = 5;
/// Environment entries a context carries (all of them, up to this).
pub const BRIEF_ENVIRONMENT: usize = 8;
/// Other entries a context carries, by weight.
pub const BRIEF_RELEVANT: usize = 5;
/// The most text the memory in a context carries, in bytes.
pub const BRIEF_BYTES: usize = 2560;

pub(super) const DAY_MS: u64 = 86_400_000;
/// Days after the last confirmation before an entry starts to fade.
pub const FADE_AFTER_DAYS: u64 = 60;
/// Days without confirmation after which reads carry a verify-first notice.
pub const NOTICE_AFTER_DAYS: u64 = 7;
/// Confirmations after which a caution, lesson or attempt is filed once as a
/// suggested project rule (it held in that many more sessions).
pub const PROPOSE_AFTER: u32 = 3;
/// Changes within this window that make an entry unstable.
pub(super) const UNSTABLE_WINDOW_MS: u64 = 7 * DAY_MS;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Facts about this machine: toolchains, paths, services.
    Environment,
    /// Pitfalls and steps that must come first.
    Caution,
    /// Ways of working that proved right.
    Lesson,
    /// What was tried and how it went (failures included).
    Attempt,
}
impl Kind {
    pub const ALL: [Kind; 4] = [
        Kind::Environment,
        Kind::Caution,
        Kind::Lesson,
        Kind::Attempt,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Environment => "environment",
            Kind::Caution => "caution",
            Kind::Lesson => "lesson",
            Kind::Attempt => "attempt",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Earlier {
    pub body: String,
    pub until_ms: u64,
    pub by: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub kind: Kind,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub confirmations: u32,
    #[serde(default)]
    pub recalls: u32,
    pub created_ms: u64,
    pub updated_ms: u64,
    pub confirmed_ms: u64,
    #[serde(default)]
    pub edited_by_operator: bool,
    /// Who wrote the current body: the agent's client label, `operator`, or
    /// none when unknown.
    pub by: Option<String>,
    /// The work session it was written in, if any.
    #[serde(default)]
    pub session: Option<String>,
    #[serde(default)]
    pub history: Vec<Earlier>,
    /// When the body last changed (within the unstable window).
    #[serde(default)]
    pub changes: Vec<u64>,
    /// The suggested rule this note was filed as, once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposed: Option<String>,
    /// The basis stated at the last reaffirmation, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basis: Option<String>,
}

impl Entry {
    /// A new body: the earlier one goes to history; a correction resets
    /// confirmations (they were for the old text) but keeps the pin.
    pub(super) fn correct(
        &mut self,
        body: String,
        by: Option<String>,
        session: Option<String>,
        now: u64,
    ) {
        self.history.insert(
            0,
            Earlier {
                body: std::mem::take(&mut self.body),
                until_ms: now,
                by: self.by.take(),
            },
        );
        self.history.truncate(MAX_HISTORY);
        self.body = body;
        self.by = by;
        self.session = session;
        self.confirmations = 0;
        self.basis = None;
        self.updated_ms = now;
        self.confirmed_ms = now;
        self.changes
            .retain(|t| now.saturating_sub(*t) <= UNSTABLE_WINDOW_MS);
        self.changes.push(now);
    }
}

/// What `remember` did.
#[derive(Debug, PartialEq, Eq)]
pub enum Remembered {
    Added,
    Reaffirmed,
    Corrected,
}
impl Remembered {
    pub fn as_str(&self) -> &'static str {
        match self {
            Remembered::Added => "added",
            Remembered::Reaffirmed => "reaffirmed",
            Remembered::Corrected => "corrected",
        }
    }
}
