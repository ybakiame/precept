use clap::Parser;
use precept_core::{ContextResolver, documents::FileDocuments, operator::Scope};
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
mod memory;
mod ojson;
#[cfg(test)]
mod ownership_tests;
mod personal_sync;
#[cfg(test)]
mod process_tests;
mod profiles;
mod rpc;
mod rule_export;
mod rule_lint;
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
/// Gate denials of one rule within this window count as repeats.
const REPEAT_WINDOW_MS: u64 = 24 * 3_600_000;
/// The window of gate denials a context carries.
const DENIALS_WINDOW_MS: u64 = 7 * 24 * 3_600_000;

#[derive(Clone)]
struct Service {
    profiles: Arc<profiles::Profiles>,
    lease: Arc<Mutex<RuntimeLease>>,
    credentials: Arc<RwLock<Vec<precept_ipc::Credential>>>,
    mcp_executable: Option<PathBuf>,
    grants: precept_ipc::peer::Peers,
    activity: activity::Activity,
    sessions: Arc<sessions::Sessions>,
    memory: Arc<memory::Memory>,
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
    /// `context.explain`: the key whose rules to explain.
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    selector: precept_core::contracts::SelectorQuery,
}
/// An agent's rule suggestion (`preference.suggest`). The scope is relative to
/// the agent's own project; there is no way to name another one.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentSuggest {
    /// `project` (this project) or `personal` (every project).
    scope: String,
    #[serde(default)]
    key: String,
    value: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    applies_to: precept_core::operator::AppliesTo,
    #[serde(default)]
    enforcement: Option<String>,
    /// A confirmed rule of this scope the suggestion would update: it takes
    /// that rule's key and, when not given, its tags and rule options.
    #[serde(default)]
    replaces: Option<i64>,
    #[serde(default)]
    #[allow(dead_code)]
    client: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentContext {
    #[serde(default)]
    include_documents: bool,
    /// `context.explain`: the key whose rules to explain.
    #[serde(default)]
    key: Option<String>,
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
            event: integration::HookEvent,
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
        let home = scope::home().ok_or(Error::Unavailable)?;
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
                let status = |event| {
                    integration::Vendor::ALL
                        .iter()
                        .map(|v| {
                            let cmd = integration::command(&launch, *v, &p.project, event);
                            integration::hook_status(&home, *v, &p.project, &cmd, event)
                        })
                        .collect::<precept_ipc::Result<Vec<_>>>()
                };
                let compact_hooks = [integration::Vendor::Claude]
                    .iter()
                    .map(|v| {
                        integration::hook_status(
                            &home,
                            *v,
                            &p.project,
                            &integration::command(
                                &launch,
                                *v,
                                &p.project,
                                integration::HookEvent::PreCompact,
                            ),
                            integration::HookEvent::PreCompact,
                        )
                    })
                    .collect::<precept_ipc::Result<Vec<_>>>()?;
                Ok(json!({
                    "available": true,
                    "hooks": status(integration::HookEvent::SessionStart)?,
                    "gate_hooks": status(integration::HookEvent::PreTool)?,
                    // The stale-document warning after a read: Claude Code only.
                    "read_hooks": [integration::hook_status(
                        &home,
                        integration::Vendor::Claude,
                        &p.project,
                        &integration::command(
                            &launch,
                            integration::Vendor::Claude,
                            &p.project,
                            integration::HookEvent::PostRead,
                        ),
                        integration::HookEvent::PostRead,
                    )?],
                    // The work-state reminder before compaction: Claude Code
                    // only.
                    "compact_hooks": compact_hooks,
                    "gate": self.profiles.gated(&p.project),
                }))
            }
            ("integration.hooks.apply", Some(vendor), Some(enabled)) => {
                if p.expected_sha256
                    .as_ref()
                    .is_some_and(|d| d.len() != 64 || !d.bytes().all(|b| b.is_ascii_hexdigit()))
                {
                    return Err(Error::InvalidRequest);
                }
                // The read warning and the compaction reminder: Claude Code only.
                if matches!(
                    p.event,
                    integration::HookEvent::PostRead | integration::HookEvent::PreCompact
                ) && vendor != integration::Vendor::Claude
                {
                    return Err(Error::Unsupported);
                }
                let cmd = integration::command(&launch, vendor, &p.project, p.event);
                integration::hook_apply(
                    &home,
                    vendor,
                    &p.project,
                    &cmd,
                    enabled,
                    p.expected_sha256.as_deref(),
                    p.event,
                )
            }
            _ => Err(Error::InvalidRequest),
        }
    }
    /// Saves a work session as the project's Handoff `task_id` (absent until
    /// now): goal as objective, done and summary as completed, todo as next
    /// action, and the repository's current revision, branch and state.
    /// A note that held in [`memory::PROPOSE_AFTER`] more sessions is filed
    /// once as a suggested project rule (the agent-suggestion path: only
    /// where agents may suggest and writes are on; never confirmed). Best
    /// effort: a refusal leaves the note as it is, to try at its next
    /// confirmation.
    fn promote(&self, project: &str, entry: memory::Entry) -> memory::Entry {
        if !memory::Memory::ripe(&entry) {
            return entry;
        }
        let mut key = String::new();
        for c in entry.title.chars().flat_map(char::to_lowercase) {
            if c.is_ascii_alphanumeric() {
                key.push(c);
            } else if !key.is_empty() && !key.ends_with('-') {
                key.push('-');
            }
        }
        let mut key: String = key.trim_end_matches('-').chars().take(60).collect();
        if key.is_empty() {
            key = format!("memory-{}", &entry.id[..8]);
        }
        let tags = vec![entry.kind.as_str().to_owned(), "memory".to_owned()];
        let filed = self.profiles.agent_suggest(
            project,
            Scope::Project {
                name: project.to_owned(),
            },
            key,
            entry.body.clone(),
            tags,
            Default::default(),
        );
        let rule = match filed {
            Ok(v) => v["id"].as_str().map(str::to_owned).unwrap_or_default(),
            // Already suggested or confirmed with this exact text.
            Err(Error::Conflict) => "existing".to_owned(),
            Err(_) => return entry,
        };
        self.memory
            .mark_proposed(self.profiles.state_dir(), project, &entry.id, &rule)
            .unwrap_or(entry)
    }
    /// The project's `precept-mcp` entry in each client's user configuration in
    /// the daemon user's home: status, or one explicit add/remove guarded by
    /// the reported digest.
    fn mcp_entries(&self, method: &str, params: Value) -> precept_ipc::Result<Value> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct P {
            #[serde(default)]
            client: Option<mcp_config::Client>,
            #[serde(default)]
            enabled: Option<bool>,
            #[serde(default)]
            expected_sha256: Option<String>,
        }
        let p: P = decode(params)?;
        let home = scope::home().ok_or(Error::Unavailable)?;
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
            ],
        };
        match (method, p.client, p.enabled) {
            ("integration.mcp", None, None) if p.expected_sha256.is_none() => {
                let clients = mcp_config::Client::ALL
                    .iter()
                    .map(|c| mcp_config::status(&home, *c, &launch))
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
            // The one `precept` server, for every project.
            "args": [
                "--runtime-dir", location.runtime_dir,
                "--endpoint", location.endpoint,
            ],
            // What a project's hook command adds.
            "project_args": [
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
            let limit = if matches!(
                method.as_str(),
                "profile.sync.export" | "profile.sync.preview"
            ) {
                Duration::from_secs(15)
            } else {
                Duration::from_secs(5)
            };
            match tokio::time::timeout(limit, job).await {
                Ok(Ok(r)) => r,
                Ok(Err(_)) => Err(Error::Unavailable),
                Err(_) => Err(if uncertain_mutation(&method) {
                    Error::OutcomeUnknown
                } else {
                    Error::Timeout
                }),
            }
        };
        // Mutation dispatch itself establishes admission. A later revocation
        // cannot turn an admitted committed write into an authorization error.
        let result = if uncertain_mutation(&method) {
            result
        } else {
            authority.check().and(result)
        };
        if let Access::Agent { project } = &access {
            let kind = match method.as_str() {
                "daemon.health" => Some(activity::Kind::Started),
                "context.get" | "context.explain" => Some(activity::Kind::Context),
                "handoff.get" => Some(activity::Kind::Handoff),
                "preference.suggest" => Some(activity::Kind::Suggested),
                "session.start" | "session.update" | "session.close" => {
                    Some(activity::Kind::Session)
                }
                "gate.check" => Some(activity::Kind::Gate),
                "memory.remember" | "memory.reaffirm" | "memory.forget" | "memory.recall"
                | "memory.get" => Some(activity::Kind::Memory),
                _ => None,
            };
            if let Some(kind) = kind {
                let counts = match (&result, kind) {
                    (Ok(v), activity::Kind::Context) => Some(activity::context_counts(v)),
                    _ => None,
                };
                // A gate answer records its decision (a fixed word), not "ok".
                let outcome = match (&result, kind) {
                    (Ok(v), activity::Kind::Gate) => {
                        match (v["decision"].as_str(), v["gate"].as_str()) {
                            (Some("deny"), _) => "deny".into(),
                            (_, Some("off")) => "off".into(),
                            _ => "allow".into(),
                        }
                    }
                    _ => activity::outcome(&result),
                };
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
                            rule: match (&result, kind) {
                                (Ok(v), activity::Kind::Gate) if v["decision"] == "deny" => v
                                    .pointer("/rule/key")
                                    .and_then(Value::as_str)
                                    .map(str::to_owned),
                                _ => None,
                            },
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
        memory: Arc::new(memory::Memory::new()),
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

fn uncertain_mutation(method: &str) -> bool {
    matches!(
        method,
        "profile.mutate" | "sessions.handoff" | "profile.sync.apply"
    )
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
    state.save_projects(&set, &Default::default()).unwrap();
    if let Some(profile) = profile {
        state.save_startup(profile).unwrap();
    }
    dir.to_path_buf()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handoff_shares_mutation_timeout_and_admission_semantics() {
        assert!(uncertain_mutation("profile.mutate"));
        assert!(uncertain_mutation("sessions.handoff"));
        assert!(!uncertain_mutation("session.update"));
        assert!(!uncertain_mutation("context.get"));
    }

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
