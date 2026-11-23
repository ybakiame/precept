//! The Sessions page over a real daemon: an agent records its work, the
//! operator reads and deletes it. Real NSH components and IPC; not native
//! GUI/Wayland/TTY evidence.
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
fn an_agents_work_session_is_shown_and_deleted() {
    let f = Fixture::new();
    let db = f.root.path().join("state/profile.sqlite3");
    precept_ipc::transport::private_dir(db.parent().unwrap(), true).unwrap();
    f.call("profile.create", json!({"path":db})).unwrap();
    let b = f.info()["binding"].clone();
    f.call(
        "profile.write.enable",
        json!({"profile_id":b["profile_id"],"generation":b["generation"],"confirm":true}),
    )
    .unwrap();
    f.register();
    f.call("project.propose", json!({"name":"alpha","enabled":true}))
        .unwrap();
    // An agent records its work through its own credential.
    let cap = f.location.runtime_dir.join("agent-alpha.cap");
    let session = tokio_runtime(|| async {
        let credential = precept_ipc::load_credential(&f.location, &cap).unwrap();
        let mut client = precept_ipc::Client::connect(&f.location, &credential)
            .await
            .unwrap();
        client
            .request(
                "session.start",
                json!({"goal":"Add import from files","client":"claude-code 2.1",
                       "doing":[{"text":"daemon scan"}],"todo":[{"text":"app dialog"}],"done":[{"text":"parser"}]}),
            )
            .await
            .unwrap()
    });
    assert_eq!(session["status"], "open");
    precept_ui::INPUTS
        .set(precept_ui_client::native::Inputs {
            runtime_dir: Some(f.location.runtime_dir.clone()),
            endpoint: Some(f.location.endpoint.clone()),
            ..Default::default()
        })
        .ok()
        .unwrap();
    let mut cfg = EngineConfig::new(Root::of::<precept_ui::App>());
    cfg.viewport = Size::new(px(1080.0), px(1000.0));
    let mut app = nsh_testing::Headless::start(cfg).unwrap();
    app.wait_for("Daemon status").unwrap();
    app.press("Sessions").unwrap();
    contains(&mut app, "Add import from files");
    contains(&mut app, "claude-code 2.1 · updated ");
    contains(&mut app, "1 doing · 1 next · 1 done");
    app.press("Add import from files").unwrap();
    app.wait_for("Doing").unwrap();
    contains(&mut app, "• daemon scan");
    contains(&mut app, "• app dialog");
    // The session becomes a Handoff, its task id taken from the goal.
    app.press("Save as Handoff…").unwrap();
    app.wait_for("Save this session as a Handoff").unwrap();
    assert_eq!(
        app.screen().input_text("Task id").unwrap(),
        Some("add-import-from-files")
    );
    app.press("Save Handoff").unwrap();
    contains(&mut app, "Saved as Handoff add-import-from-files");
    let saved = f
        .call(
            "profile.handoff.get",
            json!({"project":"alpha","task_id":"add-import-from-files"}),
        )
        .unwrap();
    assert_eq!(saved["record"]["objective"], "Add import from files");
    app.press("Delete session…").unwrap();
    app.wait_for("Delete this work session?").unwrap();
    app.press("Delete").unwrap();
    let start = Instant::now();
    while !f.call("sessions.list", json!({"project":"alpha"})).unwrap()["sessions"]
        .as_array()
        .unwrap()
        .is_empty()
    {
        assert!(start.elapsed() < Duration::from_secs(10), "deleted");
        std::thread::sleep(Duration::from_millis(20));
    }
    contains(&mut app, "No work sessions yet");
    // The Handoff page lists it; pressing it opens it, no task id typed.
    app.press("Handoff").unwrap();
    app.wait_for("Saved handoffs").unwrap();
    app.wait_until("listed", Duration::from_secs(10), |s| {
        s.count("role:button[Open handoff add-import-from-files]")
            .unwrap_or(0)
            > 0
    })
    .unwrap();
    app.press("Open handoff add-import-from-files").unwrap();
    app.wait_for("Before you continue").unwrap();
    contains(&mut app, "Handoff add-import-from-files · alpha");
}
