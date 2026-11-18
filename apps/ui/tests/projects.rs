//! The Workspace page over a real daemon: check, add and remove a project.
//! Real NSH components and IPC; not native GUI/Wayland/TTY evidence.
#![cfg(feature = "native")]
use acp_ipc::{Location, operator};
use nsh_core::prelude::*;
use nsh_engine::{EngineConfig, Root};
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
            .prefix("acp-s3-ui-")
            .tempdir()
            .unwrap();
        std::fs::create_dir(root.path().join("alpha")).unwrap();
        std::fs::write(root.path().join("alpha/AGENTS.md"), "Synthetic contract\n").unwrap();
        acp_ipc::transport::private_dir(&root.path().join("state"), true).unwrap();
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
        let child = Command::new(bin("acp-daemon"))
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
    fn call(&self, method: &str, params: Value) -> acp_ipc::Result<Value> {
        tokio_runtime(|| operator::control(&self.location, method, params))
    }
    fn info(&self) -> Value {
        self.call("profile.info", json!({})).unwrap()
    }
    #[allow(dead_code)]
    fn register(&self) {
        let info = self.info();
        let b = &info["binding"];
        let result = Command::new(bin("acp-query"))
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
    let db = f.root.path().join("state/profile.sqlite3");
    acp_ipc::transport::private_dir(db.parent().unwrap(), true).unwrap();
    f.call("profile.create", json!({"path":db})).unwrap();
    let b = f.info()["binding"].clone();
    f.call(
        "profile.write.enable",
        json!({"profile_id":b["profile_id"],"generation":b["generation"],"confirm":true}),
    )
    .unwrap();
    acp_operator_ui::INPUTS
        .set(acp_operator_client::native::Inputs {
            runtime_dir: Some(f.location.runtime_dir.clone()),
            endpoint: Some(f.location.endpoint.clone()),
            ..Default::default()
        })
        .ok()
        .unwrap();
    let mut cfg = EngineConfig::new(Root::of::<acp_operator_ui::App>());
    cfg.viewport = Size::new(px(1080.0), px(900.0));
    let mut app = nsh_testing::Headless::start(cfg).unwrap();
    // A fresh daemon has no usable project yet: the first read says so, and
    // the Workspace page must still be reachable to add one.
    app.wait_for("Daemon status").unwrap();
    app.press_key("alt+2").unwrap();
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
    app.press("Add").unwrap();
    // The daemon's list now has gamma, ready, and its agent credential.
    contains(&mut app, "gamma");
    let start = Instant::now();
    while !f.location.runtime_dir.join("agent-gamma.cap").exists() {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
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
}
