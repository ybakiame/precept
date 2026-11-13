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
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
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
        if let Some(db) = database {
            command.arg("--database").arg(db);
        }
        for name in ["alpha", "beta"] {
            command
                .arg("--allow-project")
                .arg(name)
                .arg("--allow-root")
                .arg(format!(
                    "{name}={}",
                    base.join(name).canonicalize().unwrap().to_str().unwrap()
                ));
        }
        if session {
            command
                .arg("--agent-config")
                .arg(base.join("agent.json"))
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
            version: 1,
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
            private_bytes(&temp.path().join("agent.json"),json!({"version":1,"programs":[{"id":"fake","executable":exe("acp-fake-agent"),"args":["mcp"],"env":{}}]}).to_string().as_bytes());
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
        expected: Expected::Absent,
    }
}
