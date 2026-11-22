//! The Memory page over a real daemon: agents keep notes, the operator
//! reads, restores, pins and deletes them. Real NSH components and IPC; not native
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
            .prefix("precept-memory-ui-")
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
fn agents_notes_are_shown_restored_pinned_and_deleted() {
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
    // Agents keep notes through their own credential; the second Node note
    // corrects the first.
    let cap = f.location.runtime_dir.join("agent-alpha.cap");
    tokio_runtime(|| async {
        let credential = precept_ipc::load_credential(&f.location, &cap).unwrap();
        let mut client = precept_ipc::Client::connect(&f.location, &credential)
            .await
            .unwrap();
        for (kind, title, body) in [
            ("environment", "Node", "Node 22 via mise"),
            ("caution", "WSL paths", "Builds under /mnt/c are slow"),
            ("environment", "Node", "Node 24 via mise"),
        ] {
            client
                .request(
                    "memory.remember",
                    json!({"kind":kind,"title":title,"body":body,"client":"codex 1.0"}),
                )
                .await
                .unwrap();
        }
    });
    precept_ui::INPUTS
        .set(precept_ui_client::native::Inputs {
            runtime_dir: Some(f.location.runtime_dir.clone()),
            endpoint: Some(f.location.endpoint.clone()),
            ..Default::default()
        })
        .ok()
        .unwrap();
    let mut cfg = EngineConfig::new(Root::of::<precept_ui::App>());
    cfg.viewport = Size::new(px(1180.0), px(1000.0));
    let mut app = nsh_testing::Headless::start(cfg).unwrap();
    app.wait_for("Daemon status").unwrap();
    app.press("Memory").unwrap();
    contains(&mut app, "Environment · 1");
    contains(&mut app, "Cautions · 1");
    contains(&mut app, "Node 24 via mise");
    contains(&mut app, "codex 1.0 · updated ");
    contains(&mut app, "Weight 100: Environment 100");
    // The detail keeps the earlier text; restoring it is the operator's
    // correction.
    app.press("Open memory note Node").unwrap();
    app.wait_for("Earlier versions").unwrap();
    app.press("Restore version 1").unwrap();
    let entries = |f: &Fixture| {
        f.call("memories.list", json!({"project":"alpha"})).unwrap()["entries"]
            .as_array()
            .unwrap()
            .clone()
    };
    let wait = |what: &str, ok: &dyn Fn(&[Value]) -> bool| {
        let start = Instant::now();
        while !ok(&entries(&f)) {
            assert!(start.elapsed() < Duration::from_secs(10), "{what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    wait("restored", &|e| {
        e.iter()
            .any(|x| x["title"] == "Node" && x["body"] == "Node 22 via mise")
    });
    contains(&mut app, "Edited by you");
    app.press("Pinned: always in the briefing").unwrap();
    wait("pinned", &|e| {
        e.iter()
            .any(|x| x["title"] == "Node" && x["pinned"] == true)
    });
    app.press("Delete…").unwrap();
    app.wait_for("Delete this note?").unwrap();
    app.press("Delete").unwrap();
    wait("deleted", &|e| e.len() == 1 && e[0]["title"] == "WSL paths");
    contains(
        &mut app,
        "Environment 0 · Cautions 1 · Lessons 0 · Attempts 0",
    );
}
