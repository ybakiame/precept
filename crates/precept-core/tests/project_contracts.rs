use precept_core::{Preference, Scope, Status, contracts::*};
use serde_json::{Value, json};
use std::fs;

fn rule(id: &str, key: &str, values: &[&str]) -> Value {
    json!({"id":id,"revision":1,"review":"accepted","key":key,"allowed_values":values})
}
fn markdown(rules: Vec<Value>) -> String {
    format!(
        "Synthetic repository claims.\n```acp-contracts-v1\n{}\n```\n",
        json!({"schema_version":1,"rules":rules})
    )
}
fn pref(id: u64, key: &str, value: &str, project: Option<&str>) -> Preference {
    Preference {
        id,
        key: key.into(),
        value: value.into(),
        scope: if project.is_some() {
            Scope::Project
        } else {
            Scope::Global
        },
        project: project.map(str::to_owned),
        status: Status::Confirmed,
        tags: vec!["style".into()],
    }
}
fn resolve(rules: Vec<Value>, prefs: Vec<Preference>, query: SelectorQuery) -> Projection {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join(SOURCE), markdown(rules)).unwrap();
    project("alpha", prefs, &Snapshot::read(temp.path()), &query).unwrap()
}

#[test]
fn strict_format_caps_fields_versions_and_nested_duplicate_keys() {
    let good = markdown(vec![rule("fmt", "formatter", &["rustfmt"])]);
    assert!(parse(good.as_bytes()).is_ok());
    assert!(parse(good.replace('\n', "\r\n").as_bytes()).is_ok());
    for bad in [
        good.replace("\"schema_version\":1", "\"schema_version\":2"),
        good.replace(
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_version\":1",
        ),
        good.replace("\"id\":\"fmt\"", "\"id\":\"fmt\",\"id\":\"fmt\""),
        good.replace("\"review\":\"accepted\"", "\"review\":\"confirmed\""),
        good.replace("\"revision\":1", "\"revision\":0"),
        good.replace("\"revision\":1", "\"revision\":1,\"scope\":\"global\""),
        good.replace("v1", "v2"),
        good.replace("\n```\n", "\n{}\n```\n"),
        good.trim_end_matches("```\n").into(),
        format!("{good}{good}"),
        "Only prose".into(),
        "x".repeat(MAX_BYTES + 1),
        markdown(
            (0..65)
                .map(|i| rule(&format!("r{i}"), "k", &["v"]))
                .collect(),
        ),
    ] {
        assert!(parse(bad.as_bytes()).is_err(), "invalid source accepted");
    }
    assert!(parse(&[255]).is_err());
    for field in ["agents", "paths", "languages", "task_tags"] {
        let mut oversized = rule("r", "k", &["v"]);
        oversized["selector"] = json!({field:vec!["v";65]});
        assert!(parse(markdown(vec![oversized]).as_bytes()).is_err());
        for values in [json!([]), json!(["../bad"])] {
            if field != "paths" && values == json!(["../bad"]) {
                continue;
            }
            let mut r = rule("r", "k", &["v"]);
            r["selector"] = json!({field:values});
            assert!(parse(markdown(vec![r]).as_bytes()).is_err());
        }
    }
    for field in ["allowed_values", "supersedes"] {
        let mut r = rule("r", "k", &["v"]);
        r[field] = if field == "allowed_values" {
            json!(vec!["v"; 65])
        } else {
            json!(vec![json!({"id":"r","revision":1}); 65])
        };
        assert!(parse(markdown(vec![r]).as_bytes()).is_err());
    }
    let deep = format!(
        "```acp-contracts-v1\n{}0{}\n```\n",
        "[".repeat(17),
        "]".repeat(17)
    );
    assert!(parse(deep.as_bytes()).is_err());
}

#[test]
fn ordinary_markdown_fences_do_not_activate_examples() {
    let example = "````markdown\n```acp-contracts-v99\nnot JSON\n```\n````\n";
    assert!(parse(format!("{example}{}", markdown(vec![])).as_bytes()).is_ok());
    assert!(parse(example.as_bytes()).is_err());
    assert!(
        parse(format!("~~~markdown\n{}~~~\n{}", markdown(vec![]), markdown(vec![])).as_bytes())
            .is_ok()
    );
}

