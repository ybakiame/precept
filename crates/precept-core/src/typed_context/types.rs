use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectBinding {
    pub project: String,
    /// Opaque registration generation, never a caller-selected filesystem root.
    pub root_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub binding: ProjectBinding,
    pub agent_id: Option<String>,
    pub path: Option<String>,
    pub language: Option<String>,
    pub task_tags: Option<Vec<String>>,
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selector {
    pub projects: Option<Vec<ProjectBinding>>,
    pub agents: Option<Vec<String>>,
    pub paths: Option<Vec<String>>,
    pub languages: Option<Vec<String>>,
    pub task_tags: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleRef {
    pub id: String,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Scope {
    Global,
    Project {
        binding: ProjectBinding,
    },
    Session {
        binding: ProjectBinding,
        session_id: String,
    },
}

macro_rules! enums {
    ($($name:ident { $($variant:ident),+ $(,)? })+) => {$ (
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    )+};
}
enums! {
    RuleKind { Preference, Contract, Constraint }
    Authority { Personal, Repository, SessionNarrowing }
    ReviewState { Suggested, Confirmed, Rejected, Deprecated, Unreviewed, Accepted }
    SourceState { Fresh, Stale, Unavailable, Unknown, Malformed, UnsupportedSchema }
    Origin { AgentReported, DaemonObserved, IndependentlyVerified, LegacyUnknown }
}
#[allow(clippy::derivable_impls)]
impl Default for Origin {
    fn default() -> Self {
        Self::AgentReported
    }
}
enums! {
    Applicability { Match, NoMatch, Unknown, Invalid }
    Outcome { Selected, Masked, Excluded, Unknown, Conflicted }
    ConflictCategory {
        RuleVsRule, RuleVsRepository, PreferenceVsContract, SessionVsContract,
        SessionVsPolicy, StaleSourceSnapshot, SourceUnavailable, SourceUnknown,
        UnknownApplicability, InvalidSourceSchema
    }
    ConflictStatus { ResolvedByAuthority, Unresolved }
    ReviewAction { SupplyQuery, RefreshSource, RepairInput, InspectPreference, ReviewRules, InspectRepository, ReviewConstraint }
    Reason {
        ReviewRequired, SelectorMismatch, MissingQuery, InvalidSelector, InvalidSchema,
        InvalidRuleShape, InvalidIdentity, InvalidSource, StaleSource, UnavailableSource,
        UnknownSource, UnverifiedObservation, Superseded, ProjectPreference,
        NewerPreference, RepositoryContract, SessionConstraint, RepositoryObservation,
        UnresolvedKey
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HistoricalResult {
    PASS,
    FAIL,
    SKIPPED,
    #[serde(rename = "NOT_RUN")]
    NotRun,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoricalValidation {
    pub reference: String,
    pub origin: Origin,
    pub result: HistoricalResult,
    pub revision: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub origin: Origin,
    pub references: Vec<String>,
    pub historical_validation: Vec<HistoricalValidation>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    pub id: String,
    pub version: String,
    pub digest: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub reference: SourceRef,
    pub state: SourceState,
    pub provenance: Provenance,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub schema_version: u32,
    pub reference: RuleRef,
    pub kind: RuleKind,
    pub scope: Scope,
    pub authority: Authority,
    pub review: ReviewState,
    pub selector: Selector,
    pub key: String,
    pub allowed_values: Vec<String>,
    pub legacy_preference_id: Option<u64>,
    pub supersedes: Vec<RuleRef>,
    pub source: Source,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryFact {
    pub binding: ProjectBinding,
    pub key: String,
    pub value: String,
    pub source: Source,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyBoundary {
    pub binding: ProjectBinding,
    pub key: String,
    pub allowed_values: Vec<String>,
    pub source: Source,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionInputV1 {
    pub schema_version: u32,
    pub registered_projects: Vec<ProjectBinding>,
    pub query: Query,
    pub rules: Vec<Rule>,
    pub repository_facts: Vec<RepositoryFact>,
    pub policy_boundaries: Vec<PolicyBoundary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleExplanation {
    pub rule: Rule,
    pub applicability: Applicability,
    pub outcome: Outcome,
    pub reasons: Vec<Reason>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conflict {
    pub key: String,
    pub category: ConflictCategory,
    pub status: ConflictStatus,
    pub rules: Vec<RuleRef>,
    pub sources: Vec<SourceRef>,
    pub next_action: ReviewAction,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveValue {
    pub key: String,
    pub allowed_values: Vec<String>,
    pub rules: Vec<RuleRef>,
    pub sources: Vec<SourceRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveContextV1 {
    pub schema_version: u32,
    pub binding: ProjectBinding,
    pub explanations: Vec<RuleExplanation>,
    pub effective: Vec<EffectiveValue>,
    pub conflicts: Vec<Conflict>,
    pub repository_facts: Vec<RepositoryFact>,
    pub policy_boundaries: Vec<PolicyBoundary>,
}

/// Sanitized call-level failures. Record-level invalidity is explained in the DTO.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputError {
    UnsupportedSchema,
    InvalidRegistration,
    UnregisteredProject,
    InvalidQuery,
}
impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for InputError {}
