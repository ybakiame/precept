use super::*;
use crate::state::MAX_PROJECTS;
use precept_ipc::{Access, peer::Peer};
use std::{collections::BTreeSet, path::Path};

impl Profiles {
    /// Records `name` at the canonical `root` in the selected Profile through
    /// the owner, re-checking the write grant and the root at admission.
    pub(super) fn register_root(
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
    pub(super) fn register(&self, authority: &Peer, params: Value) -> Result<Value> {
        let access = authority.access();
        if !matches!(access, Access::OperatorWrite { .. }) {
            return Err(Error::Unauthorized);
        }
        let p: NameParams = crate::decode(params)?;
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
    pub(super) fn add(&self, authority: &Peer, params: Value, hooks: &dyn Hooks) -> Result<Value> {
        let access = authority.access();
        if !matches!(access, Access::OperatorWrite { .. }) {
            return Err(Error::Unauthorized);
        }
        let p: AddParams = crate::decode(params)?;
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
        // A project (re)joins shared with agents, without agent suggestions
        // or the gate until the operator turns them on.
        let mut switches = s.switches.clone();
        switches.reset(&p.name);
        self.state_dir
            .save_projects(&file, &switches)
            .map_err(|_| Error::OutcomeUnknown)?;
        s.projects = next;
        s.switches = switches;
        s.roots.insert(p.name.clone(), root.clone());
        if hooks.added(&p.name).is_err() {
            s.projects.remove(&p.name);
            s.roots.remove(&p.name);
            let mut file = s.projects.clone();
            file.extend(s.unavailable.clone());
            let _ = self.state_dir.save_projects(&file, &s.switches);
            return Err(Error::OutcomeUnknown);
        }
        Ok(json!({"added":p.name,"root":root,"registered":!registered,"receipt":receipt}))
    }
    /// The operator turns external agent access to one project on or off: the
    /// daemon persists the choice, then publishes or withdraws that project's
    /// agent credential. No Profile write; a narrowing or restoring of agent
    /// access the operator controls like context sharing.
    pub(super) fn set_agents(&self, params: Value, hooks: &dyn Hooks) -> Result<Value> {
        let p: AgentsParams = crate::decode(params)?;
        let mut s = self.state.lock().unwrap();
        if !s.projects.contains_key(&p.name) && !s.unavailable.contains_key(&p.name) {
            return Err(Error::NotFound);
        }
        let mut switches = s.switches.clone();
        let changed = if p.enabled {
            switches.withheld.remove(&p.name)
        } else {
            switches.withheld.insert(p.name.clone())
        };
        if changed {
            let mut file = s.projects.clone();
            file.extend(s.unavailable.clone());
            self.state_dir.save_projects(&file, &switches)?;
            s.switches = switches;
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
    pub(super) fn set_propose(&self, params: Value) -> Result<Value> {
        let p: AgentsParams = crate::decode(params)?;
        self.set_switch(&p, |s| &mut s.proposers)?;
        Ok(json!({"name":p.name,"propose":p.enabled}))
    }
    /// The operator lets the pre-tool gate answer for a project's hard rules,
    /// or stops it (then every call is allowed). No Profile write.
    pub(super) fn set_gate(&self, params: Value) -> Result<Value> {
        let p: AgentsParams = crate::decode(params)?;
        self.set_switch(&p, |s| &mut s.gated)?;
        Ok(json!({"name":p.name,"gate":p.enabled}))
    }
    pub(super) fn set_switch(
        &self,
        p: &AgentsParams,
        set: impl Fn(&mut crate::state::Switches) -> &mut BTreeSet<String>,
    ) -> Result<()> {
        let mut s = self.state.lock().unwrap();
        if !s.projects.contains_key(&p.name) && !s.unavailable.contains_key(&p.name) {
            return Err(Error::NotFound);
        }
        let mut switches = s.switches.clone();
        let changed = if p.enabled {
            set(&mut switches).insert(p.name.clone())
        } else {
            set(&mut switches).remove(&p.name)
        };
        if changed {
            let mut file = s.projects.clone();
            file.extend(s.unavailable.clone());
            self.state_dir.save_projects(&file, &switches)?;
            s.switches = switches;
        }
        Ok(())
    }
    /// Whether the operator turned the pre-tool gate on for `project`.
    pub fn gated(&self, project: &str) -> bool {
        let s = self.state.lock().unwrap();
        s.switches.gated.contains(project) && s.roots.contains_key(project)
    }
}
