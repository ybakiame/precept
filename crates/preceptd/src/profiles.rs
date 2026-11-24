//! Single-assignment selection and serialized daemon-owned write admission.

mod agent;
mod handler;
mod projects;
mod selection;

use crate::state::StateDir;
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
        let (listed, switches) = state_dir.load_projects()?;
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
                switches,
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
        let s = self.state.lock().unwrap();
        let mut out: Vec<Value> = s
            .projects
            .iter()
            .map(|(name, root)| {
                // Without a selected Profile nothing is registered; a missing
                // root still reads as unavailable.
                let agents = !s.switches.withheld.contains(name);
                let propose = s.switches.proposers.contains(name);
                let gate = s.switches.gated.contains(name);
                let state = if s.roots.contains_key(name) {
                    "ready"
                } else if root.is_dir() {
                    "unregistered"
                } else {
                    "unavailable"
                };
                json!({"name":name,"root":root,"state":state,"agents":agents,"propose":propose,"gate":gate})
            })
            .collect();
        out.extend(
            s.unavailable
                .iter()
                .map(|(name, root)| {
                    json!({"name":name,"root":root,"state":"unavailable","agents":!s.switches.withheld.contains(name),"propose":s.switches.proposers.contains(name),"gate":s.switches.gated.contains(name)})
                }),
        );
        Value::Array(out)
    }
}
