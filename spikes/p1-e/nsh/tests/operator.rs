//! Real NSH components over real daemon IPC; not native GUI/Wayland/TTY evidence.
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
fn real_components_profile_grant_suggestion_confirm_reject_and_disconnect() {
    let mut f = Fixture::new();
    acp_nsh_spike::INPUTS
        .set(acp_ui_spike_model::native::Inputs {
            runtime_dir: Some(f.location.runtime_dir.clone()),
            endpoint: Some(f.location.endpoint.clone()),
            operator: true,
            ..Default::default()
        })
        .ok()
        .unwrap();
    let mut cfg = EngineConfig::new(Root::of::<acp_nsh_spike::App>());
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
    assert_eq!(f.info()["selected"], false);
    app.press_key("alt+shift+v").unwrap();
    app.wait_for("Profile selected").unwrap();
    assert_eq!(f.info()["write_enabled"], false);
    app.press_key("alt+shift+e").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    assert_eq!(f.info()["write_enabled"], false);
    app.press_key("alt+shift+v").unwrap();
    app.wait_for("Write grant active").unwrap();
    f.register();
    app.press_key("mod+r").unwrap();
    app.wait_for(" / alpha").unwrap();
    app.wait_for("Write grant active").unwrap();
    app.press_key("alt+k").unwrap();
    app.type_text("Preference key", "editor").unwrap();
    app.type_text("Preference value", "synthetic-value")
        .unwrap();
    app.type_text("Preference source", "synthetic-source")
        .unwrap();
    app.press_key("alt+shift+s").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
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
    contains(&mut app, " · commit 2");
    app.press_key("alt+shift+l").unwrap();
    app.wait_for("#1 · editor · suggested · r1").unwrap();
    app.press_key("alt+shift+i").unwrap();
    app.press_key("alt+shift+y").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 3");
    app.press_key("alt+shift+s").unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 4");
    app.press_key("alt+shift+l").unwrap();
    app.wait_for("#2 · editor · suggested · r1").unwrap();
    app.press_key("alt+shift+i").unwrap();
    app.press_key("alt+shift+i").unwrap();
    app.press_key("alt+shift+n").unwrap();
    app.wait_for("Prepared operation; no write sent yet")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    contains(&mut app, " · commit 5");
    let page = f
        .call(
            "profile.preferences.list",
            json!({"scope":{"kind":"project","name":"alpha"},"after_id":0}),
        )
        .unwrap();
    assert_eq!(page["records"][0]["status"], "confirmed");
    assert_eq!(page["records"][1]["status"], "rejected");
    let request: operator::Mutation = serde_json::from_value(json!({
        "version":1,"binding":f.info()["binding"],"operation_id":"headless-handoff",
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
    app.press_key("alt+shift+x").unwrap();
    assert_eq!(f.info()["write_enabled"], true);
    app.press_key("alt+shift+v").unwrap();
    app.wait_for("Write grant off").unwrap();
    assert_eq!(f.info()["write_enabled"], false);
    app.press_key("mod+l").unwrap();
    app.wait_for("写入授权关闭").unwrap();
    app.press_key("Escape").unwrap();
    app.press_key("alt+3").unwrap();
    app.wait_for("已生效的确认偏好 · 键 / 值 / 范围 / 来源")
        .unwrap();
    f.child.kill().unwrap();
    f.child.wait().unwrap();
    app.press_key("alt+o").unwrap();
    contains(&mut app, "Unavailable");
    assert!(app.screen().clipboard().is_none());
}
