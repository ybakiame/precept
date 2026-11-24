#![allow(dead_code)] // Shared by separate integration-test binaries.

use nsh_core::prelude::*;
use nsh_engine::{EngineConfig, Root};
use precept_ipc::{Location, operator};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub struct Fixture {
    child: Child,
    pub location: Location,
    pub root: tempfile::TempDir,
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

fn tokio_runtime<F, Fut, T>(f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f())
}

impl Fixture {
    pub fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("precept-sync-ui-")
            .tempdir()
            .unwrap();
        let runtime_dir = root.path().join("runtime");
        let endpoint = if cfg!(windows) {
            format!(
                "sync-ui-{}",
                root.path().file_name().unwrap().to_string_lossy()
            )
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
            .arg(root.path().join("daemon-state"))
            .env("HOME", root.path())
            .env("USERPROFILE", root.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("build workspace bins before NSH process tests");
        let mut fixture = Self {
            child,
            location,
            root,
        };
        let start = Instant::now();
        loop {
            if fixture.call("profile.info", json!({})).is_ok() {
                break;
            }
            assert!(fixture.child.try_wait().unwrap().is_none());
            assert!(start.elapsed() < Duration::from_secs(15));
            std::thread::sleep(Duration::from_millis(20));
        }
        fixture
    }

    pub fn call(&self, method: &str, params: Value) -> precept_ipc::Result<Value> {
        tokio_runtime(|| operator::control(&self.location, method, params))
    }

    pub fn prepare_profile(&self, value: &str) {
        let db = self.root.path().join("state/profile.sqlite3");
        precept_ipc::transport::private_dir(db.parent().unwrap(), true).unwrap();
        self.call("profile.create", json!({"path": db})).unwrap();
        let info = self.call("profile.info", json!({})).unwrap();
        self.call(
            "profile.write.enable",
            json!({
                "profile_id": info["binding"]["profile_id"],
                "generation": info["binding"]["generation"],
                "confirm": true
            }),
        )
        .unwrap();
        let binding: operator::Binding = serde_json::from_value(
            self.call("profile.info", json!({})).unwrap()["binding"].clone(),
        )
        .unwrap();
        let suggest = operator::Mutation {
            version: 2,
            binding: binding.clone(),
            operation_id: "suggest-sync-key".into(),
            command: operator::Command::SuggestPreference {
                scope: operator::Scope::Global,
                key: "sync-test".into(),
                value: value.into(),
                tags: vec!["testing".into()],
                origin: operator::Origin::Operator,
                rule: Default::default(),
                expected: operator::Expected::Absent,
            },
        };
        tokio_runtime(|| operator::mutate(&self.location, &suggest)).unwrap();
        let record = self
            .call(
                "profile.review.inspect",
                json!({"scope":{"kind":"global"},"id":1,"revision":1}),
            )
            .unwrap();
        let confirm = operator::Mutation {
            version: 2,
            binding,
            operation_id: "confirm-sync-key".into(),
            command: operator::Command::ConfirmPreference {
                scope: operator::Scope::Global,
                id: 1,
                proposal_digest: record["record"]["proposal_digest"].as_str().unwrap().into(),
                channel: operator::Channel::Cli,
                expected: operator::Expected::Revision(1),
                note: None,
            },
        };
        tokio_runtime(|| operator::mutate(&self.location, &confirm)).unwrap();
    }
}

pub fn contains(app: &mut nsh_testing::Headless, needle: &str) {
    app.wait_until("personal sync result", Duration::from_secs(15), |screen| {
        screen.texts().iter().any(|text| text.contains(needle))
    })
    .unwrap();
}

pub fn press_when_ready(app: &mut nsh_testing::Headless, label: &str) {
    let start = Instant::now();
    loop {
        match app.press(label) {
            Ok(()) => return,
            Err(error) => {
                assert!(error.contains("disabled"), "{error}");
                assert!(start.elapsed() < Duration::from_secs(15), "{label} ready");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

pub fn wait_for_file(app: &mut nsh_testing::Headless, path: &Path) {
    app.wait_until("export settles", Duration::from_secs(15), |screen| {
        path.exists()
            || screen
                .texts()
                .iter()
                .any(|text| text.contains("Personal sync failed"))
    })
    .unwrap();
    assert!(
        path.exists(),
        "export failed: {:?}",
        app.screen().texts().join("\n")
    );
}

pub fn project_write(fixture: &Fixture, method: &str, params: Value) -> Value {
    tokio_runtime(|| operator::project_write(&fixture.location, method, params)).unwrap()
}

pub fn trust(fixture: &Fixture, peer: &Fixture, method: &str) {
    let invitation: operator::PersonalSyncInvitation =
        serde_json::from_value(peer.call("profile.sync.invitation", json!({})).unwrap()).unwrap();
    let expected_owner_fingerprint = invitation.owner_fingerprint.clone();
    let expected_invitation_fingerprint =
        precept_ui_client::sync_files::invitation_fingerprint(&invitation).unwrap();
    project_write(
        fixture,
        method,
        serde_json::to_value(operator::PersonalSyncTrustRequest {
            invitation,
            expected_owner_fingerprint,
            expected_invitation_fingerprint,
        })
        .unwrap(),
    );
}

pub fn start_app(fixture: &Fixture) -> nsh_testing::Headless {
    precept_ui::INPUTS
        .set(precept_ui_client::native::Inputs {
            runtime_dir: Some(fixture.location.runtime_dir.clone()),
            endpoint: Some(fixture.location.endpoint.clone()),
            operator: true,
            ..Default::default()
        })
        .ok()
        .unwrap();
    let mut config = EngineConfig::new(Root::of::<precept_ui::App>());
    config.viewport = Size::new(px(1180.0), px(900.0));
    let mut app = nsh_testing::Headless::start(config).unwrap();
    app.wait_for("Reconnect").unwrap();
    app
}
