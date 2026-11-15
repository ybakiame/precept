//! S3 extensions. Every cell owns a fresh disposable daemon/Profile/UI; failures
//! do not suppress later independent cells. Relay faults forward real daemon
//! replies and are explicitly not described as native typing of protocol fields.
use super::{Fixture, Ui, desktop, evidence, tty};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct MissingFocus;

struct Session {
    // Declaration order keeps the real UI owned until its evidence is saved, and
    // tears it down before the daemon and disposable private root are removed.
    ui: Ui,
    f: Fixture,
    out: PathBuf,
    desktop: bool,
}
impl Session {
    fn new(surface: &str, out: &Path, script: &str) -> Self {
        fs::create_dir(out).unwrap();
        let repo = std::env::current_dir().unwrap();
        let f = Fixture::new(repo.clone());
        let nonce = format!(
            "{:016x}",
            f.root
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .bytes()
                .fold(0u64, |n, b| n.wrapping_mul(131).wrapping_add(b as u64))
        );
        let is_desktop = surface == "desktop";
        let binary = repo.join("apps/operator/target/debug").join(format!(
            "acp-nsh-{}{}",
            if is_desktop { "native" } else { "tui" },
            std::env::consts::EXE_SUFFIX
        ));
        let args = vec![
            "--runtime-dir".into(),
            f.runtime.to_str().unwrap().into(),
            "--endpoint".into(),
            f.alias.clone(),
            "--operator".into(),
            "--locale".into(),
            "en".into(),
            "--appearance".into(),
            "light".into(),
            "--project".into(),
            "alpha".into(),
        ];
        let mut ui = if is_desktop {
            Ui::Desktop(Box::new(desktop::Desktop::start(
                &binary, &args, &nonce, script, out, &f.focus,
            )))
        } else {
            Ui::Tty(tty::Tty::start(&binary, &args, out, &f.focus))
        };
        evidence::event(
            "owned-ui",
            json!({"pid":ui.pid(),"nonce":nonce,"fixture":"fresh-independent-synthetic"}),
        );
        ui.key("alt+o");
        ui.wait(
            "selected=false grant=false busy=false",
            "No Profile selected",
        );
        ui.capture("unselected-owned-profile", out);
        ui.field("alt+p", f.db.to_str().unwrap());
        ui.apply(
            "alt+shift+u",
            "selected=true grant=false busy=false",
            "Profile selected",
        );
        ui.apply(
            "alt+shift+e",
            "selected=true grant=true busy=false",
            "Write grant active",
        );
        for name in ["alpha", "beta"] {
            let root = f.root.path().join(name).canonicalize().unwrap();
            f.mutate(
                &format!("register-{name}"),
                &["project-register", name, root.to_str().unwrap()],
            );
        }
        ui.resize(
            if is_desktop { 1080 } else { 160 },
            if is_desktop { 1000 } else { 90 },
        );
        ui.key("ctrl+r");
        ui.wait("phase=Ready", "alpha");
        ui.field("alt+k", "matrix-key");
        ui.field("alt+w", "synthetic-matrix-value");
        ui.field("alt+f", "synthetic-matrix-source");
        let mut session = Self {
            ui,
            f,
            out: out.into(),
            desktop: is_desktop,
        };
        session.capture("ready");
        session
    }
    fn action(&mut self, key: &str, anchor: bool) {
        self.ui
            .action(key, &self.f.focus, anchor.then_some("alt+f"));
    }
    fn capture(&mut self, name: &str) {
        self.ui.capture(name, &self.out);
    }
    fn seed(&self, count: usize, global: bool, project: &str) {
        for i in 0..count {
            let key = format!("matrix-{i:02}");
            let op = format!(
                "seed-{}-{project}-{i}",
                if global { "global" } else { "project" }
            );
            let mut args = vec![
                "preference-suggest",
                "--scope",
                if global { "global" } else { "project" },
            ];
            if !global {
                args.extend(["--project", project]);
            }
            args.extend([
                "--key",
                &key,
                "--value",
                "synthetic-matrix-value",
                "--source",
                "synthetic-matrix-source",
                "--origin",
                "agent_reported",
            ]);
            self.f.mutate(&op, &args);
        }
    }
    fn queue(&self, global: bool, after: i64) -> Value {
        let after = after.to_string();
        let mut args = vec![
            "review-queue",
            "--scope",
            if global { "global" } else { "project" },
            "--after-id",
            &after,
        ];
        if !global {
            args.extend(["--project", "alpha"]);
        }
        let (ok, queue) = self.f.query(&args);
        assert!(ok);
        let rows = queue["records"].as_array().unwrap();
        assert!(rows.len() <= 12);
        assert!(
            rows.windows(2)
                .all(|r| r[0]["id"].as_i64().unwrap() < r[1]["id"].as_i64().unwrap())
        );
        assert!(rows.iter().all(|r| r["status"] == "suggested"
            && r["scope"]["kind"] == if global { "global" } else { "project" }));
        assert!(global || rows.iter().all(|r| r["scope"]["name"] == "alpha"));
        evidence::event(
            "independent-cli-queue-proof",
            json!({
                "scope":if global {"global"} else {"project-alpha"},"ids":rows.iter().map(|r| r["id"].clone()).collect::<Vec<_>>(),
                "after_id":after,"commit_sequence":queue["commit_sequence"],"next_after_id":queue["next_after_id"],
                "response_sha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&queue).unwrap())),"native_render":false
            }),
        );
        queue
    }
    fn select_first(&mut self, rows: usize) {
        self.action("OperatorList", true);
        self.ui.wait(&format!("rows={rows}"), "#1");
        self.ui.key("alt+shift+i");
        self.ui.key("alt+shift+y");
        self.ui
            .wait("inspected_id=1 inspected_revision=1", "Show full digest");
    }
    fn prepare(&mut self) {
        self.ui.key("alt+shift+y");
        self.ui.wait("review=true", "Prepared operation");
        self.capture("prepared-exact-review");
    }
    fn unchanged(&self, sequence: i64, rows: &[Value]) {
        assert_eq!(self.f.commit_sequence(), sequence);
        assert_eq!(self.f.rows().as_slice(), rows);
        evidence::event(
            "independent-cli-no-transaction",
            json!({"commit_sequence":sequence,"rows_identical":true,"native_render":false}),
        );
    }
    fn inspect(&self) -> Value {
        let revision = self.f.record(1)["revision"].to_string();
        let (ok, value) = self.f.query(&[
            "review-inspect",
            "1",
            "--scope",
            "project",
            "--project",
            "alpha",
            "--revision",
            &revision,
        ]);
        assert!(ok);
        assert!(value["decisions"].as_array().unwrap().len() <= 8);
        value
    }
    fn fault(&self, method: &str, action: &str) {
        fs::write(
            self.f.root.path().join("fault-next.json"),
            json!({"method":method,"action":action}).to_string(),
        )
        .unwrap();
        evidence::event(
            "contained-ipc-fault-armed",
            json!({"method":method,"action":action,"fabricated_response":false,"native_typing":false}),
        );
    }
    fn enable_cli(&self) {
        let info = self.f.query(&["profile", "info"]).1;
        let b = &info["binding"];
        let previous_epoch = b["owner_epoch"].as_i64().unwrap();
        assert!(
            self.f
                .query(&[
                    "profile",
                    "write",
                    "enable",
                    "--profile-id",
                    b["profile_id"].as_str().unwrap(),
                    "--generation",
                    b["generation"].as_str().unwrap(),
                    "--confirm"
                ])
                .0
        );
        let current_epoch = self.f.query(&["profile", "info"]).1["binding"]["owner_epoch"]
            .as_i64()
            .unwrap();
        assert_eq!(current_epoch, previous_epoch + 1);
        evidence::event(
            "independent-cli-owner-epoch-change",
            json!({"previous":previous_epoch,"current":current_epoch,"native_render":false}),
        );
    }
    fn save_fault_proof(&self) {
        for name in [
            "fault-proof.json",
            "dropped-receipt.json",
            "replayed-receipt.json",
            "dropped-request.sha256",
            "mutation-count",
            "request-hashes.jsonl",
        ] {
            let path = self.f.root.path().join(name);
            if path.exists() {
                assert!(fs::metadata(&path).unwrap().len() <= 256 * 1024);
                fs::copy(path, self.out.join(name)).unwrap();
                evidence::event(
                    "contained-process-proof",
                    json!({"artifact":name,"native_render":false}),
                );
            }
        }
    }
    fn close(&mut self) {
        self.capture("completed");
        self.save_fault_proof();
        self.ui.close();
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if std::thread::panicking() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.capture("failed-last-owned-frame")
            }));
        }
        // No content-bearing profile/request/credential files are copied.
        let _ = fs::copy(&self.f.focus, self.out.join("focus.events"));
        for name in [
            "fault-proof.json",
            "dropped-receipt.json",
            "replayed-receipt.json",
            "dropped-request.sha256",
            "mutation-count",
            "request-hashes.jsonl",
        ] {
            let _ = fs::copy(self.f.root.path().join(name), self.out.join(name));
        }
    }
}
fn empty(s: &mut Session) {
    s.action("ReviewQueueLoad", true);
    s.ui.wait("queue=0", "No pending suggestions");
    assert!(s.queue(false, 0)["records"].as_array().unwrap().is_empty());
    s.capture("initial-empty-queue");
}
fn paging(s: &mut Session, global: bool) {
    s.seed(13, global, "alpha");
    s.seed(2, false, "beta");
    if global {
        s.action("OperatorGlobal", true);
        s.ui.key("alt+shift+r");
        s.ui.wait("selected=true grant=true busy=false", "Write grant active");
    }
    s.action("ReviewQueueLoad", true);
    let first = s.queue(global, 0);
    assert_eq!(first["records"].as_array().unwrap().len(), 12);
    s.ui.wait("queue=12", "#1 ·");
    let sequence = first["commit_sequence"].as_i64().unwrap();
    s.ui.wait(
        &format!("queue_sequence={sequence}"),
        &format!("Queue watermark: commit {sequence}"),
    );
    s.capture("pending-first-page");
    let mut args = vec![
        "preference-suggest",
        "--scope",
        if global { "global" } else { "project" },
    ];
    if !global {
        args.extend(["--project", "alpha"]);
    }
    args.extend([
        "--key",
        "matrix-interference",
        "--value",
        "synthetic-matrix-value",
        "--source",
        "synthetic-matrix-source",
        "--origin",
        "legacy_unknown",
    ]);
    s.f.mutate("between-queue-pages", &args);
    s.action("ReviewQueueNext", true);
    let after = first["next_after_id"].as_i64().unwrap();
    let second = s.queue(global, after);
    assert_eq!(second["records"].as_array().unwrap().len(), 2);
    assert_eq!(second["commit_sequence"], sequence + 1);
    assert!(
        second["records"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["id"].as_i64().unwrap() > after)
    );
    s.ui.wait("queue_stale=true", "Queue changed since page loaded");
    s.ui.wait("queue=2", "#13 ·");
    s.capture("pending-next-page-stale-disclosure");
    s.action("ReviewQueueFirst", true);
    s.ui.wait("queue=12", "#1 ·");
    s.capture("pending-first-page-refreshed");
}
fn invalidation(s: &mut Session, kind: &str) {
    s.seed(2, false, "alpha");
    s.select_first(2);
    let sequence = s.f.commit_sequence();
    let before = s.f.rows();
    if kind != "scope-change" {
        s.prepare();
    }
    if kind == "scope-change" {
        evidence::event(
            "native-scope-guard",
            json!({"inspected_id":1,"prepared":false,"reason":"scope-controls-are-concealed-during-prepared-review; switching-exact-inspection"}),
        );
    }
    match kind {
        "selected-id-change" => s.ui.key("alt+shift+i"),
        "scope-change" => {
            s.action("OperatorGlobal", true);
        }
        "project-change" => s.ui.key("ctrl+p"),
        "source-change" => s.ui.key("ctrl+r"),
        "source-field-change" => s.ui.field("alt+f", "synthetic-changed-source"),
        "target-change" => {
            s.action("Connection", false);
            s.action("SyntheticTarget", false);
            s.action("ApplyConnection", false);
            s.ui.key("alt+o");
        }
        _ => unreachable!(),
    }
    s.ui.key("alt+shift+v");
    thread::sleep(Duration::from_millis(250));
    match &s.ui {
        Ui::Desktop(d) => d.wait("review=false"),
        Ui::Tty(t) => t.wait_gone("Prepared operation"),
    }
    s.unchanged(sequence, &before);
    s.capture("navigation-invalidated-review");
}
fn changed_authority(s: &mut Session, kind: &str) {
    s.seed(1, false, "alpha");
    s.select_first(1);
    s.prepare();
    let sequence = s.f.commit_sequence();
    let before = s.f.rows();
    let error = match kind {
        "profile-generation-change" => {
            s.fault("profile.mutate", "generation");
            "Unauthorized"
        }
        "profile-epoch-change" => {
            assert!(s.f.query(&["profile", "write", "disable"]).0);
            s.enable_cli();
            "Unauthorized"
        }
        "revoke-inspect-apply" => {
            assert!(s.f.query(&["profile", "write", "disable"]).0);
            "Unauthorized"
        }
        _ => unreachable!(),
    };
    s.ui.key("alt+shift+v");
    s.ui.wait(&format!("error={error}"), error);
    s.unchanged(sequence, &before);
    s.capture("authority-refused-old-inspection");
}
fn status_change(s: &mut Session) {
    s.seed(1, false, "alpha");
    s.select_first(1);
    s.prepare();
    let digest = s.f.digest(1);
    s.f.mutate(
        "independent-status-change",
        &[
            "preference-reject",
            "1",
            "--scope",
            "project",
            "--project",
            "alpha",
            "--expected-revision",
            "1",
            "--proposal-digest",
            &digest,
            "--confirm",
        ],
    );
    let sequence = s.f.commit_sequence();
    let before = s.f.rows();
    s.ui.key("alt+shift+v");
    s.ui.wait("error=Conflict", "Conflict");
    s.unchanged(sequence, &before);
    assert_eq!(s.f.record(1)["status"], "rejected");
    s.capture("competing-status-refused");
}
fn delayed_read(s: &mut Session) {
    s.seed(1, false, "alpha");
    s.action("OperatorList", true);
    s.ui.wait("rows=1", "#1");
    s.ui.key("alt+shift+i");
    let sequence = s.f.commit_sequence();
    let before = s.f.rows();
    s.fault("profile.review.inspect", "delay");
    s.ui.key("alt+shift+y");
    let end = Instant::now() + Duration::from_secs(2);
    while !s.f.root.path().join("read-held").exists() {
        assert!(Instant::now() < end, "real inspect reply was not held");
        thread::sleep(Duration::from_millis(10));
    }
    evidence::event(
        "real-read-reply-held",
        json!({"method":"profile.review.inspect"}),
    );
    s.ui.key("ctrl+p");
    fs::write(s.f.root.path().join("release-read"), b"release").unwrap();
    thread::sleep(Duration::from_millis(250));
    let proof: Value =
        serde_json::from_slice(&fs::read(s.f.root.path().join("fault-proof.json")).unwrap())
            .unwrap();
    assert_eq!(proof["released_after_navigation"], true);
    s.ui.key("alt+o");
    s.ui.wait("selected=true grant=true busy=false", "Write grant active");
    s.ui.field("alt+f", "synthetic-matrix-source");
    s.ui.tab_nav(1);
    s.ui.wait(
        "inspected_id=0 inspected_revision=0",
        "Select a queue row to inspect",
    );
    s.ui.key("alt+shift+v");
    s.unchanged(sequence, &before);
    s.capture("delayed-inspect-discarded-after-navigation");
}
fn unknown(s: &mut Session, kind: &str) {
    s.seed(1, false, "alpha");
    s.select_first(1);
    s.prepare();
    let sequence = s.f.commit_sequence() + 1;
    fs::write(s.f.root.path().join("drop-next"), b"synthetic-reply-loss").unwrap();
    s.ui.key("alt+shift+v");
    s.ui.wait("unknown=true", "Outcome unknown");
    s.capture("committed-confirm-unobserved");
    let before = s.f.rows();
    assert_eq!(before[0]["status"], "confirmed");
    let decisions = s.inspect()["decisions"].clone();
    assert_eq!(decisions.as_array().unwrap().len(), 1);
    assert_eq!(decisions[0]["action"], "confirm");
    let receipt: Value =
        serde_json::from_slice(&fs::read(s.f.root.path().join("dropped-receipt.json")).unwrap())
            .unwrap();
    assert_eq!(receipt["commit_sequence"], sequence);
    thread::sleep(Duration::from_millis(500));
    assert_eq!(
        fs::read_to_string(s.f.root.path().join("mutation-count")).unwrap(),
        "1"
    );
    s.unchanged(sequence, &before);
    evidence::event(
        "no-automatic-replay-window",
        json!({"milliseconds":500,"forwarded_mutations":1,"decisions":1}),
    );
    match kind {
        "unknown-no-auto-replay" => {}
        "manual-exact-replay" => {
            s.ui.key("alt+shift+z");
            s.ui.wait(
                &format!("receipt={sequence} "),
                &format!("commit {sequence}"),
            );
            let replayed: Value = serde_json::from_slice(
                &fs::read(s.f.root.path().join("replayed-receipt.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(receipt, replayed);
            assert_eq!(
                fs::read_to_string(s.f.root.path().join("mutation-count")).unwrap(),
                "2"
            );
        }
        "replay-changed-bytes" => {
            s.fault("profile.mutate", "changed-bytes");
            s.ui.key("alt+shift+z");
            s.ui.wait("error=IdempotencyConflict", "IdempotencyConflict");
        }
        "replay-stale-epoch" => {
            assert!(s.f.query(&["profile", "write", "disable"]).0);
            s.enable_cli();
            s.ui.key("alt+shift+z");
            s.ui.wait("error=Unauthorized", "Unauthorized");
        }
        _ => unreachable!(),
    }
    s.unchanged(sequence, &before);
    assert_eq!(s.inspect()["decisions"], decisions);
    fs::write(s.out.join("decision-replay-proof.json"), serde_json::to_vec_pretty(&json!({"receipt":receipt,"decisions_before":decisions,"decisions_identical":true,"sequence":sequence})).unwrap()).unwrap();
    s.capture("replay-result-no-duplicate-decision");
}
fn presentation(s: &mut Session, zh: bool) {
    s.seed(1, false, "alpha");
    s.select_first(1);
    if zh {
        s.ui.key("ctrl+l");
    }
    for mode in ["Light", "Dark"] {
        s.action("Settings", false);
        s.action(mode, false);
        if let Ui::Desktop(d) = &s.ui {
            d.wait(&format!("mode={mode}"));
        }
        s.ui.key("alt+o");
        s.ui.wait(
            "selected=true grant=true busy=false",
            if zh {
                "写入授权有效"
            } else {
                "Write grant active"
            },
        );
        // Re-enter Detail after the settings navigation. This is a real read,
        // not a fixture callback or a screenshot-only synthetic surface.
        s.ui.key("alt+shift+l");
        s.ui.wait("rows=1", "#1");
        s.ui.key("alt+shift+i");
        s.ui.key("alt+shift+y");
        s.ui.wait(
            "inspected_id=1 inspected_revision=1",
            if zh {
                "显示完整摘要"
            } else {
                "Show full digest"
            },
        );
        for compact in [false, true] {
            s.ui.resize(
                if s.desktop {
                    if compact { 640 } else { 1080 }
                } else if compact {
                    80
                } else {
                    160
                },
                if s.desktop {
                    if compact { 480 } else { 1000 }
                } else if compact {
                    30
                } else {
                    90
                },
            );
            s.capture(&format!(
                "review-{}-{}-{}",
                if zh { "zh" } else { "en" },
                mode.to_ascii_lowercase(),
                if compact { "compact" } else { "large" }
            ));
        }
        s.ui.resize(
            if s.desktop { 1080 } else { 160 },
            if s.desktop { 1000 } else { 90 },
        );
    }
}
pub fn run(surface: &str, out: &Path, script: &str) {
    for id in [
        "empty-queue",
        "project-paging",
        "global-paging",
        "stale-page-disclosure",
        "selected-id-change",
        "scope-change",
        "project-change",
        "profile-epoch-change",
        "profile-generation-change",
        "target-change",
        "source-change",
        "status-change",
        "source-field-change",
        "revoke-inspect-apply",
        "delayed-read-navigation",
        "unknown-no-auto-replay",
        "manual-exact-replay",
        "replay-changed-bytes",
        "replay-stale-epoch",
        "en-light-dark-compact-large",
        "zh-light-dark-compact-large",
    ] {
        let mechanism = match id {
            "profile-generation-change"
            | "delayed-read-navigation"
            | "unknown-no-auto-replay"
            | "manual-exact-replay"
            | "replay-changed-bytes" => {
                "actual-os-input + contained-process-ipc-fault-injection + independent-cli (not-native-typing-of-protocol-fields)"
            }
            "profile-epoch-change"
            | "status-change"
            | "revoke-inspect-apply"
            | "replay-stale-epoch"
            | "stale-page-disclosure" => {
                "actual-os-input + independent-real-process-cli-interference"
            }
            _ => "actual-os-input + real-owned-daemon + independent-cli-observation",
        };
        evidence::begin(id, mechanism);
        evidence::clear_owner();
        let result = std::panic::catch_unwind(|| {
            let mut s = Session::new(surface, &out.join(format!("s3-{id}")), script);
            match id {
                "empty-queue" => empty(&mut s),
                "project-paging" | "stale-page-disclosure" => paging(&mut s, false),
                "global-paging" => paging(&mut s, true),
                "selected-id-change"
                | "scope-change"
                | "project-change"
                | "target-change"
                | "source-change"
                | "source-field-change" => invalidation(&mut s, id),
                "profile-epoch-change" | "profile-generation-change" | "revoke-inspect-apply" => {
                    changed_authority(&mut s, id)
                }
                "status-change" => status_change(&mut s),
                "delayed-read-navigation" => delayed_read(&mut s),
                "unknown-no-auto-replay"
                | "manual-exact-replay"
                | "replay-changed-bytes"
                | "replay-stale-epoch" => unknown(&mut s, id),
                "en-light-dark-compact-large" => presentation(&mut s, false),
                "zh-light-dark-compact-large" => presentation(&mut s, true),
                _ => unreachable!(),
            }
            s.close();
        });
        evidence::finish(match result {
            Ok(()) => "PASS",
            Err(reason) if reason.is::<MissingFocus>() => "BLOCKED",
            Err(_) => "FAIL",
        });
    }
}
