//! Bounded, capability-scoped local IPC. No TCP or data mutation API.
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use subtle::ConstantTimeEq;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
pub mod operator;
pub mod peer;
pub mod transport;
pub mod write_grant;
pub const API_VERSION: u32 = 2;
pub const MAX_FRAME: usize = 1024 * 1024;
pub const MAX_PEERS: usize = 16;
pub const MAX_REQUESTS: usize = 32;
pub const FRAME_TIMEOUT: Duration = Duration::from_secs(2);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Error {
    InvalidRequest,
    InvalidWorkState { field: String, reason: String },
    Version,
    Unauthorized,
    Unsupported,
    NotFound,
    Unavailable,
    TooLarge,
    Timeout,
    Busy,
    UnsafeRuntime,
    EndpointCollision,
    RestartRequired,
    Conflict,
    ReviewMismatch,
    InvalidTransition,
    IdempotencyConflict,
    ScopeMismatch,
    OwnerMismatch,
    StaleGeneration,
    Capacity,
    StorageFull,
    OutcomeUnknown,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Args)]
pub struct Location {
    #[arg(long)]
    pub runtime_dir: PathBuf,
    #[arg(long)]
    pub endpoint: String,
}
impl Location {
    pub fn root(&self) -> Result<PathBuf> {
        if !self.runtime_dir.is_absolute() {
            return Err(Error::UnsafeRuntime);
        }
        transport::private_dir(&self.runtime_dir, false)?;
        let root = self
            .runtime_dir
            .canonicalize()
            .map_err(|_| Error::UnsafeRuntime)?;
        // Credentials never belong in a project checkout: any Git checkout,
        // worktree or submodule (`.git` directory or file) at or above the
        // runtime refuses, including this product's own source checkout. No
        // build-host path is compiled in. Managed temp mount roots can
        // themselves carry platform Git metadata; descendants that are actual
        // checkouts remain forbidden.
        let temp = std::env::temp_dir()
            .canonicalize()
            .map_err(|_| Error::UnsafeRuntime)?;
        if root
            .ancestors()
            .any(|p| p != temp && p.join(".git").exists())
        {
            return Err(Error::UnsafeRuntime);
        }
        Ok(root)
    }
    pub fn credential_path(&self, project: Option<&str>) -> Result<PathBuf> {
        let root = self.root()?;
        match project {
            None => Ok(root.join("operator.cap")),
            Some(p) if valid_project(p) => Ok(root.join(format!("agent-{p}.cap"))),
            _ => Err(Error::InvalidRequest),
        }
    }
}
pub fn valid_project(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 64
        && p != "."
        && p != ".."
        && p.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase", deny_unknown_fields)]
pub enum Access {
    Operator,
    #[serde(rename = "operator_write")]
    OperatorWrite {
        profile_id: String,
        generation: String,
        owner_epoch: i64,
    },
    Agent {
        project: String,
    },
}
// Deliberately no Debug: never log a token-bearing DTO.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credential {
    format: u32,
    pub access: Access,
    token: [u8; 32],
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Hello {
    api_version: u32,
    credential: [u8; 32],
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub api_version: u32,
    pub id: String,
    pub method: String,
    pub params: Value,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub api_version: u32,
    pub id: Option<String>,
    pub result: Option<Value>,
    pub error: Option<Error>,
}
impl Response {
    pub fn ok(id: String, result: Value) -> Self {
        Self {
            api_version: API_VERSION,
            id: Some(id),
            result: Some(result),
            error: None,
        }
    }
    pub fn error(id: Option<String>, error: Error) -> Self {
        Self {
            api_version: API_VERSION,
            id,
            result: None,
            error: Some(error),
        }
    }
}
pub trait LocalStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> LocalStream for T {}
pub type Stream = Box<dyn LocalStream>;
pub async fn read_frame<S: AsyncRead + Unpin>(stream: &mut S) -> Result<Value> {
    read_with_deadline(stream, FRAME_TIMEOUT).await
}
async fn read_with_deadline<S: AsyncRead + Unpin>(
    stream: &mut S,
    deadline: Duration,
) -> Result<Value> {
    tokio::time::timeout(deadline, async {
        let n = stream.read_u32().await.map_err(|_| Error::InvalidRequest)? as usize;
        if n == 0 || n > MAX_FRAME {
            return Err(Error::TooLarge);
        }
        let mut raw = vec![0; n];
        stream
            .read_exact(&mut raw)
            .await
            .map_err(|_| Error::InvalidRequest)?;
        serde_json::from_slice(&raw).map_err(|_| Error::InvalidRequest)
    })
    .await
    .map_err(|_| Error::Timeout)?
}
pub async fn write_frame<S: AsyncWrite + Unpin, T: Serialize>(
    stream: &mut S,
    value: &T,
) -> Result<()> {
    let raw = serde_json::to_vec(value).map_err(|_| Error::InvalidRequest)?;
    if raw.len() > MAX_FRAME {
        return Err(Error::TooLarge);
    }
    tokio::time::timeout(FRAME_TIMEOUT, async {
        stream
            .write_u32(raw.len() as u32)
            .await
            .map_err(|_| Error::Unavailable)?;
        stream
            .write_all(&raw)
            .await
            .map_err(|_| Error::Unavailable)?;
        stream.flush().await.map_err(|_| Error::Unavailable)
    })
    .await
    .map_err(|_| Error::Timeout)?
}
pub async fn accept_handshake(stream: &mut Stream, credentials: &[Credential]) -> Result<Access> {
    let hello: Hello =
        serde_json::from_value(read_frame(stream).await?).map_err(|_| Error::Unauthorized)?;
    if hello.api_version != API_VERSION {
        return Err(Error::Version);
    }
    let access = credentials
        .iter()
        .find(|c| bool::from(c.token.ct_eq(&hello.credential)))
        .map(|c| c.access.clone())
        .ok_or(Error::Unauthorized)?;
    write_frame(
        stream,
        &json!({"api_version":API_VERSION,"ready":true,"access":access}),
    )
    .await?;
    Ok(access)
}
pub fn load_credential(location: &Location, path: &Path) -> Result<Credential> {
    let root = location.root()?;
    if path.parent().and_then(|p| p.canonicalize().ok()).as_ref() != Some(&root) {
        return Err(Error::UnsafeRuntime);
    }
    transport::private_file(path)?;
    let mut file = transport::open_private(path, false)?;
    if file.metadata().map_err(|_| Error::UnsafeRuntime)?.len() > 4096 {
        return Err(Error::Unauthorized);
    }
    let mut raw = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(4097)
        .read_to_end(&mut raw)
        .map_err(|_| Error::Unauthorized)?;
    let c: Credential = serde_json::from_slice(&raw).map_err(|_| Error::Unauthorized)?;
    let valid_format = match (&c.access, c.format) {
        (Access::Operator | Access::Agent { .. }, 1) => true,
        (access, 3) => write_grant::binding_valid(access),
        _ => false,
    };
    if !valid_format || matches!(&c.access,Access::Agent{project} if !valid_project(project)) {
        return Err(Error::Unauthorized);
    }
    Ok(c)
}
/// Per-launch private credential rotation, protected by a process-death-safe OS lease.
pub struct RuntimeLease {
    file: File,
    location: Location,
    credentials: Vec<(PathBuf, Credential)>,
}
impl RuntimeLease {
    pub fn acquire(location: &Location) -> Result<Self> {
        if !location.runtime_dir.is_absolute() {
            return Err(Error::UnsafeRuntime);
        }
        transport::private_dir(&location.runtime_dir, true)?;
        let root = location.root()?;
        let path = root.join("daemon.lock");
        let file = if path.exists() {
            transport::open_private(&path, true)?
        } else {
            transport::new_private(&path)?
        };
        file.try_lock().map_err(|_| Error::EndpointCollision)?;
        Ok(Self {
            file,
            location: location.clone(),
            credentials: Vec::new(),
        })
    }
    pub fn credentials(&mut self, projects: &[String]) -> Result<Vec<Credential>> {
        self.issue(Access::Operator)?;
        for project in projects {
            self.issue(Access::Agent {
                project: project.clone(),
            })?;
        }
        Ok(self.current())
    }
    /// Every credential this lease currently publishes.
    pub fn current(&self) -> Vec<Credential> {
        self.credentials.iter().map(|(_, c)| c.clone()).collect()
    }
    /// Publishes a fresh agent credential for a project added at run time.
    pub fn issue_agent(&mut self, project: &str) -> Result<Credential> {
        if !valid_project(project) {
            return Err(Error::InvalidRequest);
        }
        self.issue(Access::Agent {
            project: project.into(),
        })
    }
    /// Withdraws a project's agent credential (the project was removed): the
    /// file goes only if it is still this lease's token; new connections with
    /// it are refused at once.
    pub fn revoke_agent(&mut self, project: &str) -> Result<()> {
        let access = Access::Agent {
            project: project.into(),
        };
        let Some(index) = self
            .credentials
            .iter()
            .position(|(_, c)| c.access == access)
        else {
            return Ok(());
        };
        let (path, c) = self.credentials.remove(index);
        if load_credential(&self.location, &path)
            .is_ok_and(|disk| bool::from(disk.token.ct_eq(&c.token)))
        {
            fs::remove_file(&path).map_err(|_| Error::Unavailable)?;
        }
        Ok(())
    }
    fn issue(&mut self, access: Access) -> Result<Credential> {
        if self.credentials.iter().any(|(_, c)| c.access == access) {
            return Err(Error::InvalidRequest);
        }
        let path = self.location.credential_path(match &access {
            Access::Operator => None,
            Access::Agent { project } => Some(project),
            Access::OperatorWrite { .. } => return Err(Error::Unauthorized),
        })?;
        if path.exists() {
            let _ = load_credential(&self.location, &path)?;
        }
        let mut token = [0; 32];
        getrandom::fill(&mut token).map_err(|_| Error::Unavailable)?;
        let c = Credential {
            format: 1,
            access,
            token,
        };
        let pending = path.with_extension("pending");
        let mut file = transport::new_private(&pending)?;
        file.write_all(&serde_json::to_vec(&c).map_err(|_| Error::Unavailable)?)
            .map_err(|_| Error::Unavailable)?;
        file.sync_all().map_err(|_| Error::Unavailable)?;
        drop(file);
        fs::rename(pending, &path).map_err(|_| Error::Unavailable)?;
        self.credentials.push((path, c.clone()));
        Ok(c)
    }
}
impl Drop for RuntimeLease {
    fn drop(&mut self) {
        for (path, c) in &self.credentials {
            if load_credential(&self.location, path)
                .is_ok_and(|disk| bool::from(disk.token.ct_eq(&c.token)))
            {
                let _ = fs::remove_file(path);
            }
        }
        let _ = &self.file; // Lease remains held until credential cleanup has finished.
    }
}
pub struct Client {
    stream: Stream,
    next: u64,
    pub access: Access,
}
impl Client {
    pub async fn connect(location: &Location, c: &Credential) -> Result<Self> {
        let mut stream = tokio::time::timeout(FRAME_TIMEOUT, transport::connect(location))
            .await
            .map_err(|_| Error::Timeout)??;
        write_frame(
            &mut stream,
            &Hello {
                api_version: API_VERSION,
                credential: c.token,
            },
        )
        .await?;
        let ack = read_frame(&mut stream).await?;
        if ack.get("api_version") != Some(&json!(API_VERSION))
            || ack.get("ready") != Some(&json!(true))
        {
            return Err(Error::Unauthorized);
        }
        let access: Access =
            serde_json::from_value(ack.get("access").cloned().ok_or(Error::Unauthorized)?)
                .map_err(|_| Error::Unauthorized)?;
        if access != c.access {
            return Err(Error::Unauthorized);
        }
        Ok(Self {
            stream,
            next: 0,
            access,
        })
    }
    pub async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        self.next += 1;
        let id = self.next.to_string();
        let transport_error = |e| {
            if matches!(
                method,
                "profile.mutate" | "sessions.handoff" | "profile.sync.apply"
            ) {
                Error::OutcomeUnknown
            } else {
                e
            }
        };
        write_frame(
            &mut self.stream,
            &Request {
                api_version: API_VERSION,
                id: id.clone(),
                method: method.into(),
                params,
            },
        )
        .await
        .map_err(transport_error)?;
        // The server limits computation too; the response frame has its own absolute deadline.
        let value = read_with_deadline(&mut self.stream, Duration::from_secs(7))
            .await
            .map_err(transport_error)?;
        let response: Response =
            serde_json::from_value(value).map_err(|_| transport_error(Error::InvalidRequest))?;
        if response.api_version != API_VERSION || response.id != Some(id) {
            return Err(transport_error(Error::InvalidRequest));
        }
        if let Some(e) = response.error {
            return Err(e);
        }
        response
            .result
            .ok_or_else(|| transport_error(Error::InvalidRequest))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn runtime_below(parent: &Path) -> Location {
        let runtime_dir = parent.join("runtime");
        transport::private_dir(&runtime_dir, true).unwrap();
        Location {
            runtime_dir,
            endpoint: "checkout-guard".into(),
        }
    }
    #[test]
    fn runtime_inside_any_git_checkout_or_worktree_refuses() {
        let temp = tempfile::Builder::new()
            .prefix("precept-checkout-guard-")
            .tempdir()
            .unwrap();
        // Plain private directory below the temp root is accepted.
        let plain = temp.path().join("plain");
        fs::create_dir(&plain).unwrap();
        assert!(runtime_below(&plain).root().is_ok());
        // A checkout `.git` directory, at any ancestor depth, refuses.
        let repo = temp.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir(repo.join("nested")).unwrap();
        assert_eq!(runtime_below(&repo).root(), Err(Error::UnsafeRuntime));
        assert_eq!(
            runtime_below(&repo.join("nested")).root(),
            Err(Error::UnsafeRuntime)
        );
        // A linked worktree or submodule has a `.git` FILE; it refuses too.
        let worktree = temp.path().join("worktree");
        fs::create_dir(&worktree).unwrap();
        fs::write(worktree.join(".git"), "gitdir: elsewhere\n").unwrap();
        assert_eq!(runtime_below(&worktree).root(), Err(Error::UnsafeRuntime));
        // Relative runtime paths refuse before any filesystem inspection.
        let relative = Location {
            runtime_dir: PathBuf::from("runtime"),
            endpoint: "checkout-guard".into(),
        };
        assert_eq!(relative.root(), Err(Error::UnsafeRuntime));
    }
    #[tokio::test]
    async fn framing_rejects_oversize_and_malformed() {
        let (mut a, mut b) = tokio::io::duplex(64);
        a.write_u32(MAX_FRAME as u32 + 1).await.unwrap();
        assert_eq!(read_frame(&mut b).await.unwrap_err(), Error::TooLarge);
        a.write_u32(1).await.unwrap();
        a.write_all(b"[").await.unwrap();
        assert_eq!(read_frame(&mut b).await.unwrap_err(), Error::InvalidRequest);
    }
    #[tokio::test]
    async fn lost_mutation_reply_is_unknown_but_server_denial_stays_precise() {
        let temp = tempfile::Builder::new()
            .prefix("precept-s3-ipc-")
            .tempdir()
            .unwrap();
        let location = Location {
            runtime_dir: temp.path().join("runtime"),
            endpoint: if cfg!(windows) {
                format!("s3-{}", temp.path().file_name().unwrap().to_string_lossy())
            } else {
                temp.path()
                    .join("runtime/control.sock")
                    .to_str()
                    .unwrap()
                    .into()
            },
        };
        let mut lease = RuntimeLease::acquire(&location).unwrap();
        let credentials = lease.credentials(&[]).unwrap();
        let mut listener = transport::Listener::bind(&location).await.unwrap();
        let cap = load_credential(&location, &location.credential_path(None).unwrap()).unwrap();
        let server = tokio::spawn(async move {
            for method in ["profile.mutate", "sessions.handoff", "profile.sync.apply"] {
                for denied in [true, false] {
                    let mut stream = listener.accept().await.unwrap();
                    accept_handshake(&mut stream, &credentials).await.unwrap();
                    let request: Request =
                        serde_json::from_value(read_frame(&mut stream).await.unwrap()).unwrap();
                    assert_eq!(request.method, method);
                    if denied {
                        write_frame(
                            &mut stream,
                            &Response::error(Some(request.id), Error::InvalidRequest),
                        )
                        .await
                        .unwrap();
                    }
                    // Second connection consumes submission, then loses its reply.
                }
            }
        });
        for method in ["profile.mutate", "sessions.handoff", "profile.sync.apply"] {
            let mut client = Client::connect(&location, &cap).await.unwrap();
            assert_eq!(
                client.request(method, json!({})).await,
                Err(Error::InvalidRequest)
            );
            drop(client);
            let mut client = Client::connect(&location, &cap).await.unwrap();
            assert_eq!(
                client.request(method, json!({})).await,
                Err(Error::OutcomeUnknown)
            );
        }
        server.await.unwrap();
    }
    #[test]
    fn labels_are_paths_never() {
        for p in ["", "../beta", "a/b", "a\\b", ".."] {
            assert!(!valid_project(p));
        }
        assert!(valid_project("alpha-1"));
    }
}
