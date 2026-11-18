//! S1-B: Windows-native HWND GUI -> WSL2 daemon end-to-end acceptance.
//!
//! The GUI runs on Windows with `--wsl-*` arguments and reaches the daemon only
//! through its persistent `acp-wsl-bridge` helper. Independent observations use
//! the WSL-side CLI through `wsl.exe --exec` (no shell). All state is a fresh
//! synthetic run directory inside the disposable test distribution.
use super::{Owned, Ui, desktop, evidence};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Declared S1-B cells; every one must end PASS for an acceptance PASS.
pub const CELLS: &[&str] = &[
    "wsl-connect-no-implicit-state",
    "wsl-profile-create-without-write",
    "wsl-write-grant-enable",
    "wsl-dashboard-context-handoff",
    "wsl-suggest-inspect-confirm",
    "wsl-reject",
    "wsl-deprecate",
    "wsl-delete-exact-retype",
    "wsl-bridge-loss-outcome-unknown-identical-retry",
    "wsl-session-lifecycle-agent-in-wsl",
    "wsl-no-listener-no-shell",
    "wsl-write-grant-disable",
    "wsl-gui-close-keeps-daemon-and-state",
    "wsl-daemon-absent-and-explicit-recovery",
    "wsl-wrong-target-explicit-unavailable",
    "wsl-live-link-distribution-stopped",
];

pub struct Wsl {
    pub distribution: String,
    pub user: String,
    /// Absolute Linux directory holding bin/{acp-daemon,acp-query,acp-wsl-bridge,acp-fake-agent}.
    pub prefix: String,
}
fn wsl_exe() -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").expect("Windows SystemRoot"))
        .join("System32/wsl.exe")
}
impl Wsl {
    fn command(&self, exe: &str, args: &[&str]) -> Command {
        assert!(exe.starts_with('/'), "absolute Linux executable only");
        let mut c = Command::new(wsl_exe());
        c.args([
            "--distribution",
            &self.distribution,
            "--user",
            &self.user,
            "--exec",
            exe,
        ])
        .args(args);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x08000000);
        }
        c
    }
    fn run(&self, exe: &str, args: &[&str]) -> (bool, String) {
        let out = self
            .command(exe, args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).trim().to_owned(),
        )
    }
    fn ok(&self, exe: &str, args: &[&str]) -> String {
        let (ok, out) = self.run(exe, args);
        assert!(
            ok,
            "WSL command failed: {}",
            exe.rsplit('/').next().unwrap()
        );
        out
    }
    fn write(&self, path: &str, bytes: &[u8]) {
        let mut child = self
            .command("/usr/bin/tee", &[path])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(bytes).unwrap();
        assert!(child.wait().unwrap().success());
    }
    fn bin(&self, name: &str) -> String {
        format!("{}/bin/{name}", self.prefix)
    }
    /// PIDs of this user's processes with an exact command name.
    fn pids(&self, name: &str) -> Vec<String> {
        let (_, out) = self.run("/usr/bin/pgrep", &["-u", &self.user, "-x", name]);
        out.lines().map(str::to_owned).collect()
    }
}

/// SHA-256 of the Linux product binaries actually executed, taken inside WSL.
pub fn linux_binaries(wsl: &Wsl) -> Value {
    let names = [
        "acp-daemon",
        "acp-query",
        "acp-wsl-bridge",
        "acp-fake-agent",
        "acp-mcp",
    ];
    let paths: Vec<String> = names.iter().map(|n| wsl.bin(n)).collect();
    let mut args = vec!["--"];
    args.extend(paths.iter().map(String::as_str));
    let out = wsl.ok("/usr/bin/sha256sum", &args);
    json!(
        out.lines()
            .map(|l| {
                let (hash, path) = l.split_once("  ").unwrap();
                json!({"name": path.rsplit('/').next().unwrap(), "sha256": hash})
            })
            .collect::<Vec<_>>()
    )
}

/// Windows-side count of wsl.exe processes: proves no per-request launches.
fn windows_wsl_processes() -> usize {
    let out = Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq wsl.exe", "/FO", "CSV", "/NH"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.to_ascii_lowercase().contains("wsl.exe"))
        .count()
}

