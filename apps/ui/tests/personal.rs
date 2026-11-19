//! Personal preferences over a real daemon: save as suggested, then confirm
//! the exact record (two explicit steps).
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
/// The workspace has read the Profile and its list (the list loads only after
/// the Profile reply), and writes are on: no "Writes are off" notice.
fn writes_on(app: &mut nsh_testing::Headless) {
    app.wait_until(
        "workspace read with writes on",
        Duration::from_secs(10),
        |s| {
            let t = s.texts().join("\n");
            (t.contains("Queue watermark: commit")
                || t.contains("No pending suggestions")
                || t.contains("No records in this exact scope")
                || t.contains(" · r"))
                && !t.contains("Writes are off for this daemon run")
                && !t.contains("Waiting for Daemon acknowledgement")
        },
    )
    .unwrap();
}
#[test]
fn a_personal_preference_is_saved_then_confirmed_as_the_exact_record() {
    let f = Fixture::new();
    let db = f.root.path().join("state/profile.sqlite3");
    acp_ipc::transport::private_dir(db.parent().unwrap(), true).unwrap();
    f.call("profile.create", json!({"path":db})).unwrap();
    let b = f.info()["binding"].clone();
    f.call(
        "profile.write.enable",
        json!({"profile_id":b["profile_id"],"generation":b["generation"],"confirm":true}),
    )
    .unwrap();
    let global = |f: &Fixture| {
        f.call(
            "profile.preferences.list",
            json!({"scope":{"kind":"global"},"after_id":0}),
        )
        .unwrap()["records"]
            .clone()
    };
    acp_operator_ui::INPUTS
        .set(acp_operator_client::native::Inputs {
            runtime_dir: Some(f.location.runtime_dir.clone()),
            endpoint: Some(f.location.endpoint.clone()),
            ..Default::default()
        })
        .ok()
        .unwrap();
    let mut cfg = EngineConfig::new(Root::of::<acp_operator_ui::App>());
    cfg.viewport = Size::new(px(1080.0), px(1000.0));
    let mut app = nsh_testing::Headless::start(cfg).unwrap();
    app.wait_for("Daemon status").unwrap();
    app.press("Personal preferences").unwrap();
    writes_on(&mut app);
    // A daemon read finishing afterwards keeps the chosen entry (it once
    // reopened the workspace as To review, with the scope switch).
    app.press("Reconnect").unwrap();
    writes_on(&mut app);
    app.wait_for("Personal preferences").unwrap();
    // The scope is fixed to personal (global): no switch is offered here.
    assert!(!app.screen().texts().contains(&"Global scope"));
    // No Profile summary here: Profile and writes live in Set up.
    assert!(!app.screen().shows("Inspect Profile"));
    // Step one: Add preference saves a suggestion (prepared, then applied).
    app.press("Add preference").unwrap();
    app.wait_for("Add a personal preference").unwrap();
    app.type_text("Preference key", "personal-editor").unwrap();
    app.type_text("id:preference-value", "Prefer **small** commits.")
        .unwrap();
    app.press("Save as suggestion").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    let start = Instant::now();
    while global(&f).as_array().is_none_or(|r| r.is_empty()) {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        global(&f)[0]["status"],
        "suggested",
        "saving never confirms"
    );
    // The value is stored as Markdown, as written.
    assert_eq!(global(&f)[0]["value"], "Prefer **small** commits.");
    // Step two is offered at once: the exact record, inspected, in Detail.
    contains(&mut app, "personal-editor");
    let start = Instant::now();
    loop {
        app.press_key("alt+shift+y").unwrap();
        if app
            .wait_for("Prepared operation; no write sent yet")
            .is_ok()
        {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "confirm offered");
    }
    assert_eq!(global(&f)[0]["status"], "suggested");
    app.press_key("alt+shift+v").unwrap();
    let start = Instant::now();
    while global(&f)[0]["status"] != "confirmed" {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
}
