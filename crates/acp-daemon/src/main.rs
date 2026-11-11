use acp_core::{ContextResolver, documents::FileDocuments};
use acp_ipc::{
    Access, Error, Location, MAX_PEERS, MAX_REQUESTS, Request, Response, RuntimeLease, Stream,
    read_frame, write_frame,
};
use acp_store::SqliteStore;
use clap::Parser;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
mod worktree;
#[derive(Parser)]
struct Args {
    #[command(flatten)]
    location: Location,
    #[arg(long)]
    database: PathBuf,
    #[arg(long = "allow-project", required = true)]
    projects: Vec<String>,
    /// Explicit private operator configuration; absent disables execution.
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
    database: PathBuf,
    projects: Vec<String>,
    roots: std::collections::BTreeMap<String, PathBuf>,
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
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentContext {
    #[serde(default)]
    include_documents: bool,
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
        let database = self.database.clone();
        let project = project.to_owned();
        let expected = self
            .roots
            .get(&project)
            .cloned()
            .ok_or(Error::Unauthorized)?;
        let job = tokio::task::spawn_blocking(move || {
            let _held = permit;
            let store = SqliteStore::open(&database).map_err(|_| Error::Unavailable)?;
            let root = store
                .read_project(&project)
                .map_err(|_| Error::NotFound)?
                .root
                .canonicalize()
                .map_err(|_| Error::UnsafeRuntime)?;
            if root != expected {
                return Err(Error::UnsafeRuntime);
            }
            Ok(root)
        });
        tokio::time::timeout(Duration::from_secs(5), job)
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(|_| Error::Unavailable)?
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
        let p: acp_session::runtime::Params = decode(request.params)?;
        if p.session_api_version != acp_session::SESSION_API {
            return Err(Error::Version);
        }
        if !self.projects.contains(&p.project) {
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
    fn query(
        &self,
        access: Access,
        request: Request,
        session_bound: bool,
    ) -> acp_ipc::Result<Value> {
        let params = request.params;
        let store = SqliteStore::open(&self.database).map_err(|_| Error::Unavailable)?;
        if request.method == "daemon.health" {
            let _: Empty = decode(params)?;
            return Ok(
                json!({"api_version":1,"service":"acp-daemon","version":env!("CARGO_PKG_VERSION"),"schema":format!("{:?}",store.schema().map_err(|_|Error::Unavailable)?),"ready":true,"worktree_fingerprint":"on-demand-acp-worktree-v1"}),
            );
        }
        if request.method == "project.list" {
            let _: Empty = decode(params)?;
            if access != Access::Operator {
                return Err(Error::Unauthorized);
            }
            return Ok(json!(
                self.projects
                    .iter()
                    .map(|p| json!({"name":p}))
                    .collect::<Vec<_>>()
            ));
        }
        let (project, include, task) = match (request.method.as_str(), access) {
            ("context.get", Access::Operator) => {
                let p: OperatorContext = decode(params)?;
                (p.project, p.include_documents, None)
            }
            ("context.get", Access::Agent { project }) => {
                let p: AgentContext = decode(params)?;
                (project, p.include_documents, None)
            }
            ("handoff.get", Access::Operator) => {
                let p: OperatorHandoff = decode(params)?;
                (p.project, false, Some(p.task_id))
            }
            ("handoff.get", Access::Agent { project }) => {
                let p: AgentHandoff = decode(params)?;
                (project, false, Some(p.task_id))
            }
            _ => return Err(Error::Unsupported),
        };
        if !self.projects.contains(&project) {
            return Err(Error::Unauthorized);
        }
        if task.as_ref().is_some_and(|s| s.chars().count() > 120) {
            return Err(Error::InvalidRequest);
        }
        if session_bound {
            let root = store
                .read_project(&project)
                .ok()
                .and_then(|p| p.root.canonicalize().ok());
            if root.as_ref() != self.roots.get(&project) {
                self.sessions.invalidate_context(&project);
                return Err(Error::UnsafeRuntime);
            }
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
            resolver.context(include)
        };
        result.map_err(|e| match e {
            acp_core::Error::HandoffNotFound | acp_core::Error::UnknownProject => Error::NotFound,
            _ => Error::Unavailable,
        })
    }
}
async fn peer(
    mut stream: Stream,
    credentials: Arc<Vec<acp_ipc::Credential>>,
    service: Service,
    permit: Arc<OwnedSemaphorePermit>,
) {
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
        let error = if request.api_version != 1 {
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
            let access = access.clone();
            let held = permit.clone();
            let auth = authority.clone();
            // A timed-out blocking job retains its admission slot.
            let job = tokio::task::spawn_blocking(move || {
                let _held = held;
                auth.check()?;
                service.query(access, request, auth.is_session())
            });
            match tokio::time::timeout(Duration::from_secs(5), job).await {
                Ok(Ok(r)) => r,
                Ok(Err(_)) => Err(Error::Unavailable),
                Err(_) => Err(Error::Timeout),
            }
        };
        let result = authority.check().and(result);
        if result.is_ok() {
            authority.record(&method);
        }
        let close = result == Err(Error::Timeout);
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
    if args.projects.is_empty()
        || args.projects.len() > 64
        || args.projects.iter().any(|p| !acp_ipc::valid_project(p))
        || args
            .projects
            .iter()
            .map(|p| p.to_ascii_lowercase())
            .collect::<HashSet<_>>()
            .len()
            != args.projects.len()
    {
        return Err(Error::InvalidRequest);
    }
    let store = SqliteStore::open(&args.database).map_err(|_| Error::Unavailable)?;
    // Control-file names must never alias storage, even under an explicit but
    // mistaken operator configuration (for example a database named daemon.lock).
    let runtime = if args.location.runtime_dir.exists() {
        args.location
            .runtime_dir
            .canonicalize()
            .map_err(|_| Error::UnsafeRuntime)?
    } else {
        args.location
            .runtime_dir
            .parent()
            .ok_or(Error::UnsafeRuntime)?
            .canonicalize()
            .map_err(|_| Error::UnsafeRuntime)?
            .join(
                args.location
                    .runtime_dir
                    .file_name()
                    .ok_or(Error::UnsafeRuntime)?,
            )
    };
    if args
        .database
        .canonicalize()
        .map_err(|_| Error::Unavailable)?
        .starts_with(&runtime)
    {
        return Err(Error::UnsafeRuntime);
    }
    // Registered projects need not have Git metadata. Resolve both sides before
    // any control files are created, including existing symlink components.
    let mut roots = std::collections::BTreeMap::new();
    for project in &args.projects {
        let project = store.read_project(project).map_err(|_| Error::NotFound)?;
        let root = project
            .root
            .canonicalize()
            .map_err(|_| Error::UnsafeRuntime)?;
        roots.insert(project.name.clone(), root.clone());
        if runtime.starts_with(&root) || root.starts_with(&runtime) {
            return Err(Error::UnsafeRuntime);
        }
    }
    let sessions = acp_session::runtime::Supervisor::configured(
        args.agent_config.as_deref(),
        &roots.values().cloned().collect::<Vec<_>>(),
    )?;
    drop(store);
    let mut lease = RuntimeLease::acquire(&args.location)?;
    let grants = acp_ipc::session_lease::Registry::default();
    grants.cleanup_stale(&args.location)?;
    let sessions = sessions.with_context_bridge(
        args.context_mcp.as_deref(),
        args.location.clone(),
        grants.clone(),
        Duration::from_secs(args.context_lease_seconds),
        &roots.values().cloned().collect::<Vec<_>>(),
    )?;
    let mut listener = acp_ipc::transport::Listener::bind(&args.location).await?;
    let credentials = Arc::new(lease.credentials(&args.projects)?);
    let service = Service {
        database: args.database,
        projects: args.projects,
        roots,
        sessions,
        grants,
    };
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
                    Ok(stream)=>{if let Ok(slot)=slots.clone().try_acquire_owned(){tasks.spawn(peer(stream,credentials.clone(),service.clone(),Arc::new(slot)));}},
                    Err(Error::Unauthorized)=>(),
                    Err(_)=>tokio::time::sleep(Duration::from_millis(10)).await,
                }
            }
        }
    }
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    service.sessions.shutdown().await;
    drop(listener);
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

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn case_insensitive_capability_filename_collisions_fail_before_storage_open() {
        let args = Args {
            location: Location {
                runtime_dir: PathBuf::new(),
                endpoint: String::new(),
            },
            database: PathBuf::new(),
            projects: vec!["alpha".into(), "ALPHA".into()],
            agent_config: None,
            context_mcp: None,
            context_lease_seconds: 300,
        };
        assert_eq!(run(args).await, Err(Error::InvalidRequest));
    }
}
