//! Real NSH components over real daemon IPC for the S2 review workspace; not
//! native GUI/Wayland/TTY evidence. A separate integration binary so its global
//! `INPUTS` grant never races the original operator integration test.
#![cfg(feature = "ipc")]
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
            .prefix("precept-s2-review-ui-")
            .tempdir()
            .unwrap();
        std::fs::create_dir(root.path().join("alpha")).unwrap();
        std::fs::write(root.path().join("alpha/AGENTS.md"), "Synthetic contract\n").unwrap();
        precept_ipc::transport::private_dir(&root.path().join("state"), true).unwrap();
        let runtime_dir = root.path().join("runtime");
        let endpoint = if cfg!(windows) {
            format!("s2-{}", root.path().file_name().unwrap().to_string_lossy())
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
    fn inspect(&self, id: i64, revision: i64) -> Value {
        self.call(
            "profile.review.inspect",
            json!({"scope":{"kind":"project","name":"alpha"},"id":id,"revision":revision}),
        )
        .unwrap()
    }
    fn assert_decision(
        &self,
        before: &Value,
        action: &str,
        channel: &str,
        revision: i64,
        sequence: i64,
    ) -> Value {
        let id = before["record"]["id"].as_i64().unwrap();
        let after = self.inspect(id, revision);
        let decisions = after["decisions"].as_array().unwrap();
        assert_eq!(
            after["record"]["proposal_digest"],
            before["record"]["proposal_digest"]
        );
        assert_eq!(after["record"]["scope"], before["record"]["scope"]);
        let decision = decisions.iter().find(|d| d["action"] == action).unwrap();
        assert_eq!(decision["from_revision"], before["record"]["revision"]);
        assert_eq!(decision["to_revision"], revision);
        assert_eq!(
            decision["proposal_digest"],
            before["record"]["proposal_digest"]
        );
        assert_eq!(decision["channel"], channel);
        assert_eq!(decision["receipt_commit_sequence"], sequence);
        assert!(decisions.len() <= 8);
        after
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
    let label = format!("sanitized result containing {needle:?}");
    app.wait_until(&label, Duration::from_secs(10), |s| {
        s.texts().iter().any(|t| t.contains(needle))
    })
    .unwrap();
}

/// Exact detail, peer demotion, all decisions and delete consent through actual
/// NSH components over a real owned daemon, headless; these are not native
/// input or visual acceptance.
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
fn real_components_review_queue_detail_history_deprecate_and_delete() {
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
    app.press("Writes for this daemon run").unwrap();
    app.wait_for("Writes on for this daemon run").unwrap();
    assert_eq!(f.info()["write_enabled"], true);
    app.press_key("alt+o").unwrap();
    writes_on(&mut app);
    f.register();
    app.press_key("mod+r").unwrap();
    // The newly registered project becomes the current one; reviewed rules
    // are a separate destination from the pending queue.
    app.press("Rules").unwrap();
    app.wait_for("Project conventions · alpha").unwrap();
    writes_on(&mut app);
    // Project → Preferences → Manage project preferences: alpha's records.
    manage_project(&mut app);
    // Add record #1 (editor): saved as a suggestion, then it opens to confirm.
    add(&mut app, "editor", "synthetic-value");
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 2");
    app.wait_for("Show full digest").unwrap();
    app.press("Show full digest").unwrap();
    app.wait_for("Collapse digest").unwrap();
    // Confirm #1; it becomes a same-key confirmed peer for the next suggestion.
    let first = f.inspect(1, 1);
    app.press("Confirm preference…").unwrap();
    app.wait_for("Nothing is written until you apply it")
        .unwrap();
    assert_eq!(f.inspect(1, 1)["record"]["status"], "suggested");
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 3");
    let confirmed_first = f.assert_decision(&first, "confirm", "desktop", 2, 3);
    // Add record #2 with the same key; it opens with its exact detail.
    add(&mut app, "editor", "synthetic-value");
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 4");
    app.wait_for("#2 · editor").unwrap();
    app.wait_for("Show full digest").unwrap();
    // The peer-demotion disclosure names the authoritative total and preview ids.
    app.wait_for("Deprecates 1 same-key confirmed records (ids 1)")
        .unwrap();
    // History for a never-decided suggestion is empty, never an error.
    app.wait_for("No decisions recorded for this record")
        .unwrap();
    // Confirm #2 (demotes #1), then read its decision history.
    let second = f.inspect(2, 1);
    app.press("Confirm preference…").unwrap();
    app.wait_for("Nothing is written until you apply it")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 5");
    let confirmed_second = f.assert_decision(&second, "confirm", "desktop", 2, 5);
    let demoted = f.assert_decision(&confirmed_first, "demote", "desktop", 3, 5);
    assert_eq!(demoted["record"]["status"], "deprecated");
    assert!(!app.screen().shows("#1 · editor · deprecated · r2"));
    open_row(&mut app, "#2 · editor · confirmed · r2");
    app.wait_for("Channel is client-asserted").unwrap();
    // Deprecate the confirmed #2 with its impact disclosure.
    app.wait_for("Deprecation makes the record inactive in agent effective context.")
        .unwrap();
    app.press("Deprecate preference…").unwrap();
    app.wait_for("Nothing is written until you apply it")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 6");
    let deprecated_second = f.assert_decision(&confirmed_second, "deprecate", "desktop", 3, 6);
    assert_eq!(deprecated_second["record"]["status"], "deprecated");
    // Deprecated rows stay hidden until the explicit status filter shows them.
    app.press("Show deprecated").unwrap();
    // Delete #2 through the destructive tier: a wrong retype refuses, the exact
    // retype arms, and the receipt leaves identity-only proof.
    open_row(&mut app, "#2 · editor · deprecated · r3");
    app.press_key("alt+shift+q").unwrap();
    app.wait_for("Delete project alpha preference #2?").unwrap();
    app.wait_for("Preference ID: #2").unwrap();
    app.wait_for("Copy ID").unwrap();
    app.type_text("Retype the exact ID", "9").unwrap();
    app.wait_for("The typed ID does not match").unwrap();
    app.press_key("Enter").unwrap();
    assert!(!app.screen().shows("Nothing is written until you apply it"));
    assert_eq!(f.inspect(2, 3)["record"]["status"], "deprecated");
    app.press("Cancel").unwrap();
    app.wait_for_gone(
        "Delete project alpha preference #2?",
        Duration::from_secs(5),
    )
    .unwrap();
    app.press_key("alt+shift+q").unwrap();
    app.wait_for("Delete project alpha preference #2?").unwrap();
    app.type_text("Retype the exact ID", "2").unwrap();
    app.wait_for("Deletion armed; apply to commit").unwrap();
    app.press_key("Enter").unwrap();
    app.wait_for("Nothing is written until you apply it")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 7");
    assert_eq!(
        app.screen()
            .texts()
            .iter()
            .filter(|text| text.contains("commit 7"))
            .count(),
        1,
        "the acknowledged receipt is presented once"
    );
    // Project preference receipts deliberately use a globally allocated ID
    // (empty receipt project), not the request's project-scoped read binding.
    assert!(!app.screen().shows("Retry identical request"));
    assert!(
        f.call(
            "profile.review.inspect",
            json!({"scope":{"kind":"project","name":"alpha"},"id":2,"revision":3}),
        )
        .is_err()
    );
    app.press_key("alt+shift+v").unwrap();
    contains(
        &mut app,
        "Deleted. Content and decision history are gone; receipts remain identity-only.",
    );
    // A deleted record keeps no content or decision history snapshot.
    app.wait_for_gone("Show full digest", Duration::from_secs(5))
        .unwrap();
    // To review is empty: a valid pending-review state, never "unavailable".
    app.press("Review").unwrap();
    app.wait_for("No pending suggestions").unwrap();
    assert!(!app.screen().shows("Unavailable"));
    // Both locales ship the review copy.
    app.press_key("mod+l").unwrap();
    app.wait_for("审阅").unwrap();
    app.wait_for("没有待审建议").unwrap();
    drop(app);
    keyboard_review_and_revoked_delete(&f);
    changed_root_refuses_reviewed_apply(&f);
    f.child.kill().unwrap();
    f.child.wait().unwrap();
}

/// Workspace → Preferences → Manage & add preferences, read and writable.
fn manage_project(app: &mut nsh_testing::Headless) {
    app.press_key("alt+3").unwrap();
    app.press("Manage & add preferences").unwrap();
    writes_on(app);
}
/// Add convention: saved as a suggestion (prepared; Apply commits it).
fn add(app: &mut nsh_testing::Headless, key: &str, value: &str) {
    app.press("Add convention").unwrap();
    app.type_text("Preference key", key).unwrap();
    app.type_text("id:preference-value", value).unwrap();
    app.type_text("Tags", "review\n").unwrap();
    app.press("Save as suggestion").unwrap();
    app.wait_for("Nothing is written until you apply it")
        .unwrap();
}
/// A record row in the scope's list opens its sheet with the exact record.
fn open_row(app: &mut nsh_testing::Headless, row: &str) {
    app.press_key("alt+shift+l").unwrap();
    // The row's full label is its title's accessible name.
    let selector = format!("role:button[{row}]");
    app.wait_until(&format!("row {row}"), Duration::from_secs(10), |s| {
        s.count(&selector).unwrap_or(0) > 0
    })
    .unwrap();
    app.press(row).unwrap();
    app.wait_for("Show full digest").unwrap();
}

fn keyboard_review_and_revoked_delete(f: &Fixture) {
    // The review shortcuts end to end, through the desktop channel.
    let mut cfg = EngineConfig::new(Root::of::<precept_ui::App>());
    cfg.viewport = Size::new(px(1080.0), px(760.0));
    let mut app = nsh_testing::Headless::start(cfg).unwrap();
    writes_on(&mut app);
    manage_project(&mut app);
    add(&mut app, "keyboard-editor", "keyboard-value");
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 8");
    app.wait_for("#3 · keyboard-editor").unwrap();
    app.wait_for("Show full digest").unwrap();
    let third = f.inspect(3, 1);
    app.press_key("alt+shift+y").unwrap();
    app.wait_for("Nothing is written until you apply it")
        .unwrap();
    assert_eq!(f.inspect(3, 1)["record"]["status"], "suggested");
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 9");
    let confirmed = f.assert_decision(&third, "confirm", "desktop", 2, 9);
    open_row(&mut app, "#3 · keyboard-editor · confirmed · r2");
    contains(&mut app, "confirm · r1→r2 · desktop");
    app.press_key("alt+shift+w").unwrap();
    app.wait_for("Nothing is written until you apply it")
        .unwrap();
    assert_eq!(f.inspect(3, 2)["record"]["status"], "confirmed");
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 10");
    f.assert_decision(&confirmed, "deprecate", "desktop", 3, 10);
    add(&mut app, "keyboard-editor", "keyboard-value");
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 11");
    app.wait_for("#4 · keyboard-editor").unwrap();
    app.wait_for("Show full digest").unwrap();
    let fourth = f.inspect(4, 1);
    app.press_key("alt+shift+n").unwrap();
    app.wait_for("Nothing is written until you apply it")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 12");
    f.assert_decision(&fourth, "reject", "desktop", 2, 12);
    app.press("Show deprecated").unwrap();
    open_row(&mut app, "#3 · keyboard-editor · deprecated · r3");
    app.press_key("alt+shift+q").unwrap();
    app.wait_for("Delete project alpha preference #3?").unwrap();
    app.type_text("Retype the exact ID", "3").unwrap();
    app.press_key("Enter").unwrap();
    app.wait_for("Nothing is written until you apply it")
        .unwrap();
    // The exact request names the desktop channel.
    app.press("Show request").unwrap();
    contains(&mut app, "\"channel\":\"desktop\"");
    // Revoke externally between inspection/consent and submit. The real IPC
    // adapter/daemon must refuse rather than accept an obsolete UI grant.
    f.call("profile.write.disable", json!({})).unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "Unauthorized");
    assert_eq!(f.inspect(3, 3)["record"]["status"], "deprecated");
    app.press_key("alt+shift+r").unwrap();
    app.wait_for("Writes are off for this daemon run").unwrap();
    // A new grant from Runtime checks -> Profile, then back to the records.
    app.press("Runtime checks").unwrap();
    app.press("Profile").unwrap();
    app.press("Writes for this daemon run").unwrap();
    app.wait_for("Writes on for this daemon run").unwrap();
    manage_project(&mut app);
    open_row(&mut app, "#3 · keyboard-editor · deprecated · r3");
    app.press_key("alt+shift+q").unwrap();
    app.wait_for("Delete project alpha preference #3?").unwrap();
    // Revocation and the new grant must not retain the preceding exact text.
    app.wait_for("Type the ID exactly to arm deletion").unwrap();
    app.press_key("alt+shift+v").unwrap();
    assert_eq!(f.inspect(3, 3)["record"]["status"], "deprecated");
    app.type_text("Retype the exact ID", "3").unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 13");
    assert!(!app.screen().shows("Retry identical request"));
    contains(
        &mut app,
        "Deleted. Content and decision history are gone; receipts remain identity-only.",
    );
    app.press_key("alt+shift+v").unwrap();
    let queue = f
        .call(
            "profile.review.queue",
            json!({"scope":{"kind":"project","name":"alpha"},"after_id":0}),
        )
        .unwrap();
    assert_eq!(queue["commit_sequence"], 13);
    assert!(queue["records"].as_array().unwrap().is_empty());
    assert!(
        f.call(
            "profile.review.inspect",
            json!({"scope":{"kind":"project","name":"alpha"},"id":3,"revision":3}),
        )
        .is_err()
    );
    drop(app);
}

fn changed_root_refuses_reviewed_apply(f: &Fixture) {
    let mut cfg = EngineConfig::new(Root::of::<precept_ui::App>());
    cfg.viewport = Size::new(px(1080.0), px(760.0));
    let mut app = nsh_testing::Headless::start(cfg).unwrap();
    writes_on(&mut app);
    manage_project(&mut app);
    add(&mut app, "root-bound-review", "synthetic");
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "commit 14");
    app.wait_for("#5 · root-bound-review").unwrap();
    app.wait_for("Show full digest").unwrap();
    app.press_key("alt+shift+y").unwrap();
    app.wait_for("Nothing is written until you apply it")
        .unwrap();
    let root = f.root.path().join("alpha");
    let moved = f.root.path().join("alpha-moved");
    std::fs::rename(&root, &moved).unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "Unauthorized");
    // Restore only this fixture-owned directory before inspecting its state.
    std::fs::rename(&moved, &root).unwrap();
    let after = f.inspect(5, 1);
    assert_eq!(after["record"]["status"], "suggested");
    assert_eq!(after["commit_sequence"], 14);
    assert!(after["decisions"].as_array().unwrap().is_empty());
}
