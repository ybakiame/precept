//! The Workspace page over a real daemon: check, add and remove a project.
//! Real NSH components and IPC; not native GUI/Wayland/TTY evidence.
#![cfg(feature = "native")]
#[path = "support/patience.rs"]
mod patience;
use nsh_core::prelude::*;
use nsh_engine::{EngineConfig, Root};
use patience::Patient;
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
        .map(|(n, r)| {
            let root = r.canonicalize().unwrap();
            let identity = precept_core::repo_fs::Repository::identity(&root).unwrap();
            serde_json::json!({"name":n,"root":root,"identity":identity})
        })
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
            assert!(start.elapsed() < patience::PATIENCE);
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
    app.wait_until("sanitized result", patience::PATIENCE, |s| {
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
    app.sees("Daemon status").unwrap();
    app.press_key("alt+2").unwrap();
    app.sees("Registered workspaces").unwrap();
    // No legacy bottom summary card on Workspaces.
    assert!(
        !app.screen()
            .shows("Only the daemon's projects; switching clears the task lookup.")
    );
    app.sees("Add project…").unwrap();
    app.presses("Add project…").unwrap();
    // A folder that is not a repository root is refused by the daemon.
    app.type_text("Folder", f.root.path().to_str().unwrap())
        .unwrap();
    app.presses("Check").unwrap();
    contains(
        &mut app,
        "Choose a repository inside your home, not the home itself.",
    );
    app.clear("Folder").unwrap();
    app.type_text("Folder", gamma.canonicalize().unwrap().to_str().unwrap())
        .unwrap();
    app.presses("Check").unwrap();
    contains(&mut app, "This folder can be added.");
    assert_eq!(app.screen().input_text("Name").unwrap(), Some("gamma"));
    contains(&mut app, "Set up your Profile first");
    // Profile setup lives cleanly in the Profile tab of Runtime checks.
    app.presses("Go to Profile").unwrap();
    app.sees("Set up your Profile").unwrap();
    app.presses("Create Profile").unwrap();
    let start = Instant::now();
    while f.info()["selected"] != true {
        assert!(start.elapsed() < patience::PATIENCE);
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
    app.presses("Workspaces").unwrap();
    // With writes off, the one explicit click enables them for this run and adds.
    app.sees("Enable writes and add").unwrap();
    // Register says so too while writes are off.
    app.sees("Enable writes and register in this Profile: alpha")
        .unwrap();
    app.presses("Enable writes and add").unwrap();
    // The daemon's list now has gamma, ready, and its agent credential.
    contains(&mut app, "gamma");
    let start = Instant::now();
    while !f.location.runtime_dir.join("agent-gamma.cap").exists() {
        assert!(start.elapsed() < patience::PATIENCE);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(f.info()["write_enabled"], true);
    // alpha came from the daemon's set, not this new Profile: one click
    // records it here, and only its row offers that.
    contains(&mut app, "Not in this Profile");
    assert!(!app.screen().shows("Register in this Profile: gamma"));
    app.presses("Register in this Profile: alpha").unwrap();
    let start = Instant::now();
    while f.call("project.list", json!({})).unwrap()[0]["state"] != "ready" {
        assert!(start.elapsed() < patience::PATIENCE);
        std::thread::sleep(Duration::from_millis(20));
    }
    contains(&mut app, "Registered in this Profile: alpha");
    // Each row's Remove names its project for screen readers.
    app.presses("Remove alpha").unwrap();
    app.sees("Remove this project?").unwrap();
    app.presses("Remove").unwrap();
    let start = Instant::now();
    while f.location.runtime_dir.join("agent-alpha.cap").exists() {
        assert!(start.elapsed() < patience::PATIENCE);
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
    // Agents (under Workspace): the switches by mechanism come first; the
    // manual launch line per client is folded under Advanced.
    app.presses("Agents").unwrap();
    app.sees("MCP tools · on demand").unwrap();
    contains(&mut app, "Session-start briefing · automatic");
    assert!(!app.screen().shows("Configure · precept"), "folded");
    app.presses("Advanced: manual setup").unwrap();
    app.sees("Configure · precept").unwrap();
    contains(&mut app, "claude mcp add --scope user precept -- ");
    assert!(
        !app.screen()
            .texts()
            .iter()
            .any(|t| t.contains("claude mcp add") && t.contains("--project")),
        "one server for every project"
    );
    let token = std::fs::read_to_string(f.location.runtime_dir.join("agent-gamma.cap")).unwrap();
    assert!(
        !app.screen()
            .texts()
            .iter()
            .any(|t| t.contains(token.trim())),
        "never a token on screen"
    );
    app.presses("Codex").unwrap();
    contains(&mut app, "[mcp_servers.\"precept\"]");
    app.presses("Copy").unwrap();
    app.sees("Copied").unwrap();
    assert!(
        app.screen()
            .clipboard()
            .is_some_and(|c| c.contains("[mcp_servers.\"precept\"]"))
    );
    app.presses("Share gamma with agents").unwrap();
    let start = Instant::now();
    while f.location.runtime_dir.join("agent-gamma.cap").exists() {
        assert!(start.elapsed() < patience::PATIENCE);
        std::thread::sleep(Duration::from_millis(20));
    }
    contains(&mut app, "This project is not shared");
    app.presses("Share gamma with agents").unwrap();
    let start = Instant::now();
    while !f.location.runtime_dir.join("agent-gamma.cap").exists() {
        assert!(start.elapsed() < patience::PATIENCE);
        std::thread::sleep(Duration::from_millis(20));
    }
    // Agent activity lives on Sessions now: empty until a client starts
    // precept-mcp and reads; the Agents page has setup only.
    assert!(!app.screen().shows("Recent agent activity"));
    app.presses("Sessions").unwrap();
    contains(&mut app, "No agent has used this project yet");
    {
        use std::io::{BufRead, BufReader, Write};
        let mut mcp = Command::new(bin("precept-mcp"))
            .arg("--runtime-dir")
            .arg(&f.location.runtime_dir)
            .arg("--endpoint")
            .arg(&f.location.endpoint)
            // Started in gamma's folder, as a client opened there.
            .current_dir(&gamma)
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
    app.presses("Refresh activity").unwrap();
    contains(&mut app, "Last read ");
    contains(&mut app, "by headless-agent 2.0");
    contains(
        &mut app,
        "Read context: 0 preferences, 0 contracts, 1 documents",
    );
    contains(&mut app, "Started precept-mcp");
    contains(&mut app, "Chain intact · 2 entries");
    app.presses("Agents").unwrap();
    // Agent suggestions: off until the operator's switch, then they land in
    // Review (writes are on in this run).
    contains(&mut app, "Agents cannot suggest rules");
    app.presses("Agents may suggest rules and record work for gamma")
        .unwrap();
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
        assert!(start.elapsed() < patience::PATIENCE);
        std::thread::sleep(Duration::from_millis(20));
    }
    contains(
        &mut app,
        "Agents may suggest rules (each waits for your review) and record their work sessions",
    );
    {
        use std::io::{BufRead, BufReader, Write};
        let mut mcp = Command::new(bin("precept-mcp"))
            .arg("--runtime-dir")
            .arg(&f.location.runtime_dir)
            .arg("--endpoint")
            .arg(&f.location.endpoint)
            // Started in gamma's folder, as a client opened there.
            .current_dir(&gamma)
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
    app.presses("Sessions").unwrap();
    app.presses("Refresh activity").unwrap();
    contains(&mut app, "Suggested a rule: it waits in Review");
    // A second project, then switch from the ONE sidebar workspace switcher:
    // the read it starts must finish after the menu closes (never stuck Reading).
    let delta = f.root.path().join("delta");
    std::fs::create_dir_all(delta.join(".git")).unwrap();
    app.press_key("alt+2").unwrap();
    app.sees("Add project…").unwrap();
    app.presses("Add project…").unwrap();
    // The form may still be settling from the previous add: type until the
    // field holds the path.
    let path = delta.canonicalize().unwrap().to_str().unwrap().to_owned();
    let start = Instant::now();
    while app.screen().input_text("Folder").unwrap() != Some(path.as_str()) {
        assert!(start.elapsed() < patience::PATIENCE, "folder typed");
        app.clear("Folder").unwrap();
        app.type_text("Folder", &path).unwrap();
        std::thread::sleep(Duration::from_millis(100));
    }
    app.presses("Check").unwrap();
    contains(&mut app, "This folder can be added.");
    app.presses("Add").unwrap();
    contains(&mut app, "delta");
    // Workspace list rows are not clickable project-switch buttons.
    assert_eq!(app.screen().count("role:button[delta]").unwrap(), 0);
    let settled = |app: &mut nsh_testing::Headless| {
        app.wait_until("read settled", patience::PATIENCE, |s| {
            s.texts().iter().any(|t| t.starts_with("Connected · "))
        })
        .unwrap();
    };
    settled(&mut app);
    // The single sidebar switcher updates Dashboard and Agents cleanly.
    app.presses("Dashboard").unwrap();
    app.sees("Project: gamma").unwrap();
    for (from, to) in [("gamma", "delta"), ("delta", "gamma")] {
        app.presses(from).unwrap();
        app.presses(to).unwrap();
        settled(&mut app);
        assert_eq!(
            app.screen()
                .texts()
                .iter()
                .filter(|t| t.starts_with("Reading"))
                .count(),
            0
        );
        app.sees(&format!("Project: {to}")).unwrap();
    }
    // The Agents page follows the same sidebar workspace switcher.
    app.presses("Agents").unwrap();
    // The project row shows the selected workspace's root.
    let root_shown = |app: &mut nsh_testing::Headless, name: &str| {
        let suffix = format!("{}{name}", std::path::MAIN_SEPARATOR);
        app.wait_until(&format!("{name} on Agents"), patience::PATIENCE, |s| {
            s.texts().iter().any(|t| t.ends_with(&suffix))
        })
        .unwrap();
    };
    root_shown(&mut app, "gamma");
    app.presses("gamma").unwrap();
    app.presses("delta").unwrap();
    settled(&mut app);
    root_shown(&mut app, "delta");
    // One switch adds the Claude Code hook to the user config (HOME is the
    // test root), another takes it out again; the rest of the file stays.
    let settings = f.root.path().join(".claude/settings.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    std::fs::write(&settings, "{\n  \"model\": \"keep-me\"\n}\n").unwrap();
    contains(&mut app, "The hooks run:");
    // The page read the file before it existed: the first switch is refused
    // and says so; the file is read again, and the second one applies.
    app.presses("Claude Code session-start hook").unwrap();
    contains(&mut app, "The file changed since it was read");
    assert_eq!(
        std::fs::read_to_string(&settings).unwrap(),
        "{\n  \"model\": \"keep-me\"\n}\n"
    );
    app.presses("Claude Code session-start hook").unwrap();
    contains(&mut app, "Briefs each session · ~/.claude/settings.json");
    let text = std::fs::read_to_string(&settings).unwrap();
    assert!(text.contains("keep-me") && text.contains("--vendor claude --project delta"));
    app.presses("Claude Code session-start hook").unwrap();
    let start = Instant::now();
    while std::fs::read_to_string(&settings)
        .unwrap()
        .contains("precept-hook")
    {
        assert!(start.elapsed() < patience::PATIENCE);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        std::fs::read_to_string(&settings)
            .unwrap()
            .contains("keep-me")
    );
    // The MCP switch writes only Precept's server entry into ~/.claude.json.
    let claude_json = f.root.path().join(".claude.json");
    std::fs::write(&claude_json, "{\n  \"numStartups\": 7\n}\n").unwrap();
    app.presses("Claude Code MCP server").unwrap();
    contains(&mut app, "The file changed since it was read");
    app.presses("Claude Code MCP server").unwrap();
    contains(&mut app, "Listed in every session · ~/.claude.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&claude_json).unwrap()).unwrap();
    assert_eq!(v["numStartups"], 7);
    assert_eq!(v["mcpServers"]["precept"]["type"], "stdio");
    app.presses("Claude Code MCP server").unwrap();
    let start = Instant::now();
    while std::fs::read_to_string(&claude_json)
        .unwrap()
        .contains("\"precept\"")
    {
        assert!(start.elapsed() < patience::PATIENCE);
        std::thread::sleep(Duration::from_millis(20));
    }
    // The AGENTS.md switch: previewed before writing, removed at once.
    let agents_md = f.root.path().join("delta/AGENTS.md");
    assert!(!agents_md.exists());
    app.presses("Precept section in AGENTS.md").unwrap();
    app.sees("Add the Precept section to AGENTS.md?").unwrap();
    contains(
        &mut app,
        "call `get_effective_context` on the `precept` MCP server",
    );
    app.presses("Write to AGENTS.md").unwrap();
    contains(&mut app, "In AGENTS.md · review and commit it");
    let written = std::fs::read_to_string(&agents_md).unwrap();
    assert!(written.starts_with("<!-- precept:begin v1 -->"));
    app.presses("Precept section in AGENTS.md").unwrap();
    let start = Instant::now();
    while agents_md.exists() {
        assert!(start.elapsed() < patience::PATIENCE);
        std::thread::sleep(Duration::from_millis(20));
    }
    // Edit: a display name, then a moved folder. The name stays the identity.
    let moved = f.root.path().join("delta-moved");
    std::fs::rename(&delta, &moved).unwrap();
    let moved = moved.canonicalize().unwrap().to_str().unwrap().to_owned();
    app.press_key("alt+2").unwrap();
    app.presses("Edit project delta").unwrap();
    app.sees("Edit project").unwrap();
    contains(&mut app, "Its name, delta, stays the project's identity");
    app.type_text("id:project.edit.label", "Delta · 主线")
        .unwrap();
    app.clear("id:project.edit.folder").unwrap();
    app.type_text("id:project.edit.folder", &moved).unwrap();
    // A changed folder saves only once the daemon has checked it.
    assert!(!app.screen().enabled("Save").unwrap());
    app.presses("Check").unwrap();
    contains(&mut app, "The project can move to this folder.");
    app.presses("Save").unwrap();
    contains(&mut app, "Project moved: Delta · 主线");
    let start = Instant::now();
    loop {
        let list = f.call("project.list", json!({})).unwrap();
        let entry = list
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == "delta")
            .cloned()
            .unwrap();
        if entry["root"] == moved.as_str() && entry["state"] == "ready" {
            assert_eq!(entry["label"], "Delta · 主线");
            break;
        }
        assert!(start.elapsed() < patience::PATIENCE);
        std::thread::sleep(Duration::from_millis(20));
    }
    // The display name leads everywhere; the row keeps the name beside the folder.
    contains(&mut app, &format!("delta · {moved}"));
    app.sees("Delta · 主线").unwrap();
    // Clearing it shows the name again, without touching the folder.
    app.presses("Edit project Delta · 主线").unwrap();
    app.clear("id:project.edit.label").unwrap();
    app.presses("Save").unwrap();
    contains(&mut app, "Project saved: delta");
    let start = Instant::now();
    while !f.call("project.list", json!({})).unwrap()[1]["label"].is_null() {
        assert!(start.elapsed() < patience::PATIENCE);
        std::thread::sleep(Duration::from_millis(20));
    }
}
