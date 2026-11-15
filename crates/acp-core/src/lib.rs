//! Read-only domain layer. Adapters supply records and current worktree evidence.
//! No SQLite opening, process execution, MCP transport, or mutation API lives here.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};

pub mod documents;
pub mod fingerprint;
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
    pub source: String,
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
                if args.keys().any(|k| k != "include_documents") {
                    return Err(Error::InvalidArguments);
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

pub const AUTHORITY: &str = "Current repository files and accepted ADRs outrank preferences and task handoffs; verify conflicts.";
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
        let mut candidates: Vec<_> = self
            .preferences
            .preferences(&self.project)?
            .into_iter()
            .filter(|p| {
                p.status == Status::Confirmed
                    && match p.scope {
                        Scope::Global => p.project.is_none(),
                        Scope::Project => p.project.as_deref() == Some(self.project.as_str()),
                    }
            })
            .collect();
        candidates.sort_by(|a, b| {
            a.key
                .cmp(&b.key)
                .then_with(|| (a.scope == Scope::Global).cmp(&(b.scope == Scope::Global)))
                .then_with(|| b.id.cmp(&a.id))
        });
        let mut effective = BTreeMap::new();
        for p in candidates {
            effective.entry(p.key.clone()).or_insert(p);
        }
        Ok(effective.into_values().collect())
    }
    pub fn context(&self, include_documents: bool) -> Result<Value> {
        let project = self.bound_project()?;
        let preferences: Vec<_> = self
            .resolve_preferences()?
            .into_iter()
            .map(|p| {
                json!({
                    "id":p.id,"key":p.key,"value":p.value,"scope":p.scope,"status":p.status
                })
            })
            .collect();
        let mut repository = serde_json::to_value(self.worktree.inspect(&project, false)?)
            .map_err(|_| Error::InvalidRecord)?;
        repository
            .as_object_mut()
            .ok_or(Error::InvalidRecord)?
            .remove("worktree_fingerprint");
        Ok(
            json!({"project":self.project,"authority":AUTHORITY,"preferences":preferences,
            "documents":self.documents.metadata(&project, include_documents)?,
            "repository":repository,"warnings":WARNINGS}),
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
            warnings.push("HEAD changed or cannot be verified; re-evaluate all prior checks");
        }
        if current
            .branch
            .as_deref()
            .is_none_or(|s| s.is_empty() || s != handoff.branch)
        {
            warnings.push("Branch differs from handoff; inspect worktree before proceeding");
        }
        if current.dirty != Some(false) {
            warnings
                .push("Current worktree has changes or cannot be inspected; preserve user edits");
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
            (None, _) => warnings.push(
                "Handoff has no worktree fingerprint; uncommitted changes cannot be compared",
            ),
            (_, None) => {
                warnings.push("Worktree fingerprint unavailable; inspect differences manually")
            }
            (Some(a), Some(b)) if a != b => warnings
                .push("Worktree fingerprint changed since handoff; inspect uncommitted edits"),
            _ => (),
        }
        warnings.push("Earlier validation is scoped to its recorded revision and environment");
        let mut obj = serde_json::to_value(handoff).map_err(|_| Error::InvalidRecord)?;
        let mut snapshot = serde_json::to_value(&current).map_err(|_| Error::InvalidRecord)?;
        snapshot["worktree_fingerprint"] = json!(current.worktree_fingerprint);
        obj["current_repository"] = snapshot;
        obj["resume_warnings"] = json!(warnings);
        Ok(obj)
    }
    pub fn read(&self, name: &str, arguments: &Value) -> Result<Value> {
        match ReadOnlyPermissions.evaluate(name, arguments)? {
            ReadRequest::Context { include_documents } => self.context(include_documents),
            ReadRequest::Handoff { task_id } => self.handoff(&task_id),
        }
    }
}

/// Typed operator serialization only; authority remains in the daemon.
pub mod operator;
