//! Single-assignment selection and serialized daemon-owned write admission.
use acp_ipc::{
    Access, Error, Location, Result,
    session_lease::Peer,
    write_grant::{Grant, Registry},
};
use acp_store::SqliteStore;
use acp_write::{
    Command, Scope,
    owner::{Owner, Selected},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
type Roots = BTreeMap<String, PathBuf>;
struct State {
    selected: Option<Arc<Selected>>,
    roots: Roots,
    owner: Option<Owner>,
    grant: Option<Arc<Grant>>,
}
pub struct Profiles {
    #[cfg(test)]
    pub(crate) before_begin: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    state: Mutex<State>,
    pub location: Location,
    runtime: PathBuf,
    projects: Vec<String>,
    expected: Roots,
    pub grants: Registry,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathParams {
    path: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Enable {
    profile_id: String,
    generation: String,
    confirm: bool,
}
pub fn storage_error(e: acp_write::Error) -> Error {
    match e {
        acp_write::Error::Unauthorized => Error::Unauthorized,
        acp_write::Error::Busy => Error::Busy,
        acp_write::Error::Conflict => Error::Conflict,
        acp_write::Error::InvalidTransition => Error::InvalidTransition,
        acp_write::Error::IdempotencyConflict => Error::IdempotencyConflict,
        acp_write::Error::OwnerMismatch => Error::OwnerMismatch,
        acp_write::Error::StaleGeneration => Error::StaleGeneration,
        acp_write::Error::ScopeMismatch => Error::ScopeMismatch,
        acp_write::Error::Capacity => Error::Capacity,
        acp_write::Error::StorageFull => Error::StorageFull,
        acp_write::Error::OutcomeUnknown => Error::OutcomeUnknown,
        acp_write::Error::InvalidCommand => Error::InvalidRequest,
        acp_write::Error::Collision => Error::EndpointCollision,
        acp_write::Error::UnsafeWorkspace => Error::UnsafeRuntime,
        _ => Error::Unavailable,
    }
}
impl Profiles {
    pub fn new(
        location: Location,
        projects: Vec<String>,
        roots: &[String],
        database: Option<&Path>,
        grants: Registry,
    ) -> Result<Self> {
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
        let mut expected = Roots::new();
        for root in roots {
            let (name, path) = root.split_once('=').ok_or(Error::InvalidRequest)?;
            if !projects.iter().any(|p| p == name) || !Path::new(path).is_absolute() {
                return Err(Error::InvalidRequest);
            }
            let path = Path::new(path)
                .canonicalize()
                .map_err(|_| Error::UnsafeRuntime)?;
            if !path.is_dir() || expected.insert(name.to_owned(), path).is_some() {
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
            }),
            location,
            runtime,
            projects,
            expected,
            grants,
        };
        this.validate_paths(None, &this.expected)?;
        if let Some(path) = database {
            let selected = Arc::new(Selected::select(path).map_err(storage_error)?);
            let roots = this.selected_roots(&selected)?;
            let mut state = this.state.lock().unwrap();
            state.selected = Some(selected);
            state.roots = roots;
        }
        Ok(this)
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
    fn selected_roots(&self, selected: &Selected) -> Result<Roots> {
        let store = selected.reader().map_err(storage_error)?;
        let mut roots = Roots::new();
        for name in &self.projects {
            match store.read_project(name) {
                Ok(p) => {
                    let root = p.root.canonicalize().map_err(|_| Error::UnsafeRuntime)?;
                    if !root.is_dir() || self.expected.get(name).is_some_and(|e| e != &root) {
                        return Err(Error::UnsafeRuntime);
                    }
                    roots.insert(name.clone(), root);
                }
                Err(acp_store::Error::UnknownProject) if self.expected.contains_key(name) => (),
                Err(_) => return Err(Error::NotFound),
            }
        }
        let mut all = self.expected.clone();
        all.extend(roots.clone());
        self.validate_paths(Some(selected.path()), &all)?;
        Ok(roots)
    }
    pub fn configured_roots(&self) -> Vec<PathBuf> {
        let mut roots = self.expected.clone();
        roots.extend(self.state.lock().unwrap().roots.clone());
        roots.into_values().collect()
    }
    pub fn read(&self) -> Result<(SqliteStore, Roots)> {
        let s = self.state.lock().unwrap();
        let selected = s.selected.as_ref().ok_or(Error::Unavailable)?;
        Ok((selected.reader().map_err(storage_error)?, s.roots.clone()))
    }
    pub fn selected(&self) -> bool {
        self.state.lock().unwrap().selected.is_some()
    }
    fn info(s: &State) -> Result<Value> {
        let Some(selected) = &s.selected else {
            return Ok(json!({"selected":false,"write_enabled":false}));
        };
        let binding = selected
            .reader()
            .map_err(storage_error)?
            .metadata()
            .map_err(|_| Error::Unavailable)?
            .binding;
        Ok(
            json!({"selected":true,"binding":binding,"write_enabled":s.grant.as_ref().is_some_and(|g|g.check().is_ok())}),
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
    pub fn handle(
        &self,
        authority: &Peer,
        method: &str,
        params: Value,
        validate_roots: &dyn Fn(&[PathBuf]) -> Result<()>,
    ) -> Result<Value> {
        authority.check()?;
        let access = authority.access();
        if method == "profile.mutate" {
            if !matches!(access, Access::OperatorWrite { .. }) {
                return Err(Error::Unauthorized);
            }
            let raw = serde_json::to_vec(&params).map_err(|_| Error::InvalidRequest)?;
            let request = acp_write::Request::from_json(&raw).map_err(storage_error)?;
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
                    if self.expected.get(name).map(|p| p.as_path()) != Some(Path::new(root)) {
                        return Err(Error::Unauthorized);
                    }
                    Some(name)
                }
                Command::PutHandoff { project, .. } => Some(project),
                Command::SuggestPreference { scope, .. }
                | Command::ConfirmPreference { scope, .. }
                | Command::RejectPreference { scope, .. }
                | Command::DeprecatePreference { scope, .. }
                | Command::DeletePreference { scope, .. } => match scope {
                    Scope::Global => None,
                    Scope::Project { name } => Some(name),
                },
            };
            if name.is_some_and(|name| !self.projects.contains(name)) {
                return Err(Error::Unauthorized);
            }
            #[cfg(test)]
            let observations = std::sync::atomic::AtomicUsize::new(0);
            let receipt = owner
                .execute(&request, &|| {
                    grant.check().map_err(|_| acp_write::Error::Unauthorized)?;
                    #[cfg(test)]
                    if observations.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                        let hook = self.before_begin.lock().unwrap().clone();
                        if let Some(hook) = hook {
                            hook();
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
        if access != Access::Operator {
            return Err(Error::Unauthorized);
        }
        match method {
            "profile.write.disable" => {
                super::decode::<super::Empty>(params)?;
                self.disable()
            }
            "profile.info" => {
                super::decode::<super::Empty>(params)?;
                Self::info(&self.state.lock().unwrap())
            }
            "profile.create" | "profile.select" => {
                let p: PathParams = super::decode(params)?;
                let mut s = self.state.lock().unwrap();
                if s.selected.is_some() {
                    return Err(Error::RestartRequired);
                }
                if !p.path.is_absolute() || p.path.to_str().is_none() {
                    return Err(Error::InvalidRequest);
                }
                // Validate controls before the creator can publish anything.
                let parent = p
                    .path
                    .parent()
                    .ok_or(Error::UnsafeRuntime)?
                    .canonicalize()
                    .map_err(|_| Error::UnsafeRuntime)?;
                let candidate = parent.join(p.path.file_name().ok_or(Error::UnsafeRuntime)?);
                self.validate_paths(Some(&candidate), &self.expected)?;
                validate_roots(&self.expected.values().cloned().collect::<Vec<_>>())?;
                let created = method == "profile.create";
                if created {
                    acp_write::Profile::create(&p.path).map_err(storage_error)?;
                }
                let publish_selection = (|| {
                    let selected = Arc::new(Selected::select(&p.path).map_err(storage_error)?);
                    let roots = self.selected_roots(&selected)?;
                    let mut all = self.expected.clone();
                    all.extend(roots.clone());
                    validate_roots(&all.into_values().collect::<Vec<_>>())?;
                    selected.reader().map_err(storage_error)?;
                    s.selected = Some(selected);
                    s.roots = roots;
                    Self::info(&s)
                })();
                if created {
                    publish_selection.map_err(|_| Error::OutcomeUnknown)
                } else {
                    publish_selection
                }
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
                Self::info(&s)
            }
            _ => Err(Error::Unsupported),
        }
    }
}