#[test]
fn authority_fallback_inactive_scope_and_private_projection() {
    let mut inactive = pref(4, "inactive", "INACTIVE", None);
    inactive.status = Status::Suggested;
    let mut rejected = pref(5, "rejected", "REJECTED", None);
    rejected.status = Status::Rejected;
    let mut deprecated = pref(6, "deprecated", "DEPRECATED", None);
    deprecated.status = Status::Deprecated;
    let p = resolve(
        vec![rule("fmt", "formatter", &["rustfmt"])],
        vec![
            pref(1, "formatter", "personal-global", None),
            pref(2, "formatter", "personal-project", Some("alpha")),
            pref(3, "fallback", "global", None),
            inactive,
            rejected,
            deprecated,
            pref(7, "foreign", "FOREIGN", Some("beta")),
        ],
        SelectorQuery::default(),
    );
    assert_eq!(p.preferences.len(), 1);
    assert_eq!(p.preferences[0].key, "fallback");
    assert_eq!(
        p.effective_contracts[0]["allowed_values"],
        json!(["rustfmt"])
    );
    assert_eq!(p.effective_contracts[0]["source"]["path"], SOURCE);
    let out =
        json!({"contracts":p.effective_contracts,"resolution":p.contract_resolution}).to_string();
    for private in [
        "PRIVATE_SOURCE_SENTINEL",
        "personal-1",
        "INACTIVE",
        "FOREIGN",
        "REJECTED",
        "DEPRECATED",
    ] {
        assert!(!out.contains(private));
    }
}

#[test]
fn compatible_intersection_and_equal_authority_conflict_do_not_choose_a_winner() {
    let p = resolve(
        vec![
            rule("a", "k", &["one", "two"]),
            rule("b", "k", &["two", "three"]),
        ],
        vec![pref(1, "k", "two", None)],
        SelectorQuery::default(),
    );
    assert_eq!(p.preferences.len(), 1);
    assert_eq!(p.effective_contracts[0]["allowed_values"], json!(["two"]));
    let p = resolve(
        vec![rule("a", "k", &["one"]), rule("b", "k", &["two"])],
        vec![pref(1, "k", "one", None), pref(2, "other", "keep", None)],
        SelectorQuery::default(),
    );
    assert_eq!(p.preferences.len(), 1);
    assert_eq!(p.preferences[0].key, "other");
    assert!(p.effective_contracts.is_empty());
    assert_eq!(p.contract_resolution["status"], "partial");
    assert_eq!(p.contract_resolution["unresolved_keys"], json!(["k"]));
}

#[test]
fn selectors_and_profile_representability_only_block_genuinely_affected_keys() {
    for value in ["valid\nmultiline".to_owned(), "合".repeat(4096)] {
        for (review, path, expected) in [
            ("unreviewed", None, true),
            ("accepted", Some("other.rs"), true),
            ("accepted", None, false),
            ("accepted", Some("src/lib.rs"), false),
        ] {
            let mut r = rule("r", "k", &["v"]);
            r["review"] = json!(review);
            r["selector"] = json!({"paths":["src/lib.rs"]});
            let query = SelectorQuery {
                path: path.map(str::to_owned),
                ..Default::default()
            };
            let p = resolve(vec![r], vec![pref(1, "k", &value, None)], query);
            assert_eq!(
                !p.preferences.is_empty(),
                expected,
                "pending/nonmatch must preserve legacy text"
            );
            if expected {
                assert_eq!(p.preferences[0].value, value);
            } else {
                assert_eq!(p.contract_resolution["status"], "partial");
            }
        }
    }
    let mut r = rule("r", "k", &["repo"]);
    r["selector"] = json!({"agents":["coder"],"languages":["rust"],"task_tags":["review"]});
    for (query, masked) in [
        (SelectorQuery::default(), true),
        (
            SelectorQuery {
                agent_id: Some("coder".into()),
                language: Some("rust".into()),
                task_tags: Some(vec!["review".into()]),
                ..Default::default()
            },
            true,
        ),
        (
            SelectorQuery {
                task_tags: Some(vec![]),
                ..Default::default()
            },
            false,
        ),
    ] {
        assert_eq!(
            resolve(vec![r.clone()], vec![pref(1, "k", "personal", None)], query)
                .preferences
                .is_empty(),
            masked
        );
    }
    let p = resolve(
        vec![],
        vec![pref(1, "control\nkey", "keep", None)],
        SelectorQuery::default(),
    );
    assert_eq!(p.preferences[0].key, "control\nkey");
}

