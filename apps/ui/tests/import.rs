//! Import from AGENTS.md over a real daemon: the scan lists its sections,
//! picks become suggestions in one write. Real NSH components and IPC; not
//! native GUI/Wayland/TTY evidence.
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
/// The workspace has read the Profile and its list (the list loads only after
/// the Profile reply), and writes are on: no "Writes are off" notice.
fn writes_on(app: &mut nsh_testing::Headless) {
    app.wait_until(
        "workspace read with writes on",
        Duration::from_secs(10),
        |s| {
            let t = s.texts().join("\n");
            (t.contains("No pending suggestions")
                || t.contains("No records in this exact scope")
                || t.contains("No records match this filter")
                || t.contains(" · r"))
                && !t.contains("Writes are off for this daemon run")
                && !t.contains("Waiting for Daemon acknowledgement")
        },
    )
    .unwrap();
}
#[test]
fn rules_in_the_projects_files_are_picked_and_filed_as_suggestions() {
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
    std::fs::write(
        f.root.path().join("alpha/AGENTS.md"),
        "# Alpha\n\n## Commits\n\nUse conventional commits.\n\n## Testing\n\nRun the gates.\n",
    )
    .unwrap();
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
    app.press_key("alt+o").unwrap();
    writes_on(&mut app);
    app.press("Import from AGENTS.md…").unwrap();
    app.wait_for("Import rules from alpha's AGENTS.md").unwrap();
    contains(&mut app, "agents-commits");
    contains(&mut app, "AGENTS.md · Commits");
    // New rules start picked; leave Testing out.
    app.press("Import agents-testing").unwrap();
    app.wait_for("Suggest 1…").unwrap();
    app.press("Suggest 1…").unwrap();
    app.wait_for("Nothing is written until you apply it")
        .unwrap();
    contains(
        &mut app,
        "Suggest 1 sections of AGENTS.md as project conventions",
    );
    app.press_key("alt+shift+v").unwrap();
    let queue = || {
        f.call(
            "profile.review.queue",
            json!({"scope":{"kind":"project","name":"alpha"},"after_id":0}),
        )
        .unwrap()["records"]
            .as_array()
            .unwrap()
            .clone()
    };
    let start = Instant::now();
    while queue().is_empty() {
        assert!(start.elapsed() < Duration::from_secs(10), "import applied");
        std::thread::sleep(Duration::from_millis(20));
    }
    let records = queue();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["key"], "agents-commits");
    assert_eq!(records[0]["status"], "suggested");
    assert_eq!(records[0]["value"], "Use conventional commits.");
    // A second scan marks it as already in Precept and leaves it unpicked.
    app.press("Import from AGENTS.md…").unwrap();
    contains(&mut app, "Already in Precept");
    app.wait_for("Suggest 1…").unwrap();
}
