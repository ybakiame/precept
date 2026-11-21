//! Real NSH components over real daemon IPC; not native GUI/Wayland/TTY evidence.
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
fn real_components_profile_grant_suggestion_confirm_reject_and_disconnect() {
    let mut f = Fixture::new();
    precept_ui::INPUTS
        .set(precept_ui_client::native::Inputs {
            runtime_dir: Some(f.location.runtime_dir.clone()),
            endpoint: Some(f.location.endpoint.clone()),
            operator: true,
            ..Default::default()
        })
        .ok()
        .unwrap();
    let mut cfg = EngineConfig::new(Root::of::<precept_ui::App>());
    cfg.viewport = Size::new(px(1080.0), px(760.0));
    let mut app = nsh_testing::Headless::start(cfg).unwrap();
    app.wait_for("No Profile selected").unwrap();
    // Profile setup lives in Runtime checks -> Profile: the daemon suggests a
    // path in its private state folder; nothing exists until the operator's click.
    app.press("Runtime checks").unwrap();
    app.press("Profile").unwrap();
    app.wait_for("Set up your Profile").unwrap();
    let suggested = app
        .screen()
        .input_text("Profile file")
        .unwrap()
        .unwrap()
        .to_owned();
    assert!(suggested.ends_with("profile.sqlite3"));
    assert!(!std::path::Path::new(&suggested).exists());
    assert_eq!(f.info()["selected"], false);
    app.press("Create Profile").unwrap();
    app.wait_for("Selected at every daemon start").unwrap();
    assert_eq!(f.info()["selected"], true);
    assert_eq!(
        f.info()["remembered"],
        true,
        "select at start is on by default"
    );
    assert_eq!(
        f.info()["write_enabled"],
        false,
        "creation never enables writes"
    );
    app.press("Enable writes").unwrap();
    app.wait_for("Writes on for this daemon run").unwrap();
    assert_eq!(f.info()["write_enabled"], true);
    app.press_key("alt+o").unwrap();
    writes_on(&mut app);
    f.register();
    app.press_key("mod+r").unwrap();
    // The newly registered project becomes the current one.
    app.wait_for("Project conventions · alpha").unwrap();
    writes_on(&mut app);
    // Workspace → Preferences → Manage & add preferences opens the unified
    // Review hub on alpha's project tab; Add preference saves a suggestion.
    app.press_key("alt+3").unwrap();
    app.press("Manage & add preferences").unwrap();
    writes_on(&mut app);
    let add = |app: &mut nsh_testing::Headless| {
        app.press("Add preference").unwrap();
        app.type_text("Preference key", "editor").unwrap();
        app.type_text("id:preference-value", "synthetic-value")
            .unwrap();
        // Tags are required: one suggestion button, one typed with a comma.
        app.press("Add tag style").unwrap();
        app.type_text("Tags", "Tooling,").unwrap();
        app.wait_for("One to five short labels, such as style or git. Press Enter or a comma to add one. · 2 of 5").unwrap();
        app.press("Save as suggestion").unwrap();
        app.wait_for("Prepared operation; no write sent yet")
            .unwrap();
    };
    add(&mut app);
    assert!(
        f.call(
            "profile.preferences.list",
            json!({"scope":{"kind":"project","name":"alpha"},"after_id":0})
        )
        .unwrap()["records"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 2");
    // The sidebar counts the suggestion the daemon now holds, exactly.
    let waiting = |app: &mut nsh_testing::Headless, label: &'static str| {
        app.wait_until(label, std::time::Duration::from_secs(10), |s| {
            s.count(&format!("role:button[{label}]")).unwrap_or(0) == 1
        })
        .unwrap();
    };
    waiting(&mut app, "Review & rules, 1 waiting");
    // The saved suggestion opens as the exact record; confirming it is the
    // second explicit step.
    app.wait_for("Show full digest").unwrap();
    app.wait_for("#1 · editor").unwrap();
    app.press_key("alt+shift+y").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 3");
    // Confirmed: nothing waits, and the count leaves rather than showing 0.
    waiting(&mut app, "Review & rules");
    add(&mut app);
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 4");
    app.wait_for("#2 · editor").unwrap();
    app.press_key("alt+shift+n").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 5");
    let page = f
        .call(
            "profile.preferences.list",
            json!({"scope":{"kind":"project","name":"alpha"},"after_id":0}),
        )
        .unwrap();
    assert_eq!(page["records"][0]["status"], "confirmed");
    assert_eq!(page["records"][1]["status"], "rejected");
    // The UI-initiated confirm must persist a truthful Desktop channel label,
    // not a fabricated CLI attribution (D5; review moderate finding).
    let inspect = f
        .call(
            "profile.review.inspect",
            json!({"scope":{"kind":"project","name":"alpha"},"id":1,"revision":2}),
        )
        .unwrap();
    let decisions = inspect["decisions"].as_array().unwrap();
    assert!(
        decisions
            .iter()
            .any(|d| d["action"] == "confirm" && d["channel"] == "desktop"),
        "UI confirm must record channel=desktop, got {decisions:?}"
    );
    assert!(
        decisions.iter().all(|d| d["channel"] != "cli"),
        "no UI decision may be misattributed to cli"
    );
    let request: operator::Mutation = serde_json::from_value(json!({
        "version":2,"binding":f.info()["binding"],"operation_id":"headless-handoff",
        "command":{"command":"put_handoff","project":"alpha","task_id":"shared",
            "objective":"Saved synthetic history","completed":"","next_action":"Inspect current facts",
            "validation_json":"[{\"check\":\"synthetic\",\"status\":\"NOT_RUN\"}]",
            "snapshot":{"base_sha":"","branch":"","state":"unknown","fingerprint":null},
            "expected":{"kind":"absent"}}
    })).unwrap();
    tokio_runtime(|| operator::mutate(&f.location, &request)).unwrap();
    app.press_key("alt+t").unwrap();
    app.type_text("Exact saved task ID", "shared").unwrap();
    app.press_key("alt+shift+h").unwrap();
    contains(&mut app, "Saved synthetic history");
    contains(&mut app, "NOT_RUN");
    // A different task cannot retain or display the preceding saved record.
    app.type_text("Exact saved task ID", "missing").unwrap();
    app.press_key("alt+shift+h").unwrap();
    contains(&mut app, "null");
    assert!(
        !app.screen()
            .texts()
            .iter()
            .any(|t| t.contains("Saved synthetic history"))
    );
    // Writes are turned off in Runtime checks -> Profile.
    app.press("Runtime checks").unwrap();
    app.press("Profile").unwrap();
    assert_eq!(f.info()["write_enabled"], true);
    app.press("Disable writes").unwrap();
    app.wait_for("Writes off").unwrap();
    assert_eq!(f.info()["write_enabled"], false);
    app.press_key("alt+o").unwrap();
    app.wait_for("Writes are off for this daemon run").unwrap();
    app.press_key("mod+l").unwrap();
    app.wait_for("本次 daemon 运行未开启写入").unwrap();
    app.press_key("Escape").unwrap();
    app.press_key("alt+3").unwrap();
    app.wait_for("已确认的偏好与约定").unwrap();
    f.child.kill().unwrap();
    f.child.wait().unwrap();
    app.press_key("alt+o").unwrap();
    contains(&mut app, "Unavailable");
    assert!(app.screen().clipboard().is_none());
}
