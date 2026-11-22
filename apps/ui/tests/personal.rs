//! Personal preferences over a real daemon: save as suggested, then confirm
//! the exact record (two explicit steps).
//! Real NSH components and IPC; not native GUI/Wayland/TTY evidence.
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
    precept_ipc::transport::private_dir(db.parent().unwrap(), true).unwrap();
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
    app.press("Review & rules").unwrap();
    app.press("Personal").unwrap();
    writes_on(&mut app);
    // A daemon read finishing afterwards keeps the chosen tab (it must not
    // revert Personal back to To review).
    app.press("Reconnect").unwrap();
    writes_on(&mut app);
    app.wait_for("Add preference").unwrap();
    // The scope is fixed to personal (global): no inbox scope switch here.
    assert!(!app.screen().texts().contains(&"Global scope"));
    // No Profile summary here: Profile and writes live in Runtime checks.
    assert!(!app.screen().shows("Inspect Profile"));
    // Step one: Add preference saves a suggestion (prepared, then applied).
    app.press("Add preference").unwrap();
    app.wait_for("Add a personal preference").unwrap();
    app.type_text("Preference key", "personal-editor").unwrap();
    app.type_text("id:preference-value", "Prefer **small** commits.")
        .unwrap();
    // Saving needs a tag; an invalid one is refused in the help line first.
    assert!(app.screen().shows("Save as suggestion"));
    app.type_text("Tags", "a/b").unwrap();
    app.wait_for("Letters, digits and hyphens only, up to 32 characters.")
        .unwrap();
    app.clear("Tags").unwrap();
    app.type_text("Tags", "git\n").unwrap();
    app.press("An agent suggested it").unwrap();
    // Rule options: only for Rust, priority 5, declared hard.
    app.type_text("Languages", "Rust").unwrap();
    app.type_text("Priority (-100 to 100)", "5").unwrap();
    app.press("Hard").unwrap();
    app.press("Save as suggestion").unwrap();
    app.wait_for("Nothing is written until you apply it")
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
    assert_eq!(
        global(&f)[0]["rule"]["applies_to"]["languages"],
        json!(["rust"])
    );
    assert_eq!(global(&f)[0]["rule"]["priority"], 5);
    assert_eq!(global(&f)[0]["rule"]["enforcement"], "hard");
    // The value is stored as Markdown, as written.
    assert_eq!(global(&f)[0]["value"], "Prefer **small** commits.");
    // Step two is offered at once: the exact record, inspected, in Detail.
    contains(&mut app, "personal-editor");
    let start = Instant::now();
    loop {
        app.press_key("alt+shift+y").unwrap();
        if app
            .wait_for("Nothing is written until you apply it")
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
    // The confirmed rule shows its options, and its switch takes it out of
    // what agents receive without a new review.
    contains(&mut app, "Applies to languages rust");
    contains(&mut app, "priority 5");
    app.press("#1 · personal-editor · confirmed · r2").unwrap();
    app.wait_until("switch ready", Duration::from_secs(10), |s| {
        s.texts().contains(&"Delivered to agents")
    })
    .unwrap();
    let start = Instant::now();
    while global(&f)[0]["enabled"] != false {
        let _ = app.press("Delivered to agents");
        assert!(start.elapsed() < Duration::from_secs(10), "switched off");
        std::thread::sleep(Duration::from_millis(200));
    }
    assert_eq!(global(&f)[0]["status"], "confirmed");
    contains(&mut app, "Off");

    // An agent's suggestion with the wrong key: the operator edits it, with a
    // note. The edit is a new suggestion; the old one is closed as replaced.
    let binding: operator::Binding = serde_json::from_value(f.info()["binding"].clone()).unwrap();
    let mutation = operator::Mutation {
        version: 2,
        binding,
        operation_id: "agent-tone".into(),
        command: operator::Command::SuggestPreference {
            scope: operator::Scope::Global,
            key: "tone".into(),
            value: "Short answers.".into(),
            tags: vec!["style".into()],
            origin: operator::Origin::Agent,
            rule: Default::default(),
            expected: operator::Expected::Absent,
        },
    };
    tokio_runtime(|| operator::mutate(&f.location, &mutation)).unwrap();
    app.press_key("escape").unwrap();
    // Refresh reads the list again even though the Profile did not change.
    app.press("Refresh").unwrap();
    app.wait_until("row listed", Duration::from_secs(10), |s| {
        s.count("role:button[#2 · tone · suggested · r1]")
            .unwrap_or(0)
            > 0
    })
    .unwrap();
    app.press("#2 · tone · suggested · r1").unwrap();
    app.wait_for("Edit…").unwrap();
    app.type_text("Note (optional)", "Key should say what it governs.")
        .unwrap();
    app.press("Edit…").unwrap();
    app.wait_for("Edit suggestion #2").unwrap();
    assert_eq!(
        app.screen().input_text("Preference key").unwrap(),
        Some("tone")
    );
    app.clear("Preference key").unwrap();
    app.type_text("Preference key", "answer-length").unwrap();
    app.press("Save as a new suggestion").unwrap();
    app.wait_for("Nothing is written until you apply it")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    let record = |f: &Fixture, id: i64| {
        f.call(
            "profile.preferences.get",
            json!({"scope":{"kind":"global"},"id":id}),
        )
        .ok()
        .map(|v| v["record"].clone())
    };
    let start = Instant::now();
    while record(&f, 3).is_none() {
        assert!(start.elapsed() < Duration::from_secs(10), "edit applied");
        std::thread::sleep(Duration::from_millis(20));
    }
    let edited = record(&f, 3).unwrap();
    assert_eq!(edited["key"], "answer-length");
    assert_eq!(edited["status"], "suggested");
    assert_eq!(edited["tags"], json!(["style"]));
    assert_eq!(record(&f, 2).unwrap()["status"], "rejected");
    let history = f
        .call(
            "profile.review.inspect",
            json!({"scope":{"kind":"global"},"id":2,"revision":2}),
        )
        .unwrap();
    assert_eq!(history["decisions"][0]["action"], "replace");
    assert_eq!(history["decisions"][0]["replaced_by"], 3);
    assert_eq!(
        history["decisions"][0]["note"],
        "Key should say what it governs."
    );

    // A batch: pick two suggestions, add a note, confirm both in one write.
    let binding: operator::Binding = serde_json::from_value(f.info()["binding"].clone()).unwrap();
    let another = operator::Mutation {
        version: 2,
        binding,
        operation_id: "agent-review".into(),
        command: operator::Command::SuggestPreference {
            scope: operator::Scope::Global,
            key: "review-style".into(),
            value: "Review in small batches.".into(),
            tags: vec!["review".into()],
            origin: operator::Origin::Agent,
            rule: Default::default(),
            expected: operator::Expected::Absent,
        },
    };
    tokio_runtime(|| operator::mutate(&f.location, &another)).unwrap();
    // The edit opened its replacement (#3) for confirmation; close it.
    app.wait_for("#3 · answer-length").unwrap();
    app.press("Close detail").unwrap();
    app.press("Refresh").unwrap();
    app.wait_until("row listed", Duration::from_secs(10), |s| {
        s.count("role:button[#4 · review-style · suggested · r1]")
            .unwrap_or(0)
            > 0
    })
    .unwrap();
    assert!(!app.screen().shows("Confirm 2…"));
    app.press("Pick #3 for a batch decision").unwrap();
    app.press("Pick #4 for a batch decision").unwrap();
    app.wait_for("2 picked").unwrap();
    app.type_text("Note (optional)", "Both reviewed together.")
        .unwrap();
    app.press("Confirm 2…").unwrap();
    app.wait_for("Nothing is written until you apply it")
        .unwrap();
    contains(&mut app, "Confirm 2 suggestions");
    app.press_key("alt+shift+v").unwrap();
    let start = Instant::now();
    while record(&f, 4).unwrap()["status"] != "confirmed" {
        assert!(start.elapsed() < Duration::from_secs(10), "batch applied");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(record(&f, 3).unwrap()["status"], "confirmed");
    let history = f
        .call(
            "profile.review.inspect",
            json!({"scope":{"kind":"global"},"id":4,"revision":2}),
        )
        .unwrap();
    assert_eq!(history["decisions"][0]["note"], "Both reviewed together.");
    app.wait_until("selection cleared", Duration::from_secs(10), |s| {
        !s.shows("2 picked")
    })
    .unwrap();

    // A confirmed rule is edited in place of itself: the new version takes
    // effect at once, the earlier text stays in its history.
    app.press("Refresh").unwrap();
    app.wait_until("confirmed row", Duration::from_secs(10), |s| {
        s.count("role:button[#4 · review-style · confirmed · r2]")
            .unwrap_or(0)
            > 0
    })
    .unwrap();
    app.press("#4 · review-style · confirmed · r2").unwrap();
    app.wait_for("Edit…").unwrap();
    app.type_text("Note (optional)", "Smaller still.").unwrap();
    app.press("Edit…").unwrap();
    app.wait_for("Edit rule #4").unwrap();
    app.clear("Preference key").unwrap();
    app.type_text("Preference key", "review-pace").unwrap();
    app.press("Save the new version").unwrap();
    app.wait_for("Nothing is written until you apply it")
        .unwrap();
    app.press_key("alt+shift+v").unwrap();
    let start = Instant::now();
    while record(&f, 5).is_none() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "rule edit applied"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let edited = record(&f, 5).unwrap();
    assert_eq!(edited["status"], "confirmed");
    assert_eq!(edited["key"], "review-pace");
    let earlier = record(&f, 4).unwrap();
    assert_eq!(earlier["status"], "deprecated");
    assert_eq!(earlier["key"], "review-style");

    // An agent's update of that rule: review shows what it would replace.
    let binding: operator::Binding = serde_json::from_value(f.info()["binding"].clone()).unwrap();
    let update = operator::Mutation {
        version: 2,
        binding,
        operation_id: "agent-update".into(),
        command: operator::Command::SuggestPreference {
            scope: operator::Scope::Global,
            key: "review-pace".into(),
            value: "Review two changes at a time.".into(),
            tags: vec!["review".into()],
            origin: operator::Origin::Agent,
            rule: Default::default(),
            expected: operator::Expected::Absent,
        },
    };
    tokio_runtime(|| operator::mutate(&f.location, &update)).unwrap();
    // The edit opened the new version's detail; close it first.
    app.press("Close detail").unwrap();
    app.press("Refresh").unwrap();
    app.wait_until("update listed", Duration::from_secs(10), |s| {
        s.count("role:button[#6 · review-pace · suggested · r1]")
            .unwrap_or(0)
            > 0
    })
    .unwrap();
    app.press("#6 · review-pace · suggested · r1").unwrap();
    app.wait_for("Updates #5 · review-pace").unwrap();
    contains(
        &mut app,
        "Confirming replaces the confirmed rule with this version",
    );
}
