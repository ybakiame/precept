//! Real-process bridge tests on Linux: client -> acp-wsl-bridge helper -> real
//! acp-daemon over authenticated Unix IPC. The Windows `wsl.exe` hop is covered
//! by native Windows acceptance; everything after it is exercised here.
//! Build the root workspace binaries first (`cargo build` at the repository root).
#![cfg(unix)]
use acp_core::operator::{Binding, Command as Op2, Expected, Mutation, Origin, Scope};
use acp_operator_client::bridge::{self, CallError, Client, ConnectFailure, Op, Reply};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    time::{Duration, Instant},
};

fn bin(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug")
        .join(name)
}
const HELPER: &str = env!("CARGO_BIN_EXE_acp-wsl-bridge");

/// Private synthetic root removed on drop; never a user directory.
/// The daemon's state directory for a test: its projects and, when given, the
/// Profile the operator asked to select at start.
fn daemon_state(
    dir: &std::path::Path,
    projects: &[(&str, &std::path::Path)],
) -> std::path::PathBuf {
    acp_ipc::transport::private_dir(dir, true).unwrap();
    let list: Vec<serde_json::Value> = projects
        .iter()
        .map(|(n, r)| serde_json::json!({"name":n,"root":r.canonicalize().unwrap()}))
        .collect();
    let path = dir.join("projects.json");
    let _ = std::fs::remove_file(&path);
    let mut file = acp_ipc::transport::new_private(&path).unwrap();
    std::io::Write::write_all(
        &mut file,
        serde_json::json!({"format":1,"projects":list})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    dir.to_path_buf()
}
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let mut raw = [0u8; 8];
        getrandom_fill(&mut raw);
        let path = std::env::temp_dir().join(format!(
            "acp-s1b-bridge-{}",
            raw.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ));
        acp_ipc::transport::private_dir(&path, true).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn getrandom_fill(buf: &mut [u8]) {
    let id = acp_ipc::operator::operation_id().unwrap();
    for (i, b) in buf.iter_mut().enumerate() {
        *b = u8::from_str_radix(&id[i * 2..i * 2 + 2], 16).unwrap();
    }
}

struct Daemon {
    child: Child,
    root: Root,
    runtime: PathBuf,
    endpoint: String,
    argv: Vec<std::ffi::OsString>,
}
impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Daemon {
    fn start() -> Self {
        Self::start_with(false)
    }
    /// `agent`: an explicit private agent config with the deterministic fake
    /// agent in permission mode (never a real coding agent).
    fn start_with(agent: bool) -> Self {
        if agent {
            Self::start_configured(&[("fake", &["permission"])], false, None)
        } else {
            Self::start_configured(&[], false, None)
        }
    }
    /// Explicit agent programs (deterministic fake agent modes), optional trusted
    /// context bridge (`acp-mcp`) and lease TTL.
    fn start_configured(programs: &[(&str, &[&str])], context: bool, ttl: Option<u64>) -> Self {
        let root = Root::new();
        std::fs::create_dir(root.0.join("alpha")).unwrap();
        std::fs::write(root.0.join("alpha/AGENTS.md"), "Synthetic contract\n").unwrap();
        acp_ipc::transport::private_dir(&root.0.join("state"), true).unwrap();
        let runtime = root.0.join("runtime");
        let endpoint = runtime.join("control.sock").to_str().unwrap().to_owned();
        let mut extra: Vec<std::ffi::OsString> = Vec::new();
        if !programs.is_empty() {
            use std::os::unix::fs::OpenOptionsExt;
            let config = root.0.join("agents.toml");
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&config)
                .unwrap();
            let list: Vec<Value> = programs
                .iter()
                .map(|(id, args)| json!({"id":id,"executable":bin("acp-fake-agent"),"args":args,"env":{}}))
                .collect();
            file.write_all(
                toml::to_string(&json!({"version":1,"programs":list}))
                    .unwrap()
                    .as_bytes(),
            )
            .unwrap();
            extra.extend(["--agent-config".into(), config.into()]);
        }
        if context {
            extra.extend(["--context-mcp".into(), bin("acp-mcp").into()]);
        }
        if let Some(ttl) = ttl {
            extra.extend(["--context-lease-seconds".into(), ttl.to_string().into()]);
        }
        let mut command = Command::new(bin("acp-daemon"));
        command.args(&extra);
        let child = command
            .arg("--runtime-dir")
            .arg(&runtime)
            // The test's private root is the daemon user's home for scope checks.
            .env("HOME", &root.0)
            .args(["--endpoint", &endpoint, "--state-dir"])
            .arg(daemon_state(
                &root.0.join("daemon-state"),
                &[("alpha", &root.0.join("alpha"))],
            ))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("build the root workspace binaries first");
        let argv = command.get_args().map(ToOwned::to_owned).collect();
        let d = Self {
            child,
            root,
            runtime,
            endpoint,
            argv,
        };
        d.ready();
        d
    }
    /// Graceful stop (SIGINT, as Ctrl+C) and an explicit relaunch of the same command.
    fn restart(&mut self) {
        let _ = Command::new("kill")
            .args(["-INT", &self.child.id().to_string()])
            .status();
        assert!(self.child.wait().unwrap().success(), "graceful daemon exit");
        self.child = Command::new(bin("acp-daemon"))
            .args(&self.argv)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        self.ready();
    }
    fn ready(&self) {
        let d = self;
        let start = Instant::now();
        while !d.runtime.join("operator.cap").exists() || !Path::new(&d.endpoint).exists() {
            assert!(
                start.elapsed() < Duration::from_secs(15),
                "daemon readiness"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn helper(&self) -> Command {
        let mut c = Command::new(HELPER);
        c.arg("--runtime-dir")
            .arg(&self.runtime)
            .args(["--endpoint", &self.endpoint]);
        c
    }
    fn client(&self) -> Client {
        Client::connect(self.helper(), Duration::from_secs(10)).unwrap()
    }
    fn register(&self, b: &Binding) {
        let out = Command::new(bin("acp-query"))
            .arg("--runtime-dir")
            .arg(&self.runtime)
            .args(["--endpoint", &self.endpoint, "project-register", "alpha"])
            .arg(self.root.0.join("alpha").canonicalize().unwrap())
            .args(["--profile-id", &b.profile_id, "--generation", &b.generation])
            .args(["--owner-epoch", &b.owner_epoch.to_string()])
            .args(["--operation-id", "s1b-register"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "CLI-only registration: {} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    /// CLI-only saved Handoff `shared`, as the synthetic context the agent reads.
    fn put_handoff(&self, b: &Binding) {
        let input = self.root.0.join("handoff.json");
        std::fs::write(&input, json!({"objective":"Saved synthetic history","completed":"","next_action":"Inspect current repository facts","validation_json":"[{\"status\":\"NOT_RUN\",\"check\":\"synthetic\"}]","snapshot":{"base_sha":"","branch":"","state":"unknown","fingerprint":null}}).to_string()).unwrap();
        let out = Command::new(bin("acp-query"))
            .arg("--runtime-dir")
            .arg(&self.runtime)
            .args([
                "--endpoint",
                &self.endpoint,
                "handoff-put",
                "alpha",
                "shared",
                "--expected-absent",
                "--input",
            ])
            .arg(&input)
            .args(["--profile-id", &b.profile_id, "--generation", &b.generation])
            .args(["--owner-epoch", &b.owner_epoch.to_string()])
            .args(["--operation-id", "s1b-handoff"])
            .output()
            .unwrap();
        assert!(out.status.success(), "CLI-only handoff put");
    }
    /// Every token byte sequence a capability file in the runtime holds.
    fn tokens(&self) -> Vec<String> {
        let mut found = Vec::new();
        for entry in std::fs::read_dir(&self.runtime).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "cap") {
                let v: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                let bytes: Vec<u8> = serde_json::from_value(v["token"].clone()).unwrap();
                found.push(serde_json::to_string(&bytes).unwrap());
                found.push(bytes.iter().map(|b| format!("{b:02x}")).collect());
            }
        }
        assert!(!found.is_empty());
        found
    }
}

fn control(c: &Client, method: &str, params: Value) -> Result<Value, CallError> {
    c.call(
        Op::Control {
            method: method.into(),
            params,
        },
        Duration::from_secs(15),
    )
}
fn mutate(c: &Client, m: &Mutation, timeout: Duration) -> Result<Value, CallError> {
    c.call(
        Op::Mutate {
            mutation: Box::new(m.clone()),
        },
        timeout,
    )
}
fn binding(c: &Client) -> Binding {
    serde_json::from_value(control(c, "profile.info", json!({})).unwrap()["binding"].clone())
        .unwrap()
}
fn suggest(b: &Binding, op: &str, key: &str) -> Mutation {
    Mutation {
        version: 2,
        binding: b.clone(),
        operation_id: op.into(),
        command: Op2::SuggestPreference {
            scope: Scope::Project {
                name: "alpha".into(),
            },
            key: key.into(),
            value: "synthetic".into(),
            source: "s1b-bridge-test".into(),
            origin: Origin::AgentReported,
            expected: Expected::Absent,
        },
    }
}
/// Fresh Profile + write grant + registered project, all through the bridge
/// except the CLI-only registration.
fn writable(d: &Daemon, c: &Client) -> Binding {
    let path = d.root.0.join("state/profile.sqlite3");
    control(c, "profile.create", json!({"path": path})).unwrap();
    grant_and_register(d, c)
}
/// Explicit write grant for the selected Profile, then CLI-only registration.
fn grant_and_register(d: &Daemon, c: &Client) -> Binding {
    let b = binding(c);
    control(
        c,
        "profile.write.enable",
        json!({"profile_id":b.profile_id,"generation":b.generation,"confirm":true}),
    )
    .unwrap();
    // Register with the CURRENT binding; a pre-grant binding is stale and refused.
    d.register(&binding(c));
    binding(c)
}

#[test]
fn operator_profile_review_and_session_parity_through_the_bridge() {
    let d = Daemon::start();
    let started = Instant::now();
    let c = d.client();
    let connect = started.elapsed();
    let snapshot = |c: &Client| {
        c.call(
            Op::Snapshot {
                project: None,
                task: String::new(),
            },
            Duration::from_secs(20),
        )
    };
    // Before a Profile exists the read still succeeds (a connected daemon is
    // not "unreachable"): no navigable project, the daemon's set unregistered,
    // and the bridge answers exactly as the local client (no fixture fallback).
    let local = acp_operator_client::native::Inputs {
        runtime_dir: Some(d.runtime.clone()),
        endpoint: Some(d.endpoint.clone()),
        ..Default::default()
    };
    let before = local.fetch().unwrap();
    assert!(before.projects.is_empty());
    assert!(
        !before.directory.is_empty() && before.directory.iter().all(|p| p.state == "unregistered")
    );
    let bridged: acp_operator_core::Snapshot =
        serde_json::from_value(snapshot(&c).unwrap()).unwrap();
    assert!(bridged.projects.is_empty());
    assert_eq!(
        serde_json::to_value(&bridged.directory).unwrap(),
        serde_json::to_value(&before.directory).unwrap()
    );
    // No implicit Profile, selection or write grant.
    let info = control(&c, "profile.info", json!({})).unwrap();
    assert_eq!(
        (info["selected"].clone(), info["write_enabled"].clone()),
        (json!(false), json!(false))
    );
    let path = d.root.0.join("state/profile.sqlite3");
    control(&c, "profile.create", json!({"path": path})).unwrap();
    // Creation/selection never enables writes.
    assert_eq!(
        control(&c, "profile.info", json!({})).unwrap()["write_enabled"],
        false
    );
    let b = binding(&c);
    // No OperatorWrite capability yet: a mutation is refused, never approved by the bridge.
    assert_eq!(
        mutate(
            &c,
            &suggest(&b, "s1b-op-early", "k0"),
            Duration::from_secs(15)
        ),
        Err(CallError::Remote("Unauthorized"))
    );
    // A second create is refused rather than silently replacing the Profile.
    assert_eq!(
        control(&c, "profile.create", json!({"path": path})),
        Err(CallError::Remote("RestartRequired"))
    );
    let b = grant_and_register(&d, &c);
    // Dashboard snapshot over the bridge equals the local client's snapshot.
    let snap = snapshot(&c).unwrap();
    assert_eq!(snap["projects"][0]["name"], "alpha");
    assert_eq!(
        serde_json::from_value::<acp_operator_core::Snapshot>(snap)
            .unwrap()
            .projects,
        local.fetch().unwrap().projects
    );
    // Suggest, then exact identical replay is idempotent; changed payload conflicts.
    let first = suggest(&b, "s1b-op-1", "k1");
    let receipt = mutate(&c, &first, Duration::from_secs(15)).unwrap();
    assert_eq!(
        mutate(&c, &first, Duration::from_secs(15)).unwrap(),
        receipt
    );
    assert_eq!(
        mutate(
            &c,
            &suggest(&b, "s1b-op-1", "k1-changed"),
            Duration::from_secs(15)
        ),
        Err(CallError::Remote("IdempotencyConflict"))
    );
    // Stale binding (generation) is refused before any write.
    let stale = Binding {
        generation: "0".repeat(b.generation.len()),
        ..b.clone()
    };
    assert_eq!(
        mutate(
            &c,
            &suggest(&stale, "s1b-op-2", "k2"),
            Duration::from_secs(15)
        ),
        Err(CallError::Remote("Unauthorized"))
    );
    // Review queue and exact inspect through typed Operator reads.
    let scope = json!({"kind":"project","name":"alpha"});
    let queue = control(
        &c,
        "profile.review.queue",
        json!({"scope":scope,"after_id":0}),
    )
    .unwrap();
    let record = queue["records"][0].clone();
    assert_eq!(record["key"], "k1");
    let inspect = control(
        &c,
        "profile.review.inspect",
        json!({"scope":scope,"id":record["id"],"revision":record["revision"]}),
    )
    .unwrap();
    assert_eq!(inspect["record"]["id"], record["id"]);
    let list = control(
        &c,
        "profile.preferences.list",
        json!({"scope":scope,"after_id":0}),
    );
    assert!(list.is_ok(), "{list:?}");
    // Revoke: the same request now lacks OperatorWrite authority.
    control(&c, "profile.write.disable", json!({})).unwrap();
    assert_eq!(
        mutate(&c, &suggest(&b, "s1b-op-3", "k3"), Duration::from_secs(15)),
        Err(CallError::Remote("Unauthorized"))
    );
    // Sessions: discovery and refresh through the bridge; an unconfigured start is denied.
    let session = |method: &str, fields: Value| {
        c.call(
            Op::Session {
                project: "alpha".into(),
                method: method.into(),
                fields,
            },
            Duration::from_secs(30),
        )
    };
    let refreshed = session("session.refresh", json!({})).unwrap();
    acp_operator_core::session::observation(
        refreshed["capabilities"].clone(),
        refreshed["view"].clone(),
    )
    .unwrap();
    assert_eq!(
        session(
            "session.start",
            json!({"executable":"/bin/sh","confirmed":true})
        ),
        Err(CallError::Remote("Session unavailable or request denied"))
    );
    // Wrong project never reaches the daemon as an authorized request.
    assert!(matches!(
        c.call(
            Op::Session {
                project: "beta".into(),
                method: "session.refresh".into(),
                fields: json!({})
            },
            Duration::from_secs(15)
        ),
        Err(CallError::Remote(_))
    ));
    // Ordinary request latency over the persistent bridge (no process per call).
    let pid = c.process_id();
    let t = Instant::now();
    for _ in 0..20 {
        control(&c, "profile.info", json!({})).unwrap();
    }
    let per_call = t.elapsed() / 20;
    assert_eq!(c.process_id(), pid);
    eprintln!(
        "S1B_TIMING connect_ms={} profile_info_avg_us={}",
        connect.as_millis(),
        per_call.as_micros()
    );
    c.close(Duration::from_secs(5));
    // Closing the bridge never stops the daemon or touches Profile/runtime files.
    let mut daemon = d;
    assert!(daemon.child.try_wait().unwrap().is_none());
    assert!(path.exists() && daemon.runtime.join("operator.cap").exists());
    let again = daemon.client();
    assert_eq!(
        control(&again, "profile.info", json!({})).unwrap()["selected"],
        true
    );
    again.close(Duration::from_secs(5));
    let _ = daemon.child.kill();
}

/// Raw stdio access to a helper for protocol violation tests.
struct Raw {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
    seen: Vec<u8>,
}
impl Raw {
    fn new(cmd: &mut Command) -> Self {
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        Self {
            stdin: child.stdin.take(),
            stdout: child.stdout.take().unwrap(),
            child,
            seen: Vec::new(),
        }
    }
    fn send_bytes(&mut self, bytes: &[u8]) {
        let s = self.stdin.as_mut().unwrap();
        s.write_all(bytes).unwrap();
        s.flush().unwrap();
    }
    fn send(&mut self, v: Value) {
        let body = serde_json::to_vec(&v).unwrap();
        let mut frame = (body.len() as u32).to_be_bytes().to_vec();
        frame.extend(body);
        self.send_bytes(&frame);
    }
    fn reply(&mut self) -> Reply {
        let mut header = [0u8; 4];
        self.stdout.read_exact(&mut header).unwrap();
        let mut body = vec![0u8; u32::from_be_bytes(header) as usize];
        self.stdout.read_exact(&mut body).unwrap();
        self.seen.extend(&header);
        self.seen.extend(&body);
        serde_json::from_slice(&body).unwrap()
    }
}

#[test]
fn helper_refuses_malformed_unknown_versioned_and_oversized_frames_without_leaking_tokens() {
    let d = Daemon::start();
    let mut raw = Raw::new(&mut d.helper());
    let hello = raw.reply();
    assert_eq!(
        (hello.id, hello.ok.clone().unwrap()["bridge"].clone()),
        (0, json!("acp-wsl-bridge"))
    );
    // Malformed JSON: refused with id 0, stream continues.
    raw.send_bytes(&[0, 0, 0, 3, b'{', b'x', b'}']);
    assert_eq!(raw.reply(), Reply::error(0, "InvalidRequest"));
    // Unknown op type and unknown fields fail strict decoding.
    raw.send(json!({"version":1,"id":5,"op":{"type":"shell","command":"id"}}));
    assert_eq!(raw.reply(), Reply::error(0, "InvalidRequest"));
    raw.send(json!({"version":1,"id":6,"op":{"type":"control","method":"profile.info","params":{}},"token":"x"}));
    assert_eq!(raw.reply(), Reply::error(0, "InvalidRequest"));
    // Version mismatch and reserved id 0.
    raw.send(
        json!({"version":2,"id":7,"op":{"type":"control","method":"profile.info","params":{}}}),
    );
    assert_eq!(raw.reply(), Reply::error(7, "Version"));
    raw.send(
        json!({"version":1,"id":0,"op":{"type":"control","method":"profile.info","params":{}}}),
    );
    assert_eq!(raw.reply(), Reply::error(0, "InvalidRequest"));
    // Fixed allowlist: no generic IPC forwarding.
    raw.send(
        json!({"version":1,"id":8,"op":{"type":"control","method":"daemon.health","params":{}}}),
    );
    assert_eq!(raw.reply(), Reply::error(8, "Unsupported"));
    raw.send(json!({"version":1,"id":9,"op":{"type":"session","project":"alpha","method":"session.capabilities","fields":{}}}));
    assert_eq!(raw.reply(), Reply::error(9, "Unsupported"));
    // A valid request still works after every refusal.
    raw.send(
        json!({"version":1,"id":10,"op":{"type":"control","method":"profile.info","params":{}}}),
    );
    let ok = raw.reply();
    assert_eq!(
        (ok.id, ok.ok.unwrap()["selected"].clone()),
        (10, json!(false))
    );
    // Cancel of an unknown request is silent.
    raw.send(json!({"version":1,"id":11,"op":{"type":"cancel","target":999}}));
    raw.send(
        json!({"version":1,"id":12,"op":{"type":"control","method":"profile.info","params":{}}}),
    );
    assert_eq!(raw.reply().id, 12);
    // Oversized frame: refused and the helper stops (boundaries are lost).
    raw.send_bytes(&((bridge::MAX_REQUEST as u32) + 1).to_be_bytes());
    assert_eq!(raw.reply(), Reply::error(0, "TooLarge"));
    assert_eq!(raw.child.wait().unwrap().code(), Some(2));
    // No capability token crossed the stdio boundary.
    let seen = String::from_utf8_lossy(&raw.seen).to_string();
    for token in d.tokens() {
        assert!(!seen.contains(&token));
    }
}

#[test]
fn eof_exits_cleanly_and_daemon_absence_or_unsafe_runtime_is_explicit() {
    let d = Daemon::start();
    // Stdin closed => exit 0; the daemon keeps running.
    let mut raw = Raw::new(&mut d.helper());
    raw.reply();
    raw.stdin.take();
    assert_eq!(raw.child.wait().unwrap().code(), Some(0));
    let mut d = d;
    assert!(d.child.try_wait().unwrap().is_none());
    // Daemon absent: an explicit unavailable state, never a fixture or a retry.
    let _ = d.child.kill();
    let _ = d.child.wait();
    let c = d.client();
    let snap = c.call(
        Op::Snapshot {
            project: None,
            task: String::new(),
        },
        Duration::from_secs(20),
    );
    assert_eq!(
        snap,
        Err(CallError::Remote(
            "Local daemon unavailable or access refused"
        ))
    );
    assert_eq!(
        control(&c, "profile.info", json!({})),
        Err(CallError::Remote("Unavailable"))
    );
    c.close(Duration::from_secs(5));
    // Symlinked runtime directory and a wrong endpoint are refused, not followed.
    let alias = d.root.0.join("alias");
    std::os::unix::fs::symlink(&d.runtime, &alias).unwrap();
    let mut cmd = Command::new(HELPER);
    cmd.arg("--runtime-dir")
        .arg(&alias)
        .args(["--endpoint", &d.endpoint]);
    let c = Client::connect(cmd, Duration::from_secs(10)).unwrap();
    assert_eq!(
        c.call(
            Op::Snapshot {
                project: None,
                task: String::new()
            },
            Duration::from_secs(20)
        ),
        Err(CallError::Remote("Permission denied"))
    );
    c.close(Duration::from_secs(5));
    // Relative paths are refused before any protocol starts.
    let status = Command::new(HELPER)
        .args(["--runtime-dir", "relative", "--endpoint", "/x"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(2));
}

#[test]
fn bridge_loss_after_submit_is_unknown_and_only_an_identical_retry_settles_it() {
    let d = Daemon::start();
    let c = d.client();
    let b = writable(&d, &c);
    // A reply arriving after the caller gave up is ignored; the bridge stays healthy.
    let late = suggest(&b, "s1b-late", "late");
    assert_eq!(
        mutate(&c, &late, Duration::from_micros(1)),
        Err(CallError::Timeout)
    );
    assert!(c.alive());
    assert!(
        mutate(&c, &late, Duration::from_secs(15)).is_ok(),
        "identical retry settles"
    );
    // Helper killed while a mutation may be in flight.
    let request = suggest(&b, "s1b-killed", "killed");
    let caller = {
        let c = c.clone();
        let request = request.clone();
        std::thread::spawn(move || mutate(&c, &request, Duration::from_secs(30)))
    };
    let _ = Command::new("kill")
        .args(["-9", &c.process_id().to_string()])
        .status();
    let outcome = caller.join().unwrap();
    // Either the reply won the race, or the outcome is unknown (never a fabricated failure).
    assert!(
        matches!(outcome, Ok(_) | Err(CallError::Lost)),
        "{outcome:?}"
    );
    assert!(!c.alive());
    assert_eq!(
        control(&c, "profile.info", json!({})),
        Err(CallError::NotSent)
    );
    // Explicit reconnect, then the identical request with the same operation ID
    // settles; a changed payload under that ID is refused.
    let c = d.client();
    assert!(mutate(&c, &request, Duration::from_secs(15)).is_ok());
    assert_eq!(
        mutate(
            &c,
            &suggest(&b, "s1b-killed", "other"),
            Duration::from_secs(15)
        ),
        Err(CallError::Remote("IdempotencyConflict"))
    );
    c.close(Duration::from_secs(5));
}

#[test]
fn client_rejects_wrong_hello_mismatched_ids_and_ambiguous_replies() {
    let root = Root::new();
    let frame = |v: Value| {
        let body = serde_json::to_vec(&v).unwrap();
        let mut f = (body.len() as u32).to_be_bytes().to_vec();
        f.extend(body);
        f
    };
    let script = |name: &str, frames: Vec<Value>| {
        let path = root.0.join(name);
        std::fs::write(
            &path,
            frames.into_iter().flat_map(frame).collect::<Vec<u8>>(),
        )
        .unwrap();
        // Test fixture only: replays fixed frames, then keeps stdout open so
        // that only violation detection (not EOF) can end the bridge.
        let mut c = Command::new("sh");
        c.args(["-c", "cat \"$0\"; exec sleep 30"]).arg(path);
        c
    };
    let hello = json!({"version":1,"id":0,"ok":{"bridge":"acp-wsl-bridge","version":1}});
    assert!(
        Client::connect(
            script(
                "v2",
                vec![json!({"version":1,"id":0,"ok":{"bridge":"acp-wsl-bridge","version":2}})]
            ),
            Duration::from_secs(5)
        )
        .is_err()
    );
    assert!(
        Client::connect(
            script(
                "other",
                vec![json!({"version":1,"id":0,"ok":{"bridge":"other","version":1}})]
            ),
            Duration::from_secs(5)
        )
        .is_err()
    );
    for (name, bad) in [
        ("unsolicited", json!({"version":1,"id":99,"ok":{}})),
        ("both", json!({"version":1,"id":1,"ok":{},"error":"Busy"})),
        ("extra", json!({"version":1,"id":1,"ok":{},"token":"x"})),
    ] {
        let c = Client::connect(
            script(name, vec![hello.clone(), bad]),
            Duration::from_secs(5),
        )
        .unwrap();
        let start = Instant::now();
        while c.alive() {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "{name}: violation not detected"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(matches!(
            control(&c, "profile.info", json!({})),
            Err(CallError::NotSent | CallError::Lost)
        ));
    }
}

#[test]
fn session_lifecycle_permissions_and_replay_through_the_bridge() {
    let d = Daemon::start_with(true);
    let c = d.client();
    writable(&d, &c);
    let call = |method: &str, fields: Value| {
        c.call(
            Op::Session {
                project: "alpha".into(),
                method: method.into(),
                fields,
            },
            Duration::from_secs(30),
        )
    };
    let observe = |pred: &dyn Fn(&Value) -> bool| {
        let end = Instant::now() + Duration::from_secs(20);
        loop {
            let v = call("session.refresh", json!({})).unwrap();
            if pred(&v["view"]) {
                return v;
            }
            assert!(Instant::now() < end, "session state: {}", v["view"]);
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    let first = call("session.refresh", json!({})).unwrap();
    assert_eq!(first["capabilities"]["executables"], json!(["fake"]));
    // Only a configured program, and only with explicit confirmation.
    let denied = Err(CallError::Remote("Session unavailable or request denied"));
    assert_eq!(
        call(
            "session.start",
            json!({"executable":"/bin/sh","confirmed":true})
        ),
        denied
    );
    assert_eq!(call("session.start", json!({"executable":"fake"})), denied);
    call(
        "session.start",
        json!({"executable":"fake","confirmed":true}),
    )
    .unwrap();
    let ready = observe(&|v| v["process_alive"] == true && v["state"] == "ready");
    let session_id = ready["view"]["session_id"].as_str().unwrap().to_owned();
    // A second start while alive is refused (no duplicate agent).
    assert_eq!(
        call(
            "session.start",
            json!({"executable":"fake","confirmed":true})
        ),
        denied
    );
    // Prompt -> permission request -> one-time allow; the replay is refused.
    call(
        "session.prompt",
        json!({"session_id":session_id,"prompt":"first synthetic turn"}),
    )
    .unwrap();
    let waiting = observe(&|v| v["permissions"].as_array().is_some_and(|p| p.len() == 1));
    let permission = waiting["view"]["permissions"][0].clone();
    let request = permission["request_id"].as_str().unwrap().to_owned();
    let allow = permission["allow_once"].clone();
    assert!(allow.is_string(), "{permission}");
    let decision = json!({"session_id":session_id,"request_id":request,"option_id":allow});
    call("session.permission", decision.clone()).unwrap();
    assert_eq!(
        call("session.permission", decision),
        denied,
        "permission replay"
    );
    observe(&|v| {
        v["state"] == "completed" && v["permissions"].as_array().is_some_and(Vec::is_empty)
    });
    // Second turn: explicit deny.
    call(
        "session.prompt",
        json!({"session_id":session_id,"prompt":"second synthetic turn"}),
    )
    .unwrap();
    let waiting = observe(&|v| v["permissions"].as_array().is_some_and(|p| p.len() == 1));
    let request = waiting["view"]["permissions"][0]["request_id"].clone();
    call(
        "session.permission",
        json!({"session_id":session_id,"request_id":request,"option_id":null}),
    )
    .unwrap();
    observe(&|v| {
        v["permissions"].as_array().is_some_and(Vec::is_empty) && v["state"] != "waiting_permission"
    });
    // Cancel with no running turn is refused; stop ends the agent process.
    assert_eq!(
        call("session.cancel", json!({"session_id":session_id})),
        denied
    );
    call("session.stop", json!({"session_id":session_id})).unwrap();
    observe(&|v| v["process_alive"] == false);
    c.close(Duration::from_secs(5));
}

#[test]
fn launch_failures_are_typed_from_the_launcher_exit_status() {
    let sh = |script: &str| {
        let mut c = Command::new("sh");
        c.args(["-c", script]);
        c
    };
    // `wsl.exe` exits 1 when the helper cannot be exec'd, 2 is a helper refusal.
    assert_eq!(
        Client::connect(sh("exit 1"), Duration::from_secs(5)).err(),
        Some(ConnectFailure::Exited(Some(1)))
    );
    assert_eq!(
        Client::connect(sh("exit 2"), Duration::from_secs(5)).err(),
        Some(ConnectFailure::Exited(Some(2)))
    );
    // The real helper refuses relative paths with exit 2.
    let mut refused = Command::new(HELPER);
    refused.args(["--runtime-dir", "relative", "--endpoint", "/x"]);
    assert_eq!(
        Client::connect(refused, Duration::from_secs(5)).err(),
        Some(ConnectFailure::Exited(Some(2)))
    );
    // A non-hello first frame is an incompatible helper; silence is a timeout.
    assert_eq!(
        Client::connect(
            sh("printf '\\000\\000\\000\\002{}'; exec sleep 30"),
            Duration::from_secs(5)
        )
        .err(),
        Some(ConnectFailure::Incompatible)
    );
    assert_eq!(
        Client::connect(sh("exec sleep 30"), Duration::from_millis(300)).err(),
        Some(ConnectFailure::Timeout)
    );
}

#[test]
fn health_reports_daemon_reachability_over_the_live_bridge() {
    let mut d = Daemon::start();
    let c = d.client();
    let health = c.call(Op::Health {}, Duration::from_secs(5)).unwrap();
    assert_eq!(health["service"], "acp-daemon");
    // Daemon stops while the bridge stays alive: an explicit unavailable answer.
    let _ = d.child.kill();
    let _ = d.child.wait();
    assert!(c.alive());
    assert_eq!(
        c.call(Op::Health {}, Duration::from_secs(5)),
        Err(CallError::Remote(
            "Local daemon unavailable or access refused"
        ))
    );
    c.close(Duration::from_secs(5));
}

/// Session helpers over a bridge client for one project.
struct Sessions<'a>(&'a Client);
impl Sessions<'_> {
    fn call(&self, method: &str, fields: Value) -> Result<Value, CallError> {
        self.0.call(
            Op::Session {
                project: "alpha".into(),
                method: method.into(),
                fields,
            },
            Duration::from_secs(30),
        )
    }
    /// Polls refresh until `pred(view, context)` holds; returns the last refresh.
    fn until(&self, what: &str, pred: impl Fn(&Value, &Value) -> bool) -> Value {
        let end = Instant::now() + Duration::from_secs(20);
        loop {
            let v = self.call("session.refresh", json!({})).unwrap();
            if pred(&v["view"], &v["context"]) {
                return v;
            }
            assert!(Instant::now() < end, "{what}: {v}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    fn start(&self, executable: &str, share: bool) -> String {
        let mut fields = json!({"executable":executable,"confirmed":true});
        if share {
            fields["share_context"] = true.into();
        }
        self.call("session.start", fields).unwrap();
        let v = self.until("ready", |v, _| {
            v["state"] == "ready" && v["process_alive"] == true
        });
        v["view"]["session_id"].as_str().unwrap().to_owned()
    }
    fn stop(&self, id: &str) {
        self.call("session.stop", json!({"session_id":id})).unwrap();
        self.until("stopped", |v, _| v["process_alive"] == false);
    }
}

#[test]
fn context_lease_opt_in_cancel_failure_and_restart_through_the_bridge() {
    let mut d = Daemon::start_configured(
        &[
            ("crash", &["mcp-crash"]),
            ("ctx", &["mcp"]),
            ("plain", &["permission"]),
            ("slow", &["mcp-cancel"]),
        ],
        true,
        None,
    );
    let c = d.client();
    let b = writable(&d, &c);
    d.put_handoff(&b);
    let s = Sessions(&c);
    // Default: no opt-in, no lease, no MCP servers given to the agent.
    let id = s.start("plain", false);
    let v = s.call("session.refresh", json!({})).unwrap();
    assert_eq!(
        (
            v["context"]["enabled"].clone(),
            v["context"]["active"].clone()
        ),
        (json!(false), json!(false))
    );
    s.stop(&id);
    // Explicit opt-in: an active lease the agent actually uses; Stop revokes it.
    let id = s.start("ctx", true);
    let v = s.call("session.refresh", json!({})).unwrap();
    assert_eq!(
        (
            v["context"]["enabled"].clone(),
            v["context"]["active"].clone()
        ),
        (json!(true), json!(true))
    );
    s.call(
        "session.prompt",
        json!({"session_id":id,"prompt":"use the shared context"}),
    )
    .unwrap();
    let v = s.until("context used", |v, c| {
        v["state"] == "completed"
            && c["context_calls"].as_u64() >= Some(1)
            && c["handoff_calls"].as_u64() >= Some(1)
    });
    assert_eq!(v["context"]["active"], true);
    s.stop(&id);
    s.until("revoked on stop", |_, c| c["active"] == false);
    // Cancel of a RUNNING turn succeeds, ends the turn and revokes the lease.
    let id = s.start("slow", true);
    s.call(
        "session.prompt",
        json!({"session_id":id,"prompt":"wait for cancel"}),
    )
    .unwrap();
    s.until("running", |v, c| {
        v["state"] == "running" && c["active"] == true
    });
    s.call("session.cancel", json!({"session_id":id})).unwrap();
    let v = s.until("cancelled", |v, _| {
        v["state"] == "completed" && v["stop_reason"] == "cancelled"
    });
    assert_eq!(v["context"]["active"], false, "Cancel revokes the lease");
    assert_eq!(
        v["view"]["process_alive"], true,
        "Cancel ends the turn, not the agent"
    );
    s.stop(&id);
    // Child failure revokes the lease.
    let id = s.start("crash", true);
    s.call("session.prompt", json!({"session_id":id,"prompt":"crash"}))
        .unwrap();
    let v = s.until("failed", |v, _| v["process_alive"] == false);
    assert_eq!(
        v["context"]["active"], false,
        "child failure revokes the lease"
    );
    // Daemon restart: no lease or session survives, and only the standard
    // per-launch credentials exist in the runtime.
    let _id = s.start("ctx", true);
    c.close(Duration::from_secs(5));
    d.restart();
    let c = d.client();
    // No Profile adoption after restart: sessions are unavailable until the
    // operator selects the Profile explicitly; then no session or lease survives.
    assert_eq!(
        Sessions(&c).call("session.refresh", json!({})),
        Err(CallError::Remote("Session unavailable"))
    );
    let path = d.root.0.join("state/profile.sqlite3");
    control(&c, "profile.select", json!({"path": path})).unwrap();
    let v = Sessions(&c).call("session.refresh", json!({})).unwrap();
    assert!(v["view"].is_null() && v["context"].is_null(), "{v}");
    let mut caps: Vec<String> = std::fs::read_dir(&d.runtime)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".cap"))
        .collect();
    caps.sort();
    assert_eq!(
        caps,
        ["agent-alpha.cap", "operator.cap"],
        "no leftover lease credential"
    );
    c.close(Duration::from_secs(5));
}

#[test]
fn context_lease_expires_while_the_session_runs() {
    let d = Daemon::start_configured(&[("slow", &["mcp-cancel"])], true, Some(2));
    let c = d.client();
    writable(&d, &c);
    let s = Sessions(&c);
    let id = s.start("slow", true);
    s.until("active", |_, c| c["active"] == true);
    std::thread::sleep(Duration::from_secs(3));
    let v = s.until("expired", |_, c| c["active"] == false);
    assert_eq!(
        v["view"]["process_alive"], true,
        "expiry revokes context, not the agent"
    );
    s.stop(&id);
    c.close(Duration::from_secs(5));
}

#[test]
fn project_check_add_and_remove_through_the_bridge() {
    let d = Daemon::start();
    let c = d.client();
    let gamma = d.root.0.join("gamma");
    std::fs::create_dir_all(gamma.join(".git")).unwrap();
    let gamma = gamma.canonicalize().unwrap();
    let write = |method: &str, params: Value| {
        c.call(
            Op::ProjectWrite {
                method: method.into(),
                params,
            },
            Duration::from_secs(15),
        )
    };
    // The same check answers as over local IPC; agents' methods are refused.
    let check = control(&c, "project.check", json!({"root":gamma})).unwrap();
    assert_eq!(check["ok"], true);
    assert_eq!(check["name"], "gamma");
    assert_eq!(
        control(&c, "project.check", json!({"root":d.root.0.join("state")})).unwrap()["reason"],
        "not_git_root"
    );
    // No write grant yet: the helper finds no write credential.
    assert_eq!(
        write("project.add", json!({"name":"gamma","root":gamma})),
        Err(CallError::Remote("Unauthorized"))
    );
    writable(&d, &c);
    let added = write("project.add", json!({"name":"gamma","root":gamma})).unwrap();
    assert_eq!(added["added"], "gamma");
    assert!(d.runtime.join("agent-gamma.cap").exists());
    assert_eq!(
        write("project.add", json!({"name":"gamma","root":gamma})),
        Err(CallError::Remote("Conflict"))
    );
    assert_eq!(
        write("project.remove", json!({"name":"gamma"})).unwrap()["removed"],
        "gamma"
    );
    assert!(!d.runtime.join("agent-gamma.cap").exists());
    // Only the two project writes travel as ProjectWrite.
    assert_eq!(
        write("profile.write.disable", json!({})),
        Err(CallError::Remote("Unsupported"))
    );
    // Forgetting a startup Profile that was never remembered is harmless.
    assert_eq!(
        control(&c, "profile.startup.forget", json!({})).unwrap()["startup_profile"],
        false
    );
}
