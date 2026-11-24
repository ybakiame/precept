use super::*;
use crate::scope;
use precept_ipc::Access;
use precept_store::SqliteStore;
use std::path::Path;

impl Profiles {
    pub(super) fn validate_paths(&self, profile: Option<&Path>, roots: &Roots) -> Result<()> {
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
    pub(super) fn selected_roots(&self, selected: &Selected, projects: &Roots) -> Result<Roots> {
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
    pub(super) fn info(&self, s: &State) -> Result<Value> {
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
    pub(super) fn scope_check_reason(
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
    pub(super) fn scope_check(&self, s: &State, path: &Path) -> Result<PathBuf> {
        self.scope_check_reason(s, path)
            .map_err(|_| Error::UnsafeRuntime)
    }
    /// Creates (optionally) and selects `path` with every check of an explicit
    /// selection. Used by `profile.create` / `profile.select` and at start.
    pub(super) fn select_into(&self, s: &mut State, path: &Path, create: bool) -> Result<()> {
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
}