struct Remote {
    wsl: Wsl,
    base: String,
    runtime: String,
    endpoint: String,
    alpha: String,
    db: String,
    daemon: Option<Owned>,
}
impl Remote {
    fn new(wsl: Wsl, nonce: &str) -> Self {
        let base = format!("{}/run-{nonce}", wsl.prefix);
        wsl.ok("/usr/bin/install", &["-d", "-m", "0700", &base]);
        wsl.ok(
            "/usr/bin/install",
            &["-d", "-m", "0700", &format!("{base}/state")],
        );
        wsl.ok("/usr/bin/mkdir", &[&format!("{base}/alpha")]);
        wsl.write(
            &format!("{base}/alpha/AGENTS.md"),
            b"Synthetic S1-B contract\n",
        );
        let config = format!("{base}/agents.toml");
        wsl.write(
            &config,
            toml::to_string(&json!({"version":1,"programs":[
                {"id":"ctx","executable":wsl.bin("acp-fake-agent"),"args":["mcp"],"env":{}},
                {"id":"fake","executable":wsl.bin("acp-fake-agent"),"args":["permission"],"env":{}},
                {"id":"slow","executable":wsl.bin("acp-fake-agent"),"args":["mcp-cancel"],"env":{}}
            ]}))
            .unwrap()
            .as_bytes(),
        );
        wsl.ok("/usr/bin/chmod", &["0600", &config]);
        let alpha = wsl.ok("/usr/bin/realpath", &[&format!("{base}/alpha")]);
        let runtime = format!("{base}/runtime");
        let mut r = Self {
            endpoint: format!("{runtime}/control.sock"),
            db: format!("{base}/state/profile.sqlite3"),
            runtime,
            alpha,
            base,
            wsl,
            daemon: None,
        };
        r.start_daemon();
        r
    }
    /// Explicit daemon start inside WSL (never performed by the GUI or bridge).
    fn start_daemon(&mut self) {
        let config = format!("{}/agents.toml", self.base);
        // The daemon's private state: alpha is its one project.
        let state = format!("{}/daemon-state", self.base);
        self.wsl
            .ok("/usr/bin/install", &["-d", "-m", "0700", &state]);
        let projects = format!("{state}/projects.json");
        self.wsl.ok("/usr/bin/rm", &["-f", &projects]);
        self.wsl
            .ok("/usr/bin/install", &["-m", "0600", "/dev/null", &projects]);
        self.wsl.write(
            &projects,
            json!({"format":1,"projects":[{"name":"alpha","root":self.alpha}]})
                .to_string()
                .as_bytes(),
        );
        let child = self
            .wsl
            .command(
                &self.wsl.bin("acp-daemon"),
                &[
                    "--runtime-dir",
                    &self.runtime,
                    "--endpoint",
                    &self.endpoint,
                    "--state-dir",
                    &state,
                    "--agent-config",
                    &config,
                    // Trusted read-only context bridge; sharing still needs a
                    // per-start operator opt-in.
                    "--context-mcp",
                    &self.wsl.bin("acp-mcp"),
                ],
            )
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        self.daemon = Some(Owned(child));
        let end = Instant::now() + Duration::from_secs(60);
        while !self.query(&["health"]).0 {
            assert!(Instant::now() < end, "WSL daemon readiness");
            thread::sleep(Duration::from_millis(200));
        }
    }
    fn daemon_pid(&self) -> String {
        let pids = self.wsl.pids("acp-daemon");
        assert_eq!(pids.len(), 1, "exactly one owned WSL daemon");
        pids[0].clone()
    }
    /// Graceful stop: SIGINT (Ctrl+C) to the exact daemon process inside WSL.
    fn stop_daemon(&mut self) {
        let pid = self.daemon_pid();
        self.wsl.ok("/usr/bin/kill", &["-INT", &pid]);
        let end = Instant::now() + Duration::from_secs(15);
        while !self.wsl.pids("acp-daemon").is_empty() {
            assert!(Instant::now() < end, "graceful WSL daemon exit");
            thread::sleep(Duration::from_millis(100));
        }
        self.daemon = None;
    }
    fn query(&self, args: &[&str]) -> (bool, Value) {
        let mut all = vec!["--runtime-dir", &self.runtime, "--endpoint", &self.endpoint];
        all.extend_from_slice(args);
        let (ok, out) = self.wsl.run(&self.wsl.bin("acp-query"), &all);
        (ok, serde_json::from_str(&out).unwrap_or(Value::Null))
    }
    fn info(&self) -> Value {
        self.query(&["profile", "info"]).1
    }
    fn mutate(&self, op: &str, args: &[&str]) {
        let b = self.info()["binding"].clone();
        let epoch = b["owner_epoch"].to_string();
        let mut all = args.to_vec();
        all.extend([
            "--profile-id",
            b["profile_id"].as_str().unwrap(),
            "--generation",
            b["generation"].as_str().unwrap(),
            "--owner-epoch",
            &epoch,
            "--operation-id",
            op,
        ]);
        let (ok, v) = self.query(&all);
        assert!(ok, "CLI-only mutation {op} refused: {}", v["error"]);
    }
    fn rows(&self) -> Vec<Value> {
        let v = self
            .query(&[
                "preferences",
                "list",
                "--scope",
                "project",
                "--project",
                "alpha",
            ])
            .1;
        v["records"].as_array().cloned().unwrap_or_default()
    }
    fn record(&self, id: i64) -> Value {
        self.rows()
            .into_iter()
            .find(|r| r["id"] == id)
            .unwrap_or(Value::Null)
    }
    fn commit_sequence(&self) -> i64 {
        self.query(&["review-queue", "--scope", "project", "--project", "alpha"])
            .1["commit_sequence"]
            .as_i64()
            .unwrap()
    }
    fn state_digest(&self) -> String {
        self.wsl.ok("/usr/bin/sha256sum", &[&self.db])
    }
}

