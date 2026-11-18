use acp_core::{ContextResolver, documents::FileDocuments};
use acp_ipc::{
    Access, Error, Location, MAX_PEERS, MAX_REQUESTS, Request, Response, RuntimeLease, Stream,
    read_frame, write_frame,
};
use clap::Parser;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
#[cfg(test)]
mod ownership_tests;
#[cfg(test)]
mod process_tests;
mod profiles;
mod scope;
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
    /// Private TOML session configuration; no path or no file disables execution.
    #[arg(long)]
    agent_config: Option<PathBuf>,
    /// Trusted absolute stdio bridge; per-start share_context opt-in is still required.
    #[arg(long)]
    context_mcp: Option<PathBuf>,
    #[arg(long, default_value_t = 300, value_parser = clap::value_parser!(u64).range(1..=3600))]
    context_lease_seconds: u64,
}
#[derive(Clone)]
struct Service {
    profiles: Arc<profiles::Profiles>,
    lease: Arc<Mutex<RuntimeLease>>,
    credentials: Arc<RwLock<Vec<acp_ipc::Credential>>>,
    agent_config: Option<PathBuf>,
    context_mcp: Option<PathBuf>,
    context_lease_seconds: u64,
    sessions: acp_session::runtime::Supervisor,
    grants: acp_ipc::session_lease::Registry,
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
    selector: acp_core::contracts::SelectorQuery,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentContext {
    #[serde(default)]
    include_documents: bool,
    #[serde(default)]
    selector: acp_core::contracts::SelectorQuery,
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
}
fn decode<T: serde::de::DeserializeOwned>(v: Value) -> acp_ipc::Result<T> {
    serde_json::from_value(v).map_err(|_| Error::InvalidRequest)
}
impl Service {
    async fn bound_root(
        &self,
        project: &str,
        permit: Arc<OwnedSemaphorePermit>,
    ) -> acp_ipc::Result<PathBuf> {
        let profiles = self.profiles.clone();
        let owned_project = project.to_owned();
        let job = tokio::task::spawn_blocking(move || {
            let _held = permit;
            let (store, roots) = profiles.read()?;
            let expected = roots.get(&owned_project).ok_or(Error::Unauthorized)?;
            let root = store
                .read_project(&owned_project)
                .map_err(|_| Error::NotFound)?
                .root
                .canonicalize()
                .map_err(|_| Error::UnsafeRuntime)?;
            if &root != expected {
                return Err(Error::UnsafeRuntime);
            }
            Ok(root)
        });
        let result = match tokio::time::timeout(Duration::from_secs(5), job).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(Error::Unavailable),
            Err(_) => Err(Error::Timeout),
        };
        if result.is_err() {
            self.sessions.invalidate_context(project);
        }
        result
    }
    async fn session(
        &self,
        access: Access,
        request: Request,
        permit: Arc<OwnedSemaphorePermit>,
    ) -> acp_ipc::Result<Value> {
        // Deny before parsing params, opening storage or disclosing session/config.
        if access != Access::Operator {
            return Err(Error::Unauthorized);
        }
        if !self.profiles.selected() {
            return Err(Error::Unavailable);
        }
        let p: acp_session::runtime::Params = decode(request.params)?;
        if p.session_api_version != acp_session::SESSION_API {
            return Err(Error::Version);
        }
        if !self.profiles.allowed(&p.project) {
            return Err(Error::Unauthorized);
        }
        match request.method.as_str() {
            "session.capabilities" | "session.list" | "session.get" | "session.context" => {
                if p.share_context.is_some()
                    || p.session_id.is_some()
                    || p.executable.is_some()
                    || p.confirmed.is_some()
                    || p.prompt.is_some()
                    || p.request_id.is_some()
                    || p.option_id.is_some()
                {
                    return Err(Error::InvalidRequest);
                }
                match request.method.as_str() {
                    "session.capabilities" => Ok(json!(self.sessions.capabilities())),
                    "session.context" => Ok(self.sessions.context(&p.project)),
                    "session.list" => Ok(json!(
                        self.sessions
                            .get(&p.project)
                            .into_iter()
                            .collect::<Vec<_>>()
                    )),
                    _ => Ok(json!({"session": self.sessions.get(&p.project)})),
                }
            }
            "session.start" => {
                let root = self.bound_root(&p.project, permit.clone()).await?;
                Ok(json!(self.sessions.start(p, root)?))
            }
            "session.prompt" | "session.cancel" | "session.stop" | "session.permission" => {
                if request.method == "session.prompt"
                    || (request.method == "session.permission" && p.option_id.is_some())
                {
                    self.bound_root(&p.project, permit.clone()).await?;
                }
                Ok(json!(self.sessions.control(&request.method, p).await?))
            }
            _ => Err(Error::Unsupported),
        }
    }
    fn validate_roots(&self, roots: &[PathBuf]) -> acp_ipc::Result<()> {
        acp_session::runtime::Supervisor::configured(self.agent_config.as_deref(), roots)?
            .with_context_bridge(
                self.context_mcp.as_deref(),
                self.profiles.location.clone(),
                self.grants.clone(),
                Duration::from_secs(self.context_lease_seconds),
                roots,
            )?;
        Ok(())
    }
    fn query(
        &self,
        authority: &acp_ipc::session_lease::Peer,
        request: Request,
    ) -> acp_ipc::Result<Value> {
        let access = authority.access();
        let session_bound = authority.is_session();
        let params = request.params;
        if request.method.starts_with("profile.")
            || matches!(
                request.method.as_str(),
                "project.check" | "project.add" | "project.remove"
            )
        {
            return self
                .profiles
                .handle(authority, &request.method, params, self);
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
                json!({"api_version":acp_ipc::API_VERSION,"service":"acp-daemon","version":env!("CARGO_PKG_VERSION"),"schema":schema,"ready":true,"profile_selected":selected,"worktree_fingerprint":"on-demand-acp-worktree-v1"}),
            );
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
        let (store, roots) = self.profiles.read().inspect_err(|_| {
            // Full Profile validation can refuse an unavailable registered root
            // before the per-project check below. Established session authority
            // must be revoked on this earlier refusal as well.
            if session_bound && let Access::Agent { project } = &access {
                self.sessions.invalidate_context(project);
            }
        })?;
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
            if session_bound {
                self.sessions.invalidate_context(&project);
            }
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
                if session_bound {
                    self.sessions.invalidate_context(&project);
                }
                return Err(Error::UnsafeRuntime);
            }
            prepared.finish(current.as_ref().ok_or(Error::UnsafeRuntime)?)
        };
        result.map_err(|e| match e {
            acp_core::Error::HandoffNotFound | acp_core::Error::UnknownProject => Error::NotFound,
            _ => Error::Unavailable,
        })
    }
}
impl profiles::Hooks for Service {
    fn validate_roots(&self, roots: &[PathBuf]) -> acp_ipc::Result<()> {
        Service::validate_roots(self, roots)
    }
    fn added(&self, name: &str) -> acp_ipc::Result<()> {
        let credential = self.lease.lock().unwrap().issue_agent(name)?;
        self.credentials.write().unwrap().push(credential);
        Ok(())
    }
    fn removed(&self, name: &str) {
        let _ = self.lease.lock().unwrap().revoke_agent(name);
        self.credentials
            .write()
            .unwrap()
            .retain(|c| !matches!(&c.access, Access::Agent { project } if project == name));
        let sessions = self.sessions.clone();
        let project = name.to_owned();
        // Revoke the context lease now; stop its session off this thread.
        sessions.invalidate_context(&project);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move { sessions.stop_project(&project).await });
        }
    }
}
async fn peer(mut stream: Stream, service: Service, permit: Arc<OwnedSemaphorePermit>) {
    // A snapshot: credentials added or revoked later apply to new connections.
    let credentials = service.credentials.read().unwrap().clone();
    let authority = match service.grants.accept(&mut stream, &credentials).await {
        Ok(a) => a,
        Err(e) => {
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
        let request: Request = match serde_json::from_value(value) {
            Ok(r) => r,
            Err(_) => {
                let _ =
                    write_frame(&mut stream, &Response::error(None, Error::InvalidRequest)).await;
                return;
            }
        };
        let id = request.id.clone();
        let error = if request.api_version != acp_ipc::API_VERSION {
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
        let result = if request.method.starts_with("session.") {
            service
                .session(access.clone(), request, permit.clone())
                .await
        } else {
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
        if result.is_ok() {
            authority.record(&method);
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
async fn run(args: Args) -> acp_ipc::Result<()> {
    let state_dir = state::StateDir::open(&args.state_dir)?;
    let grants = acp_ipc::session_lease::Registry::default();
    let profiles = Arc::new(profiles::Profiles::new(
        args.location.clone(),
        state_dir,
        grants.1.clone(),
    )?);
    #[cfg(test)]
    process_tests::install_checkpoint(&profiles);
    let roots = profiles.configured_roots();
    let sessions =
        acp_session::runtime::Supervisor::configured(args.agent_config.as_deref(), &roots)?;
    let mut lease = RuntimeLease::acquire(&args.location)?;
    grants.cleanup_stale(&args.location)?;
    grants.1.cleanup_stale(&args.location)?;
    let sessions = sessions.with_context_bridge(
        args.context_mcp.as_deref(),
        args.location.clone(),
        grants.clone(),
        Duration::from_secs(args.context_lease_seconds),
        &roots,
    )?;
    let mut listener = acp_ipc::transport::Listener::bind(&args.location).await?;
    let credentials = Arc::new(RwLock::new(lease.credentials(&profiles.names())?));
    let service = Service {
        profiles,
        lease: Arc::new(Mutex::new(lease)),
        credentials,
        agent_config: args.agent_config,
        context_mcp: args.context_mcp,
        context_lease_seconds: args.context_lease_seconds,
        sessions,
        grants,
    };
    // The Profile the operator asked to select at start, if any.
    {
        let service = service.clone();
        tokio::task::spawn_blocking(move || service.profiles.select_remembered(&service))
            .await
            .map_err(|_| Error::Unavailable)?;
    }
    let slots = Arc::new(Semaphore::new(MAX_PEERS));
    let mut tasks = tokio::task::JoinSet::new();
    let shutdown = stop();
    tokio::pin!(shutdown);
    eprintln!("acp-daemon ready");
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
    service.grants.1.close();
    let profiles = service.profiles.clone();
    let _ = tokio::task::spawn_blocking(move || profiles.disable()).await;
    service.sessions.shutdown().await;
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
        eprintln!("acp-daemon: {error}");
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
    state.save_projects(&set).unwrap();
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
        acp_ipc::transport::private_dir(&state, true).unwrap();
        let mut file = acp_ipc::transport::new_private(&state.join(state::PROJECTS)).unwrap();
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
            agent_config: None,
            context_mcp: None,
            context_lease_seconds: 300,
        };
        assert_eq!(run(args).await, Err(Error::InvalidRequest));
        assert!(
            !temp.path().join("runtime").exists(),
            "refused before the runtime"
        );
    }
}
