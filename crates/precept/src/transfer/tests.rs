use super::*;
use precept_ipc::{
    Error,
    operator::{Binding, Origin, Scope},
};
use serde_json::json;

#[test]
fn files_are_new_private_versioned_and_bounded() {
    let dir = std::env::temp_dir().join(format!("precept-transfer-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("export.json");
    let _ = std::fs::remove_file(&path);
    let export = Export {
        format: FORMAT.into(),
        version: VERSION,
        preferences: vec![Exported {
            scope: Scope::Global,
            key: "k".into(),
            value: "v".into(),
            tags: vec!["style".into()],
            origin: Origin::Operator,
            status: "confirmed".into(),
            rule: Default::default(),
            enabled: true,
        }],
        memory: vec![ExportedNote {
            project: "alpha".into(),
            kind: "lesson".into(),
            title: "t".into(),
            body: "b".into(),
            pinned: true,
        }],
        dropped: vec![],
    };
    write_new(&path, &export).unwrap();
    assert_eq!(write_new(&path, &export), Err(Error::Conflict));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    assert!(read(&path).ok() == Some(export));
    std::fs::write(&path, r#"{"format":"other","version":1,"preferences":[]}"#).unwrap();
    assert!(matches!(read(&path), Err(Error::Version)));
    std::fs::write(
        &path,
        r#"{"format":"precept-profile-export","version":1,"preferences":[],"x":1}"#,
    )
    .unwrap();
    assert!(matches!(read(&path), Err(Error::InvalidRequest)));
    // Environment records of older files are skipped, not refused.
    std::fs::write(
        &path,
        r#"{"format":"precept-profile-export","version":2,"preferences":[
          {"scope":{"kind":"environment","name":"alpha"},"key":"shell","value":"WSL",
           "tags":["setup"],"origin":"agent","status":"confirmed"},
          {"scope":{"kind":"global"},"key":"k","value":"v","tags":["style"],
           "origin":"operator","status":"confirmed"}]}"#,
    )
    .unwrap();
    let read_back = read(&path).unwrap();
    assert_eq!(read_back.dropped, vec![0]);
    assert_eq!(read_back.preferences.len(), 1);
    assert_eq!(read_back.preferences[0].key, "k");
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(valid_prefix("move-2026"));
    assert!(!valid_prefix("bad prefix") && !valid_prefix("") && !valid_prefix(&"x".repeat(41)));
}

fn preference(status: &str, key: &str, scope: Scope) -> Exported {
    Exported {
        scope,
        key: key.into(),
        value: format!("private-value-{key}"),
        tags: vec!["test".into()],
        origin: Origin::Operator,
        status: status.into(),
        rule: Default::default(),
        enabled: true,
    }
}

#[test]
fn preference_plans_keep_original_indexes_and_name_every_refusal() {
    let mut duplicate = preference("confirmed", "duplicate", Scope::Global);
    duplicate.value = "private-value-duplicate".into();
    let mut bad_tags = preference("confirmed", "bad-tags", Scope::Global);
    bad_tags.tags = vec!["bad/tag".into()];
    let mut bad_rule = preference("suggested", "bad-rule", Scope::Global);
    bad_rule.rule.priority = 101;
    let export = Export {
        format: FORMAT.into(),
        version: VERSION,
        preferences: vec![
            duplicate.clone(),
            preference("suggested", "new", Scope::Global),
            preference("confirmed", "new", Scope::Global),
            preference("rejected", "rejected", Scope::Global),
            preference("deprecated", "deprecated", Scope::Global),
            preference(
                "confirmed",
                "remote",
                Scope::Project {
                    name: "gamma".into(),
                },
            ),
            bad_tags,
            bad_rule,
        ],
        memory: vec![],
        dropped: vec![0, 3],
    };
    let projects = vec!["alpha".to_owned()];
    let existing = vec![preference_identity(&duplicate).unwrap()];
    let plans = preference_plan(&export, &projects, &existing);
    assert_eq!(file_indexes(&export), vec![1, 2, 4, 5, 6, 7, 8, 9]);
    let summarized: Vec<_> = plans
        .iter()
        .map(|plan| {
            (
                plan["file_index"].as_u64().unwrap(),
                plan["action"].as_str().unwrap(),
                plan["conflict"].as_str(),
                plan["skip_reason"].as_str(),
            )
        })
        .collect();
    assert_eq!(
        summarized,
        vec![
            (0, "skip", None, Some("environment")),
            (1, "skip", Some("exists"), Some("exists")),
            (2, "suggest", None, None),
            (3, "skip", None, Some("environment")),
            (
                4,
                "skip",
                Some("source_duplicate"),
                Some("source_duplicate")
            ),
            (5, "skip", None, Some("status")),
            (6, "skip", None, Some("status")),
            (7, "skip", None, Some("not_served")),
            (8, "skip", None, Some("tags")),
            (9, "skip", None, Some("invalid_record")),
        ]
    );
    let redacted = serde_json::to_string(&plans).unwrap();
    assert!(!redacted.contains("private-value"));
    assert!(!redacted.contains("bad-rule"));
    assert!(!redacted.contains("remote"));
}

#[test]
fn memory_plans_are_selective_sequential_and_redacted() {
    let export = Export {
        format: FORMAT.into(),
        version: VERSION,
        preferences: vec![],
        memory: vec![
            ExportedNote {
                project: "alpha".into(),
                kind: "lesson".into(),
                title: " Private title ".into(),
                body: "private-body".into(),
                pinned: true,
            },
            ExportedNote {
                project: "alpha".into(),
                kind: "lesson".into(),
                title: "private title".into(),
                body: "private-body".into(),
                pinned: false,
            },
            ExportedNote {
                project: "alpha".into(),
                kind: "environment".into(),
                title: "runtime".into(),
                body: "private-environment".into(),
                pinned: false,
            },
            ExportedNote {
                project: "gamma".into(),
                kind: "lesson".into(),
                title: "remote".into(),
                body: "private-remote".into(),
                pinned: false,
            },
        ],
        dropped: vec![],
    };
    let observation = Observation {
        binding: Binding {
            profile_id: "0".repeat(32),
            generation: "1".repeat(32),
            owner_epoch: 1,
        },
        write_enabled: false,
        projects: vec!["alpha".into()],
        existing: vec![],
        commit_sequence: 0,
        schema: "Version1",
        notes: [(
            "alpha".into(),
            vec![ExistingNote {
                kind: "lesson".into(),
                title: "Private title".into(),
                body: "old-private-body".into(),
                pinned: false,
            }],
        )]
        .into(),
        handoffs: json!({}),
    };
    let plan = memory_plan(&export, &observation);
    assert_eq!(plan["notes"][0]["action"], "correct");
    assert_eq!(plan["notes"][0]["pin_change"], true);
    assert_eq!(plan["notes"][1]["action"], "reaffirm");
    assert_eq!(plan["notes"][2]["action"], "add");
    assert_eq!(plan["notes"][3]["skip_reason"], "not_served");
    assert_eq!(plan["environment"]["included_count"], 1);
    assert_eq!(plan["projects"]["alpha"]["planned"]["correct"], 1);
    assert_eq!(
        plan["projects"]["alpha"]["planned"]["reaffirmed"],
        json!(null)
    );
    let redacted = serde_json::to_string(&plan).unwrap();
    assert!(!redacted.contains("private-"));
    assert!(!redacted.contains("Private title"));
    let without_environment = Export {
        memory: vec![export.memory[0].clone()],
        ..export
    };
    let plan = memory_plan(
        &without_environment,
        &Observation {
            notes: Default::default(),
            ..observation
        },
    );
    assert_eq!(plan["environment"]["omitted"], true);
    assert_eq!(plan["notes"][0]["action"], "add");
}

#[test]
fn preflight_validation_refuses_other_schemas_without_fallback() {
    let export = Export {
        format: FORMAT.into(),
        version: VERSION + 1,
        preferences: vec![],
        memory: vec![],
        dropped: vec![],
    };
    assert_eq!(validate_export(&export), Err(Error::Version));
    assert_eq!(
        validate_export(&Export {
            format: "other".into(),
            ..export
        }),
        Err(Error::Version)
    );
}
