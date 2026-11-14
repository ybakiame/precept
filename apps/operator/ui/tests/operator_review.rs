//! Real NSH components over real daemon IPC for the S2 review workspace; not
//! native GUI/Wayland/TTY evidence. A separate integration binary so its global
//! `INPUTS` grant never races the original operator integration test.
#![cfg(feature = "ipc")]
use acp_ipc::{Location, operator};
use nsh_core::prelude::*;
use nsh_engine::{EngineConfig, Root};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
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
        .join("../../../target/debug")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("acp-s2-review-ui-")
            .tempdir()
            .unwrap();
        std::fs::create_dir(root.path().join("alpha")).unwrap();
        std::fs::write(root.path().join("alpha/AGENTS.md"), "Synthetic contract\n").unwrap();
        acp_ipc::transport::private_dir(&root.path().join("state"), true).unwrap();
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
        let child = Command::new(bin("acp-daemon"))
            .arg("--runtime-dir")
            .arg(&location.runtime_dir)
            .arg("--endpoint")
            .arg(&location.endpoint)
            .arg("--allow-project")
            .arg("alpha")
            .arg("--allow-root")
            .arg(format!(
                "alpha={}",
                root.path()
                    .join("alpha")
                    .canonicalize()
                    .unwrap()
                    .to_str()
                    .unwrap()
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
    fn call(&self, method: &str, params: Value) -> acp_ipc::Result<Value> {
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
    let label = format!("sanitized result containing {needle:?}");
    app.wait_until(&label, Duration::from_secs(10), |s| {
        s.texts().iter().any(|t| t.contains(needle))
    })
    .unwrap();
}

/// Exact detail, peer demotion, all decisions and delete consent through actual
/// NSH components over a real owned daemon. Desktop/TUI channel selection is
/// configured headlessly; these are not native input or visual acceptance.
#[test]
fn real_components_review_queue_detail_history_deprecate_and_delete() {
    let mut f = Fixture::new();
    acp_operator_ui::INPUTS
        .set(acp_operator_model::native::Inputs {
            runtime_dir: Some(f.location.runtime_dir.clone()),
            endpoint: Some(f.location.endpoint.clone()),
            operator: true,
            ..Default::default()
        })
        .ok()
        .unwrap();
    let mut cfg = EngineConfig::new(Root::of::<acp_operator_ui::App>());
    cfg.viewport = Size::new(px(1080.0), px(760.0));
    let mut app = nsh_testing::Headless::start(cfg).unwrap();
    app.wait_for("No Profile selected").unwrap();
    app.type_text(
        "Explicit Profile path",
        f.root
            .path()
            .join("state/profile.sqlite3")
            .to_str()
            .unwrap(),
    )
    .unwrap();
    app.press_key("alt+shift+u").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    app.wait_for("Profile selected").unwrap();
    app.press_key("alt+shift+e").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    app.wait_for("Write grant active").unwrap();
    f.register();
    app.press_key("mod+r").unwrap();
    app.wait_for(" / alpha").unwrap();
    app.wait_for("Write grant active").unwrap();
    // Suggest record #1 (editor) and apply it.
    app.press_key("alt+k").unwrap();
    app.type_text("Preference key", "editor").unwrap();
    app.type_text("Preference value", "synthetic-value")
        .unwrap();
    app.type_text("Preference source", "synthetic-source")
        .unwrap();
    app.press_key("alt+shift+s").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 2");
    // Queue tab loads the suggested-only page; the row opens the exact detail.
    app.press("Queue").unwrap();
    app.press("Inspect review queue").unwrap();
    contains(&mut app, "#1 · editor · r1");
    app.press("#1 · editor · r1").unwrap();
    app.wait_for("Show full digest").unwrap();
    app.press("Show full digest").unwrap();
    app.wait_for("Collapse digest").unwrap();
    // Confirm #1; it becomes a same-key confirmed peer for the next suggestion.
    let first = f.inspect(1, 1);
    app.press("Confirm preference…").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    assert_eq!(f.inspect(1, 1)["record"]["status"], "suggested");
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 3");
    let confirmed_first = f.assert_decision(&first, "confirm", "desktop", 2, 3);
    // Suggest record #2 with the same key, then inspect it from the queue.
    app.press_key("alt+shift+s").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 4");
    app.press("Queue").unwrap();
    app.press("Inspect review queue").unwrap();
    contains(&mut app, "#2 · editor · r1");
    app.press("#2 · editor · r1").unwrap();
    app.wait_for("Show full digest").unwrap();
    // The peer-demotion disclosure names the authoritative total and preview ids.
    app.wait_for("Deprecates 1 same-key confirmed records (ids 1)")
        .unwrap();
    // History for a never-decided suggestion is empty, never an error.
    app.press("History").unwrap();
    app.wait_for("No decisions recorded for this record")
        .unwrap();
    // Confirm #2 (demotes #1), then read its decision history.
    let second = f.inspect(2, 1);
    app.press("Detail").unwrap();
    app.wait_for("Show full digest").unwrap();
    app.press("Confirm preference…").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 5");
    let confirmed_second = f.assert_decision(&second, "confirm", "desktop", 2, 5);
    let demoted = f.assert_decision(&confirmed_first, "demote", "desktop", 3, 5);
    assert_eq!(demoted["record"]["status"], "deprecated");
    app.press_key("alt+shift+l").unwrap();
    contains(&mut app, "#2 · editor · confirmed · r2");
    app.press("#2 · editor · confirmed · r2").unwrap();
    app.press("Detail").unwrap();
    app.press("Inspect exact record").unwrap();
    app.wait_for("Show full digest").unwrap();
    app.press("History").unwrap();
    app.wait_for("Channel is client-asserted").unwrap();
    // Deprecate the confirmed #2 with its impact disclosure.
    app.press("Detail").unwrap();
    app.wait_for("Deprecation makes the record inactive in agent effective context.")
        .unwrap();
    app.press("Deprecate preference…").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 6");
    let deprecated_second = f.assert_decision(&confirmed_second, "deprecate", "desktop", 3, 6);
    assert_eq!(deprecated_second["record"]["status"], "deprecated");
    // Delete #2 through the destructive tier: a wrong retype refuses, the exact
    // retype arms, and the receipt leaves identity-only proof.
    app.press_key("alt+shift+l").unwrap();
    contains(&mut app, "#2 · editor · deprecated · r3");
    app.press("#2 · editor · deprecated · r3").unwrap();
    app.press("Detail").unwrap();
    app.press("Inspect exact record").unwrap();
    app.wait_for("Show full digest").unwrap();
    app.press_key("alt+shift+q").unwrap();
    app.wait_for("Delete project alpha preference #2?").unwrap();
    app.type_text("Retype the exact ID", "9").unwrap();
    app.wait_for("The typed ID does not match").unwrap();
    app.press_key("Enter").unwrap();
    assert!(!app.screen().shows("Prepared operation; no write sent yet"));
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
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 7");
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
    app.wait_for("Deleted. Content and decision history are gone; receipts remain identity-only.")
        .unwrap();
    // A deleted record keeps no content or decision history snapshot.
    app.press("History").unwrap();
    app.wait_for("Select a queue row to inspect").unwrap();
    // The queue is now empty: a valid pending-review state, never "unavailable".
    app.press("Queue").unwrap();
    app.press("Inspect review queue").unwrap();
    app.wait_for("No pending suggestions").unwrap();
    assert!(!app.screen().shows("Unavailable"));
    // Both locales ship the review copy.
    app.press_key("mod+l").unwrap();
    app.wait_for("待审队列").unwrap();
    app.wait_for("没有待审建议").unwrap();
    drop(app);
    terminal_channel_and_revoked_delete(&f);
    changed_root_refuses_reviewed_apply(&f);
    f.child.kill().unwrap();
    f.child.wait().unwrap();
}

fn terminal_channel_and_revoked_delete(f: &Fixture) {
    // Exercise the shared App's shell-supplied channel, not a fabricated CLI
    // mutation. This flag is the TUI entry point's own configuration seam.
    acp_operator_ui::TERMINAL_SURFACE.store(true, std::sync::atomic::Ordering::Relaxed);
    let mut cfg = EngineConfig::new(Root::of::<acp_operator_ui::App>());
    cfg.viewport = Size::new(px(1080.0), px(760.0));
    let mut app = nsh_testing::Headless::start(cfg).unwrap();
    app.wait_for("Write grant active").unwrap();
    app.press_key("alt+k").unwrap();
    app.type_text("Preference key", "terminal-editor").unwrap();
    app.type_text("Preference value", "terminal-value").unwrap();
    app.type_text("Preference source", "terminal-source")
        .unwrap();
    app.press_key("alt+shift+s").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 8");
    app.press("Queue").unwrap();
    app.press("Inspect review queue").unwrap();
    app.press("#3 · terminal-editor · r1").unwrap();
    app.wait_for("Show full digest").unwrap();
    let third = f.inspect(3, 1);
    app.press_key("alt+shift+y").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    assert_eq!(f.inspect(3, 1)["record"]["status"], "suggested");
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 9");
    let confirmed = f.assert_decision(&third, "confirm", "tui", 2, 9);
    app.press_key("alt+shift+l").unwrap();
    app.press("#3 · terminal-editor · confirmed · r2").unwrap();
    app.press("Detail").unwrap();
    app.press("Inspect exact record").unwrap();
    app.wait_for("Show full digest").unwrap();
    app.press("History").unwrap();
    contains(&mut app, "confirm · r1→r2 · tui");
    app.press("Detail").unwrap();
    app.press_key("alt+shift+w").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    assert_eq!(f.inspect(3, 2)["record"]["status"], "confirmed");
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 10");
    f.assert_decision(&confirmed, "deprecate", "tui", 3, 10);
    app.press_key("alt+shift+s").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 11");
    app.press("Queue").unwrap();
    app.press("Inspect review queue").unwrap();
    app.press("#4 · terminal-editor · r1").unwrap();
    app.wait_for("Show full digest").unwrap();
    let fourth = f.inspect(4, 1);
    app.press_key("alt+shift+n").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 12");
    f.assert_decision(&fourth, "reject", "tui", 2, 12);
    app.press_key("alt+shift+l").unwrap();
    app.press("#3 · terminal-editor · deprecated · r3").unwrap();
    app.press("Detail").unwrap();
    app.press("Inspect exact record").unwrap();
    app.wait_for("Show full digest").unwrap();
    app.press_key("alt+shift+q").unwrap();
    app.wait_for("Delete project alpha preference #3?").unwrap();
    app.type_text("Retype the exact ID", "3").unwrap();
    app.press_key("Enter").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    assert!(
        app.screen()
            .texts()
            .join("")
            .contains("\"channel\":\"tui\"")
    );
    // Revoke externally between inspection/consent and submit. The real IPC
    // adapter/daemon must refuse rather than accept an obsolete UI grant.
    f.call("profile.write.disable", json!({})).unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, "Unauthorized");
    assert_eq!(f.inspect(3, 3)["record"]["status"], "deprecated");
    app.press_key("alt+shift+r").unwrap();
    app.wait_for("Write grant off").unwrap();
    app.press_key("alt+shift+e").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    app.wait_for("Write grant active").unwrap();
    app.press_key("alt+shift+l").unwrap();
    app.press("#3 · terminal-editor · deprecated · r3").unwrap();
    app.press("Detail").unwrap();
    app.press("Inspect exact record").unwrap();
    app.wait_for("Show full digest").unwrap();
    app.press_key("alt+shift+q").unwrap();
    app.wait_for("Delete project alpha preference #3?").unwrap();
    // Revocation and the new grant must not retain the preceding exact text.
    app.wait_for("Type the ID exactly to arm deletion").unwrap();
    app.press_key("alt+shift+v").unwrap();
    assert_eq!(f.inspect(3, 3)["record"]["status"], "deprecated");
    app.type_text("Retype the exact ID", "3").unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 13");
    assert!(!app.screen().shows("Retry identical request"));
    app.wait_for("Deleted. Content and decision history are gone; receipts remain identity-only.")
        .unwrap();
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
    acp_operator_ui::TERMINAL_SURFACE.store(false, std::sync::atomic::Ordering::Relaxed);
}

fn changed_root_refuses_reviewed_apply(f: &Fixture) {
    let mut cfg = EngineConfig::new(Root::of::<acp_operator_ui::App>());
    cfg.viewport = Size::new(px(1080.0), px(760.0));
    let mut app = nsh_testing::Headless::start(cfg).unwrap();
    app.wait_for("Write grant active").unwrap();
    app.press_key("alt+k").unwrap();
    app.type_text("Preference key", "root-bound-review")
        .unwrap();
    app.type_text("Preference value", "synthetic").unwrap();
    app.type_text("Preference source", "synthetic").unwrap();
    app.press_key("alt+shift+s").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 14");
    app.press("Queue").unwrap();
    app.press("Inspect review queue").unwrap();
    app.press("#5 · root-bound-review · r1").unwrap();
    app.wait_for("Show full digest").unwrap();
    app.press_key("alt+shift+y").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
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
