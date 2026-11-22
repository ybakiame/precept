//! Read-only domain layer. Adapters supply records and current worktree evidence.
//! No SQLite opening, process execution, MCP transport, or mutation API lives here.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};

pub mod briefing;
pub mod contracts;
pub mod documents;
pub mod fingerprint;
pub mod import;
pub mod pointer;
pub mod typed_context;

pub type Result<T> = std::result::Result<T, Error>;

/// Sanitized errors: never include records, supplied selectors or host paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    UnknownProject,
    HandoffNotFound,
    InvalidArguments,
    UnknownTool,
    ProjectOverride,
    InvalidRecord,
    AdapterUnavailable,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub root: PathBuf,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Suggested,
    Confirmed,
    Rejected,
    Deprecated,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Global,
    Project,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preference {
    pub id: u64,
    pub scope: Scope,
    pub project: Option<String>,
    pub key: String,
    pub value: String,
    pub status: Status,
    /// Canonical tags (see [`operator::canonical_tags`]); agents may use them
    /// to find relevant preferences, never to drop confirmed ones.
    pub tags: Vec<String>,
    /// Where it applies, priority, declared enforcement.
    #[serde(default)]
    pub rule: operator::RuleOptions,
    /// Off rules are never delivered.
    #[serde(default = "on")]
    pub enabled: bool,
}
fn on() -> bool {
    true
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Worktree {
    pub head: Option<String>,
    pub branch: Option<String>,
    pub dirty: Option<bool>,
    pub change_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_fingerprint: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ValidationStatus {
    PASS,
    FAIL,
    SKIPPED,
    #[serde(rename = "NOT_RUN")]
    NotRun,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Validation {
    pub check: String,
    pub status: ValidationStatus,
    /// Python P0 preserves additional evidence/provenance keys unchanged.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Handoff {
    pub project: String,
    pub task_id: String,
    pub objective: String,
    pub base_sha: String,
    pub branch: String,
    pub worktree_state: String,
    #[serde(default)]
    pub worktree_fingerprint: Option<String>,
    pub completed: String,
    pub next_action: String,
    pub validation: Vec<Validation>,
    pub updated_at: String,
}

pub trait ProjectRegistry {
    fn project(&self, name: &str) -> Result<Project>;
}
pub trait PreferenceRepository {
    fn preferences(&self, project: &str) -> Result<Vec<Preference>>;
}
pub trait HandoffRepository {
    fn handoff(&self, project: &str, task_id: &str) -> Result<Handoff>;
}
/// Implementations must report unavailable evidence as None, never invent a clean state.
/// Fingerprints must use Python's acp-worktree-v1 algorithm or remain None.
pub trait WorktreeInspector {
    fn inspect(&self, project: &Project, fingerprint: bool) -> Result<Worktree>;
    /// File names (without `.md`) directly under `dir` that Git tracks with
    /// no uncommitted change; `None` when Git cannot tell.
    fn reviewed(
        &self,
        _project: &Project,
        _dir: &str,
    ) -> Option<std::collections::BTreeSet<String>> {
        None
    }
}
/// Supplies live allowlisted metadata without coupling resolution to filesystem access.
pub trait DocumentRepository {
    fn metadata(&self, project: &Project, include_content: bool) -> Result<Vec<Value>>;
}
pub trait PermissionEvaluator {
    fn evaluate(&self, name: &str, arguments: &Value) -> Result<ReadRequest>;
}
#[derive(Debug, PartialEq, Eq)]
pub enum ReadRequest {
    Context { include_documents: bool },
    Handoff { task_id: String },
}
/// Validates tool-shaped requests only; this is not an MCP/JSON-RPC implementation.
pub struct ReadOnlyPermissions;
impl PermissionEvaluator for ReadOnlyPermissions {
    fn evaluate(&self, name: &str, arguments: &Value) -> Result<ReadRequest> {
        let args = arguments.as_object().ok_or(Error::InvalidArguments)?;
        if args.contains_key("project") || args.contains_key("project_id") {
            return Err(Error::ProjectOverride);
        }
        match name {
            "get_effective_context" => {
                if args
                    .keys()
                    .any(|k| k != "include_documents" && k != "selector")
                {
                    return Err(Error::InvalidArguments);
                }
                if let Some(selector) = args.get("selector") {
                    serde_json::from_value::<contracts::SelectorQuery>(selector.clone())
                        .map_err(|_| Error::InvalidArguments)?
                        .validate()?;
                }
                let include_documents = match args.get("include_documents") {
                    None => false,
                    Some(Value::Bool(b)) => *b,
                    _ => return Err(Error::InvalidArguments),
                };
                Ok(ReadRequest::Context { include_documents })
            }
            "get_handoff" => {
                if args.len() != 1 {
                    return Err(Error::InvalidArguments);
                }
                Ok(ReadRequest::Handoff {
                    task_id: args
                        .get("task_id")
                        .and_then(Value::as_str)
                        .ok_or(Error::InvalidArguments)?
                        .to_owned(),
                })
            }
            _ => Err(Error::UnknownTool),
        }
    }
}

pub const AUTHORITY: &str = "Current repository files and contracts outrank preferences and task handoffs; verify conflicts.";
/// The fixed warnings a Handoff read may carry, in this order of checks.
pub const RESUME_WARNINGS: [&str; 7] = [
    "HEAD changed or cannot be verified; re-evaluate all prior checks",
    "Branch differs from handoff; inspect worktree before proceeding",
    "Current worktree has changes or cannot be inspected; preserve user edits",
    "Handoff has no worktree fingerprint; uncommitted changes cannot be compared",
    "Worktree fingerprint unavailable; inspect differences manually",
    "Worktree fingerprint changed since handoff; inspect uncommitted edits",
    "Earlier validation is scoped to its recorded revision and environment",
];
pub const WARNINGS: [&str; 3] = [
    "Only confirmed preferences are included.",
    "Documents are live references, not a complete project audit.",
    "This context does not authorize filesystem edits or external actions.",
];

/// Immutable project binding. Every lookup revalidates the registry and record scope.
/// Trustworthy adapters are required; this is not a hostile same-user sandbox.
pub struct ContextResolver<'a> {
    project: String,
    registry: &'a dyn ProjectRegistry,
    preferences: &'a dyn PreferenceRepository,
    handoffs: &'a dyn HandoffRepository,
    worktree: &'a dyn WorktreeInspector,
    documents: &'a dyn DocumentRepository,
}
impl<'a> ContextResolver<'a> {
    pub fn new(
        project: &str,
        registry: &'a dyn ProjectRegistry,
        preferences: &'a dyn PreferenceRepository,
        handoffs: &'a dyn HandoffRepository,
        worktree: &'a dyn WorktreeInspector,
        documents: &'a dyn DocumentRepository,
    ) -> Result<Self> {
        let resolver = Self {
            project: project.to_owned(),
            registry,
            preferences,
            handoffs,
            worktree,
            documents,
        };
        resolver.bound_project()?;
        Ok(resolver)
    }
    fn bound_project(&self) -> Result<Project> {
        let p = self.registry.project(&self.project)?;
        if p.name != self.project {
            return Err(Error::UnknownProject);
        }
        Ok(p)
    }
    pub fn resolve_preferences(&self) -> Result<Vec<Preference>> {
        self.bound_project()?;
        Ok(legacy_preferences(confirmed_preferences(
            &self.project,
            self.preferences.preferences(&self.project)?,
        )))
    }
    pub fn context(&self, include_documents: bool) -> Result<Value> {
        self.prepare_context(include_documents, &contracts::SelectorQuery::default())?
            .finish(&self.bound_project()?.root)
    }
    /// Assemble one live read. The daemon rechecks admission before finish(); the
    /// source is checked again there. No raw typed explanation is ever projected.
    pub fn prepare_context(
        &self,
        include_documents: bool,
        selector: &contracts::SelectorQuery,
    ) -> Result<PreparedContext> {
        selector.validate()?;
        let project = self.bound_project()?;
        let snapshot = contracts::Snapshot::read(&project.root)
            .with_reviewed(self.worktree.reviewed(&project, contracts::DIR));
        let projection = contracts::project(
            &self.project,
            self.preferences.preferences(&self.project)?,
            &snapshot,
            selector,
        )?;
        let preferences: Vec<_> = projection
            .preferences
            .into_iter()
            .map(|p| {
                let mut v = json!({
                    "id":p.id,"key":p.key,"value":p.value,"scope":p.scope,"status":p.status,"tags":p.tags
                });
                if !p.rule.applies_to.is_empty() {
                    v["applies_to"] = json!(p.rule.applies_to);
                }
                if p.rule.priority != 0 {
                    v["priority"] = json!(p.rule.priority);
                }
                if p.rule.enforcement != operator::Enforcement::Advisory {
                    v["enforcement"] = json!(p.rule.enforcement);
                }
                v
            })
            .collect();
        let mut repository = serde_json::to_value(self.worktree.inspect(&project, false)?)
            .map_err(|_| Error::InvalidRecord)?;
        repository
            .as_object_mut()
            .ok_or(Error::InvalidRecord)?
            .remove("worktree_fingerprint");
        Ok(PreparedContext {
            snapshot,
            response: json!({"project":self.project,"authority":AUTHORITY,"preferences":preferences,
            "documents":self.documents.metadata(&project, include_documents)?,
            "repository":repository,"warnings":WARNINGS,
            "contract_resolution":projection.contract_resolution,"repository_rules":projection.repository_rules,
            "pending_rules":projection.pending_rules}),
        })
    }
    /// Why each rule of `key` reaches a request with `selector`, or not.
    pub fn explain(&self, key: &str, selector: &contracts::SelectorQuery) -> Result<Value> {
        selector.validate()?;
        if key.is_empty() || key.chars().count() > 120 {
            return Err(Error::InvalidArguments);
        }
        let project = self.bound_project()?;
        let snapshot = contracts::Snapshot::read(&project.root)
            .with_reviewed(self.worktree.reviewed(&project, contracts::DIR));
        contracts::explain(
            &self.project,
            self.preferences.preferences(&self.project)?,
            &snapshot,
            selector,
            key,
        )
    }
    pub fn handoff(&self, task_id: &str) -> Result<Value> {
        let project = self.bound_project()?;
        let handoff = self.handoffs.handoff(&self.project, task_id)?;
        if handoff.project != self.project || handoff.task_id != task_id {
            return Err(Error::HandoffNotFound);
        }
        let current = self.worktree.inspect(&project, true)?;
        let mut warnings = Vec::new();
        if current
            .head
            .as_deref()
            .is_none_or(|s| s.is_empty() || s != handoff.base_sha)
        {
            warnings.push(RESUME_WARNINGS[0]);
        }
        if current
            .branch
            .as_deref()
            .is_none_or(|s| s.is_empty() || s != handoff.branch)
        {
            warnings.push(RESUME_WARNINGS[1]);
        }
        if current.dirty != Some(false) {
            warnings.push(RESUME_WARNINGS[2]);
        }
        match (
            handoff
                .worktree_fingerprint
                .as_deref()
                .filter(|s| !s.is_empty()),
            current
                .worktree_fingerprint
                .as_deref()
                .filter(|s| !s.is_empty()),
        ) {
            (None, _) => warnings.push(RESUME_WARNINGS[3]),
            (_, None) => warnings.push(RESUME_WARNINGS[4]),
            (Some(a), Some(b)) if a != b => warnings.push(RESUME_WARNINGS[5]),
            _ => (),
        }
        warnings.push(RESUME_WARNINGS[6]);
        let mut obj = serde_json::to_value(handoff).map_err(|_| Error::InvalidRecord)?;
        let mut snapshot = serde_json::to_value(&current).map_err(|_| Error::InvalidRecord)?;
        snapshot["worktree_fingerprint"] = json!(current.worktree_fingerprint);
        obj["current_repository"] = snapshot;
        obj["resume_warnings"] = json!(warnings);
        Ok(obj)
    }
    pub fn read(&self, name: &str, arguments: &Value) -> Result<Value> {
        match ReadOnlyPermissions.evaluate(name, arguments)? {
            ReadRequest::Context { include_documents } => {
                let selector = arguments
                    .get("selector")
                    .map(|v| serde_json::from_value::<contracts::SelectorQuery>(v.clone()))
                    .transpose()
                    .map_err(|_| Error::InvalidArguments)?
                    .unwrap_or_default();
                self.prepare_context(include_documents, &selector)?
                    .finish(&self.bound_project()?.root)
            }
            ReadRequest::Handoff { task_id } => self.handoff(&task_id),
        }
    }
}

pub struct PreparedContext {
    snapshot: contracts::Snapshot,
    response: Value,
}
impl PreparedContext {
    pub fn finish(mut self, root: &std::path::Path) -> Result<Value> {
        if !self.snapshot.unchanged(root) {
            self.response["preferences"] = json!([]);
            self.response["repository_rules"] = json!([]);
            self.response["pending_rules"] = json!([]);
            self.response["contract_resolution"] = json!({"status":"source_changed"});
        }
        Ok(self.response)
    }
}
/// Confirmed, enabled rules of this project's scopes (personal and project).
fn confirmed_preferences(project: &str, preferences: Vec<Preference>) -> Vec<Preference> {
    preferences
        .into_iter()
        .filter(|p| {
            p.status == Status::Confirmed
                && p.enabled
                && match p.scope {
                    Scope::Global => p.project.is_none(),
                    Scope::Project => p.project.as_deref() == Some(project),
                }
        })
        .collect()
}
/// Whether a rule may apply to `query`: only a definite mismatch excludes it.
/// A dimension the request does not name keeps the rule (it is projected with
/// its `applies_to`, so the agent can judge).
fn applicable(p: &Preference, query: &contracts::SelectorQuery) -> bool {
    applies(&p.rule.applies_to, query)
}
/// Whether rules limited to `a` may apply to `query` (see [`applicable`]).
fn applies(a: &operator::AppliesTo, query: &contracts::SelectorQuery) -> bool {
    let one = |wanted: &[String], actual: &Option<String>| match actual {
        Some(v) if !wanted.is_empty() => wanted.contains(&v.to_lowercase()),
        _ => true,
    };
    let path = match &query.path {
        Some(q) if !a.paths.is_empty() => a.paths.iter().any(|e| match e.strip_suffix('/') {
            Some(dir) => q == dir || q.starts_with(e.as_str()),
            None => q == e || q.starts_with(&format!("{e}/")),
        }),
        _ => true,
    };
    let tags = match &query.task_tags {
        Some(t) if !a.task_tags.is_empty() && !t.is_empty() => t
            .iter()
            .any(|tag| a.task_tags.contains(&tag.to_lowercase())),
        _ => true,
    };
    one(&a.agents, &query.agent_id) && one(&a.languages, &query.language) && path && tags
}
/// Where a rule applies, in words ("languages rust; paths apps/").
pub(crate) fn where_(a: &operator::AppliesTo) -> String {
    let parts: Vec<String> = [
        ("agents", &a.agents),
        ("languages", &a.languages),
        ("paths", &a.paths),
        ("tasks tagged", &a.task_tags),
    ]
    .iter()
    .filter(|(_, v)| !v.is_empty())
    .map(|(w, v)| format!("{w} {}", v.join(", ")))
    .collect();
    if parts.is_empty() {
        "everywhere".into()
    } else {
        parts.join("; ")
    }
}
/// One effective rule per key among personal and project preferences:
/// project before personal, then higher priority, then the newest.
fn legacy_preferences(mut candidates: Vec<Preference>) -> Vec<Preference> {
    candidates.sort_by(|a, b| {
        a.key
            .cmp(&b.key)
            .then_with(|| (a.scope == Scope::Global).cmp(&(b.scope == Scope::Global)))
            .then_with(|| b.rule.priority.cmp(&a.rule.priority))
            .then_with(|| b.id.cmp(&a.id))
    });
    let mut effective = BTreeMap::new();
    for p in candidates {
        effective.entry(p.key.clone()).or_insert(p);
    }
    effective.into_values().collect()
}

/// Typed operator serialization only; authority remains in the daemon.
pub mod operator;
