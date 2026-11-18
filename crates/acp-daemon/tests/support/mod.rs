//! Owned synthetic roots and independent native daemons; never a default Profile.
use acp_ipc::{Client, Credential, Location};
use acp_write::{Binding, Command, Expected, Request, Scope};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command as Process, Stdio},
    thread,
    time::{Duration, Instant},
};
pub fn exe(name: &str) -> PathBuf {
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    // Standard cargo: the test binary lives in <target>/<profile>/deps/, so the
    // product binary is two levels up. This also honors a custom target dir.
    let current = std::env::current_exe().unwrap();
    if let Some(deps) = current.parent()
        && deps.file_name().is_some_and(|n| n == "deps")
        && let Some(target) = deps.parent()
    {
        let candidate = target.join(&file);
        if candidate.exists() {
            return candidate;
        }
    }
    // Shared-target cargo forks run the test binary outside deps/ but still write
    // product binaries to the workspace ./target/debug; resolve from the source tree.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug")
        .join(file)
}
pub fn private_bytes(path: &Path, bytes: &[u8]) {
    let mut f = acp_ipc::transport::new_private(path).unwrap();
    f.write_all(bytes).unwrap();
    f.sync_all().unwrap();
}
pub fn logical(path: &Path) -> Vec<u8> {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut hash = Sha256::new();
    for q in [
        "SELECT * FROM acp_profile",
        "SELECT * FROM projects ORDER BY name",
        "SELECT * FROM preferences ORDER BY id",
        "SELECT * FROM handoffs ORDER BY project,task_id",
        "SELECT * FROM acp_receipts ORDER BY commit_sequence",
        "SELECT * FROM acp_review_decisions ORDER BY id",
    ] {
        let mut s = db.prepare(q).unwrap();
        let n = s.column_count();
        for row in s
            .query_map([], |r| {
                (0..n)
                    .map(|i| r.get::<_, rusqlite::types::Value>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
        {
            hash.update(format!("{:?}", row.unwrap()));
        }
    }
    hash.finalize().to_vec()
}
/// The daemon's state for this test: the projects (`base/<name>`) and, like
/// an operator who asked for it, the Profile to select at start.
pub fn daemon_state(base: &Path, names: &[&str], profile: Option<&Path>) -> std::path::PathBuf {
    let dir = base.join("daemon-state");
    acp_ipc::transport::private_dir(&dir, true).unwrap();
    let projects: Vec<Value> = names
        .iter()
        .map(|name| json!({"name":name,"root":base.join(name).canonicalize().unwrap()}))
        .collect();
    replace_private(
        &dir.join("projects.json"),
        json!({"format":1,"projects":projects}),
    );
    match profile {
        Some(profile) => replace_private(
            &dir.join("startup.json"),
            json!({"format":1,"profile":profile}),
        ),
        None => {
            let _ = std::fs::remove_file(dir.join("startup.json"));
        }
    }
    dir
}
fn replace_private(path: &Path, value: Value) {
    let _ = std::fs::remove_file(path);
    let mut file = acp_ipc::transport::new_private(path).unwrap();
    std::io::Write::write_all(&mut file, value.to_string().as_bytes()).unwrap();
}
pub struct Daemon {
    pub child: Child,
    pub location: Location,
    pub runtime: tokio::runtime::Runtime,
}
impl Daemon {
    pub fn launch(base: &Path, id: &str, database: Option<&Path>, session: bool) -> Self {
        Self::launch_with_ttl(base, id, database, session, 300)
    }
    pub fn launch_with_ttl(
        base: &Path,
        id: &str,
        database: Option<&Path>,
        session: bool,
        ttl: u64,
    ) -> Self {
        daemon_state(base, &["alpha", "beta"], database);
        Self::start(base, id, session, ttl)
    }
    /// Starts again on the state the previous daemon left (no rewrite).
    #[allow(dead_code)]
    pub fn relaunch(base: &Path, id: &str) -> Self {
        Self::start(base, id, false, 300)
    }
    fn start(base: &Path, id: &str, session: bool, ttl: u64) -> Self {
        let runtime_dir = base.join(id);
        let endpoint = if cfg!(windows) {
            format!("s2-{}-{id}", base.file_name().unwrap().to_string_lossy())
        } else {
            runtime_dir
                .join("control.sock")
                .to_string_lossy()
                .into_owned()
        };
        let location = Location {
            runtime_dir,
            endpoint,
        };
        let mut command = Process::new(exe("acp-daemon"));
        command
            .arg("--runtime-dir")
            .arg(&location.runtime_dir)
            .arg("--endpoint")
            .arg(&location.endpoint);
        // The test's private base is the daemon user's home for scope checks.
        command
            .arg("--state-dir")
            .arg(base.join("daemon-state"))
            // The fixture base is the daemon user's home for scope checks
            // (the daemon reads %USERPROFILE% on Windows).
            .env("HOME", base)
            .env("USERPROFILE", base);
        if session {
            command
                .arg("--agent-config")
                .arg(base.join("agent.toml"))
                .arg("--context-mcp")
                .arg(exe("acp-mcp"))
                .arg("--context-lease-seconds")
                .arg(ttl.to_string())
                .env("ACP_TEST_PARENT_SENTINEL", "must-not-reach-child");
        }
        let child = command
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        Self {
            child,
            location,
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap(),
        }
    }
    pub fn ready(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Ok(c) = acp_ipc::load_credential(
                &self.location,
                &self.location.runtime_dir.join("operator.cap"),
            ) && self.call(&c, "daemon.health", json!({})).is_ok()
            {
                return;
            }
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "synthetic daemon refused startup"
            );
            assert!(
                Instant::now() < deadline,
                "synthetic daemon readiness timeout"
            );
            thread::sleep(Duration::from_millis(15));
        }
    }
    pub fn credential(&self, name: &str) -> Credential {
        acp_ipc::load_credential(&self.location, &self.location.runtime_dir.join(name)).unwrap()
    }
    pub fn client(&self, c: &Credential) -> acp_ipc::Result<Client> {
        self.runtime.block_on(Client::connect(&self.location, c))
    }
    pub fn call(&self, c: &Credential, method: &str, params: Value) -> acp_ipc::Result<Value> {
        self.runtime.block_on(async {
            Client::connect(&self.location, c)
                .await?
                .request(method, params)
                .await
        })
    }
    pub fn control(&self, method: &str, params: Value) -> acp_ipc::Result<Value> {
        self.call(&self.credential("operator.cap"), method, params)
    }
    pub fn enable(&self) -> Binding {
        let info = self.control("profile.info", json!({})).unwrap();
        let b = &info["binding"];
        let info = self
            .control(
                "profile.write.enable",
                json!({"profile_id":b["profile_id"],"generation":b["generation"],"confirm":true}),
            )
            .unwrap();
        serde_json::from_value(info["binding"].clone()).unwrap()
    }
    pub fn mutate(&self, b: &Binding, op: &str, command: Command) -> acp_ipc::Result<Value> {
        let request = Request {
            version: 2,
            binding: b.clone(),
            operation_id: op.into(),
            command,
        };
        self.call(
            &self.credential("operator-write.cap"),
            "profile.mutate",
            serde_json::to_value(request).unwrap(),
        )
    }
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Drop for Daemon {
    fn drop(&mut self) {
        self.kill();
    }
}
pub struct Fixture {
    pub daemon: Daemon,
    pub db: PathBuf,
    pub temp: tempfile::TempDir,
}
impl Fixture {
    pub fn unselected(session: bool) -> Self {
        let temp = tempfile::Builder::new()
            .prefix("acp-s2-")
            .tempdir()
            .unwrap();
        for name in ["alpha", "beta"] {
            fs::create_dir(temp.path().join(name)).unwrap();
            fs::write(
                temp.path().join(name).join("AGENTS.md"),
                "Synthetic contract.\n",
            )
            .unwrap();
        }
        acp_ipc::transport::private_dir(&temp.path().join("state"), true).unwrap();
        if session {
            private_bytes(&temp.path().join("agent.toml"),toml::to_string(&json!({"version":1,"programs":[{"id":"fake","executable":exe("acp-fake-agent"),"args":["mcp"],"env":{}}]})).unwrap().as_bytes());
        }
        let mut daemon = Daemon::launch(temp.path(), "run-a", None, session);
        daemon.ready();
        let db = temp.path().join("state/profile.sqlite3");
        Self { daemon, db, temp }
    }
    pub fn created(session: bool) -> Self {
        let f = Self::unselected(session);
        f.daemon
            .control("profile.create", json!({"path":f.db}))
            .unwrap();
        f
    }
    pub fn register(&self, b: &Binding, name: &str) -> Value {
        self.daemon
            .mutate(
                b,
                &format!("register-{name}"),
                Command::RegisterProject {
                    name: name.into(),
                    root: self
                        .temp
                        .path()
                        .join(name)
                        .canonicalize()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .into(),
                    expected: Expected::Absent,
                },
            )
            .unwrap()
    }
}
pub fn suggest() -> Command {
    Command::SuggestPreference {
        scope: Scope::Global,
        key: "synthetic-key".into(),
        value: "synthetic-value".into(),
        source: "synthetic-test".into(),
        origin: acp_core::operator::Origin::AgentReported,
        expected: Expected::Absent,
    }
}