#[test]
fn supersession_normalization_and_cross_domain_identity_are_deterministic() {
    let old = rule("personal-1", "k", &["old"]);
    let mut new = rule("personal-1", "k", &["new"]);
    new["revision"] = json!(2);
    new["supersedes"] = json!([{"id":"personal-1","revision":1}]);
    for rules in [vec![old.clone(), new.clone()], vec![new, old]] {
        let p = resolve(
            rules,
            vec![pref(1, "k", "personal", None)],
            SelectorQuery::default(),
        );
        assert!(p.preferences.is_empty());
        assert_eq!(p.effective_contracts[0]["allowed_values"], json!(["new"]));
    }
}

#[test]
fn missing_invalid_and_changed_snapshots_are_distinct_without_last_good_cache() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let path = root.join(SOURCE);
    let missing = Snapshot::read(root);
    assert_eq!(missing.status(), "not_configured");
    assert!(missing.unchanged(root));
    fs::write(&path, markdown(vec![])).unwrap();
    assert!(!missing.unchanged(root));
    let empty = Snapshot::read(root);
    assert_eq!(empty.status(), "ready");
    fs::write(&path, "malformed PRIVATE_PARSER_DIAGNOSTIC").unwrap();
    assert!(!empty.unchanged(root));
    let invalid = Snapshot::read(root);
    assert_eq!(invalid.status(), "invalid_source");
    let p = project(
        "alpha",
        vec![pref(1, "unrelated", "keep", None)],
        &invalid,
        &SelectorQuery::default(),
    )
    .unwrap();
    assert!(p.preferences.is_empty());
    assert!(p.effective_contracts.is_empty());
    assert!(
        !p.contract_resolution
            .to_string()
            .contains("PRIVATE_PARSER_DIAGNOSTIC")
    );
    fs::remove_file(&path).unwrap();
    assert_eq!(Snapshot::read(root).status(), "not_configured");
    fs::create_dir(&path).unwrap();
    assert_eq!(Snapshot::read(root).status(), "invalid_source");
    fs::remove_dir(&path).unwrap();
    fs::write(&path, markdown(vec![])).unwrap();
    let before = Snapshot::read(root);
    fs::write(&path, format!("New prose\n{}", markdown(vec![]))).unwrap();
    assert!(!before.unchanged(root));
}

#[cfg(unix)]
#[test]
fn symlink_dangling_and_unreadable_sources_are_not_absence() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join(SOURCE);
    symlink(temp.path().join("missing"), &path).unwrap();
    assert_eq!(Snapshot::read(temp.path()).status(), "invalid_source");
    fs::remove_file(&path).unwrap();
    fs::write(&path, markdown(vec![])).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o0)).unwrap();
    let status = Snapshot::read(temp.path()).status();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(status, "source_unavailable");
}

#[test]
fn malformed_queries_never_degrade_to_unrestricted_even_without_contracts() {
    for query in [
        json!({"root":"/foreign"}),
        json!({"path":"../bad"}),
        json!({"path":"C:\\bad"}),
        json!({"language":""}),
        json!({"task_tags":[""]}),
    ] {
        let result = serde_json::from_value::<SelectorQuery>(query);
        assert!(result.is_err() || result.unwrap().validate().is_err());
    }
}

#[cfg(windows)]
#[test]
fn native_windows_reparse_source_is_refused_without_following_it() {
    use std::os::windows::fs::MetadataExt;
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("owned-target");
    fs::create_dir(&target).unwrap();
    let root = temp.path().join("root");
    fs::create_dir(&root).unwrap();
    let link = root.join(SOURCE);
    let status = std::process::Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(&link)
        .arg(&target)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "owned synthetic junction creation");
    assert_ne!(
        fs::symlink_metadata(&link).unwrap().file_attributes() & 0x400,
        0
    );
    assert_eq!(Snapshot::read(&root).status(), "invalid_source");
    fs::remove_dir(&link).unwrap();
    assert!(target.is_dir());
}
