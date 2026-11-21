use clap::Parser;
use precept_core::{ContextResolver, WorktreeInspector, documents::FileDocuments, operator::Scope};
use precept_ipc::{
    Access, Error, Location, MAX_PEERS, MAX_REQUESTS, Request, Response, RuntimeLease, Stream,
    read_frame, write_frame,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
mod activity;
mod evidence;
mod importer;
mod integration;
mod mcp_config;
mod ojson;
#[cfg(test)]
mod ownership_tests;
#[cfg(test)]
mod process_tests;
mod profiles;
mod scope;
mod sessions;
mod state;
mod worktree;
#[derive(Parser)]
struct Args {
    #[command(flatten)]
    location: Location,
    /// The daemon's private state directory: the project set and the
    /// remembered startup Profile live here, changed only through the daemon.
    #[arg(long)]
    state_dir: PathBuf,
    /// The `precept-mcp` executable MCP clients are told to run (the Agents
    /// page's launch line); default: the one beside this daemon.
    #[arg(long)]
    mcp_executable: Option<PathBuf>,
}
#[derive(Clone)]
struct Service {
    profiles: Arc<profiles::Profiles>,
    lease: Arc<Mutex<RuntimeLease>>,
    credentials: Arc<RwLock<Vec<precept_ipc::Credential>>>,
    mcp_executable: Option<PathBuf>,
    grants: precept_ipc::peer::Peers,
    activity: activity::Activity,
    sessions: Arc<sessions::Sessions>,
    evidence: Arc<evidence::Evidence>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorContext {
    project: String,
    #[serde(default)]
    include_documents: bool,
    #[serde(default)]
    selector: precept_core::contracts::SelectorQuery,
}
/// An agent's rule suggestion (`preference.suggest`). The scope is relative to
/// the agent's own project; there is no way to name another one.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentSuggest {
    /// `project` (this project), `environment` (this machine, for this
    /// project) or `personal` (every project).
    scope: String,
    key: String,
    value: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    applies_to: precept_core::operator::AppliesTo,
    #[serde(default)]
    enforcement: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    client: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentContext {
    #[serde(default)]
    include_documents: bool,
    #[serde(default)]
    selector: precept_core::contracts::SelectorQuery,
    /// The MCP client's self-reported name, for the activity log only.
    #[serde(default)]
    #[allow(dead_code)]
    client: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorHandoff {
    project: String,
    task_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentHandoff {
    task_id: String,
    /// The MCP client's self-reported name, for the activity log only.
    #[serde(default)]
    #[allow(dead_code)]
    client: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActivityList {
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    before: Option<u64>,
}
fn decode<T: serde::de::DeserializeOwned>(v: Value) -> precept_ipc::Result<T> {
    serde_json::from_value(v).map_err(|_| Error::InvalidRequest)
}
impl Service {
    fn query(
        &self,
        authority: &precept_ipc::peer::Peer,
        request: Request,
    ) -> precept_ipc::Result<Value> {
        let access = authority.access();
        let params = request.params;
        if request.method == "project.mcp" {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            return self.mcp_launch(params);
        }
        if request.method.starts_with("profile.")
            || matches!(
                request.method.as_str(),
                "project.check"
                    | "project.add"
                    | "project.register"
                    | "project.remove"
                    | "project.agents"
                    | "project.propose"
            )
        {
            return self
                .profiles
                .handle(authority, &request.method, params, self);
        }
        // A work session becomes a Handoff: the operator's write, through the
        // ordinary reviewed write path, with the repository snapshot taken
        // here (never supplied by the caller).
        if request.method == "sessions.handoff" {
            return self.session_handoff(authority, params);
        }
        if matches!(access, Access::OperatorWrite { .. }) {
            return Err(Error::Unauthorized);
        }
        if request.method == "daemon.health" {
            let _: Empty = decode(params)?;
            let selected = self.profiles.selected();
            if !selected && access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            let schema = if selected {
                format!(
                    "{:?}",
                    self.profiles
                        .read()?
                        .0
                        .schema()
                        .map_err(|_| Error::Unavailable)?
                )
            } else {
                "Unselected".into()
            };
            return Ok(
                json!({"api_version":precept_ipc::API_VERSION,"service":"preceptd","version":env!("CARGO_PKG_VERSION"),"schema":schema,"ready":true,"profile_selected":selected,"worktree_fingerprint":"on-demand-acp-worktree-v1"}),
            );
        }
        // The agent activity log is the daemon's own memory: no Profile needed.
        if request.method == "activity.list" {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            let p: ActivityList = decode(params)?;
            if p.project
                .as_deref()
                .is_some_and(|n| !precept_ipc::valid_project(n))
            {
                return Err(Error::InvalidRequest);
            }
            let Some(project) = p.project else {
                return Ok(self.activity.list(None, p.before));
            };
            // A project's activity is its persisted evidence chain; refused
            // handshakes (no project known) stay in memory and are merged in.
            let state = self.profiles.state_dir();
            let (entries, more) = self
                .evidence
                .list(state, &project, p.before, activity::PAGE)?;
            let chain = self.evidence.status(state, &project)?;
            let memory = self.activity.list(Some(&project), None);
            let mut events: Vec<Value> = entries
                .into_iter()
                .map(|b| {
                    json!({"id":b.seq,"at_ms":b.at_ms,"kind":b.kind,"via":b.via,"project":project,
                        "client":b.client,"session":b.session,"outcome":b.outcome,
                        "duration_ms":b.duration_ms,"counts":b.counts})
                })
                .collect();
            if p.before.is_none() {
                events.extend(
                    memory["events"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|e| e["project"].is_null())
                        .take(20)
                        .cloned(),
                );
                events.sort_by_key(|e| std::cmp::Reverse(e["at_ms"].as_u64().unwrap_or(0)));
            }
            return Ok(json!({
                "daemon_started_ms": memory["daemon_started_ms"],
                "events": events,
                "more": more,
                "chain": chain,
            }));
        }
        // The AGENTS.md pointer of a served project: status, and an explicit
        // add/remove guarded by the previewed file digest.
        if matches!(
            request.method.as_str(),
            "integration.pointer" | "integration.pointer.apply"
        ) {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Pointer {
                project: String,
                #[serde(default)]
                enabled: Option<bool>,
                #[serde(default)]
                expected_sha256: Option<String>,
            }
            let p: Pointer = decode(params)?;
            if !self.profiles.allowed(&p.project) {
                return Err(Error::NotFound);
            }
            let (_, roots) = self.profiles.read()?;
            let root = roots.get(&p.project).ok_or(Error::NotFound)?;
            return match (request.method.as_str(), p.enabled) {
                ("integration.pointer", None) if p.expected_sha256.is_none() => {
                    integration::status(root, &p.project)
                }
                ("integration.pointer.apply", Some(enabled)) => {
                    if p.expected_sha256
                        .as_ref()
                        .is_some_and(|d| d.len() != 64 || !d.bytes().all(|b| b.is_ascii_hexdigit()))
                    {
                        return Err(Error::InvalidRequest);
                    }
                    integration::apply(root, &p.project, enabled, p.expected_sha256.as_deref())
                }
                _ => Err(Error::InvalidRequest),
            };
        }
        if matches!(
            request.method.as_str(),
            "integration.hooks" | "integration.hooks.apply"
        ) {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            return self.hooks(&request.method, params);
        }
        // What an import would suggest from the project's AGENTS.md: a
        // bounded read in the served root; nothing is written here.
        if request.method == "import.scan" {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct P {
                project: String,
            }
            let p: P = decode(params)?;
            if !self.profiles.allowed(&p.project) {
                return Err(Error::NotFound);
            }
            let (store, roots) = self.profiles.read()?;
            let root = roots.get(&p.project).ok_or(Error::NotFound)?;
            let (candidates, skipped) = importer::scan(root);
            let candidates: Vec<Value> = candidates
                .into_iter()
                .map(|c| {
                    // AGENTS.md sections are this project's conventions.
                    let scope = Scope::Project {
                        name: p.project.clone(),
                    };
                    // Already in the Profile (suggested or confirmed) as is.
                    let exists = store
                        .agent_suggestion_state(&scope, &c.key, &c.value)
                        .map(|(duplicate, _)| duplicate)
                        .unwrap_or(false);
                    let mut v = serde_json::to_value(&c).unwrap_or(Value::Null);
                    v["scope"] = serde_json::to_value(&scope).unwrap_or(Value::Null);
                    v["exists"] = json!(exists);
                    v
                })
                .collect();
            return Ok(json!({"candidates": candidates, "skipped": skipped}));
        }
        if matches!(
            request.method.as_str(),
            "integration.mcp" | "integration.mcp.apply"
        ) {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            return self.mcp_entries(&request.method, params);
        }
        // The daemon's own project set needs no Profile: an operator connecting
        // before creating or selecting one must still get a successful read.
        if request.method == "project.list" {
            let _: Empty = decode(params)?;
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            return Ok(self.profiles.list());
        }
        // Work sessions: an agent records its own goal and work state (needs
        // the operator's permission for agent writes); the operator lists and
        // deletes them.
        if matches!(
            request.method.as_str(),
            "session.start" | "session.update" | "session.close"
        ) {
            let Access::Agent { project } = access else {
                return Err(Error::Unauthorized);
            };
            if !self.profiles.can_propose(&project) {
                return Err(Error::Unauthorized);
            }
            let u: sessions::Update = decode(params)?;
            let state = self.profiles.state_dir();
            let session = match request.method.as_str() {
                "session.start" => self.sessions.start(state, &project, u)?,
                "session.update" => self.sessions.update(state, &project, u, false)?,
                _ => self.sessions.update(state, &project, u, true)?,
            };
            return serde_json::to_value(session).map_err(|_| Error::Unavailable);
        }
        if matches!(request.method.as_str(), "sessions.list" | "sessions.delete") {
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct P {
                project: String,
                #[serde(default)]
                id: Option<String>,
            }
            let p: P = decode(params)?;
            if !precept_ipc::valid_project(&p.project) || !self.profiles.listed(&p.project) {
                return Err(Error::NotFound);
            }
            let state = self.profiles.state_dir();
            return match (request.method.as_str(), p.id) {
                ("sessions.list", None) => Ok(json!({
                    "sessions": self.sessions.list(state, &p.project)?
                })),
                ("sessions.delete", Some(id)) => {
                    self.sessions.delete(state, &p.project, &id)?;
                    Ok(json!({"deleted": id}))
                }
                _ => Err(Error::InvalidRequest),
            };
        }
        if request.method == "preference.suggest" {
            let Access::Agent { project } = access else {
                return Err(Error::Unauthorized);
            };
            let p: AgentSuggest = decode(params)?;
            let scope = match p.scope.as_str() {
                "project" => Scope::Project {
                    name: project.clone(),
                },
                "environment" => Scope::Environment {
                    name: project.clone(),
                },
                "personal" => Scope::Global,
                _ => return Err(Error::InvalidRequest),
            };
            // Every record carries a tag; an untagged suggestion is filed under `agent`.
            let tags = if p.tags.is_empty() {
                vec!["agent".to_owned()]
            } else {
                p.tags
            };
            let tags = precept_ipc::operator::canonical_tags(&tags).ok_or(Error::InvalidRequest)?;
            let enforcement = match p.enforcement.as_deref() {
                None => Default::default(),
                Some(e) => {
                    precept_core::operator::Enforcement::parse(e).ok_or(Error::InvalidRequest)?
                }
            };
            let rule = precept_core::operator::RuleOptions {
                applies_to: p.applies_to.canonical().ok_or(Error::InvalidRequest)?,
                priority: 0,
                enforcement,
            };
            if !rule.valid_for(&scope) {
                return Err(Error::InvalidRequest);
            }
            return self
                .profiles
                .agent_suggest(&project, scope, p.key, p.value, tags, rule);
        }
        let (store, roots) = self.profiles.read()?;
        let (project, include, task, selector) = match (request.method.as_str(), access) {
            ("context.get", Access::Operator) => {
                let p: OperatorContext = decode(params)?;
                (p.project, p.include_documents, None, p.selector)
            }
            ("context.get", Access::Agent { project }) => {
                let p: AgentContext = decode(params)?;
                (project, p.include_documents, None, p.selector)
            }
            ("handoff.get", Access::Operator) => {
                let p: OperatorHandoff = decode(params)?;
                (p.project, false, Some(p.task_id), Default::default())
            }
            ("handoff.get", Access::Agent { project }) => {
                let p: AgentHandoff = decode(params)?;
                (project, false, Some(p.task_id), Default::default())
            }
            _ => return Err(Error::Unsupported),
        };
        if !self.profiles.allowed(&project) {
            return Err(Error::Unauthorized);
        }
        selector.validate().map_err(|_| Error::InvalidRequest)?;
        if task.as_ref().is_some_and(|s| s.chars().count() > 120) {
            return Err(Error::InvalidRequest);
        }
        let root = store
            .read_project(&project)
            .ok()
            .and_then(|p| p.root.canonicalize().ok());
        if root.as_ref() != roots.get(&project) {
            return Err(Error::UnsafeRuntime);
        }
        let resolver = ContextResolver::new(
            &project,
            &store,
            &store,
            &store,
            &worktree::GitInspector,
            &FileDocuments,
        )
        .map_err(|_| Error::NotFound)?;
        let result = if let Some(task) = task {
            resolver.handoff(&task)
        } else {
            let prepared = resolver
                .prepare_context(include, &selector)
                .map_err(|_| Error::Unavailable)?;
            #[cfg(test)]
            process_tests::before_context_finish();
            // Recheck the selected/authorized root after assembly, then let the
            // core re-read the bounded source. Lease publication checks follow.
            let current = store
                .read_project(&project)
                .ok()
                .and_then(|p| p.root.canonicalize().ok());
            if current.as_ref() != roots.get(&project) {
                return Err(Error::UnsafeRuntime);
            }
            prepared.finish(current.as_ref().ok_or(Error::UnsafeRuntime)?)
        };
        let mut result = result.map_err(|e| match e {
            precept_core::Error::HandoffNotFound | precept_core::Error::UnknownProject => {
                Error::NotFound
            }
            _ => Error::Unavailable,
        })?;
        // Context carries the project's recent work sessions: where other
        // agents left the work (agents' own records, not reviewed rules).
        if request.method == "context.get" && result.is_object() {
            let work = self
                .sessions
                .for_context(self.profiles.state_dir(), &project);
            result["work_sessions"] = serde_json::to_value(work).unwrap_or(Value::Array(vec![]));
        }
        Ok(result)
    }
}
impl Service {
    /// How an MCP client starts `precept-mcp` for one project of this daemon: the
    /// executable (the configured context bridge, else the `precept-mcp` beside
    /// this daemon) and its exact arguments. Paths only, never a token.
    /// The `precept-mcp` MCP clients run: the configured one, else beside this daemon.
    fn mcp_path(&self) -> Option<PathBuf> {
        self.mcp_executable.clone().or_else(|| {
            let beside = std::env::current_exe()
                .ok()?
                .with_file_name(format!("precept-mcp{}", std::env::consts::EXE_SUFFIX));
            beside.is_file().then_some(beside)
        })
    }
    /// The session-start hook, which ships beside `precept-mcp`.
    fn hook_path(&self) -> Option<PathBuf> {
        let hook = self
            .mcp_path()?
            .with_file_name(format!("precept-hook{}", std::env::consts::EXE_SUFFIX));
        hook.is_file().then_some(hook)
    }
    /// The hook entries of a served project in the agents' user configuration:
    /// status per harness, or one explicit add/remove guarded by the digest.
    fn hooks(&self, method: &str, params: Value) -> precept_ipc::Result<Value> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct P {
            project: String,
            #[serde(default)]
            vendor: Option<integration::Vendor>,
            #[serde(default)]
            enabled: Option<bool>,
            #[serde(default)]
            expected_sha256: Option<String>,
        }
        let p: P = decode(params)?;
        if !self.profiles.allowed(&p.project) {
            return Err(Error::NotFound);
        }
        let (_, roots) = self.profiles.read()?;
        let root = roots.get(&p.project).ok_or(Error::NotFound)?.clone();
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|h| h.is_absolute())
            .ok_or(Error::Unavailable)?;
        let location = self.profiles.location();
        let Some(hook) = self.hook_path() else {
            return Ok(json!({"available": false, "hooks": []}));
        };
        let launch = integration::HookLaunch {
            hook,
            runtime_dir: location.runtime_dir.clone(),
            endpoint: location.endpoint.clone(),
            capability: location.credential_path(Some(&p.project))?,
            root,
        };
        match (method, p.vendor, p.enabled) {
            ("integration.hooks", None, None) if p.expected_sha256.is_none() => {
                let hooks = integration::Vendor::ALL
                    .iter()
                    .map(|v| {
                        let cmd = integration::command(&launch, *v, &p.project);
                        integration::hook_status(&home, *v, &p.project, &cmd)
                    })
                    .collect::<precept_ipc::Result<Vec<_>>>()?;
                Ok(json!({"available": true, "hooks": hooks}))
            }
            ("integration.hooks.apply", Some(vendor), Some(enabled)) => {
                if p.expected_sha256
                    .as_ref()
                    .is_some_and(|d| d.len() != 64 || !d.bytes().all(|b| b.is_ascii_hexdigit()))
                {
                    return Err(Error::InvalidRequest);
                }
                let cmd = integration::command(&launch, vendor, &p.project);
                integration::hook_apply(
                    &home,
                    vendor,
                    &p.project,
                    &cmd,
                    enabled,
                    p.expected_sha256.as_deref(),
                )
            }
            _ => Err(Error::InvalidRequest),
        }
    }
    /// Saves a work session as the project's Handoff `task_id` (absent until
    /// now): goal as objective, done and summary as completed, todo as next
    /// action, and the repository's current revision, branch and state.
    fn session_handoff(
        &self,
        authority: &precept_ipc::peer::Peer,
        params: Value,
    ) -> precept_ipc::Result<Value> {
        let Access::OperatorWrite {
            profile_id,
            generation,
            owner_epoch,
        } = authority.access()
        else {
            return Err(Error::Unauthorized);
        };
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct P {
            project: String,
            id: String,
            task_id: String,
        }
        let p: P = decode(params)?;
        if !self.profiles.allowed(&p.project) {
            return Err(Error::NotFound);
        }
        let session = self
            .sessions
            .list(self.profiles.state_dir(), &p.project)?
            .into_iter()
            .find(|s| s.id == p.id)
            .ok_or(Error::NotFound)?;
        let (_, roots) = self.profiles.read()?;
        let root = roots.get(&p.project).ok_or(Error::NotFound)?.clone();
        let tree = worktree::GitInspector
            .inspect(
                &precept_core::Project {
                    name: p.project.clone(),
                    root,
                },
                true,
            )
            .map_err(|_| Error::Unavailable)?;
        let lines = |items: &[String]| {
            items
                .iter()
                .map(|i| format!("- {i}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let mut completed = lines(&session.done);
        if let Some(summary) = &session.summary {
            if !completed.is_empty() {
                completed.push_str("\n\n");
            }
            completed.push_str(summary);
        }
        let next_action = if session.todo.is_empty() {
            "Review the completed work and decide the next step.".to_owned()
        } else {
            lines(&session.todo)
        };
        let request = json!({
            "version": 2,
            "binding": {"profile_id":profile_id,"generation":generation,"owner_epoch":owner_epoch},
            "operation_id": precept_ipc::operator::operation_id()?,
            "command": {
                "command": "put_handoff",
                "project": p.project,
                "task_id": p.task_id,
                "objective": session.goal,
                "completed": completed,
                "next_action": next_action,
                "validation_json": "[]",
                "snapshot": {
                    "base_sha": tree.head.unwrap_or_default(),
                    "branch": tree.branch.unwrap_or_default(),
                    "state": match tree.dirty {
                        Some(true) => "dirty",
                        Some(false) => "clean",
                        None => "unknown",
                    },
                    "fingerprint": tree.worktree_fingerprint,
                },
                "expected": {"kind":"absent"},
            },
        });
        self.profiles
            .handle(authority, "profile.mutate", request, self)
    }
    /// The project's `precept-mcp` entry in each client's user configuration in
    /// the daemon user's home: status, or one explicit add/remove guarded by
    /// the reported digest.
    fn mcp_entries(&self, method: &str, params: Value) -> precept_ipc::Result<Value> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct P {
            project: String,
            #[serde(default)]
            client: Option<mcp_config::Client>,
            #[serde(default)]
            enabled: Option<bool>,
            #[serde(default)]
            expected_sha256: Option<String>,
        }
        let p: P = decode(params)?;
        if !self.profiles.allowed(&p.project) {
            return Err(Error::NotFound);
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|h| h.is_absolute())
            .ok_or(Error::Unavailable)?;
        let Some(command) = self.mcp_path() else {
            return Ok(json!({"available": false, "clients": []}));
        };
        let location = self.profiles.location();
        let path = |p: &std::path::Path| p.to_string_lossy().into_owned();
        let launch = mcp_config::Launch {
            command: path(&command),
            args: vec![
                "--runtime-dir".into(),
                path(&location.runtime_dir),
                "--endpoint".into(),
                location.endpoint.clone(),
                "--project".into(),
                p.project.clone(),
                "--capability-file".into(),
                path(&location.credential_path(Some(&p.project))?),
            ],
        };
        match (method, p.client, p.enabled) {
            ("integration.mcp", None, None) if p.expected_sha256.is_none() => {
                let clients = mcp_config::Client::ALL
                    .iter()
                    .map(|c| mcp_config::status(&home, *c, &p.project, &launch))
                    .collect::<precept_ipc::Result<Vec<_>>>()?;
                Ok(json!({"available": true, "clients": clients}))
            }
            ("integration.mcp.apply", Some(client), Some(enabled)) => {
                if p.expected_sha256
                    .as_ref()
                    .is_some_and(|d| d.len() != 64 || !d.bytes().all(|b| b.is_ascii_hexdigit()))
                {
                    return Err(Error::InvalidRequest);
                }
                mcp_config::apply(
                    &home,
                    client,
                    &p.project,
                    &launch,
                    enabled,
                    p.expected_sha256.as_deref(),
                )
            }
            _ => Err(Error::InvalidRequest),
        }
    }
    fn mcp_launch(&self, params: serde_json::Value) -> precept_ipc::Result<serde_json::Value> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct P {
            name: String,
        }
        let p: P = decode(params)?;
        if !self.profiles.allowed(&p.name) {
            return Err(Error::NotFound);
        }
        let location = self.profiles.location();
        let capability = location.credential_path(Some(&p.name))?;
        let executable = self.mcp_path();
        let hook = self.hook_path();
        let shared = self.profiles.shared_names().contains(&p.name);
        Ok(serde_json::json!({
            "name": p.name,
            "shared": shared,
            "executable": executable,
            "hook": hook,
            "args": [
                "--runtime-dir", location.runtime_dir,
                "--endpoint", location.endpoint,
                "--project", p.name,
                "--capability-file", capability,
            ],
        }))
    }
}
impl profiles::Hooks for Service {
    fn added(&self, name: &str) -> precept_ipc::Result<()> {
        let credential = self.lease.lock().unwrap().issue_agent(name)?;
        self.credentials.write().unwrap().push(credential);
        Ok(())
    }
    fn withheld(&self, name: &str) {
        let _ = self.lease.lock().unwrap().revoke_agent(name);
        self.credentials
            .write()
            .unwrap()
            .retain(|c| !matches!(&c.access, Access::Agent { project } if project == name));
    }
    fn removed(&self, name: &str) {
        let _ = self.lease.lock().unwrap().revoke_agent(name);
        self.credentials
            .write()
            .unwrap()
            .retain(|c| !matches!(&c.access, Access::Agent { project } if project == name));
    }
}
async fn peer(mut stream: Stream, service: Service, permit: Arc<OwnedSemaphorePermit>) {
    // A snapshot: credentials added or revoked later apply to new connections.
    let credentials = service.credentials.read().unwrap().clone();
    let authority = match service.grants.accept(&mut stream, &credentials).await {
        Ok(a) => a,
        Err(e) => {
            service.activity.record(activity::Record {
                kind: activity::Kind::Refused,
                via: activity::Via::Unknown,
                project: None,
                client: None,
                outcome: e.to_string(),
                duration_ms: 0,
                counts: None,
            });
            let _ = write_frame(&mut stream, &Response::error(None, e)).await;
            return;
        }
    };
    let access = authority.access();
    let mut ids = HashSet::new();
    for _ in 0..MAX_REQUESTS {
        let value = match read_frame(&mut stream).await {
            Ok(v) => v,
            Err(e) => {
                let _ = write_frame(&mut stream, &Response::error(None, e)).await;
                return;
            }
        };
        let mut request: Request = match serde_json::from_value(value) {
            Ok(r) => r,
            Err(_) => {
                let _ =
                    write_frame(&mut stream, &Response::error(None, Error::InvalidRequest)).await;
                return;
            }
        };
        let id = request.id.clone();
        let error = if request.api_version != precept_ipc::API_VERSION {
            Some(Error::Version)
        } else if id.is_empty() || id.len() > 64 || !ids.insert(id.clone()) {
            Some(Error::InvalidRequest)
        } else {
            None
        };
        if let Some(e) = error {
            let _ = write_frame(&mut stream, &Response::error(Some(id), e)).await;
            return;
        }
        if let Err(e) = authority.check() {
            let _ = write_frame(&mut stream, &Response::error(Some(id), e)).await;
            return;
        }
        let method = request.method.clone();
        let began = std::time::Instant::now();
        let client = request
            .params
            .get("client")
            .and_then(Value::as_str)
            .and_then(activity::client_label);
        // The work session the request belongs to (precept-mcp remembers it):
        // evidence only, removed before the method sees its parameters.
        let session = request
            .params
            .as_object_mut()
            .and_then(|o| o.remove("session"))
            .and_then(|v| v.as_str().map(str::to_owned))
            .filter(|s| s.len() <= 32 && s.bytes().all(|b| b.is_ascii_hexdigit()));
        let result = {
            let service = service.clone();
            let held = permit.clone();
            let auth = authority.clone();
            // A timed-out blocking job retains its admission slot.
            let job = tokio::task::spawn_blocking(move || {
                let _held = held;
                auth.check()?;
                service.query(&auth, request)
            });
            match tokio::time::timeout(Duration::from_secs(5), job).await {
                Ok(Ok(r)) => r,
                Ok(Err(_)) => Err(Error::Unavailable),
                Err(_) => Err(if method == "profile.mutate" {
                    Error::OutcomeUnknown
                } else {
                    Error::Timeout
                }),
            }
        };
        // Mutation dispatch itself establishes admission. A later revocation
        // cannot turn an admitted committed write into an authorization error.
        let result = if method == "profile.mutate" {
            result
        } else {
            authority.check().and(result)
        };
        if let Access::Agent { project } = &access {
            let kind = match method.as_str() {
                "daemon.health" => Some(activity::Kind::Started),
                "context.get" => Some(activity::Kind::Context),
                "handoff.get" => Some(activity::Kind::Handoff),
                "preference.suggest" => Some(activity::Kind::Suggested),
                "session.start" | "session.update" | "session.close" => {
                    Some(activity::Kind::Session)
                }
                _ => None,
            };
            if let Some(kind) = kind {
                let counts = match (&result, kind) {
                    (Ok(v), activity::Kind::Context) => Some(activity::context_counts(v)),
                    _ => None,
                };
                let outcome = activity::outcome(&result);
                let duration_ms = began.elapsed().as_millis() as u64;
                // The session a start opened is its own evidence's session.
                let session = session
                    .clone()
                    .or_else(|| match (&result, method.as_str()) {
                        (Ok(v), "session.start") => v["id"].as_str().map(str::to_owned),
                        _ => None,
                    });
                // Persisted only for a served project (a stale credential's
                // project may be gone); best effort, never failing the call.
                if service.profiles.allowed(project) {
                    let _ = service.evidence.append(
                        service.profiles.state_dir(),
                        project,
                        evidence::Entry {
                            kind,
                            via: activity::Via::Mcp,
                            client: client.clone(),
                            session,
                            outcome: outcome.clone(),
                            duration_ms,
                            counts: counts.clone(),
                            at_ms: activity::now(),
                        },
                    );
                }
                service.activity.record(activity::Record {
                    kind,
                    via: activity::Via::Mcp,
                    project: Some(project.clone()),
                    client: client.clone(),
                    outcome,
                    duration_ms,
                    counts,
                });
            }
        }
        let close = matches!(result, Err(Error::Timeout | Error::OutcomeUnknown));
        let response = match result {
            Ok(v) => Response::ok(id.clone(), v),
            Err(e) => Response::error(Some(id.clone()), e),
        };
        if let Err(e) = write_frame(&mut stream, &response).await {
            if e == Error::TooLarge {
                let _ = write_frame(&mut stream, &Response::error(Some(id), e)).await;
            }
            return;
        }
        if close {
            return;
        }
    }
}
async fn stop() {
    #[cfg(unix)]
    {
        let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        else {
            return;
        };
        tokio::select! {_=tokio::signal::ctrl_c()=>(),_=terminate.recv()=>()}
    }
    #[cfg(windows)]
    {
        let Ok(mut interrupt) = tokio::signal::windows::ctrl_break() else {
            return;
        };
        tokio::select! {_=tokio::signal::ctrl_c()=>(),_=interrupt.recv()=>()}
    }
}
async fn run(args: Args) -> precept_ipc::Result<()> {
    let state_dir = state::StateDir::open(&args.state_dir)?;
    let grants = precept_ipc::peer::Peers::default();
    let profiles = Arc::new(profiles::Profiles::new(
        args.location.clone(),
        state_dir,
        grants.writes.clone(),
    )?);
    #[cfg(test)]
    process_tests::install_checkpoint(&profiles);
    let mut lease = RuntimeLease::acquire(&args.location)?;
    grants.writes.cleanup_stale(&args.location)?;
    let mut listener = precept_ipc::transport::Listener::bind(&args.location).await?;
    // Agent credentials only for projects shared with external agents.
    let credentials = Arc::new(RwLock::new(lease.credentials(&profiles.shared_names())?));
    let service = Service {
        profiles,
        lease: Arc::new(Mutex::new(lease)),
        credentials,
        mcp_executable: args.mcp_executable,
        grants,
        activity: activity::Activity::default(),
        sessions: Arc::new(sessions::Sessions::new()),
        evidence: Arc::new(evidence::Evidence::new()),
    };
    // The Profile the operator asked to select at start, if any.
    {
        let service = service.clone();
        tokio::task::spawn_blocking(move || service.profiles.select_remembered())
            .await
            .map_err(|_| Error::Unavailable)?;
    }
    let slots = Arc::new(Semaphore::new(MAX_PEERS));
    let mut tasks = tokio::task::JoinSet::new();
    let shutdown = stop();
    tokio::pin!(shutdown);
    eprintln!("preceptd ready");
    loop {
        tokio::select! {
            _=&mut shutdown=>break,
            Some(_)=tasks.join_next(),if !tasks.is_empty()=>(),
            accepted=listener.accept()=>{
                match accepted {
                    Ok(stream)=>{if let Ok(slot)=slots.clone().try_acquire_owned(){tasks.spawn(peer(stream,service.clone(),Arc::new(slot)));}},
                    Err(Error::Unauthorized)=>(),
                    Err(_)=>tokio::time::sleep(Duration::from_millis(10)).await,
                }
            }
        }
    }
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    service.grants.writes.close();
    let profiles = service.profiles.clone();
    let _ = tokio::task::spawn_blocking(move || profiles.disable()).await;
    drop(listener);
    let Service { lease, .. } = service;
    drop(lease);
    Ok(())
}
fn main() {
    let args = Args::parse();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(MAX_PEERS)
        .enable_all()
        .build()
        .unwrap();
    let result = runtime.block_on(run(args));
    runtime.shutdown_timeout(Duration::from_secs(2));
    if let Err(error) = result {
        eprintln!("preceptd: {error}");
        std::process::exit(2);
    }
}

/// Test state: writes the project set and the remembered startup Profile into
/// a new private state directory, as the daemon would after operator actions.
#[cfg(test)]
pub(crate) fn test_state(
    dir: &std::path::Path,
    projects: &[(&str, &std::path::Path)],
    profile: Option<&std::path::Path>,
) -> PathBuf {
    let state = state::StateDir::open(dir).unwrap();
    let set = projects
        .iter()
        .map(|(n, r)| ((*n).to_owned(), r.canonicalize().unwrap()))
        .collect();
    state
        .save_projects(&set, &Default::default(), &Default::default())
        .unwrap();
    if let Some(profile) = profile {
        state.save_startup(profile).unwrap();
    }
    dir.to_path_buf()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn case_insensitive_capability_filename_collisions_fail_before_storage_open() {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("state");
        precept_ipc::transport::private_dir(&state, true).unwrap();
        let mut file = precept_ipc::transport::new_private(&state.join(state::PROJECTS)).unwrap();
        std::io::Write::write_all(
            &mut file,
            br#"{"format":1,"projects":[{"name":"alpha","root":"/a"},{"name":"ALPHA","root":"/b"}]}"#,
        )
        .unwrap();
        let args = Args {
            location: Location {
                runtime_dir: temp.path().join("runtime"),
                endpoint: temp
                    .path()
                    .join("runtime/control.sock")
                    .to_string_lossy()
                    .into(),
            },
            state_dir: state,
            mcp_executable: None,
        };
        assert_eq!(run(args).await, Err(Error::InvalidRequest));
        assert!(
            !temp.path().join("runtime").exists(),
            "refused before the runtime"
        );
    }
}
