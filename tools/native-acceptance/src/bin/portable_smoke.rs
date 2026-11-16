//! Windows-only packaged process smoke. Not shipped; never opens user state.
#[allow(dead_code)]
#[path = "../desktop.rs"]
mod desktop;
#[allow(dead_code)]
#[path = "../screen.rs"]
mod screen;
#[allow(dead_code)]
#[path = "../tty.rs"]
mod tty;
use acp_ipc::{Access, Client, Error, Location, load_credential, transport};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn command(binary: &Path) -> Command {
    let mut cmd = Command::new(binary);
    // No inherited preview, endpoint, external project or agent auth hints.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("ACP_") {
            cmd.env_remove(key);
        }
    }
    cmd
}
fn bounded(mut cmd: Command) -> (bool, Value) {
    let mut child = Owned(
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(bytes.len() <= 1024 * 1024, "bounded synthetic CLI output");
        bytes
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    let success = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status.success();
        }
        assert!(Instant::now() < deadline, "owned process deadline");
        thread::sleep(Duration::from_millis(25));
    };
    let bytes = reader.join().unwrap();
    (
        success,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
struct Fixture {
    // Preserve on failure and success for private failure/retention evidence.
    root: PathBuf,
    bins: PathBuf,
    location: Location,
    daemon: Owned,
}
impl Fixture {
    fn start(bins: PathBuf) -> Self {
        let parent = tempfile::Builder::new()
            .prefix("acp-m5-zip-")
            .tempdir()
            .unwrap()
            .keep();
        let root = parent.join("private");
        transport::private_dir(&root, true).unwrap();
        transport::private_dir(&root.join("state"), true).unwrap();
        fs::create_dir(root.join("synthetic")).unwrap();
        fs::write(
            root.join("synthetic/AGENTS.md"),
            "Synthetic archive smoke only.\n",
        )
        .unwrap();
        fs::write(root.join("state/auth-retain.txt"), "SYNTHETIC RETAIN\n").unwrap();
        fs::write(root.join("state/unknown-retain.txt"), "SYNTHETIC RETAIN\n").unwrap();
        let suffix = root.file_name().unwrap().to_str().unwrap();
        let location = Location {
            runtime_dir: root.join("runtime"),
            endpoint: suffix.to_owned(),
        };
        let daemon = Self::daemon(&bins, &root, &location);
        let f = Self {
            root,
            bins,
            location,
            daemon,
        };
        let end = Instant::now() + Duration::from_secs(15);
        while !f.query(&["profile", "info"]).0 {
            assert!(Instant::now() < end, "packaged daemon readiness");
            thread::sleep(Duration::from_millis(50));
        }
        f
    }
    fn daemon(bins: &Path, root: &Path, location: &Location) -> Owned {
        let mut cmd = command(&bins.join("acp-daemon.exe"));
        cmd.arg("--runtime-dir")
            .arg(&location.runtime_dir)
            .args([
                "--endpoint",
                &location.endpoint,
                "--allow-project",
                "synthetic",
                "--allow-root",
            ])
            .arg(format!(
                "synthetic={}",
                root.join("synthetic").to_str().unwrap()
            ))
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // Ctrl+Break is sent only to this exact newly owned process group.
            cmd.creation_flags(0x200);
        }
        Owned(cmd.spawn().unwrap())
    }
    fn query(&self, args: &[&str]) -> (bool, Value) {
        let mut cmd = command(&self.bins.join("acp-query.exe"));
        cmd.arg("--runtime-dir")
            .arg(&self.location.runtime_dir)
            .args(["--endpoint", &self.location.endpoint])
            .args(args);
        bounded(cmd)
    }
    fn ok(&self, args: &[&str]) -> Value {
        let (ok, v) = self.query(args);
        assert!(
            ok,
            "synthetic operator request refused: {} {}",
            args[0], v["error"]
        );
        v
    }
    fn mutation(&self, args: &[&str]) -> Value {
        let binding = self.ok(&["profile", "info"])["binding"].clone();
        let epoch = binding["owner_epoch"].to_string();
        let id = acp_ipc::operator::operation_id().unwrap();
        let mut all = args.to_vec();
        all.extend([
            "--profile-id",
            binding["profile_id"].as_str().unwrap(),
            "--generation",
            binding["generation"].as_str().unwrap(),
            "--owner-epoch",
            &epoch,
            "--operation-id",
            &id,
        ]);
        self.ok(&all)
    }
    fn stop(&mut self) {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent};
            assert_ne!(
                GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, self.daemon.0.id()),
                0,
                "owned graceful stop signal"
            );
        }
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.daemon.0.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < end, "graceful daemon exit deadline");
            thread::sleep(Duration::from_millis(30));
        }
        assert!(!self.location.runtime_dir.join("operator.cap").exists());
        assert!(
            !self
                .location
                .runtime_dir
                .join("agent-synthetic.cap")
                .exists()
        );
        assert!(
            !self
                .location
                .runtime_dir
                .join("operator-write.cap")
                .exists()
        );
        // The stable coordination file is retained; acquiring it proves release.
        let _lease = acp_ipc::RuntimeLease::acquire(&self.location).unwrap();
    }
}
struct Mcp {
    child: Owned,
    replies: mpsc::Receiver<Value>,
}
impl Mcp {
    fn start(f: &Fixture) -> Self {
        let mut cmd = command(&f.bins.join("acp-mcp.exe"));
        cmd.arg("--runtime-dir")
            .arg(&f.location.runtime_dir)
            .args([
                "--endpoint",
                &f.location.endpoint,
                "--project",
                "synthetic",
                "--capability-file",
            ])
            .arg(f.location.runtime_dir.join("agent-synthetic.cap"));
        let mut child = Owned(
            cmd.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let stdout = child.0.stdout.take().unwrap();
        let (tx, replies) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout.take(1024 * 1024)).lines() {
                let Ok(line) = line else { break };
                if line.len() > 128 * 1024 {
                    break;
                }
                let Ok(value) = serde_json::from_str(&line) else {
                    break;
                };
                if tx.send(value).is_err() {
                    break;
                }
            }
        });
        Self { child, replies }
    }
    fn request(&mut self, method: &str, mut params: Value) -> Value {
        params["_meta"] = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28"});
        writeln!(
            self.child.0.stdin.as_mut().unwrap(),
            "{}",
            json!({"jsonrpc":"2.0","id":1,"method":method,"params":params})
        )
        .unwrap();
        self.child.0.stdin.as_mut().unwrap().flush().unwrap();
        self.replies
            .recv_timeout(Duration::from_secs(10))
            .expect("packaged MCP deadline")
    }
}
fn modules(pid: u32, executable: &Path, out: &Path, worker: &str, surface: &str) {
    let script = Path::new(worker)
        .parent()
        .unwrap()
        .join("portable/loaded-modules.ps1");
    let mut cmd = command(Path::new("pwsh.exe"));
    cmd.args(["-NoProfile", "-NonInteractive", "-File"])
        .arg(script)
        .args(["-OwnedPid", &pid.to_string(), "-ExpectedExe"])
        .arg(executable);
    let (ok, value) = bounded(cmd);
    assert!(ok && value.is_array(), "owned loaded-module inspection");
    fs::write(
        out.join(format!("{surface}-modules.json")),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
}
fn hashes(root: &Path) -> Vec<Value> {
    [
        "acp-daemon.exe",
        "acp-query.exe",
        "acp-mcp.exe",
        "acp-nsh-native.exe",
        "acp-nsh-tui.exe",
    ]
    .into_iter()
    .map(|name| {
        let bytes = fs::read(root.join(name)).unwrap();
        json!({"file":name,"sha256":format!("{:x}", Sha256::digest(bytes))})
    })
    .collect()
}
#[tokio::main]
async fn main() {
    if std::env::consts::OS != "windows" {
        panic!("Windows packaged acceptance only");
    }
    #[cfg(windows)]
    unsafe {
        // Supply a console to this harness only when launched by WSL without one.
        use windows_sys::Win32::System::Console::{AllocConsole, GetConsoleProcessList};
        let mut ids = [0_u32; 1];
        if GetConsoleProcessList(ids.as_mut_ptr(), 1) == 0 {
            assert_ne!(AllocConsole(), 0);
        }
    }
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4, "EXTRACTED_PACKAGE NEW_REPORT WINDOW_WORKER");
    let bins = PathBuf::from(&args[1]).canonicalize().unwrap();
    let out = PathBuf::from(&args[2]);
    fs::create_dir(&out).unwrap();
    let before = hashes(&bins);
    let manifest: Value =
        serde_json::from_slice(&fs::read(bins.join("manifest.json")).unwrap()).unwrap();
    for b in &before {
        assert!(
            manifest["binaries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["file"] == b["file"] && m["sha256"] == b["sha256"])
        );
    }
    transport::private_dir(&bins, false).unwrap();
    for b in &before {
        drop(transport::open_private(&bins.join(b["file"].as_str().unwrap()), false).unwrap());
    }
    let mut f = Fixture::start(bins.clone());
    // Keep the retained root private and out of public evidence. Only this file names it.
    transport::private_dir(&out, true).unwrap();
    fs::write(
        out.join("private-fixture-root.txt"),
        f.root.to_str().unwrap(),
    )
    .unwrap();
    let mut cells = Vec::new();
    let mut record = |id: &str| {
        cells.push(json!({"id":id,"status":"PASS"}));
        fs::write(
            out.join("progress.json"),
            serde_json::to_vec_pretty(
                &json!({"source_sha":manifest["source_sha"],"finished":false,"cells":cells}),
            )
            .unwrap(),
        )
        .unwrap();
    };
    let initial = f.ok(&["profile", "info"]);
    assert_eq!(initial["selected"], false);
    assert_eq!(initial["write_enabled"], false);
    record("no-implicit-profile");
    let unsafe_root = f.root.join("unsafe-runtime");
    fs::create_dir(&unsafe_root).unwrap();
    let mut acl = command(Path::new("pwsh.exe"));
    acl.args(["-NoProfile", "-NonInteractive", "-File"])
        .arg(
            Path::new(&args[3])
                .parent()
                .unwrap()
                .join("portable/unsafe-runtime.ps1"),
        )
        .arg("-Directory")
        .arg(&unsafe_root);
    assert!(bounded(acl).0);
    let mut unsafe_daemon = command(&bins.join("acp-daemon.exe"));
    unsafe_daemon
        .arg("--runtime-dir")
        .arg(&unsafe_root)
        .args(["--endpoint", "synthetic-unsafe"]);
    assert!(!bounded(unsafe_daemon).0);
    assert!(!unsafe_root.join("operator.cap").exists());
    let file_root = f.root.join("file-runtime");
    fs::write(&file_root, "RETAIN").unwrap();
    let mut file_daemon = command(&bins.join("acp-daemon.exe"));
    file_daemon
        .arg("--runtime-dir")
        .arg(&file_root)
        .args(["--endpoint", "synthetic-file"]);
    assert!(!bounded(file_daemon).0);
    assert_eq!(fs::read(&file_root).unwrap(), b"RETAIN");
    f.ok(&["health"]);
    record("unsafe-acl-and-file-runtime-refused-without-owner-disruption");
    let mut collision = Fixture::daemon(&bins, &f.root, &f.location);
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(s) = collision.0.try_wait().unwrap() {
            assert!(!s.success());
            break;
        }
        assert!(Instant::now() < end);
        thread::sleep(Duration::from_millis(30));
    }
    f.ok(&["health"]);
    record("endpoint-collision-preserves-owner");
    let db = f.root.join("state/profile.sqlite3");
    f.ok(&["profile", "create", db.to_str().unwrap()]);
    assert!(!f.query(&["profile", "create", db.to_str().unwrap()]).0);
    let binding = f.ok(&["profile", "info"])["binding"].clone();
    f.ok(&[
        "profile",
        "write",
        "enable",
        "--profile-id",
        binding["profile_id"].as_str().unwrap(),
        "--generation",
        binding["generation"].as_str().unwrap(),
        "--confirm",
    ]);
    f.mutation(&[
        "project-register",
        "synthetic",
        f.root
            .join("synthetic")
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap(),
    ]);
    record("explicit-synthetic-profile-grant-register-no-overwrite");
    let agent = load_credential(
        &f.location,
        &f.location.credential_path(Some("synthetic")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        agent.access,
        Access::Agent {
            project: "synthetic".into()
        }
    );
    let mut client = Client::connect(&f.location, &agent).await.unwrap();
    assert_eq!(
        client
            .request("profile.write.disable", json!({}))
            .await
            .unwrap_err(),
        Error::Unauthorized
    );
    record("project-credential-cannot-write");
    let mut wrong = command(&bins.join("acp-mcp.exe"));
    wrong
        .arg("--runtime-dir")
        .arg(&f.location.runtime_dir)
        .args([
            "--endpoint",
            &f.location.endpoint,
            "--project",
            "synthetic",
            "--capability-file",
        ])
        .arg(f.location.runtime_dir.join("operator.cap"));
    assert!(!bounded(wrong).0);
    record("mcp-refuses-operator-credential");
    let mut mcp = Mcp::start(&f);
    let list = mcp.request("tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["name"], "get_effective_context");
    assert_eq!(tools[1]["name"], "get_handoff");
    let context = mcp.request(
        "tools/call",
        json!({"name":"get_effective_context","arguments":{}}),
    );
    assert_eq!(context["result"]["isError"], false);
    let handoff = mcp.request(
        "tools/call",
        json!({"name":"get_handoff","arguments":{"task_id":"synthetic"}}),
    );
    assert_eq!(handoff["result"]["isError"], true); // Known missing; no fabricated history.
    let denied = mcp.request(
        "tools/call",
        json!({"name":"confirm_preference","arguments":{}}),
    );
    assert!(denied["error"].is_object());
    record("packaged-mcp-exact-two-read-tools-context-missing-handoff-write-refusal");
    drop(mcp);
    drop(client);
    f.ok(&["profile", "write", "disable"]);
    assert_eq!(f.ok(&["profile", "info"])["write_enabled"], false);
    record("write-grant-explicit-revocation");
    let focus = f.root.join("state/focus.events");
    fs::write(&focus, "").unwrap();
    let ui_args = vec![
        "--runtime-dir".into(),
        f.location.runtime_dir.to_str().unwrap().into(),
        "--endpoint".into(),
        f.location.endpoint.clone(),
        "--operator".into(),
    ];
    {
        let mut ui = tty::Tty::start(&bins.join("acp-nsh-tui.exe"), &ui_args, &out, &focus);
        ui.wait("Agent Control Plane");
        modules(
            ui.pid(),
            &bins.join("acp-nsh-tui.exe"),
            &out,
            &args[3],
            "conpty",
        );
        ui.capture(&out.join("conpty.txt"));
        ui.resize(100, 35);
        ui.wait("Agent Control Plane");
        ui.close();
        let end = Instant::now() + Duration::from_secs(2);
        loop {
            let raw = ui.raw.lock().unwrap();
            if raw.windows(8).any(|w| w == b"\x1b[?1049l")
                && raw.windows(8).any(|w| w == b"\x1b[?2004l")
            {
                break;
            }
            drop(raw);
            assert!(Instant::now() < end, "terminal restore sequences missing");
            thread::sleep(Duration::from_millis(20));
        }
    }
    record("packaged-conpty-render-resize-normal-restore");
    {
        let nonce = &acp_ipc::operator::operation_id().unwrap()[..16];
        let mut ui = desktop::Desktop::start(
            &bins.join("acp-nsh-native.exe"),
            &ui_args,
            nonce,
            &args[3],
            &out,
            &focus,
        );
        ui.wait_after("ACP_NSH_STATE phase=Ready", 0);
        modules(
            ui.pid(),
            &bins.join("acp-nsh-native.exe"),
            &out,
            &args[3],
            "hwnd",
        );
        ui.resize(920, 640);
        ui.capture("hwnd", &out);
        ui.close();
    }
    record("packaged-hwnd-render-resize-normal-close");
    f.ok(&["health"]);
    record("ui-close-leaves-daemon-running");
    let state_before = Sha256::digest(fs::read(&db).unwrap());
    let operator =
        load_credential(&f.location, &f.location.credential_path(None).unwrap()).unwrap();
    let pid = f.daemon.0.id();
    f.stop();
    record("graceful-stop-cleans-owned-credentials-releases-runtime-lease");
    f.daemon = Fixture::daemon(&bins, &f.root, &f.location);
    let end = Instant::now() + Duration::from_secs(10);
    while !f.query(&["profile", "info"]).0 {
        assert!(Instant::now() < end);
        thread::sleep(Duration::from_millis(30));
    }
    assert_eq!(f.ok(&["profile", "info"])["selected"], false);
    assert!(Client::connect(&f.location, &operator).await.is_err());
    let pre_crash =
        load_credential(&f.location, &f.location.credential_path(None).unwrap()).unwrap();
    f.daemon.0.kill().unwrap();
    f.daemon.0.wait().unwrap();
    f.daemon = Fixture::daemon(&bins, &f.root, &f.location);
    let end = Instant::now() + Duration::from_secs(10);
    while !f.query(&["profile", "info"]).0 {
        assert!(Instant::now() < end);
        thread::sleep(Duration::from_millis(30));
    }
    assert_eq!(f.ok(&["profile", "info"])["selected"], false);
    assert!(Client::connect(&f.location, &pre_crash).await.is_err());
    f.stop();
    record("normal-and-crash-restart-no-adoption-stale-credential-refused");
    assert_eq!(state_before[..], Sha256::digest(fs::read(&db).unwrap())[..]);
    assert_eq!(
        fs::read(f.root.join("state/auth-retain.txt")).unwrap(),
        b"SYNTHETIC RETAIN\n"
    );
    assert_eq!(
        fs::read(f.root.join("state/unknown-retain.txt")).unwrap(),
        b"SYNTHETIC RETAIN\n"
    );
    assert_eq!(before, hashes(&bins));
    fs::write(out.join("smoke.json"), serde_json::to_vec_pretty(&json!({"source_sha":manifest["source_sha"],"source_tree":manifest["source_tree"],"platform":"Windows x86_64 developer host, NOT clean-target","binaries":before,"initial_daemon_pid":pid,"cells":cells,"clean_target":"NOT_RUN","second_user":"NOT_RUN","ime_at":"NOT_RUN"})).unwrap()).unwrap();
    println!("PASS: owned packaged Windows smoke; clean-target NOT_RUN");
}
