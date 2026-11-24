use super::*;
use crate::scope;
use precept_ipc::{Access, peer::Peer};
use precept_write::Command;
use std::path::Path;

impl Profiles {
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
                Command::DecideBatch { .. } | Command::SuggestBatch { .. } => {
                    let scopes: Vec<&Scope> = match &request.command {
                        Command::DecideBatch { items, .. } => {
                            items.iter().map(|i| &i.scope).collect()
                        }
                        Command::SuggestBatch { items } => items.iter().map(|i| &i.scope).collect(),
                        _ => Vec::new(),
                    };
                    let names: Vec<&String> = scopes
                        .into_iter()
                        .filter_map(|scope| match scope {
                            Scope::Global => None,
                            Scope::Project { name } => Some(name),
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
                | Command::RevisePreference { scope, .. }
                | Command::DeprecatePreference { scope, .. }
                | Command::DeletePreference { scope, .. } => match scope {
                    Scope::Global => None,
                    Scope::Project { name } => Some(name),
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
            let p: NameParams = crate::decode(params)?;
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
            let mut switches = s.switches.clone();
            switches.reset(&p.name);
            self.state_dir.save_projects(&file, &switches)?;
            s.projects = next;
            s.unavailable = gone;
            s.switches = switches;
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
        if method == "project.gate" {
            return self.set_gate(params);
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
                | "profile.handoff.list"
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
                    let p: PreferenceList = crate::decode(params)?;
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
                    let p: PreferenceGet = crate::decode(params)?;
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
                    let p: ReviewQueueParams = crate::decode(params)?;
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
                    let p: ReviewCountParams = crate::decode(params)?;
                    check_scope(&p.scope)?;
                    serde_json::to_value(
                        store
                            .operator_review_count(&p.scope)
                            .map_err(|_| Error::Unavailable)?,
                    )
                }
                "profile.review.inspect" => {
                    let p: ReviewInspectParams = crate::decode(params)?;
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
                "profile.handoff.list" => {
                    #[derive(Deserialize)]
                    #[serde(deny_unknown_fields)]
                    struct P {
                        project: String,
                    }
                    let p: P = crate::decode(params)?;
                    if !precept_ipc::valid_project(&p.project) {
                        return Err(Error::InvalidRequest);
                    }
                    check_scope(&Scope::Project {
                        name: p.project.clone(),
                    })?;
                    serde_json::to_value(
                        store
                            .operator_handoffs(&p.project)
                            .map_err(|_| Error::Unavailable)?,
                    )
                }
                _ => {
                    let p: HandoffGet = crate::decode(params)?;
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
                crate::decode::<crate::Empty>(params)?;
                self.disable()
            }
            "profile.info" => {
                crate::decode::<crate::Empty>(params)?;
                self.info(&self.state.lock().unwrap())
            }
            "profile.create" | "profile.select" => {
                let p: PathParams = crate::decode(params)?;
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
                crate::decode::<crate::Empty>(params)?;
                self.state_dir.forget_startup()?;
                let mut s = self.state.lock().unwrap();
                s.startup_failure = None;
                self.info(&s)
            }
            "project.check" => {
                let p: RootParams = crate::decode(params)?;
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
                let p: Enable = crate::decode(params)?;
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
