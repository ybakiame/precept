//! Profile export → import between two real daemons through the real CLI:
//! confirmed and pending preferences arrive as suggestions in the new Profile,
//! others are skipped, a repeat replays, and the file is private.
#[allow(dead_code)] // Shared harness; this suite uses part of it.
mod support;
use precept_core::operator::{Channel, Origin};
use precept_write::{Binding, Command, Expected, Scope, Snapshot};
use serde_json::{Value, json};
use support::*;

fn cli(f: &Fixture, args: &[&str]) -> (bool, Value) {
    let output = std::process::Command::new(exe("precept"))
        .arg("--runtime-dir")
        .arg(&f.daemon.location.runtime_dir)
        .arg("--endpoint")
        .arg(&f.daemon.location.endpoint)
        .args(args)
        .output()
        .unwrap();
    assert!(output.stderr.is_empty(), "CLI errors stay on stdout");
    (
        output.status.success(),
        serde_json::from_slice(&output.stdout).expect("CLI JSON"),
    )
}

fn suggest(f: &Fixture, b: &Binding, op: &str, scope: Scope, key: &str) -> (i64, String) {
    let receipt = f
        .daemon
        .mutate(
            b,
            op,
            Command::SuggestPreference {
                // The confirmed record carries rule options; they must survive.
                rule: if key == "confirmed-key" {
                    precept_core::operator::RuleOptions {
                        applies_to: precept_core::operator::AppliesTo {
                            languages: vec!["rust".into()],
                            ..Default::default()
                        },
                        priority: 7,
                        enforcement: precept_core::operator::Enforcement::Verify,
                    }
                } else {
                    Default::default()
                },
                scope: scope.clone(),
                key: key.into(),
                value: format!("{key}-value\nsecond line"),
                tags: vec!["style".into(), "移动".into()],
                origin: Origin::Agent,
                expected: Expected::Absent,
            },
        )
        .unwrap();
    let id: i64 = receipt["affected"][0]["record_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let preview = f
        .daemon
        .control("profile.preferences.get", json!({"scope":scope,"id":id}))
        .unwrap();
    (
        id,
        preview["record"]["proposal_digest"]
            .as_str()
            .unwrap()
            .to_owned(),
    )
}

fn records(f: &Fixture, scope: Scope) -> Vec<Value> {
    f.daemon
        .control(
            "profile.preferences.list",
            json!({"scope":scope,"after_id":0}),
        )
        .unwrap()["records"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn export_then_import_moves_preferences_as_suggestions_and_replays() {
    let alpha = Scope::Project {
        name: "alpha".into(),
    };
    // Source: one confirmed personal, one pending project, one rejected.
    let src = Fixture::created();
    let b = src.daemon.enable();
    src.register(&b, "alpha");
    let (id, digest) = suggest(&src, &b, "s1", Scope::Global, "confirmed-key");
    src.daemon
        .mutate(
            &b,
            "s1-confirm",
            Command::ConfirmPreference {
                id,
                scope: Scope::Global,
                proposal_digest: digest,
                channel: Channel::Desktop,
                expected: Expected::Revision(1),
                note: None,
            },
        )
        .unwrap();
    suggest(&src, &b, "s2", alpha.clone(), "pending-key");
    let (id, digest) = suggest(&src, &b, "s3", Scope::Global, "rejected-key");
    src.daemon
        .mutate(
            &b,
            "s3-reject",
            Command::RejectPreference {
                id,
                scope: Scope::Global,
                proposal_digest: digest,
                channel: Channel::Desktop,
                expected: Expected::Revision(1),
                note: None,
            },
        )
        .unwrap();

    let file = src.temp.path().join("export.json");
    let (ok, out) = cli(
        &src,
        &["profile", "export", "--out", file.to_str().unwrap()],
    );
    assert!(ok, "{out}");
    assert_eq!(out["exported"], 3);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    // Never overwrites.
    let (ok, out) = cli(
        &src,
        &["profile", "export", "--out", file.to_str().unwrap()],
    );
    assert!(!ok);
    assert_eq!(out["error"], "Conflict");

    // Destination without alpha: the project record is skipped, writes needed.
    let bare = Fixture::created();
    let (ok, report) = cli(
        &bare,
        &["profile", "import", file.to_str().unwrap(), "--preflight"],
    );
    assert!(ok, "{report}");
    assert_eq!(report["destination"]["write_enabled"], false);
    assert_eq!(report["preferences"][0]["action"], "suggest");
    assert_eq!(report["preferences"][1]["skip_reason"], "status");
    assert_eq!(report["preferences"][2]["skip_reason"], "not_served");
    assert_eq!(report["omitted"]["handoffs"]["carried"], 0);
    assert!(records(&bare, Scope::Global).is_empty());
    let (ok, out) = cli(
        &bare,
        &[
            "profile",
            "import",
            file.to_str().unwrap(),
            "--operation-prefix",
            "move",
        ],
    );
    assert!(!ok, "import needs writes on: {out}");
    let bb = bare.daemon.enable();
    let (ok, out) = cli(
        &bare,
        &[
            "profile",
            "import",
            file.to_str().unwrap(),
            "--operation-prefix",
            "move",
        ],
    );
    assert!(ok, "{out}");
    assert_eq!(out["suggested"], 1);
    assert_eq!(out["skipped"].as_array().unwrap().len(), 2);
    drop(bb);

    // Destination with alpha: both arrive as suggestions; a repeat replays.
    let dst = Fixture::created();
    let db = dst.daemon.enable();
    dst.register(&db, "alpha");
    for round in 0..2 {
        let (ok, out) = cli(
            &dst,
            &[
                "profile",
                "import",
                file.to_str().unwrap(),
                "--operation-prefix",
                "move",
            ],
        );
        assert!(ok, "{out}");
        if round == 0 {
            assert_eq!(out["suggested"], 2);
            assert_eq!(out["skipped"], json!([{"file_index":1,"reason":"status"}]));
            assert_eq!(
                out["preflight"]["source"]["format"],
                "precept-profile-export"
            );
            assert_eq!(out["preflight"]["source"]["version"], 3);
            assert_eq!(out["preflight"]["destination"]["schema"], "Version1");
            assert_eq!(
                out["preflight"]["consistency"]["preflight_rechecked_before_apply"],
                true
            );
            assert_eq!(out["preflight"]["omitted"]["handoffs"]["carried"], 0);
            assert_eq!(
                out["preflight"]["omitted"]["handoffs"]["destination_count_complete"],
                true
            );
            assert_eq!(out["preflight"]["memory"]["environment"]["omitted"], true);
            assert_eq!(out["preflight"]["preferences"][1]["file_index"], 1);
        } else {
            assert_eq!(out["suggested"], 0, "a repeat adds nothing");
            assert_eq!(
                out["skipped"],
                json!([{"file_index":0,"reason":"exists"},{"file_index":1,"reason":"status"},{"file_index":2,"reason":"exists"}])
            );
        }
    }
    let global = records(&dst, Scope::Global);
    assert_eq!(global.len(), 1);
    assert_eq!(global[0]["key"], "confirmed-key");
    assert_eq!(global[0]["status"], "suggested");
    assert_eq!(global[0]["value"], "confirmed-key-value\nsecond line");
    assert_eq!(global[0]["tags"], json!(["style", "移动"]));
    assert_eq!(global[0]["origin"], "agent");
    let project = records(&dst, alpha);
    assert_eq!(project.len(), 1);
    assert_eq!(project[0]["key"], "pending-key");
    assert_eq!(project[0]["status"], "suggested");
    // Another prefix finds the keys taken: skipped, nothing added.
    let (ok, out) = cli(
        &dst,
        &[
            "profile",
            "import",
            file.to_str().unwrap(),
            "--operation-prefix",
            "again",
        ],
    );
    assert!(ok, "{out}");
    assert_eq!(out["suggested"], 0);
    assert_eq!(out["preflight"]["conflicts"].as_array().unwrap().len(), 3);
    assert_eq!(records(&dst, Scope::Global).len(), 1);
    assert_eq!(
        global[0]["rule"]["applies_to"]["languages"],
        json!(["rust"])
    );
    assert_eq!(global[0]["rule"]["priority"], 7);
    assert_eq!(global[0]["rule"]["enforcement"], "verify");
    // A version 1 export (before rule options) still imports, with defaults.
    let v1 = dst.temp.path().join("v1.json");
    std::fs::write(
        &v1,
        r#"{"format":"precept-profile-export","version":1,"preferences":[{"scope":{"kind":"global"},"key":"old-key","value":"old value","tags":["legacy"],"origin":"operator","status":"confirmed"}]}"#,
    )
    .unwrap();
    let (ok, out) = cli(
        &dst,
        &[
            "profile",
            "import",
            v1.to_str().unwrap(),
            "--operation-prefix",
            "v1",
        ],
    );
    assert!(ok, "{out}");
    assert_eq!(out["suggested"], 1);
    assert_eq!(out["preflight"]["source"]["version"], 1);
    // Malformed files, unsupported schemas and prefixes are refused.
    std::fs::write(dst.temp.path().join("bad.json"), "{}").unwrap();
    let bad = dst.temp.path().join("bad.json");
    let future = dst.temp.path().join("future.json");
    std::fs::write(
        &future,
        r#"{"format":"precept-profile-export","version":4,"preferences":[]}"#,
    )
    .unwrap();
    let (ok, out) = cli(
        &dst,
        &[
            "profile",
            "import",
            future.to_str().unwrap(),
            "--operation-prefix",
            "x",
        ],
    );
    assert!(!ok);
    assert_eq!(out["error"], "Version");
    assert!(
        !cli(
            &dst,
            &[
                "profile",
                "import",
                bad.to_str().unwrap(),
                "--operation-prefix",
                "x"
            ]
        )
        .0
    );
    assert!(
        !cli(
            &dst,
            &[
                "profile",
                "import",
                file.to_str().unwrap(),
                "--operation-prefix",
                "bad prefix"
            ]
        )
        .0
    );
}

#[test]
fn project_memory_travels_only_when_asked_and_environment_only_when_asked_too() {
    let src = Fixture::created();
    let b = src.daemon.enable();
    src.register(&b, "alpha");
    src.daemon
        .control("project.propose", json!({"name":"alpha","enabled":true}))
        .unwrap();
    let agent = src.daemon.credential("agent-alpha.cap");
    for (kind, title) in [
        ("environment", "Node"),
        ("lesson", "Run tests"),
        ("caution", "WSL"),
    ] {
        src.daemon
            .call(
                &agent,
                "memory.remember",
                json!({"kind":kind,"title":title,"body":format!("{title} body")}),
            )
            .unwrap();
    }
    let lesson = src
        .daemon
        .control("memories.list", json!({"project":"alpha"}))
        .unwrap()["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["title"] == "Run tests")
        .unwrap()["id"]
        .clone();
    src.daemon
        .control(
            "memories.edit",
            json!({"project":"alpha","id":lesson,"pinned":true}),
        )
        .unwrap();
    let export = |name: &str, flags: &[&str]| {
        let file = src.temp.path().join(name);
        let mut args = vec!["profile", "export", "--out", file.to_str().unwrap()];
        args.extend_from_slice(flags);
        let (ok, out) = cli(&src, &args);
        assert!(ok, "{out}");
        (file, out)
    };
    let (_, plain) = export("plain.json", &[]);
    assert_eq!(plain["memory"], 0);
    let (notes, out) = export("notes.json", &["--with-memory"]);
    assert_eq!(out["memory"], 2, "environment stays on this machine");
    let (all_file, all) = export("all.json", &["--with-memory", "--with-environment"]);
    assert_eq!(all["memory"], 3);

    let dst = Fixture::created();
    let db = dst.daemon.enable();
    dst.register(&db, "alpha");
    for round in 0..2 {
        let (ok, out) = cli(
            &dst,
            &[
                "profile",
                "import",
                notes.to_str().unwrap(),
                "--operation-prefix",
                "mem",
            ],
        );
        assert!(ok, "{out}");
        if round == 0 {
            assert_eq!(out["memory"]["alpha"], json!({"added":2}));
            assert_eq!(out["preflight"]["memory"]["environment"]["omitted"], true);
            assert_eq!(out["preflight"]["memory"]["notes"][0]["action"], "add");
        } else {
            assert_eq!(out["memory"]["alpha"], json!({"reaffirmed":2}));
            assert_eq!(out["preflight"]["memory"]["notes"][0]["action"], "reaffirm");
        }
    }
    let entries = dst
        .daemon
        .control("memories.list", json!({"project":"alpha"}))
        .unwrap()["entries"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(entries.len(), 2);
    let pinned = entries.iter().find(|e| e["title"] == "Run tests").unwrap();
    assert_eq!(pinned["pinned"], true);
    assert_eq!(pinned["by"], "import");
    assert!(entries.iter().all(|e| e["kind"] != "environment"));
    let (ok, out) = cli(
        &dst,
        &[
            "profile",
            "import",
            all_file.to_str().unwrap(),
            "--operation-prefix",
            "env",
        ],
    );
    assert!(ok, "{out}");
    assert_eq!(out["memory"]["alpha"], json!({"added":1,"reaffirmed":2}));
    assert_eq!(out["preflight"]["memory"]["environment"]["included"], true);
    assert_eq!(
        out["preflight"]["memory"]["environment"]["included_count"],
        1
    );
    assert!(
        out["preflight"]["memory"]["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|plan| plan["action"] == "add")
    );
    // Agents cannot import memory.
    let agent = dst.daemon.credential("agent-alpha.cap");
    assert_eq!(
        dst.daemon.call(
            &agent,
            "memories.import",
            json!({"project":"alpha","entries":[]})
        ),
        Err(precept_ipc::Error::Unauthorized)
    );
}

#[test]
fn preflight_reports_every_omission_and_conflict_without_confirming() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    let alpha = Scope::Project {
        name: "alpha".into(),
    };
    f.register(&b, "alpha");
    f.daemon
        .control("project.propose", json!({"name":"alpha","enabled":true}))
        .unwrap();
    // An exact duplicate in any destination status is a conflict.
    suggest(&f, &b, "existing", Scope::Global, "confirmed-key");
    f.daemon
        .mutate(
            &b,
            "existing-memory",
            Command::PutHandoff {
                project: "alpha".into(),
                task_id: "saved".into(),
                objective: "private-handoff-body".into(),
                completed: String::new(),
                next_action: "inspect".into(),
                validation_json: "[]".into(),
                snapshot: Snapshot {
                    base_sha: String::new(),
                    branch: String::new(),
                    state: "unknown".into(),
                    fingerprint: None,
                },
                expected: Expected::Absent,
            },
        )
        .unwrap();
    let agent = f.daemon.credential("agent-alpha.cap");
    f.daemon
        .call(
            &agent,
            "memory.remember",
            json!({"kind":"lesson","title":"Existing lesson","body":"old-private-body"}),
        )
        .unwrap();
    let db =
        rusqlite::Connection::open_with_flags(&f.db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let confirmations = || -> i64 {
        db.query_row(
            "SELECT COUNT(*) FROM acp_review_decisions WHERE action='confirm'",
            [],
            |row| row.get(0),
        )
        .unwrap()
    };
    assert_eq!(confirmations(), 0);

    let preference = |scope: Value, key: &str, status: &str| {
        json!({
            "scope":scope,
            "key":key,
            "value":format!("private-value-{key}"),
            "tags":["test"],
            "origin":"operator",
            "status":status,
            "enabled":true
        })
    };
    let mut duplicate = preference(json!({"kind":"global"}), "confirmed-key", "confirmed");
    duplicate["value"] = json!("confirmed-key-value\nsecond line");
    duplicate["tags"] = json!(["style", "移动"]);
    duplicate["origin"] = json!("agent");
    let mut bad_rule = preference(json!({"kind":"global"}), "bad-rule", "suggested");
    bad_rule["rule"] = json!({"priority":101});
    let file = f.temp.path().join("mixed.json");
    let mixed = json!({
        "format":"precept-profile-export",
        "version":3,
        "preferences":[
            {"scope":{"kind":"environment","name":"alpha"}},
            duplicate,
            preference(json!({"kind":"project","name":"alpha"}), "pending-key", "suggested"),
            preference(json!({"kind":"global"}), "rejected-key", "rejected"),
            preference(json!({"kind":"global"}), "deprecated-key", "deprecated"),
            preference(json!({"kind":"project","name":"gamma"}), "remote-key", "confirmed"),
            {"scope":{"kind":"global"},"key":"bad-tags","value":"private-value-bad-tags",
             "tags":["bad/tag"],"origin":"operator","status":"confirmed"},
            bad_rule,
            {"scope":{"kind":"global"},"key":"","value":"private-value-empty",
             "tags":["test"],"origin":"operator","status":"confirmed"},
            preference(json!({"kind":"global"}), "new-key", "confirmed"),
            preference(json!({"kind":"global"}), "new-key", "suggested")
        ],
        "memory":[
            {"project":"alpha","kind":"lesson","title":"Existing lesson",
             "body":"new-private-body","pinned":true},
            {"project":"alpha","kind":"environment","title":"Runtime",
             "body":"private-environment","pinned":false},
            {"project":"gamma","kind":"lesson","title":"Remote",
             "body":"private-remote","pinned":false}
        ]
    });
    std::fs::write(&file, serde_json::to_vec_pretty(&mixed).unwrap()).unwrap();
    let args = || {
        [
            "profile".to_owned(),
            "import".into(),
            file.to_str().unwrap().into(),
            "--operation-prefix".into(),
            "mixed".into(),
        ]
    };
    let args = args();
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let (ok, out) = cli(&f, &refs);
    assert!(ok, "{out}");
    assert_eq!(out["suggested"], 2);
    assert_eq!(out["skipped"].as_array().unwrap().len(), 9);
    assert_eq!(out["memory"]["alpha"], json!({"corrected":1,"added":1}));
    let report = &out["preflight"];
    assert_eq!(report["destination"]["schema"], "Version1");
    assert_eq!(report["destination"]["write_enabled"], true);
    assert!(report["consistency"]["watermark"].is_u64());
    assert_eq!(
        report["consistency"]["preflight_rechecked_before_apply"],
        true
    );
    assert_eq!(
        report["omitted"]["environment_scope"]["file_indexes"],
        json!([0])
    );
    assert_eq!(report["omitted"]["handoffs"]["destination_listed"], 1);
    assert_eq!(
        report["omitted"]["handoffs"]["destination_count_complete"],
        true
    );
    assert_eq!(report["unserved_projects"], json!(["gamma"]));
    assert_eq!(report["memory"]["environment"]["included_count"], 1);
    assert_eq!(report["memory"]["notes"][0]["action"], "correct");
    assert_eq!(report["memory"]["notes"][0]["pin_change"], true);
    assert_eq!(report["memory"]["notes"][2]["skip_reason"], "not_served");
    assert_eq!(
        report["conflicts"],
        json!([
            {"file_index":0,"reason":"environment"},
            {"file_index":1,"reason":"exists"},
            {"file_index":3,"reason":"status"},
            {"file_index":4,"reason":"status"},
            {"file_index":5,"reason":"not_served"},
            {"file_index":6,"reason":"tags"},
            {"file_index":7,"reason":"invalid_record"},
            {"file_index":8,"reason":"invalid_record"},
            {"file_index":10,"reason":"source_duplicate"}
        ])
    );
    let indexes: Vec<_> = report["preferences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|plan| plan["file_index"].as_u64().unwrap())
        .collect();
    assert_eq!(indexes, (0..=10).collect::<Vec<_>>());
    assert_eq!(report["preferences"][9]["action"], "suggest");
    for (index, reason) in [
        (3, "status"),
        (4, "status"),
        (5, "not_served"),
        (6, "tags"),
        (7, "invalid_record"),
        (8, "invalid_record"),
    ] {
        assert_eq!(report["preferences"][index]["skip_reason"], reason);
    }
    let redacted = serde_json::to_string(&out).unwrap();
    for private in [
        "private-value",
        "private-handoff",
        "private-environment",
        "old-private",
    ] {
        assert!(!redacted.contains(private), "report leaked {private}");
    }
    assert!(!redacted.contains(&f.temp.path().to_string_lossy().to_string()));
    assert_eq!(confirmations(), 0);
    assert_eq!(records(&f, Scope::Global).len(), 2);
    assert_eq!(records(&f, alpha).len(), 1);
    assert!(
        records(&f, Scope::Global)
            .iter()
            .all(|record| record["status"] == "suggested")
    );

    // The receipt for the stable original file index cannot be reused with
    // different content, even though preflight cannot read receipts itself.
    let before = logical(&f.db);
    assert_eq!(
        f.daemon.mutate(
            &b,
            "mixed-9",
            Command::SuggestPreference {
                rule: Default::default(),
                scope: Scope::Global,
                key: "collision".into(),
                value: "different-private-value".into(),
                tags: vec!["test".into()],
                origin: Origin::Operator,
                expected: Expected::Absent,
            },
        ),
        Err(precept_ipc::Error::IdempotencyConflict)
    );
    assert_eq!(logical(&f.db), before);
    let (ok, replay) = cli(&f, &refs);
    assert!(ok, "{replay}");
    assert_eq!(replay["suggested"], 0);
    assert_eq!(records(&f, Scope::Global).len(), 2);
    assert_eq!(confirmations(), 0);
    assert_eq!(
        f.daemon
            .control("profile.handoff.list", json!({"project":"alpha"}))
            .unwrap()["handoffs"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // Only an exact operator review of the imported revision confirms it.
    let queue = f
        .daemon
        .control(
            "profile.review.queue",
            json!({"scope":Scope::Global,"after_id":0}),
        )
        .unwrap();
    let imported = queue["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["key"] == "new-key")
        .unwrap()
        .clone();
    f.daemon
        .mutate(
            &b,
            "operator-review",
            Command::ConfirmPreference {
                id: imported["id"].as_i64().unwrap(),
                scope: Scope::Global,
                proposal_digest: imported["proposal_digest"].as_str().unwrap().into(),
                channel: Channel::Desktop,
                expected: Expected::Revision(imported["revision"].as_i64().unwrap()),
                note: None,
            },
        )
        .unwrap();
    assert_eq!(confirmations(), 1);
    assert_eq!(records(&f, Scope::Global).len(), 2);
    assert!(
        records(&f, Scope::Global)
            .iter()
            .any(|record| record["key"] == "new-key" && record["status"] == "confirmed")
    );
}