fn gui_args(r: &Remote) -> Vec<String> {
    [
        "--wsl-distribution",
        &r.wsl.distribution,
        "--wsl-user",
        &r.wsl.user,
        "--wsl-bridge",
        &r.wsl.bin("acp-wsl-bridge"),
        "--wsl-runtime-dir",
        &r.runtime,
        "--wsl-endpoint",
        &r.endpoint,
        "--operator",
        "--locale",
        "en",
        "--appearance",
        "light",
        "--project",
        "alpha",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Alt E cycles the configured programs (ctx, fake, slow) until `name` is
/// selected. Each press waits for the GUI's own fresh probe line before deciding.
fn select_program(ui: &mut Ui, name: &str) {
    let Ui::Desktop(d) = ui else {
        panic!("desktop only")
    };
    for _ in 0..4 {
        let before = d.lines.lock().unwrap().len();
        d.key("alt+e");
        let end = Instant::now() + Duration::from_secs(5);
        let selected = loop {
            let line = d.lines.lock().unwrap()[before..]
                .iter()
                .rev()
                .find(|l| l.starts_with("ACP_NSH_CONTEXT exe="))
                .cloned();
            if let Some(line) = line {
                break line;
            }
            assert!(Instant::now() < end, "Alt E produced no program change");
            thread::sleep(Duration::from_millis(25));
        };
        if selected.starts_with(&format!("ACP_NSH_CONTEXT exe={name} ")) {
            evidence::event("program-selected", json!({"program":name,"key":"alt+e"}));
            return;
        }
    }
    panic!("program {name} not selectable with Alt E");
}

fn timed(label: &str, f: impl FnOnce()) {
    let t = Instant::now();
    f();
    evidence::event(
        "timing",
        json!({"step":label,"ms":t.elapsed().as_millis() as u64}),
    );
}

pub fn run(gui: &Path, out: &Path, script: &str, wsl: Wsl) {
    // The window worker accepts exactly 16 lowercase hex digits per window.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    let nonce = format!("{nanos:016x}");
    let relaunch_nonce = format!("{:016x}", nanos.wrapping_add(1));
    let focus = out.join("focus.events");
    fs::write(&focus, "").unwrap();
    let wsl_before = windows_wsl_processes();
    let mut r = Remote::new(wsl, &nonce);
    evidence::event(
        "wsl-target",
        json!({"distribution":r.wsl.distribution,"user":r.wsl.user,"daemon_pid":r.daemon_pid()}),
    );
    assert!(
        r.wsl.pids("acp-wsl-bridge").is_empty(),
        "no bridge before the GUI"
    );

    evidence::begin("wsl-connect-no-implicit-state", "actual-os-input + wsl-cli");
    let started = Instant::now();
    let mut ui = Ui::Desktop(Box::new(desktop::Desktop::start(
        gui,
        &gui_args(&r),
        &nonce,
        script,
        out,
        &focus,
    )));
    evidence::event(
        "owned-ui",
        json!({"pid":ui.pid(),"nonce":nonce,"target":"wsl"}),
    );
    ui.key("alt+o");
    ui.wait(
        "selected=false grant=false busy=false",
        "No Profile selected",
    );
    evidence::event(
        "timing",
        json!({"step":"gui-launch-to-operator-state","ms":started.elapsed().as_millis() as u64}),
    );
    let bridge = r.wsl.pids("acp-wsl-bridge");
    assert_eq!(bridge.len(), 1, "one persistent bridge per connection");
    assert_eq!(r.info()["selected"], false);
    ui.capture("wsl-unselected", out);
    evidence::finish("PASS");

    evidence::begin(
        "wsl-profile-create-without-write",
        "actual-os-input + wsl-cli",
    );
    ui.field("alt+p", &r.db);
    ui.key("alt+shift+u");
    ui.wait("review=true", "Prepared operation");
    ui.capture("wsl-create-review", out);
    timed("profile-create-apply", || {
        ui.key("alt+shift+v");
        ui.wait("selected=true grant=false busy=false", "Profile selected");
    });
    assert_eq!(
        (
            r.info()["selected"].clone(),
            r.info()["write_enabled"].clone()
        ),
        (json!(true), json!(false))
    );
    evidence::finish("PASS");

    evidence::begin("wsl-write-grant-enable", "actual-os-input + wsl-cli");
    ui.apply(
        "alt+shift+e",
        "selected=true grant=true busy=false",
        "Write grant active",
    );
    assert_eq!(r.info()["write_enabled"], true);
    evidence::finish("PASS");

    // CLI-only workflows stay CLI-only (inside WSL).
    r.mutate(
        "s1b-register-alpha",
        &["project-register", "alpha", &r.alpha.clone()],
    );
    let handoff = format!("{}/handoff.json", r.base);
    r.wsl.write(&handoff, json!({"objective":"Saved synthetic S1-B history","completed":"","next_action":"Inspect current repository facts","validation_json":"[{\"status\":\"NOT_RUN\",\"check\":\"synthetic\"}]","snapshot":{"base_sha":"","branch":"","state":"unknown","fingerprint":null}}).to_string().as_bytes());
    r.mutate(
        "s1b-handoff",
        &[
            "handoff-put",
            "alpha",
            "shared",
            "--expected-absent",
            "--input",
            &handoff,
        ],
    );

    evidence::begin("wsl-dashboard-context-handoff", "actual-os-input + wsl-cli");
    ui.resize(1080, 1000);
    timed("reconnect-to-ready", || {
        ui.key("ctrl+r");
        ui.wait("phase=Ready", "alpha");
    });
    assert_eq!(
        r.wsl.pids("acp-wsl-bridge"),
        bridge,
        "reconnect reuses the live bridge"
    );
    ui.capture("wsl-ready", out);
    ui.field("alt+t", "shared");
    ui.key("alt+shift+h");
    ui.wait("handoff_revision=1", "Saved synthetic S1-B history");
    ui.capture("wsl-handoff", out);
    ui.key("alt+shift+r");
    ui.wait("selected=true grant=true busy=false", "Write grant active");
    evidence::finish("PASS");

    evidence::begin("wsl-suggest-inspect-confirm", "actual-os-input + wsl-cli");
    ui.field("alt+k", "editor");
    ui.field("alt+w", "synthetic-value");
    ui.field("alt+f", "synthetic-source");
    let n = r.commit_sequence() + 1;
    timed("suggest-apply", || ui.receipt("alt+shift+s", n));
    assert_eq!(r.rows()[0]["status"], "suggested");
    ui.key("alt+shift+l");
    ui.wait("rows=1", "#1");
    ui.key("alt+shift+i");
    ui.wait("record_revision=1", "#1");
    ui.key("alt+shift+y");
    ui.wait("inspected_id=1 inspected_revision=1", "Show full digest");
    ui.capture("wsl-inspected", out);
    ui.receipt("alt+shift+y", n + 1);
    assert_eq!(r.record(1)["status"], "confirmed");
    evidence::finish("PASS");

    evidence::begin("wsl-reject", "actual-os-input + wsl-cli");
    let n = r.commit_sequence() + 1;
    ui.receipt("alt+shift+s", n);
    ui.key("alt+shift+l");
    ui.wait("rows=2", "#2");
    ui.key("alt+shift+i");
    ui.key("alt+shift+i");
    ui.wait("record_revision=1", "#2");
    ui.key("alt+shift+n");
    ui.wait("inspected_id=2 inspected_revision=1", "Show full digest");
    ui.receipt("alt+shift+n", n + 1);
    assert_eq!(r.record(2)["status"], "rejected");
    evidence::finish("PASS");

    evidence::begin("wsl-deprecate", "actual-os-input + wsl-cli");
    ui.key("alt+shift+l");
    ui.wait("rows=2", "#2");
    ui.key("alt+shift+i");
    ui.wait("record_revision=2", "#1");
    ui.key("alt+shift+w");
    ui.wait("inspected_id=1 inspected_revision=2", "Show full digest");
    let n = r.commit_sequence() + 1;
    ui.receipt("alt+shift+w", n);
    assert_eq!(r.record(1)["status"], "deprecated");
    evidence::finish("PASS");

    evidence::begin("wsl-delete-exact-retype", "actual-os-input + wsl-cli");
    ui.key("alt+shift+l");
    ui.wait("rows=2", "#2");
    ui.key("alt+shift+i");
    ui.wait("record_revision=3", "#1");
    ui.key("alt+shift+q");
    ui.wait("inspected_id=1 inspected_revision=3", "Show full digest");
    // A wrong retype never prepares a delete.
    ui.key("alt+shift+q");
    ui.wait(
        "ACP_UI_DIALOG opened=true",
        "Delete project alpha preference #1?",
    );
    ui.text("9");
    ui.wait("delete_retype_chars=1 delete_ready=false", "Retype");
    ui.key("enter");
    thread::sleep(Duration::from_millis(300));
    assert_eq!(
        r.record(1)["status"],
        "deprecated",
        "a wrong retype must not write"
    );
    ui.key("escape");
    ui.wait("delete_armed=false", "Show full digest");
    ui.key("alt+shift+q");
    ui.wait(
        "ACP_UI_DIALOG opened=true",
        "Delete project alpha preference #1?",
    );
    ui.text("1");
    ui.wait(
        "delete_retype_chars=1 delete_ready=true",
        "Deletion armed; apply to commit",
    );
    let n = r.commit_sequence() + 1;
    ui.key("alt+shift+v");
    ui.wait(&format!("receipt={n} "), "Deleted.");
    assert!(r.record(1).is_null(), "record #1 deleted");
    ui.capture("wsl-deleted", out);
    evidence::finish("PASS");

    evidence::begin(
        "wsl-bridge-loss-outcome-unknown-identical-retry",
        "actual-os-input + wsl-process-fault + wsl-cli",
    );
    ui.key("alt+shift+r");
    ui.wait("selected=true grant=true busy=false", "Write grant active");
    ui.field("alt+k", "uncertain");
    ui.field("alt+w", "synthetic-value");
    ui.field("alt+f", "synthetic-source");
    ui.key("alt+shift+s");
    ui.wait("review=true", "Prepared operation");
    let before = r.rows();
    // Freeze the helper, submit, then kill it: the request was written to the
    // bridge but no answer can arrive, so the outcome is unknown to the GUI.
    let helper = r.wsl.pids("acp-wsl-bridge");
    assert_eq!(helper.len(), 1);
    r.wsl.ok("/usr/bin/kill", &["-STOP", &helper[0]]);
    ui.key("alt+shift+v");
    thread::sleep(Duration::from_millis(500));
    r.wsl.ok("/usr/bin/kill", &["-KILL", &helper[0]]);
    ui.wait("unknown=true", "Outcome unknown");
    ui.capture("wsl-outcome-unknown", out);
    thread::sleep(Duration::from_secs(2));
    assert!(
        r.wsl.pids("acp-wsl-bridge").is_empty(),
        "no automatic bridge restart"
    );
    assert_eq!(r.rows().len(), before.len(), "no automatic replay");
    // Explicit identical retry: restarts the bridge and settles exactly once.
    let n = r.commit_sequence() + 1;
    timed("retry-with-bridge-restart", || {
        ui.key("alt+shift+z");
        ui.wait(&format!("receipt={n} "), &format!("commit {n}"));
    });
    assert_eq!(r.rows().len(), before.len() + 1, "settled exactly once");
    let restarted = r.wsl.pids("acp-wsl-bridge");
    assert_eq!(restarted.len(), 1);
    assert_ne!(restarted, helper);
    evidence::event(
        "bridge-restart",
        json!({"old_helper_killed":true,"new_helper":true,"rows_added":1}),
    );
    evidence::finish("PASS");

    evidence::begin(
        "wsl-session-lifecycle-agent-in-wsl",
        "actual-os-input + wsl-process",
    );
    ui.key("alt+6");
    ui.wait("configured=3", "ctx");
    ui.capture("wsl-sessions", out);
    // 1) `fake` without context sharing: the opt-in is off by default.
    select_program(&mut ui, "fake");
    ui.key("alt+7");
    ui.wait("confirm=true", "Confirm start");
    ui.wait("share=false enabled=", "off");
    timed("session-start", || {
        ui.key("alt+8");
        ui.wait("state=Some(Ready) alive=true", "Ready");
    });
    ui.wait("share=false enabled=false active=false", "not shared");
    let agents = r.wsl.pids("acp-fake-agent");
    assert_eq!(agents.len(), 1, "agent process runs inside WSL");
    let helper = r.wsl.pids("acp-wsl-bridge");
    let wsl_during = windows_wsl_processes();
    ui.key("alt+j");
    ui.text("first synthetic turn");
    ui.key("alt+9");
    ui.wait("pending=1", "Synthetic test operation");
    ui.capture("wsl-permission", out);
    ui.key("alt+i");
    ui.key("alt+y");
    ui.wait("state=Some(Completed) alive=true pending=0", "Completed");
    // While the agent is alive: no listener owned by any of this user's processes.
    let listeners = r
        .wsl
        .ok("/usr/bin/ss", &["-H", "-l", "-t", "-u", "-n", "-p"]);
    for name in ["acp-wsl-bridge", "acp-daemon", "acp-fake-agent"] {
        assert!(!listeners.contains(name), "{name} owns a network listener");
    }
    evidence::event(
        "no-listener-with-live-agent",
        json!({"checked":["acp-wsl-bridge","acp-daemon","acp-fake-agent"]}),
    );
    ui.key("alt+j");
    ui.text("second synthetic turn");
    ui.key("alt+9");
    ui.wait("pending=1", "Synthetic test operation");
    ui.key("alt+i");
    ui.key("alt+n");
    ui.wait("pending=0", "Completed");
    // ~10 polling intervals over the same bridge, with no new WSL processes.
    thread::sleep(Duration::from_secs(8));
    assert_eq!(
        r.wsl.pids("acp-wsl-bridge"),
        helper,
        "polling reuses the bridge"
    );
    assert_eq!(windows_wsl_processes(), wsl_during, "no wsl.exe per poll");
    ui.key("alt+x");
    ui.wait("alive=false", "Stopped");
    assert!(
        r.wsl.pids("acp-fake-agent").is_empty(),
        "agent stopped in WSL"
    );

    // 2) `ctx` with the separately confirmed opt-in: an active lease the agent
    //    uses through the trusted context bridge; Stop revokes it.
    select_program(&mut ui, "ctx");
    ui.key("alt+7");
    ui.wait("confirm=true", "Confirm start");
    ui.wait("share=false enabled=", "off");
    ui.key("alt+g");
    ui.wait("share=true enabled=", "ON");
    ui.capture("wsl-context-opt-in", out);
    ui.key("alt+8");
    ui.wait("state=Some(Ready) alive=true", "Ready");
    ui.wait("share=false enabled=true active=true", "active");
    ui.key("alt+j");
    ui.text("use the shared context");
    ui.key("alt+9");
    ui.wait("active=true context_calls=1 handoff_calls=1", "active");
    ui.wait("state=Some(Completed) alive=true", "Completed");
    ui.capture("wsl-context-lease-used", out);
    ui.key("alt+x");
    ui.wait("enabled=true active=false", "revoked");
    ui.wait("alive=false", "Stopped");
    evidence::event(
        "context-lease",
        json!({"opt_in":"alt+g","active_before_stop":true,"calls":{"context":1,"handoff":1},"revoked_on_stop":true}),
    );

    // 3) Cancel of a RUNNING turn from the GUI, with an active lease: the turn
    //    ends as cancelled, the agent stays alive and the lease is revoked.
    select_program(&mut ui, "slow");
    ui.key("alt+7");
    ui.wait("share=false enabled=", "off");
    ui.key("alt+g");
    ui.wait("share=true enabled=", "ON");
    ui.key("alt+8");
    ui.wait("share=false enabled=true active=true", "active");
    ui.key("alt+j");
    ui.text("wait for cancellation");
    ui.key("alt+9");
    ui.wait("state=Some(Running) alive=true", "Running");
    timed("cancel-running-turn", || {
        ui.key("alt+q");
        ui.wait("enabled=true active=false", "revoked");
    });
    ui.wait("stop_reason=Some(\"cancelled\")", "cancelled");
    ui.capture("wsl-cancelled", out);
    assert_eq!(
        r.wsl.pids("acp-fake-agent").len(),
        1,
        "Cancel ends the turn, not the agent"
    );
    ui.key("alt+x");
    ui.wait("alive=false", "Stopped");
    assert!(
        r.wsl.pids("acp-fake-agent").is_empty(),
        "agent stopped in WSL"
    );
    evidence::event(
        "cancel",
        json!({"running_turn_cancelled":true,"stop_reason":"cancelled","lease_revoked":true,"agent_kept_until_stop":true}),
    );
    evidence::finish("PASS");

    evidence::begin("wsl-no-listener-no-shell", "wsl-process-inspection");
    let listeners = r
        .wsl
        .ok("/usr/bin/ss", &["-H", "-l", "-t", "-u", "-n", "-p"]);
    for name in ["acp-wsl-bridge", "acp-daemon"] {
        assert!(!listeners.contains(name), "{name} owns a network listener");
    }
    // The helper is exec'd directly by WSL interop, never through a shell, and
    // its argv carries only the runtime and endpoint (no token, no project data).
    let helper_pid = r.wsl.pids("acp-wsl-bridge")[0].clone();
    let parent = r.wsl.ok("/usr/bin/ps", &["-o", "ppid=", "-p", &helper_pid]);
    let parent_comm = r
        .wsl
        .ok("/usr/bin/ps", &["-o", "comm=", "-p", parent.trim()]);
    assert!(
        !["sh", "bash", "dash", "zsh", "fish"].contains(&parent_comm.trim()),
        "helper parent is a shell: {parent_comm}"
    );
    let cmdline = r
        .wsl
        .ok("/usr/bin/cat", &[&format!("/proc/{helper_pid}/cmdline")]);
    let argv: Vec<&str> = cmdline.split('\0').filter(|a| !a.is_empty()).collect();
    assert_eq!(
        argv.len(),
        5,
        "helper argv: exe --runtime-dir R --endpoint E"
    );
    assert!(
        argv[0].ends_with("/acp-wsl-bridge")
            && argv[1] == "--runtime-dir"
            && argv[3] == "--endpoint"
    );
    evidence::event(
        "process-boundary",
        json!({"listeners_owned":0,"helper_parent":parent_comm.trim(),"helper_argc":argv.len()}),
    );
    evidence::finish("PASS");

    evidence::begin("wsl-write-grant-disable", "actual-os-input + wsl-cli");
    ui.key("alt+o");
    ui.apply(
        "alt+shift+x",
        "selected=true grant=false busy=false",
        "Write grant off",
    );
    assert_eq!(r.info()["write_enabled"], false);
    evidence::finish("PASS");

    evidence::begin(
        "wsl-gui-close-keeps-daemon-and-state",
        "actual-os-input + wsl-process",
    );
    let digest = r.state_digest();
    let daemon = r.daemon_pid();
    ui.close();
    let end = Instant::now() + Duration::from_secs(15);
    while !r.wsl.pids("acp-wsl-bridge").is_empty() {
        assert!(Instant::now() < end, "bridge helper exits after GUI close");
        thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        r.daemon_pid(),
        daemon,
        "GUI close never stops the WSL daemon"
    );
    assert_eq!(r.state_digest(), digest);
    assert!(r.query(&["health"]).0);
    evidence::finish("PASS");

    evidence::begin(
        "wsl-daemon-absent-and-explicit-recovery",
        "actual-os-input + wsl-process",
    );
    let mut ui = Ui::Desktop(Box::new(desktop::Desktop::start(
        gui,
        &gui_args(&r),
        &relaunch_nonce,
        script,
        out,
        &focus,
    )));
    ui.key("alt+o");
    ui.wait("selected=true grant=false busy=false", "Profile selected");
    r.stop_daemon();
    // Detected live, with no user action: the watcher's health check over the
    // still-running bridge reports the stopped daemon.
    ui.wait(
        "ACP_UI_WSL link=Some(\"WSL daemon not running or not reachable\")",
        "unavailable",
    );
    ui.capture("wsl-daemon-stopped-live", out);
    ui.key("ctrl+r");
    ui.wait("phase=Unavailable", "unavailable");
    ui.wait(
        "failure=Some(\"WSL daemon not running or not reachable\")",
        "unavailable",
    );
    ui.capture("wsl-daemon-absent", out);
    // Explicit restart: no Profile is adopted; the operator selects it again.
    r.start_daemon();
    assert_eq!(r.info()["selected"], false);
    ui.key("alt+o");
    ui.wait(
        "selected=false grant=false busy=false",
        "No Profile selected",
    );
    ui.field("alt+p", &r.db);
    ui.key("alt+shift+m");
    ui.wait("review=true", "Prepared operation");
    ui.key("alt+shift+v");
    ui.wait("selected=true grant=false busy=false", "Profile selected");
    ui.key("ctrl+r");
    ui.wait("phase=Ready", "alpha");
    ui.close();
    evidence::finish("PASS");

    evidence::begin("wsl-wrong-target-explicit-unavailable", "actual-os-input");
    // Wrong distribution, wrong Linux user, wrong runtime: explicit unavailable,
    // never a fixture or a local Windows daemon.
    // Each wrong input names its exact cause (from wsl.exe's exit status and the
    // bridge), never a generic "local daemon" message, fixture or local fallback.
    for (i, (field, value, cause)) in [
        (
            1,
            "acp-s1b-missing".to_owned(),
            "WSL distribution not found",
        ),
        (
            3,
            "acp-nobody".to_owned(),
            "WSL user not available in this distribution",
        ),
        (
            5,
            format!("{}/bin/acp-missing-helper", r.wsl.prefix),
            "Bridge helper not found or not executable in WSL",
        ),
        (
            7,
            format!("{}/absent", r.base),
            "WSL daemon not running or not reachable",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut args = gui_args(&r);
        args[field] = value;
        let nonce = format!("{:016x}", nanos.wrapping_add(2 + i as u64));
        let mut ui = Ui::Desktop(Box::new(desktop::Desktop::start(
            gui, &args, &nonce, script, out, &focus,
        )));
        ui.wait("phase=Unavailable", "unavailable");
        ui.wait(&format!("failure=Some(\"{cause}\")"), "unavailable");
        evidence::event("wrong-target-cause", json!({"case":i,"cause":cause}));
        ui.capture(&format!("wsl-wrong-target-{i}"), out);
        ui.close();
    }
    evidence::finish("PASS");

    evidence::begin(
        "wsl-live-link-distribution-stopped",
        "actual-os-input + wsl-terminate",
    );
    // Connected GUI, then the disposable distribution is terminated (daemon and
    // helper die). The GUI reports the lost bridge by itself, without restarting it.
    let mut ui = Ui::Desktop(Box::new(desktop::Desktop::start(
        gui,
        &gui_args(&r),
        &format!("{:016x}", nanos.wrapping_add(9)),
        script,
        out,
        &focus,
    )));
    ui.key("alt+o");
    ui.wait("selected=true grant=false busy=false", "Profile selected");
    ui.wait("ACP_UI_WSL link=None", "Profile selected");
    let terminated = Command::new(wsl_exe())
        .args(["--terminate", &r.wsl.distribution])
        .status()
        .unwrap();
    assert!(terminated.success());
    r.daemon = None;
    ui.wait(
        "ACP_UI_WSL link=Some(\"WSL bridge disconnected\")",
        "disconnected",
    );
    ui.capture("wsl-bridge-disconnected-live", out);
    thread::sleep(Duration::from_secs(3));
    assert!(
        r.wsl.pids("acp-wsl-bridge").is_empty(),
        "no automatic bridge restart"
    );
    ui.close();
    evidence::finish("PASS");

    evidence::event(
        "windows-wsl-processes",
        json!({"before":wsl_before,"after":windows_wsl_processes()}),
    );
}
