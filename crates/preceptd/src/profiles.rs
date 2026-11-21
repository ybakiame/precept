//! Single-assignment selection and serialized daemon-owned write admission.
use crate::{
    scope,
    state::{MAX_PROJECTS, StateDir},
};
use precept_ipc::{
    Access, Error, Location, Result,
    peer::Peer,
    write_grant::{Grant, Registry},
};
use precept_store::SqliteStore;
use precept_write::{
    Command, Scope,
    owner::{Owner, Selected},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
type Roots = BTreeMap<String, PathBuf>;
struct State {
    selected: Option<Arc<Selected>>,
    /// Roots registered in the selected Profile, for projects in the set.
    roots: Roots,
    owner: Option<Owner>,
    grant: Option<Arc<Grant>>,
    /// The daemon-owned project set (`projects.json`): name → canonical root.
    projects: Roots,
    /// Entries of the file whose root no longer resolves to the same directory:
    /// kept (and removable), never served.
    unavailable: Roots,
    /// Why the remembered startup Profile was not selected, if it was not.
    startup_failure: Option<&'static str>,
    /// Projects whose agent credential is not published (external agents off).
    withheld: BTreeSet<String>,
    /// Projects whose agents may suggest rules (the operator's switch).
    proposers: BTreeSet<String>,
    /// Agent suggestions accepted this daemon run, per project.
    suggested: BTreeMap<String, u32>,
}
/// The most suggestions one project's agents may make in one daemon run.
pub const MAX_AGENT_SUGGESTIONS: u32 = 20;
/// The most agent suggestions one scope may hold waiting for review.
pub const MAX_PENDING_AGENT: i64 = 30;
/// The longest agent suggestion value, in bytes.
pub const MAX_AGENT_VALUE: usize = 4096;
/// What the daemon does around a project-set change, outside the Profile.
/// The file name offered (never created) for a first Profile in the state directory.
pub const SUGGESTED_PROFILE: &str = "profile.sqlite3";

pub trait Hooks {
    /// External agent access to a project was turned off: withdraw only its
    /// agent credential (the project stays).
    fn withheld(&self, name: &str);
    /// A project joined the set: publish its agent credential.
    fn added(&self, name: &str) -> Result<()>;
    /// A project left the set: revoke its agent credential.
    fn removed(&self, name: &str);
}
pub struct Profiles {
    #[cfg(test)]
    pub(crate) before_begin: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    state: Mutex<State>,
    pub location: Location,
    runtime: PathBuf,
    state_dir: StateDir,
    pub grants: Registry,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathParams {
    path: PathBuf,
    /// Select this Profile again when the daemon starts (explicit operator choice).
    #[serde(default)]
    remember: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RootParams {
    root: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NameParams {
    name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentsParams {
    name: String,
    enabled: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddParams {
    name: String,
    root: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Enable {
    profile_id: String,
    generation: String,
    confirm: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreferenceList {
    scope: Scope,
    after_id: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreferenceGet {
    scope: Scope,
    id: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HandoffGet {
    project: String,
    task_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewQueueParams {
    scope: Scope,
    after_id: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewCountParams {
    scope: Scope,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewInspectParams {
    scope: Scope,
    id: i64,
    revision: i64,
}
pub fn storage_error(e: precept_write::Error) -> Error {
    match e {
        precept_write::Error::Unauthorized => Error::Unauthorized,
        precept_write::Error::Busy => Error::Busy,
        precept_write::Error::Conflict => Error::Conflict,
        precept_write::Error::InvalidTransition => Error::InvalidTransition,
        precept_write::Error::IdempotencyConflict => Error::IdempotencyConflict,
        precept_write::Error::OwnerMismatch => Error::OwnerMismatch,
        precept_write::Error::StaleGeneration => Error::StaleGeneration,
        precept_write::Error::ScopeMismatch => Error::ScopeMismatch,
        precept_write::Error::ReviewMismatch => Error::ReviewMismatch,
        precept_write::Error::Capacity => Error::Capacity,
        precept_write::Error::StorageFull => Error::StorageFull,
        precept_write::Error::OutcomeUnknown => Error::OutcomeUnknown,
        precept_write::Error::InvalidCommand => Error::InvalidRequest,
        precept_write::Error::Collision => Error::EndpointCollision,
        precept_write::Error::UnsafeWorkspace => Error::UnsafeRuntime,
        _ => Error::Unavailable,
    }
}
impl Profiles {
    /// Loads the project set from the state directory. Entries whose root no
    /// longer resolves to the same directory are kept as unavailable; a set
    /// that overlaps the runtime or itself refuses the start.
    pub fn new(location: Location, state_dir: StateDir, grants: Registry) -> Result<Self> {
        let runtime = if location.runtime_dir.exists() {
            location.runtime_dir.canonicalize()
        } else {
            location
                .runtime_dir
                .parent()
                .ok_or(Error::UnsafeRuntime)?
                .canonicalize()
                .map(|p| p.join(location.runtime_dir.file_name().unwrap_or_default()))
        }
        .map_err(|_| Error::UnsafeRuntime)?;
        let mut projects = Roots::new();
        let mut unavailable = Roots::new();
        let (listed, withheld, proposers) = state_dir.load_projects()?;
        for (name, root) in listed {
            match root.canonicalize() {
                Ok(c) if c == root && c.is_dir() => {
                    projects.insert(name, c);
                }
                _ => {
                    unavailable.insert(name, root);
                }
            }
        }
        let roots: Vec<&PathBuf> = projects.values().collect();
        for (i, a) in roots.iter().enumerate() {
            if roots[i + 1..]
                .iter()
                .any(|b| a.starts_with(b) || b.starts_with(a))
            {
                return Err(Error::InvalidRequest);
            }
        }
        let this = Self {
            #[cfg(test)]
            before_begin: Mutex::new(None),
            state: Mutex::new(State {
                selected: None,
                roots: Roots::new(),
                owner: None,
                grant: None,
                projects,
                unavailable,
                startup_failure: None,
                withheld,
                proposers,
                suggested: BTreeMap::new(),
            }),
            location,
            runtime,
            state_dir,
            grants,
        };
        {
            let s = this.state.lock().unwrap();
            this.validate_paths(None, &s.projects)?;
        }
        Ok(this)
    }
    /// Selects the Profile the operator asked to select at start, through the
    /// same checks as `profile.select`. A failure starts unselected and is
    /// reported by `profile.info`; nothing is created, scanned or replaced.
    pub fn select_remembered(&self) {
        let path = match self.state_dir.load_startup() {
            Ok(Some(path)) => path,
            Ok(None) => return,
            Err(_) => {
                self.state.lock().unwrap().startup_failure = Some("startup_unreadable");
                return;
            }
        };
        let mut s = self.state.lock().unwrap();
        if std::fs::symlink_metadata(&path).is_err() {
            s.startup_failure = Some("startup_missing");
            return;
        }
        let failure = match self.select_into(&mut s, &path, false) {
            Ok(()) => None,
            Err(Error::UnsafeRuntime) => Some("startup_unsafe"),
            Err(_) => Some("startup_refused"),
        };
        s.startup_failure = failure;
    }
    pub fn allowed(&self, name: &str) -> bool {
        self.state.lock().unwrap().projects.contains_key(name)
    }
    /// Projects whose agent credential is published (shared with agents).
    pub fn shared_names(&self) -> Vec<String> {
        let s = self.state.lock().unwrap();
        s.projects
            .keys()
            .filter(|n| !s.withheld.contains(*n))
            .cloned()
            .collect()
    }
    pub fn location(&self) -> &Location {
        &self.location
    }
    /// The project set for `project.list`: name, root and state.
    pub fn list(&self) -> Value {
        let s = self.state.lock().unwrap();
        let mut out: Vec<Value> = s
            .projects
            .iter()
            .map(|(name, root)| {
                // Without a selected Profile nothing is registered; a missing
                // root still reads as unavailable.
                let agents = !s.withheld.contains(name);
                let propose = s.proposers.contains(name);
                let state = if s.roots.contains_key(name) {
                    "ready"
                } else if root.is_dir() {
                    "unregistered"
                } else {
                    "unavailable"
                };
                json!({"name":name,"root":root,"state":state,"agents":agents,"propose":propose})
            })
            .collect();
        out.extend(
            s.unavailable
                .iter()
                .map(|(name, root)| {
                    json!({"name":name,"root":root,"state":"unavailable","agents":!s.withheld.contains(name),"propose":s.proposers.contains(name)})
                }),
        );
        Value::Array(out)
    }
    fn validate_paths(&self, profile: Option<&Path>, roots: &Roots) -> Result<()> {
        if profile.is_some_and(|p| p.ancestors().any(|a| a.join(".git").exists())) {
            return Err(Error::UnsafeRuntime);
        }
        if profile.is_some_and(|p| p.starts_with(&self.runtime)) {
            return Err(Error::UnsafeRuntime);
        }
        for root in roots.values() {
            if self.runtime.starts_with(root)
                || root.starts_with(&self.runtime)
                || profile.is_some_and(|p| p.starts_with(root))
            {
                return Err(Error::UnsafeRuntime);
            }
        }
        Ok(())
    }
    fn selected_roots(&self, selected: &Selected, projects: &Roots) -> Result<Roots> {
        let store = selected.reader().map_err(storage_error)?;
        let mut roots = Roots::new();
        for (name, expected) in projects {
            match store.read_project(name) {
                Ok(p) => {
                    let root = p.root.canonicalize().map_err(|_| Error::UnsafeRuntime)?;
                    if !root.is_dir() || &root != expected {
                        return Err(Error::UnsafeRuntime);
                    }
                    roots.insert(name.clone(), root);
                }
                Err(precept_store::Error::UnknownProject) => (),
                Err(_) => return Err(Error::NotFound),
            }
        }
        self.validate_paths(Some(selected.path()), projects)?;
        Ok(roots)
    }
    pub fn read(&self) -> Result<(SqliteStore, Roots)> {
        let s = self.state.lock().unwrap();
        let selected = s.selected.as_ref().ok_or(Error::Unavailable)?;
        Ok((selected.reader().map_err(storage_error)?, s.roots.clone()))
    }
    pub fn selected(&self) -> bool {
        self.state.lock().unwrap().selected.is_some()
    }
    fn info(&self, s: &State) -> Result<Value> {
        let startup = self.state_dir.load_startup().ok().flatten();
        let Some(selected) = &s.selected else {
            // A suggestion for the operator's explicit create/select, inside the
            // daemon's own private state directory (so its parent always exists
            // and is private on this system). Never created or selected here.
            let suggested = self.state_dir.path().join(SUGGESTED_PROFILE);
            return Ok(json!({
                "selected":false,
                "write_enabled":false,
                "startup_profile":startup.is_some(),
                "startup_failure":s.startup_failure,
                "suggested_path":suggested,
                "suggested_exists":suggested.exists(),
            }));
        };
        let remembered = startup.as_deref() == Some(selected.path());
        let binding = selected
            .reader()
            .map_err(storage_error)?
            .metadata()
            .map_err(|_| Error::Unavailable)?
            .binding;
        let access = Access::OperatorWrite {
            profile_id: binding.profile_id.clone(),
            generation: binding.generation.clone(),
            owner_epoch: binding.owner_epoch,
        };
        Ok(
            json!({"selected":true,"binding":binding,"write_enabled":s.grant.as_ref().is_some_and(|g|g.check().is_ok() && g.access()==access),"remembered":remembered,"startup_profile":startup.is_some(),"startup_failure":s.startup_failure}),
        )
    }
    pub fn disable(&self) -> Result<Value> {
        self.grants.close(); // Close before waiting for already-admitted work.
        let mut s = self.state.lock().unwrap();
        self.grants.close();
        let cleanup = self.grants.finish_revoke(&self.location);
        s.grant = None;
        s.owner = None; // Only after credential cleanup; no lock-file unlink.
        cleanup?;
        Ok(json!({"write_enabled":false}))
    }
    /// The scope context for a candidate root: home, the daemon's directories,
    /// the selected Profile's directory and every other project root.
    fn scope_check_reason(
        &self,
        s: &State,
        path: &Path,
    ) -> std::result::Result<PathBuf, &'static str> {
        let daemon = [self.runtime.as_path(), self.state_dir.path()];
        let profile = s
            .selected
            .as_ref()
            .and_then(|x| x.path().parent().map(Path::to_path_buf));
        let mut projects: Vec<PathBuf> = s.projects.values().cloned().collect();
        projects.extend(s.roots.values().cloned());
        projects.extend(s.unavailable.values().cloned());
        scope::check(
            path,
            &scope::Context {
                home: scope::home(),
                daemon: &daemon,
                profile: profile.as_deref(),
                projects: &projects,
            },
        )
        .map_err(scope::Refusal::code)
    }
    fn scope_check(&self, s: &State, path: &Path) -> Result<PathBuf> {
        self.scope_check_reason(s, path)
            .map_err(|_| Error::UnsafeRuntime)
    }
    /// Creates (optionally) and selects `path` with every check of an explicit
    /// selection. Used by `profile.create` / `profile.select` and at start.
    fn select_into(&self, s: &mut State, path: &Path, create: bool) -> Result<()> {
        if s.selected.is_some() {
            return Err(Error::RestartRequired);
        }
        if !path.is_absolute()
            || !path
                .to_str()
                .is_some_and(|v| v.len() <= 32768 && !v.contains('\0'))
        {
            return Err(Error::InvalidRequest);
        }
        // Validate controls before the creator can publish anything.
        let parent = path
            .parent()
            .ok_or(Error::UnsafeRuntime)?
            .canonicalize()
            .map_err(|_| Error::UnsafeRuntime)?;
        let candidate = parent.join(path.file_name().ok_or(Error::UnsafeRuntime)?);
        self.validate_paths(Some(&candidate), &s.projects)?;
        if create {
            precept_write::Profile::create(path).map_err(storage_error)?;
        }
        let projects = s.projects.clone();
        let publish_selection = (|| {
            let selected = Arc::new(Selected::select(path).map_err(storage_error)?);
            let roots = self.selected_roots(&selected, &projects)?;
            let mut all = projects.clone();
            all.extend(roots.clone());
            selected.reader().map_err(storage_error)?;
            s.selected = Some(selected);
            s.roots = roots;
            Ok(())
        })();
        if create {
            publish_selection.map_err(|_| Error::OutcomeUnknown)
        } else {
            publish_selection
        }
    }
    /// Records `name` at the canonical `root` in the selected Profile through
    /// the owner, re-checking the write grant and the root at admission.
    fn register_root(
        s: &State,
        grant: &Grant,
        access: &Access,
        name: &str,
        root: &Path,
    ) -> Result<Value> {
        let Access::OperatorWrite {
            profile_id,
            generation,
            owner_epoch,
        } = access
        else {
            return Err(Error::Unauthorized);
        };
        let request = precept_write::Request::from_json(
            &serde_json::to_vec(&json!({
                "version": 2,
                "binding": {"profile_id":profile_id,"generation":generation,"owner_epoch":owner_epoch},
                "operation_id": precept_ipc::operator::operation_id()?,
                "command": {"command":"register_project","name":name,"root":root,"expected":{"kind":"absent"}},
            }))
            .map_err(|_| Error::InvalidRequest)?,
        )
        .map_err(storage_error)?;
        let owner = s.owner.as_ref().ok_or(Error::Unauthorized)?;
        let receipt = owner
            .execute(&request, &|| {
                grant
                    .check()
                    .map_err(|_| precept_write::Error::Unauthorized)?;
                // The root must still be the same directory at admission.
                match root.canonicalize() {
                    Ok(current) if current == root && current.is_dir() => Ok(()),
                    _ => Err(precept_write::Error::UnsafeWorkspace),
                }
            })
            .map_err(storage_error)?;
        serde_json::to_value(receipt).map_err(|_| Error::Unavailable)
    }
    /// `project.register`: an operator write. A project already in the daemon's
    /// set but not in the selected Profile (a new Profile, or one made before
    /// the project was added) is recorded there at its pinned root. Nothing
    /// else changes: the set, sharing and credentials stay as they are.
    fn register(&self, authority: &Peer, params: Value) -> Result<Value> {
        let access = authority.access();
        if !matches!(access, Access::OperatorWrite { .. }) {
            return Err(Error::Unauthorized);
        }
        let p: NameParams = super::decode(params)?;
        let mut s = self.state.lock().unwrap();
        authority.check()?;
        let grant = s.grant.as_ref().ok_or(Error::Unauthorized)?.clone();
        if grant.access() != access || grant.check().is_err() {
            return Err(Error::Unauthorized);
        }
        let root = s.projects.get(&p.name).cloned().ok_or(Error::NotFound)?;
        if s.roots.contains_key(&p.name) {
            return Err(Error::Conflict);
        }
        match root.canonicalize() {
            Ok(current) if current == root && current.is_dir() => {}
            _ => return Err(Error::UnsafeRuntime),
        }
        let reader = s
            .selected
            .as_ref()
            .ok_or(Error::Unavailable)?
            .reader()
            .map_err(storage_error)?;
        // A Profile that names this project at another root keeps it unserved.
        match reader.read_project(&p.name) {
            Ok(_) => return Err(Error::Conflict),
            Err(precept_store::Error::UnknownProject) => {}
            Err(_) => return Err(Error::Unavailable),
        }
        let receipt = Self::register_root(&s, &grant, &access, &p.name, &root)?;
        s.roots.insert(p.name.clone(), root.clone());
        Ok(json!({"registered":p.name,"root":root,"receipt":receipt}))
    }
    /// `project.add`: an operator write. Validates the root with the scope
    /// rules and the session checks, registers it in the Profile unless the
    /// Profile already has exactly this name at this root, then persists the
    /// set, applies it and publishes the agent credential, without a restart.
    fn add(&self, authority: &Peer, params: Value, hooks: &dyn Hooks) -> Result<Value> {
        let access = authority.access();
        if !matches!(access, Access::OperatorWrite { .. }) {
            return Err(Error::Unauthorized);
        }
        let p: AddParams = super::decode(params)?;
        let mut s = self.state.lock().unwrap();
        authority.check()?;
        let grant = s.grant.as_ref().ok_or(Error::Unauthorized)?.clone();
        if grant.access() != access || grant.check().is_err() {
            return Err(Error::Unauthorized);
        }
        if !precept_ipc::valid_project(&p.name)
            || s.projects
                .keys()
                .chain(s.unavailable.keys())
                .any(|n| n.eq_ignore_ascii_case(&p.name))
        {
            return Err(Error::Conflict);
        }
        if s.projects.len() + s.unavailable.len() >= MAX_PROJECTS {
            return Err(Error::Capacity);
        }
        let root = self.scope_check(&s, &p.root)?;
        if root != p.root {
            // Only the canonical root `project.check` returned.
            return Err(Error::InvalidRequest);
        }
        let mut all: Vec<PathBuf> = s.projects.values().cloned().collect();
        all.extend(s.roots.values().cloned());
        all.push(root.clone());
        let reader = s
            .selected
            .as_ref()
            .ok_or(Error::Unavailable)?
            .reader()
            .map_err(storage_error)?;
        let registered = match reader.read_project(&p.name) {
            Ok(existing) => {
                if existing.root.canonicalize().ok().as_ref() != Some(&root) {
                    return Err(Error::Conflict);
                }
                true
            }
            Err(precept_store::Error::UnknownProject) => false,
            Err(_) => return Err(Error::Unavailable),
        };
        let receipt = if registered {
            Value::Null
        } else {
            Self::register_root(&s, &grant, &access, &p.name, &root)?
        };
        // Persist, then apply, then publish the credential. A failure here
        // leaves at most a registered but unlisted project: inert, and adding
        // it again only rejoins the set.
        let mut next = s.projects.clone();
        next.insert(p.name.clone(), root.clone());
        let mut file = next.clone();
        file.extend(s.unavailable.clone());
        // A project (re)joins shared with agents.
        let mut withheld = s.withheld.clone();
        withheld.remove(&p.name);
        // ...and without agent suggestions until the operator allows them.
        let mut proposers = s.proposers.clone();
        proposers.remove(&p.name);
        self.state_dir
            .save_projects(&file, &withheld, &proposers)
            .map_err(|_| Error::OutcomeUnknown)?;
        s.projects = next;
        s.withheld = withheld;
        s.proposers = proposers;
        s.roots.insert(p.name.clone(), root.clone());
        if hooks.added(&p.name).is_err() {
            s.projects.remove(&p.name);
            s.roots.remove(&p.name);
            let mut file = s.projects.clone();
            file.extend(s.unavailable.clone());
            let _ = self
                .state_dir
                .save_projects(&file, &s.withheld, &s.proposers);
            return Err(Error::OutcomeUnknown);
        }
        Ok(json!({"added":p.name,"root":root,"registered":!registered,"receipt":receipt}))
    }
    /// The operator turns external agent access to one project on or off: the
    /// daemon persists the choice, then publishes or withdraws that project's
    /// agent credential. No Profile write; a narrowing or restoring of agent
    /// access the operator controls like context sharing.
    fn set_agents(&self, params: Value, hooks: &dyn Hooks) -> Result<Value> {
        let p: AgentsParams = super::decode(params)?;
        let mut s = self.state.lock().unwrap();
        if !s.projects.contains_key(&p.name) && !s.unavailable.contains_key(&p.name) {
            return Err(Error::NotFound);
        }
        let mut withheld = s.withheld.clone();
        let changed = if p.enabled {
            withheld.remove(&p.name)
        } else {
            withheld.insert(p.name.clone())
        };
        if changed {
            let mut file = s.projects.clone();
            file.extend(s.unavailable.clone());
            self.state_dir
                .save_projects(&file, &withheld, &s.proposers)?;
            s.withheld = withheld;
            let served = s.projects.contains_key(&p.name);
            drop(s);
            if !p.enabled {
                hooks.withheld(&p.name);
            } else if served {
                hooks.added(&p.name).map_err(|_| Error::OutcomeUnknown)?;
            }
        }
        Ok(json!({"name":p.name,"agents":p.enabled}))
    }
    /// The operator lets a project's agents suggest rules, or stops them. No
    /// Profile write and no credential change: only which agent requests the
    /// daemon accepts.
    fn set_propose(&self, params: Value) -> Result<Value> {
        let p: AgentsParams = super::decode(params)?;
        let mut s = self.state.lock().unwrap();
        if !s.projects.contains_key(&p.name) && !s.unavailable.contains_key(&p.name) {
            return Err(Error::NotFound);
        }
        let mut proposers = s.proposers.clone();
        let changed = if p.enabled {
            proposers.insert(p.name.clone())
        } else {
            proposers.remove(&p.name)
        };
        if changed {
            let mut file = s.projects.clone();
            file.extend(s.unavailable.clone());
            self.state_dir
                .save_projects(&file, &s.withheld, &proposers)?;
            s.proposers = proposers;
        }
        Ok(json!({"name":p.name,"propose":p.enabled}))
    }
    /// An agent of `project` suggests a rule. Accepted only while the operator
    /// allows suggestions for the project and has writes on for this run;
    /// bounded per run and per scope, duplicates refused. The record lands as
    /// `suggested` with origin `agent`: confirming it stays the operator's.
    pub fn agent_suggest(
        &self,
        project: &str,
        scope: Scope,
        key: String,
        value: String,
        tags: Vec<String>,
        rule: precept_core::operator::RuleOptions,
    ) -> Result<Value> {
        let mut s = self.state.lock().unwrap();
        if !s.proposers.contains(project) || !s.roots.contains_key(project) {
            return Err(Error::Unauthorized);
        }
        // Agents reach their own project, its environment, or personal scope.
        match &scope {
            Scope::Global => {}
            Scope::Project { name } | Scope::Environment { name } if name == project => {}
            _ => return Err(Error::ScopeMismatch),
        }
        if value.trim().is_empty() || value.len() > MAX_AGENT_VALUE {
            return Err(Error::InvalidRequest);
        }
        let (owner, grant) = match (s.owner.as_ref(), s.grant.as_ref()) {
            (Some(o), Some(g)) if g.check().is_ok() => (o, g.clone()),
            // Writes are off for this run: nothing is queued anywhere.
            _ => return Err(Error::Unavailable),
        };
        if s.suggested.get(project).copied().unwrap_or(0) >= MAX_AGENT_SUGGESTIONS {
            return Err(Error::Capacity);
        }
        let reader = s
            .selected
            .as_ref()
            .ok_or(Error::Unavailable)?
            .reader()
            .map_err(storage_error)?;
        let (duplicate, pending) = reader
            .agent_suggestion_state(&scope, &key, &value)
            .map_err(|_| Error::Unavailable)?;
        if duplicate {
            return Err(Error::Conflict);
        }
        if pending >= MAX_PENDING_AGENT {
            return Err(Error::Capacity);
        }
        let b = owner.binding();
        let request = precept_write::Request {
            version: 2,
            binding: precept_write::Binding {
                profile_id: b.profile_id,
                generation: b.generation,
                owner_epoch: b.owner_epoch,
            },
            operation_id: precept_ipc::operator::operation_id()?,
            command: Command::SuggestPreference {
                scope,
                key,
                value,
                tags,
                origin: precept_core::operator::Origin::Agent,
                rule,
                expected: precept_write::Expected::Absent,
            },
        };
        let receipt = owner
            .execute(&request, &|| {
                grant
                    .check()
                    .map_err(|_| precept_write::Error::Unauthorized)
            })
            .map_err(storage_error)?;
        *s.suggested.entry(project.to_owned()).or_insert(0) += 1;
        let id = receipt
            .affected
            .first()
            .map(|a| a.record_id.clone())
            .unwrap_or_default();
        Ok(json!({"status":"suggested","id":id}))
    }
    pub fn handle(
        &self,
        authority: &Peer,
        method: &str,
        params: Value,
        hooks: &dyn Hooks,
    ) -> Result<Value> {
        authority.check()?;
        let access = authority.access();
        if method == "profile.mutate" {
            if !matches!(access, Access::OperatorWrite { .. }) {
                return Err(Error::Unauthorized);
            }
            let raw = serde_json::to_vec(&params).map_err(|_| Error::InvalidRequest)?;
            let request = precept_write::Request::from_json(&raw).map_err(storage_error)?;
            let Access::OperatorWrite {
                profile_id,
                generation,
                owner_epoch,
            } = access
            else {
                unreachable!()
            };
            if request.binding.profile_id != profile_id
                || request.binding.generation != generation
                || request.binding.owner_epoch != owner_epoch
            {
                return Err(Error::Unauthorized);
            }
            let mut s = self.state.lock().unwrap();
            authority.check()?;
            let owner = s.owner.as_ref().ok_or(Error::Unauthorized)?;
            let grant = s.grant.as_ref().ok_or(Error::Unauthorized)?;
            if grant.access() != authority.access() {
                return Err(Error::Unauthorized);
            }
            let name = match &request.command {
                Command::RegisterProject { name, root, .. } => {
                    // Only a project already in the set; `project.add` adds one.
                    if s.projects.get(name).map(|p| p.as_path()) != Some(Path::new(root)) {
                        return Err(Error::Unauthorized);
                    }
                    Some(name)
                }
                Command::PutHandoff { project, .. } => Some(project),
                // Every item's project must be served; the first is the one
                // whose root is rechecked at admission.
                Command::DecideBatch { items, .. } => {
                    let names: Vec<&String> = items
                        .iter()
                        .filter_map(|i| match &i.scope {
                            Scope::Global => None,
                            Scope::Project { name } | Scope::Environment { name } => Some(name),
                        })
                        .collect();
                    if names.iter().any(|n| !s.projects.contains_key(*n)) {
                        return Err(Error::Unauthorized);
                    }
                    names.first().copied()
                }
                Command::SuggestPreference { scope, .. }
                | Command::SetPreferenceEnabled { scope, .. }
                | Command::ConfirmPreference { scope, .. }
                | Command::RejectPreference { scope, .. }
                | Command::ReviseSuggestion { scope, .. }
                | Command::DeprecatePreference { scope, .. }
                | Command::DeletePreference { scope, .. } => match scope {
                    Scope::Global => None,
                    Scope::Project { name } | Scope::Environment { name } => Some(name),
                },
            };
            if name.is_some_and(|name| !s.projects.contains_key(name)) {
                return Err(Error::Unauthorized);
            }
            #[cfg(test)]
            let observations = std::sync::atomic::AtomicUsize::new(0);
            let receipt = owner
                .execute(&request, &|| {
                    grant
                        .check()
                        .map_err(|_| precept_write::Error::Unauthorized)?;
                    #[cfg(test)]
                    if observations.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                        let hook = self.before_begin.lock().unwrap().clone();
                        if let Some(hook) = hook {
                            hook();
                        }
                    }
                    // Recheck the existing pinned root at transaction admission,
                    // not only when the operator inspected the proposal.
                    if let Some(name) = name {
                        let unsafe_root = || precept_write::Error::UnsafeWorkspace;
                        let pinned = s
                            .roots
                            .get(name)
                            .or_else(|| s.projects.get(name))
                            .ok_or_else(unsafe_root)?;
                        let current = pinned.canonicalize().map_err(|_| unsafe_root())?;
                        if current != *pinned || !current.is_dir() {
                            return Err(unsafe_root());
                        }
                        let reader = s
                            .selected
                            .as_ref()
                            .ok_or_else(unsafe_root)?
                            .reader()
                            .map_err(|_| unsafe_root())?;
                        match reader.read_project(name) {
                            Ok(project) => {
                                let root =
                                    project.root.canonicalize().map_err(|_| unsafe_root())?;
                                if root != *pinned {
                                    return Err(unsafe_root());
                                }
                            }
                            // Registration and unregistered-scope errors remain
                            // the writer's typed command validation.
                            Err(precept_store::Error::UnknownProject) => (),
                            Err(_) => return Err(unsafe_root()),
                        }
                    }
                    Ok(())
                })
                .map_err(storage_error)?;
            if let Command::RegisterProject { name, root, .. } = &request.command {
                s.roots.insert(name.clone(), PathBuf::from(root));
            }
            return serde_json::to_value(receipt).map_err(|_| Error::Unavailable);
        }
        if method == "project.add" {
            return self.add(authority, params, hooks);
        }
        if method == "project.register" {
            return self.register(authority, params);
        }
        if method == "project.remove" {
            // An operator write: the current Profile's write grant, exactly.
            if !matches!(access, Access::OperatorWrite { .. }) {
                return Err(Error::Unauthorized);
            }
            let p: NameParams = super::decode(params)?;
            let mut s = self.state.lock().unwrap();
            authority.check()?;
            let grant = s.grant.as_ref().ok_or(Error::Unauthorized)?;
            if grant.access() != access || grant.check().is_err() {
                return Err(Error::Unauthorized);
            }
            let mut next = s.projects.clone();
            let mut gone = s.unavailable.clone();
            if next.remove(&p.name).is_none() && gone.remove(&p.name).is_none() {
                return Err(Error::NotFound);
            }
            let mut file = next.clone();
            file.extend(gone.clone());
            let mut withheld = s.withheld.clone();
            withheld.remove(&p.name);
            let mut proposers = s.proposers.clone();
            proposers.remove(&p.name);
            self.state_dir.save_projects(&file, &withheld, &proposers)?;
            s.projects = next;
            s.unavailable = gone;
            s.withheld = withheld;
            s.proposers = proposers;
            s.roots.remove(&p.name);
            drop(s);
            hooks.removed(&p.name);
            return Ok(json!({"removed":p.name}));
        }
        if access != Access::Operator {
            return Err(Error::Unauthorized);
        }
        if method == "project.propose" {
            return self.set_propose(params);
        }
        if method == "project.agents" {
            return self.set_agents(params, hooks);
        }
        if matches!(
            method,
            "profile.preferences.list"
                | "profile.preferences.get"
                | "profile.review.queue"
                | "profile.review.count"
                | "profile.review.inspect"
                | "profile.handoff.get"
        ) {
            let (store, roots) = self.read()?;
            let check_scope = |scope: &Scope| -> Result<()> {
                if let Scope::Project { name } = scope {
                    if !self.allowed(name) {
                        return Err(Error::Unauthorized);
                    }
                    let root = store
                        .read_project(name)
                        .map_err(|_| Error::NotFound)?
                        .root
                        .canonicalize()
                        .map_err(|_| Error::UnsafeRuntime)?;
                    if roots.get(name) != Some(&root) {
                        return Err(Error::UnsafeRuntime);
                    }
                }
                Ok(())
            };
            let value = match method {
                "profile.preferences.list" => {
                    let p: PreferenceList = super::decode(params)?;
                    if p.after_id < 0 {
                        return Err(Error::InvalidRequest);
                    }
                    check_scope(&p.scope)?;
                    serde_json::to_value(
                        store
                            .operator_preferences(&p.scope, p.after_id)
                            .map_err(|_| Error::Unavailable)?,
                    )
                }
                "profile.preferences.get" => {
                    let p: PreferenceGet = super::decode(params)?;
                    if p.id <= 0 {
                        return Err(Error::InvalidRequest);
                    }
                    check_scope(&p.scope)?;
                    serde_json::to_value(store.operator_preference(&p.scope, p.id).map_err(
                        |e| match e {
                            precept_store::Error::PreferenceNotFound => Error::NotFound,
                            _ => Error::Unavailable,
                        },
                    )?)
                }
                "profile.review.queue" => {
                    let p: ReviewQueueParams = super::decode(params)?;
                    if p.after_id < 0 {
                        return Err(Error::InvalidRequest);
                    }
                    check_scope(&p.scope)?;
                    serde_json::to_value(
                        store
                            .operator_review_queue(&p.scope, p.after_id)
                            .map_err(|_| Error::Unavailable)?,
                    )
                }
                "profile.review.count" => {
                    let p: ReviewCountParams = super::decode(params)?;
                    check_scope(&p.scope)?;
                    serde_json::to_value(
                        store
                            .operator_review_count(&p.scope)
                            .map_err(|_| Error::Unavailable)?,
                    )
                }
                "profile.review.inspect" => {
                    let p: ReviewInspectParams = super::decode(params)?;
                    if p.id <= 0 || p.revision <= 0 {
                        return Err(Error::InvalidRequest);
                    }
                    check_scope(&p.scope)?;
                    let inspect =
                        store
                            .operator_review_inspect(&p.scope, p.id)
                            .map_err(|e| match e {
                                precept_store::Error::PreferenceNotFound => Error::NotFound,
                                _ => Error::Unavailable,
                            })?;
                    // The caller names the exact revision it inspected; a moved
                    // record is a conflict, never a silently different preview.
                    if inspect.record.revision != p.revision {
                        return Err(Error::Conflict);
                    }
                    serde_json::to_value(inspect)
                }
                _ => {
                    let p: HandoffGet = super::decode(params)?;
                    if !precept_ipc::valid_project(&p.project)
                        || !precept_store::profile::text(&p.task_id, 120, true)
                    {
                        return Err(Error::InvalidRequest);
                    }
                    check_scope(&Scope::Project {
                        name: p.project.clone(),
                    })?;
                    serde_json::to_value(
                        store
                            .operator_handoff(&p.project, &p.task_id)
                            .map_err(|_| Error::Unavailable)?,
                    )
                }
            };
            return value.map_err(|_| Error::Unavailable);
        }
        match method {
            "profile.write.disable" => {
                super::decode::<super::Empty>(params)?;
                self.disable()
            }
            "profile.info" => {
                super::decode::<super::Empty>(params)?;
                self.info(&self.state.lock().unwrap())
            }
            "profile.create" | "profile.select" => {
                let p: PathParams = super::decode(params)?;
                let mut s = self.state.lock().unwrap();
                self.select_into(&mut s, &p.path, method == "profile.create")?;
                s.startup_failure = None;
                // Remembering is part of the same explicit choice. If it cannot
                // be saved, the selection stands and `remembered` says so.
                if p.remember {
                    let path = s.selected.as_ref().map(|x| x.path().to_path_buf());
                    if let Some(path) = path {
                        let _ = self.state_dir.save_startup(&path);
                    }
                }
                self.info(&s)
            }
            "profile.startup.forget" => {
                super::decode::<super::Empty>(params)?;
                self.state_dir.forget_startup()?;
                let mut s = self.state.lock().unwrap();
                s.startup_failure = None;
                self.info(&s)
            }
            "project.check" => {
                let p: RootParams = super::decode(params)?;
                let s = self.state.lock().unwrap();
                Ok(match self.scope_check_reason(&s, &p.root) {
                    Ok(root) => {
                        // A Profile record at this root (a project removed
                        // earlier) keeps its name: adding it rejoins the set.
                        let known = s
                            .selected
                            .as_ref()
                            .and_then(|x| x.reader().ok())
                            .and_then(|r| r.project_at_root(&root).ok().flatten());
                        let name = known.clone().or_else(|| scope::suggested_name(&root));
                        json!({"ok":true,"root":root,"name":name,"known":known.is_some()})
                    }
                    Err(code) => json!({"ok":false,"reason":code}),
                })
            }
            "profile.write.enable" => {
                let p: Enable = super::decode(params)?;
                if !p.confirm {
                    return Err(Error::InvalidRequest);
                }
                let mut s = self.state.lock().unwrap();
                if s.owner.is_some() {
                    return Err(Error::Busy);
                }
                let selected = s.selected.as_ref().ok_or(Error::Unavailable)?;
                let binding = selected.binding();
                if binding.profile_id != p.profile_id || binding.generation != p.generation {
                    return Err(Error::Unauthorized);
                }
                let owner = selected.acquire().map_err(storage_error)?;
                let b = owner.binding();
                let grant = self.grants.issue(
                    &self.location,
                    Access::OperatorWrite {
                        profile_id: b.profile_id,
                        generation: b.generation,
                        owner_epoch: b.owner_epoch,
                    },
                )?;
                s.owner = Some(owner);
                s.grant = Some(grant);
                self.info(&s)
            }
            _ => Err(Error::Unsupported),
        }
    }
}
