use acp_core::{ContextResolver, documents::FileDocuments};
use acp_ipc::{
    Access, Error, Location, MAX_PEERS, MAX_REQUESTS, Request, Response, RuntimeLease, Stream,
    accept_handshake, read_frame, write_frame,
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
}
#[derive(Clone)]
struct Service {
    database: PathBuf,
    projects: Vec<String>,
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
    fn query(&self, access: Access, request: Request) -> acp_ipc::Result<Value> {
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
    let access = match accept_handshake(&mut stream, &credentials).await {
        Ok(a) => a,
        Err(e) => {
            let _ = write_frame(&mut stream, &Response::error(None, e)).await;
            return;
        }
    };
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
        let service = service.clone();
        let access = access.clone();
        let held = permit.clone();
        // The lease stays with a timed-out blocking job, preventing unbounded queued work.
        let job = tokio::task::spawn_blocking(move || {
            let _held = held;
            service.query(access, request)
        });
        let result = match tokio::time::timeout(Duration::from_secs(5), job).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err(Error::Unavailable),
            Err(_) => Err(Error::Timeout),
        };
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
    for project in &args.projects {
        let project = store.read_project(project).map_err(|_| Error::NotFound)?;
        let root = project
            .root
            .canonicalize()
            .map_err(|_| Error::UnsafeRuntime)?;
        if runtime.starts_with(&root) || root.starts_with(&runtime) {
            return Err(Error::UnsafeRuntime);
        }
    }
    drop(store);
    let mut lease = RuntimeLease::acquire(&args.location)?;
    let mut listener = acp_ipc::transport::Listener::bind(&args.location).await?;
    let credentials = Arc::new(lease.credentials(&args.projects)?);
    let service = Service {
        database: args.database,
        projects: args.projects,
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
        };
        assert_eq!(run(args).await, Err(Error::InvalidRequest));
    }
}
