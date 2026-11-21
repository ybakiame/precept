//! The Workspace page over a real daemon: check, add and remove a project.
//! Real NSH components and IPC; not native GUI/Wayland/TTY evidence.
#![cfg(feature = "native")]
use nsh_core::prelude::*;
use nsh_engine::{EngineConfig, Root};
use precept_ipc::{Location, operator};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
/// The daemon's state directory for a test: its projects and, when given, the
/// Profile the operator asked to select at start.
fn daemon_state(
    dir: &std::path::Path,
    projects: &[(&str, &std::path::Path)],
) -> std::path::PathBuf {
    precept_ipc::transport::private_dir(dir, true).unwrap();
    let list: Vec<serde_json::Value> = projects
        .iter()
        .map(|(n, r)| serde_json::json!({"name":n,"root":r.canonicalize().unwrap()}))
        .collect();
    let path = dir.join("projects.json");
    let _ = std::fs::remove_file(&path);
    let mut file = precept_ipc::transport::new_private(&path).unwrap();
    std::io::Write::write_all(
        &mut file,
        serde_json::json!({"format":1,"projects":list})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    dir.to_path_buf()
}
struct Fixture {
    child: Child,
    location: Location,
    root: tempfile::TempDir,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn bin(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("precept-s3-ui-")
            .tempdir()
            .unwrap();
        std::fs::create_dir(root.path().join("alpha")).unwrap();
        std::fs::write(root.path().join("alpha/AGENTS.md"), "Synthetic contract\n").unwrap();
        precept_ipc::transport::private_dir(&root.path().join("state"), true).unwrap();
        let runtime_dir = root.path().join("runtime");
        let endpoint = if cfg!(windows) {
            format!("s3-{}", root.path().file_name().unwrap().to_string_lossy())
        } else {
            runtime_dir.join("control.sock").to_str().unwrap().into()
        };
        let location = Location {
            runtime_dir,
            endpoint,
        };
        let child = Command::new(bin("preceptd"))
            .arg("--runtime-dir")
            .arg(&location.runtime_dir)
            .arg("--endpoint")
            .arg(&location.endpoint)
            .arg("--state-dir")
            .arg(daemon_state(
                &root.path().join("daemon-state"),
                &[("alpha", &root.path().join("alpha"))],
            ))
            // The test's private root is the daemon user's home for scope checks.
            .env("HOME", root.path())
            .env("USERPROFILE", root.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("build workspace bins before NSH process tests");
        let mut f = Self {
            child,
            location,
            root,
        };
        let start = Instant::now();
        loop {
            if f.call("profile.info", json!({})).is_ok() {
                break;
            }
            assert!(f.child.try_wait().unwrap().is_none());
            assert!(start.elapsed() < Duration::from_secs(15));
            std::thread::sleep(Duration::from_millis(20));
        }
        f
    }
    fn call(&self, method: &str, params: Value) -> precept_ipc::Result<Value> {
        tokio_runtime(|| operator::control(&self.location, method, params))
    }
    fn info(&self) -> Value {
        self.call("profile.info", json!({})).unwrap()
    }
    #[allow(dead_code)]
    fn register(&self) {
        let info = self.info();
        let b = &info["binding"];
        let result = Command::new(bin("precept"))
            .arg("--runtime-dir")
            .arg(&self.location.runtime_dir)
            .arg("--endpoint")
            .arg(&self.location.endpoint)
            .arg("project-register")
            .arg("alpha")
            .arg(self.root.path().join("alpha").canonicalize().unwrap())
            .arg("--profile-id")
            .arg(b["profile_id"].as_str().unwrap())
            .arg("--generation")
            .arg(b["generation"].as_str().unwrap())
            .arg("--owner-epoch")
            .arg(b["owner_epoch"].to_string())
            .arg("--operation-id")
            .arg("headless-register")
            .output()
            .unwrap();
        assert!(result.status.success());
    }
}
fn tokio_runtime<F, Fut, T>(f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    // Reuse the existing shared native adapter's synchronous execution boundary.
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f())
}
fn contains(app: &mut nsh_testing::Headless, needle: &str) {
    app.wait_until("sanitized result", Duration::from_secs(10), |s| {
        s.texts().iter().any(|t| t.contains(needle))
    })
    .unwrap();
}
#[test]
fn workspace_adds_a_checked_repository_and_removes_a_project_with_confirmation() {
    let f = Fixture::new();
    let gamma = f.root.path().join("gamma");
    std::fs::create_dir_all(gamma.join(".git")).unwrap();
    std::fs::write(gamma.join("AGENTS.md"), "Synthetic contract.\n").unwrap();
    // A fresh daemon: no Profile, no writes. Everything below is the GUI.
    assert_eq!(f.info()["selected"], false);
    precept_ui::INPUTS
        .set(precept_ui_client::native::Inputs {
            runtime_dir: Some(f.location.runtime_dir.clone()),
            endpoint: Some(f.location.endpoint.clone()),
            ..Default::default()
        })
        .ok()
        .unwrap();
    let mut cfg = EngineConfig::new(Root::of::<precept_ui::App>());
    cfg.viewport = Size::new(px(1080.0), px(900.0));
    let mut app = nsh_testing::Headless::start(cfg).unwrap();
    // A fresh daemon has no usable project yet: Dashboard shows onboarding and
    // Daemon status, and Workspaces is reachable under Runtime checks (alt+2).
    app.wait_for("Daemon status").unwrap();
    app.press_key("alt+2").unwrap();
    app.wait_for("Registered workspaces").unwrap();
    // No legacy bottom summary card on Workspaces.
    assert!(
        !app.screen()
            .shows("Only the daemon's projects; switching clears the task lookup.")
    );
    app.wait_for("Add project…").unwrap();
    app.press("Add project…").unwrap();
    // A folder that is not a repository root is refused by the daemon.
    app.type_text("Folder", f.root.path().to_str().unwrap())
        .unwrap();
    app.press("Check").unwrap();
    contains(
        &mut app,
        "Choose a repository inside your home, not the home itself.",
    );
    app.clear("Folder").unwrap();
    app.type_text("Folder", gamma.canonicalize().unwrap().to_str().unwrap())
        .unwrap();
    app.press("Check").unwrap();
    contains(&mut app, "This folder can be added.");
    assert_eq!(app.screen().input_text("Name").unwrap(), Some("gamma"));
    contains(&mut app, "Set up your Profile first");
    // Profile setup lives cleanly in the Profile tab of Runtime checks.
    app.press("Go to Profile").unwrap();
    app.wait_for("Set up your Profile").unwrap();
    app.press("Create Profile").unwrap();
    let start = Instant::now();
    while f.info()["selected"] != true {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        f.info()["remembered"],
        true,
        "select at start is on by default"
    );
    assert_eq!(
        f.info()["write_enabled"],
        false,
        "creating never enables writes"
    );
    app.press("Workspaces").unwrap();
    // With writes off, the one explicit click enables them for this run and adds.
    app.wait_for("Enable writes and add").unwrap();
    app.press("Enable writes and add").unwrap();
    // The daemon's list now has gamma, ready, and its agent credential.
    contains(&mut app, "gamma");
    let start = Instant::now();
    while !f.location.runtime_dir.join("agent-gamma.cap").exists() {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(f.info()["write_enabled"], true);
    // alpha came from the daemon's set, not this new Profile: one click
    // records it here, and only its row offers that.
    contains(&mut app, "Not in this Profile");
    assert!(!app.screen().shows("Register in this Profile: gamma"));
    app.press("Register in this Profile: alpha").unwrap();
    let start = Instant::now();
    while f.call("project.list", json!({})).unwrap()[0]["state"] != "ready" {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    contains(&mut app, "Registered in this Profile: alpha");
    // Each row's Remove names its project for screen readers.
    app.press("Remove alpha").unwrap();
    app.wait_for("Remove this project?").unwrap();
    app.press("Remove").unwrap();
    let start = Instant::now();
    while f.location.runtime_dir.join("agent-alpha.cap").exists() {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    let listed = f
        .call(
            "project.check",
            json!({"root":gamma.canonicalize().unwrap()}),
        )
        .unwrap();
    assert_eq!(
        listed["ok"], false,
        "gamma is now a project: it overlaps itself"
    );
    // Agents (under Workspace): the launch line for gamma, per client; sharing per project.
    app.press("Agents").unwrap();
    app.wait_for("Configure · precept-gamma").unwrap();
    contains(&mut app, "claude mcp add --scope user precept-gamma -- ");
    contains(&mut app, "--project gamma --capability-file ");
    let token = std::fs::read_to_string(f.location.runtime_dir.join("agent-gamma.cap")).unwrap();
    assert!(
        !app.screen()
            .texts()
            .iter()
            .any(|t| t.contains(token.trim())),
        "never a token on screen"
    );
    app.press("Codex").unwrap();
    contains(&mut app, "[mcp_servers.\"precept-gamma\"]");
    app.press("Copy").unwrap();
    app.wait_for("Copied").unwrap();
    assert!(
        app.screen()
            .clipboard()
            .is_some_and(|c| c.contains("[mcp_servers.\"precept-gamma\"]"))
    );
    app.press("Share gamma with agents").unwrap();
    let start = Instant::now();
    while f.location.runtime_dir.join("agent-gamma.cap").exists() {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    contains(&mut app, "This project is not shared");
    app.press("Share gamma with agents").unwrap();
    let start = Instant::now();
    while !f.location.runtime_dir.join("agent-gamma.cap").exists() {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    // Recent agent activity: empty until a client starts precept-mcp and reads.
    contains(
        &mut app,
        "No agent has used this project since the daemon started",
    );
    {
        use std::io::{BufRead, BufReader, Write};
        let mut mcp = Command::new(bin("precept-mcp"))
            .arg("--runtime-dir")
            .arg(&f.location.runtime_dir)
            .arg("--endpoint")
            .arg(&f.location.endpoint)
            .args(["--project", "gamma", "--capability-file"])
            .arg(f.location.runtime_dir.join("agent-gamma.cap"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut input = mcp.stdin.take().unwrap();
        let mut output = BufReader::new(mcp.stdout.take().unwrap());
        let mut line = String::new();
        for message in [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"headless-agent","version":"2.0"}}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_effective_context","arguments":{}}}),
        ] {
            writeln!(input, "{message}").unwrap();
            if message.get("id").is_some() {
                line.clear();
                output.read_line(&mut line).unwrap();
            }
        }
        assert!(line.contains("\"isError\":false"), "context read");
        let _ = mcp.kill();
        let _ = mcp.wait();
    }
    app.press("Refresh activity").unwrap();
    contains(&mut app, "Last read ");
    contains(&mut app, "by headless-agent 2.0");
    contains(
        &mut app,
        "Read context: 0 preferences, 0 contracts, 1 documents",
    );
    contains(&mut app, "Started precept-mcp");
    // Agent suggestions: off until the operator's switch, then they land in
    // Review (writes are on in this run).
    contains(&mut app, "Agents cannot suggest rules");
    app.press("Agents may suggest rules for gamma").unwrap();
    let start = Instant::now();
    let proposes = |f: &Fixture| {
        f.call("project.list", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["name"] == "gamma" && p["propose"] == true)
    };
    while !proposes(&f) {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    contains(
        &mut app,
        "Agents may suggest rules; each waits for your review",
    );
    {
        use std::io::{BufRead, BufReader, Write};
        let mut mcp = Command::new(bin("precept-mcp"))
            .arg("--runtime-dir")
            .arg(&f.location.runtime_dir)
            .arg("--endpoint")
            .arg(&f.location.endpoint)
            .args(["--project", "gamma", "--capability-file"])
            .arg(f.location.runtime_dir.join("agent-gamma.cap"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut input = mcp.stdin.take().unwrap();
        let mut output = BufReader::new(mcp.stdout.take().unwrap());
        let mut line = String::new();
        for message in [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"headless-agent","version":"2.0"}}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"suggest_rule","arguments":{"scope":"project","key":"commit-style","value":"Use conventional commits."}}}),
        ] {
            writeln!(input, "{message}").unwrap();
            if message.get("id").is_some() {
                line.clear();
                output.read_line(&mut line).unwrap();
            }
        }
        assert!(line.contains("not active"), "suggested: {line}");
        let _ = mcp.kill();
        let _ = mcp.wait();
    }
    app.press("Refresh activity").unwrap();
    contains(&mut app, "Suggested a rule: it waits in Review");
    // A second project, then switch from the ONE sidebar workspace switcher:
    // the read it starts must finish after the menu closes (never stuck Reading).
    let delta = f.root.path().join("delta");
    std::fs::create_dir_all(delta.join(".git")).unwrap();
    app.press_key("alt+2").unwrap();
    app.wait_for("Add project…").unwrap();
    app.press("Add project…").unwrap();
    app.clear("Folder").unwrap();
    app.type_text("Folder", delta.canonicalize().unwrap().to_str().unwrap())
        .unwrap();
    app.press("Check").unwrap();
    contains(&mut app, "This folder can be added.");
    app.press("Add").unwrap();
    contains(&mut app, "delta");
    // Workspace list rows are not clickable project-switch buttons.
    assert_eq!(app.screen().count("role:button[delta]").unwrap(), 0);
    let settled = |app: &mut nsh_testing::Headless| {
        app.wait_until("read settled", Duration::from_secs(10), |s| {
            s.texts().iter().any(|t| t.starts_with("Connected · "))
        })
        .unwrap();
    };
    settled(&mut app);
    // The single sidebar switcher updates Dashboard and Agents cleanly.
    app.press("Dashboard").unwrap();
    app.wait_for("Project: gamma").unwrap();
    for (from, to) in [("gamma", "delta"), ("delta", "gamma")] {
        app.press(from).unwrap();
        app.press(to).unwrap();
        settled(&mut app);
        assert_eq!(
            app.screen()
                .texts()
                .iter()
                .filter(|t| t.starts_with("Reading"))
                .count(),
            0
        );
        app.wait_for(&format!("Project: {to}")).unwrap();
    }
    // The Agents page follows the same sidebar workspace switcher.
    app.press("Agents").unwrap();
    app.wait_for("Configure · precept-gamma").unwrap();
    app.press("gamma").unwrap();
    app.press("delta").unwrap();
    settled(&mut app);
    app.wait_for("Configure · precept-delta").unwrap();
    // One switch adds the Claude Code hook to the user config (HOME is the
    // test root), another takes it out again; the rest of the file stays.
    let settings = f.root.path().join(".claude/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    std::fs::write(&settings, "{\n  \"model\": \"keep-me\"\n}\n").unwrap();
    contains(&mut app, "The hooks run:");
    // The page read the file before it existed: the first switch is refused
    // and says so; the file is read again, and the second one applies.
    app.press("Claude Code session-start hook").unwrap();
    contains(&mut app, "The file changed since it was read");
    assert_eq!(
        std::fs::read_to_string(&settings).unwrap(),
        "{\n  \"model\": \"keep-me\"\n}\n"
    );
    app.press("Claude Code session-start hook").unwrap();
    contains(&mut app, "Briefs each session · ~/.claude/settings.json");
    let text = std::fs::read_to_string(&settings).unwrap();
    assert!(text.contains("keep-me") && text.contains("--vendor claude --project delta"));
    app.press("Claude Code session-start hook").unwrap();
    let start = Instant::now();
    while std::fs::read_to_string(&settings)
        .unwrap()
        .contains("precept-hook")
    {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        std::fs::read_to_string(&settings)
            .unwrap()
            .contains("keep-me")
    );
    // The AGENTS.md switch: previewed before writing, removed at once.
    let agents_md = f.root.path().join("delta/AGENTS.md");
    assert!(!agents_md.exists());
    app.press("Precept section in AGENTS.md").unwrap();
    app.wait_for("Add the Precept section to AGENTS.md?")
        .unwrap();
    contains(
        &mut app,
        "call `get_effective_context` on the `precept-delta` MCP server",
    );
    app.press("Write to AGENTS.md").unwrap();
    contains(&mut app, "In AGENTS.md · review and commit it");
    let written = std::fs::read_to_string(&agents_md).unwrap();
    assert!(written.starts_with("<!-- precept:begin v1 -->"));
    app.press("Precept section in AGENTS.md").unwrap();
    let start = Instant::now();
    while agents_md.exists() {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
}
