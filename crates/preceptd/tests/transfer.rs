//! Profile export → import between two real daemons through the real CLI:
//! confirmed and pending preferences arrive as suggestions in the new Profile,
//! others are skipped, a repeat replays, and the file is private.
#[allow(dead_code)] // Shared harness; this suite uses part of it.
mod support;
use precept_core::operator::{Channel, Origin};
use precept_write::{Binding, Command, Expected, Scope};
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
            assert_eq!(out["skipped"], json!([{"index":1,"reason":"status"}]));
        } else {
            assert_eq!(out["suggested"], 0, "a repeat adds nothing");
            assert_eq!(
                out["skipped"],
                json!([{"index":0,"reason":"exists"},{"index":1,"reason":"status"},{"index":2,"reason":"exists"}])
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
    // Malformed files and prefixes are refused.
    std::fs::write(dst.temp.path().join("bad.json"), "{}").unwrap();
    let bad = dst.temp.path().join("bad.json");
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
