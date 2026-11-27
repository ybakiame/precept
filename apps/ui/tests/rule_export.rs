//! Writing a confirmed project convention to the repository over a real
//! daemon: the preview, the write, and the dialog closing on success.
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
            .prefix("precept-export-ui-")
            .tempdir()
            .unwrap();
        std::fs::create_dir(root.path().join("alpha")).unwrap();
        std::fs::write(root.path().join("alpha/AGENTS.md"), "Synthetic contract\n").unwrap();
        precept_ipc::transport::private_dir(&root.path().join("state"), true).unwrap();
        let runtime_dir = root.path().join("runtime");
        let endpoint = if cfg!(windows) {
            format!("ex-{}", root.path().file_name().unwrap().to_string_lossy())
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
/// The workspace has read the Profile and its list (the list loads only after
/// the Profile reply), and writes are on: no "Writes are off" notice.

#[test]
fn a_confirmed_convention_is_written_after_its_preview() {
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
    let scope = operator::Scope::Project {
        name: "alpha".into(),
    };
    let binding: operator::Binding = serde_json::from_value(f.info()["binding"].clone()).unwrap();
    let suggest = operator::Mutation {
        version: 2,
        binding: binding.clone(),
        operation_id: "convention".into(),
        command: operator::Command::SuggestPreference {
            scope: scope.clone(),
            key: "commit-style".into(),
            value: "Small commits.".into(),
            tags: vec!["git".into()],
            origin: operator::Origin::Operator,
            rule: Default::default(),
            expected: operator::Expected::Absent,
        },
    };
    tokio_runtime(|| operator::mutate(&f.location, &suggest)).unwrap();
    let record = f
        .call(
            "profile.review.inspect",
            json!({"scope":scope,"id":1,"revision":1}),
        )
        .unwrap();
    let confirm = operator::Mutation {
        version: 2,
        binding,
        operation_id: "confirm".into(),
        command: operator::Command::ConfirmPreference {
            scope,
            id: 1,
            proposal_digest: record["record"]["proposal_digest"].as_str().unwrap().into(),
            channel: operator::Channel::Cli,
            expected: operator::Expected::Revision(1),
            note: None,
        },
    };
    tokio_runtime(|| operator::mutate(&f.location, &confirm)).unwrap();
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
    app.sees("Daemon status").unwrap();
    app.presses("Rules").unwrap();
    app.presses("Project conventions · alpha").unwrap();
    app.wait_until("row listed", Duration::from_secs(10), |s| {
        s.count("role:button[#1 · commit-style · confirmed · r2]")
            .unwrap_or(0)
            > 0
    })
    .unwrap();
    app.presses("#1 · commit-style · confirmed · r2").unwrap();
    app.sees("Write to repository…").unwrap();
    app.presses("Write to repository…").unwrap();
    app.sees(".precept/rules/commit-style.md").unwrap();
    app.presses("Write file").unwrap();
    let file = f.root.path().join("alpha/.precept/rules/commit-style.md");
    app.wait_until(
        "dialog closed after the write",
        Duration::from_secs(10),
        |s| !s.shows("Write file"),
    )
    .unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.starts_with("+++\nkey = \"commit-style\"\n"));
    assert!(text.ends_with("Small commits.\n"));
    // Writing again offers nothing to overwrite.
    app.wait_until("row listed again", Duration::from_secs(10), |s| {
        s.count("role:button[#1 · commit-style · confirmed · r2]")
            .unwrap_or(0)
            > 0
    })
    .unwrap();
    app.presses("#1 · commit-style · confirmed · r2").unwrap();
    app.sees("Write to repository…").unwrap();
    app.presses("Write to repository…").unwrap();
    app.sees("This file is already in the repository with this text.")
        .unwrap();
    app.presses("Close").unwrap();
    // The file drifted from the record: the preview offers an update.
    std::fs::write(&file, text.replace("Small commits.", "Big commits.")).unwrap();
    app.presses("Write to repository…").unwrap();
    app.sees("Update file").unwrap();
    contains_text(
        &mut app,
        "The file in the repository differs; this updates it.",
    );
    app.presses("Update file").unwrap();
    app.wait_until("updated", Duration::from_secs(10), |s| {
        !s.shows("Update file")
    })
    .unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
}
fn contains_text(app: &mut nsh_testing::Headless, needle: &str) {
    app.wait_until(needle, Duration::from_secs(10), |s| {
        s.texts().iter().any(|t| t.contains(needle))
    })
    .unwrap();
}
