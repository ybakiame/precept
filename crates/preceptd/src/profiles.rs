//! Single-assignment selection and serialized daemon-owned write admission.

mod agent;
mod handler;
mod projects;
mod selection;
mod sync;

use crate::state::StateDir;
use precept_core::repo_fs::{Repository, RootIdentity};
use precept_ipc::{
    Error, Location, Result,
    write_grant::{Grant, Registry},
};
use precept_write::{
    Scope,
    owner::{Owner, Selected},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

type Roots = BTreeMap<String, PathBuf>;
type Identities = BTreeMap<String, RootIdentity>;
struct State {
    selected: Option<Arc<Selected>>,
    /// Roots registered in the selected Profile, for projects in the set.
    roots: Roots,
    owner: Option<Owner>,
    grant: Option<Arc<Grant>>,
    /// The daemon-owned project set (`projects.json`): name → canonical root.
    projects: Roots,
    /// Persistent identity for every listed entry, including unavailable ones.
    identities: Identities,
    /// Entries of the file whose root no longer resolves to the same directory:
    /// kept (and removable), never served.
    unavailable: Roots,
    /// Why the remembered startup Profile was not selected, if it was not.
    startup_failure: Option<&'static str>,
    /// Why an explicitly remembered write grant could not be restored.
    write_restore_failure: Option<&'static str>,
    /// The operator's per-project switches (agents, suggestions, gate).
    switches: crate::state::Switches,
}
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
    #[cfg(test)]
    pub(crate) before_add_admission: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    state: Mutex<State>,
    pub location: Location,
    runtime: PathBuf,
    home: Option<PathBuf>,
    state_dir: StateDir,
    sync_lock: Mutex<()>,
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
        let home = crate::scope::home();
        let mut projects = Roots::new();
        let mut unavailable = Roots::new();
        let (listed, identities, switches) = state_dir.load_projects()?;
        for (name, root) in listed {
            match identities.get(&name) {
                Some(identity) => match Repository::open_expected(&root, identity) {
                    Ok(repository) if repository.path() == root => {
                        projects.insert(name, root);
                    }
                    _ => {
                        unavailable.insert(name, root);
                    }
                },
                None => {
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
            #[cfg(test)]
            before_add_admission: Mutex::new(None),
            state: Mutex::new(State {
                selected: None,
                roots: Roots::new(),
                owner: None,
                grant: None,
                projects,
                identities,
                unavailable,
                startup_failure: None,
                write_restore_failure: None,
                switches,
            }),
            location,
            runtime,
            home,
            state_dir,
            sync_lock: Mutex::new(()),
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
        let startup = match self.state_dir.load_startup() {
            Ok(Some(startup)) => startup,
            Ok(None) => return,
            Err(_) => {
                self.state.lock().unwrap().startup_failure = Some("startup_unreadable");
                return;
            }
        };
        let mut s = self.state.lock().unwrap();
        if std::fs::symlink_metadata(&startup.profile).is_err() {
            s.startup_failure = Some("startup_missing");
            return;
        }
        let failure = match self.select_into(&mut s, &startup.profile, false) {
            Ok(()) => None,
            Err(Error::UnsafeRuntime) => Some("startup_unsafe"),
            Err(_) => Some("startup_refused"),
        };
        s.startup_failure = failure;
        if failure.is_some() || !startup.write_enabled {
            return;
        }
        let binding = s.selected.as_ref().map(|selected| selected.binding());
        let identity_matches = binding.as_ref().is_some_and(|binding| {
            startup.profile_id.as_deref() == Some(binding.profile_id.as_str())
                && startup.generation.as_deref() == Some(binding.generation.as_str())
        });
        if !identity_matches {
            s.write_restore_failure = Some("identity_mismatch");
        } else if self.enable_selected(&mut s).is_err() {
            s.write_restore_failure = Some("write_restore_failed");
        }
    }
    pub fn allowed(&self, name: &str) -> bool {
        self.state.lock().unwrap().projects.contains_key(name)
    }
    pub fn root_identity(&self, name: &str) -> Result<RootIdentity> {
        let s = self.state.lock().unwrap();
        if !s.projects.contains_key(name) {
            return Err(Error::NotFound);
        }
        s.identities.get(name).cloned().ok_or(Error::UnsafeRuntime)
    }
    pub fn verify_root(&self, name: &str) -> Result<()> {
        let (root, identity) = {
            let s = self.state.lock().unwrap();
            if !s.projects.contains_key(name) {
                return Err(Error::NotFound);
            }
            (
                s.projects.get(name).ok_or(Error::NotFound)?.clone(),
                s.identities.get(name).ok_or(Error::UnsafeRuntime)?.clone(),
            )
        };
        let repository =
            Repository::open_expected(&root, &identity).map_err(|_| Error::UnsafeRuntime)?;
        if repository.path() != root {
            return Err(Error::UnsafeRuntime);
        }
        Ok(())
    }
    pub fn repository(&self, name: &str) -> Result<Repository> {
        let (root, identity) = {
            let s = self.state.lock().unwrap();
            if !s.projects.contains_key(name) {
                return Err(Error::NotFound);
            }
            (
                s.roots.get(name).ok_or(Error::NotFound)?.clone(),
                s.identities.get(name).ok_or(Error::UnsafeRuntime)?.clone(),
            )
        };
        Repository::open_expected(&root, &identity).map_err(|_| Error::UnsafeRuntime)
    }
    pub fn snapshot(&self, name: &str) -> Result<precept_core::contracts::Snapshot> {
        Ok(precept_core::contracts::Snapshot::from_repository(
            self.repository(name)?,
        ))
    }
    /// Projects whose agent credential is published (shared with agents).
    pub fn shared_names(&self) -> Vec<String> {
        let s = self.state.lock().unwrap();
        s.projects
            .keys()
            .filter(|n| !s.switches.withheld.contains(*n))
            .cloned()
            .collect()
    }
    /// The daemon's private state directory.
    pub fn state_dir(&self) -> &StateDir {
        &self.state_dir
    }
    /// Whether the operator lets this served project's agents write their
    /// own records (suggestions, work sessions).
    pub fn can_propose(&self, project: &str) -> bool {
        let s = self.state.lock().unwrap();
        s.switches.proposers.contains(project) && s.roots.contains_key(project)
    }
    /// Whether `project` is in the daemon's set (served or not).
    pub fn listed(&self, project: &str) -> bool {
        let s = self.state.lock().unwrap();
        s.projects.contains_key(project) || s.unavailable.contains_key(project)
    }
    pub fn location(&self) -> &Location {
        &self.location
    }
    /// The project set for `project.list`: name, root and state.
    pub fn list(&self) -> Value {
        let (projects, unavailable, roots, identities, switches) = {
            let s = self.state.lock().unwrap();
            (
                s.projects.clone(),
                s.unavailable.clone(),
                s.roots.clone(),
                s.identities.clone(),
                s.switches.clone(),
            )
        };
        let current = |name: &str, root: &PathBuf| {
            identities
                .get(name)
                .and_then(|identity| Repository::open_expected(root, identity).ok())
                .is_some_and(|repository| repository.path() == root)
        };
        let mut out: Vec<Value> = projects
            .iter()
            .map(|(name, root)| {
                let agents = !switches.withheld.contains(name);
                let propose = switches.proposers.contains(name);
                let gate = switches.gated.contains(name);
                let state = if !current(name, root) {
                    "unavailable"
                } else if roots.contains_key(name) {
                    "ready"
                } else {
                    "unregistered"
                };
                json!({"name":name,"root":root,"state":state,"agents":agents,"propose":propose,"gate":gate})
            })
            .collect();
        out.extend(unavailable.iter().map(|(name, root)| {
            json!({"name":name,"root":root,"state":"unavailable","agents":!switches.withheld.contains(name),"propose":switches.proposers.contains(name),"gate":switches.gated.contains(name)})
        }));
        Value::Array(out)
    }
}
