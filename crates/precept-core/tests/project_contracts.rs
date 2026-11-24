use precept_core::{Preference, Scope, Status, contracts::*};
use serde_json::json;
use std::{collections::BTreeSet, fs, path::Path};

fn file(key: &str, allowed: &[&str], extra: &str) -> String {
    let allowed = if allowed.is_empty() {
        String::new()
    } else {
        format!("allowed = {}\n", json!(allowed))
    };
    format!("+++\nkey = \"{key}\"\n{allowed}{extra}+++\nRule text for {key}.\n")
}
fn write(root: &Path, id: &str, raw: &str) {
    let dir = root.join(DIR);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{id}.md")), raw).unwrap();
}
fn pref(id: u64, key: &str, value: &str, project: Option<&str>) -> Preference {
    Preference {
        rule: Default::default(),
        enabled: true,
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
fn all(ids: &[&str]) -> Option<BTreeSet<String>> {
    Some(ids.iter().map(|s| s.to_string()).collect())
}
/// Resolve `files` (`(id, raw)`), all committed unless listed in `pending`.
fn resolve(
    files: &[(&str, String)],
    pending: &[&str],
    prefs: Vec<Preference>,
    query: SelectorQuery,
) -> Projection {
    let temp = tempfile::tempdir().unwrap();
    for (id, raw) in files {
        write(temp.path(), id, raw);
    }
    let reviewed = files
        .iter()
        .map(|(id, _)| id.to_string())
        .filter(|id| !pending.contains(&id.as_str()))
        .collect();
    let snapshot = Snapshot::read(temp.path()).with_reviewed(Some(reviewed));
    project("alpha", prefs, &snapshot, &query).unwrap()
}

#[test]
fn a_committed_rule_replaces_preferences_of_its_key_and_is_served_with_its_text() {
    let mut inactive = pref(4, "inactive", "INACTIVE", None);
    inactive.status = Status::Suggested;
    let p = resolve(
        &[("formatter", file("formatter", &[], "tags = [\"rust\"]\n"))],
        &[],
        vec![
            pref(1, "formatter", "personal-global", None),
            pref(2, "formatter", "personal-project", Some("alpha")),
            pref(3, "fallback", "global", None),
            inactive,
            pref(7, "foreign", "FOREIGN", Some("beta")),
        ],
        SelectorQuery::default(),
    );
    assert_eq!(p.preferences.len(), 1);
    assert_eq!(p.preferences[0].key, "fallback");
    let r = &p.repository_rules[0];
    assert_eq!(r["key"], "formatter");
    assert_eq!(r["file"], ".precept/rules/formatter.md");
    assert_eq!(r["text"], "Rule text for formatter.");
    assert_eq!(r["review"], "accepted");
    assert_eq!(r["checks"]["status"], "unchecked");
    let out = json!({"rules":p.repository_rules,"resolution":p.contract_resolution}).to_string();
    for private in ["personal-", "INACTIVE", "FOREIGN"] {
        assert!(!out.contains(private));
    }
}

#[test]
fn an_uncommitted_rule_is_pending_and_masks_nothing() {
    let p = resolve(
        &[("formatter", file("formatter", &[], ""))],
        &["formatter"],
        vec![pref(1, "formatter", "personal", None)],
        SelectorQuery::default(),
    );
    assert_eq!(p.preferences.len(), 1);
    assert!(p.repository_rules.is_empty());
    assert_eq!(p.pending_rules[0]["key"], "formatter");
    assert!(!json!(p.pending_rules).to_string().contains("Rule text"));
    // Git could not tell: every rule is pending.
    let temp = tempfile::tempdir().unwrap();
    write(temp.path(), "f", &file("f", &[], ""));
    let p = project(
        "alpha",
        vec![],
        &Snapshot::read(temp.path()),
        &SelectorQuery::default(),
    )
    .unwrap();
    assert_eq!(p.pending_rules.len(), 1);
}

#[test]
fn allowed_values_keep_allowed_preferences_and_conflicts_choose_no_winner() {
    let p = resolve(
        &[
            ("a", file("k", &["one", "two"], "")),
            ("b", file("k", &["two", "three"], "")),
        ],
        &[],
        vec![pref(1, "k", "two", None)],
        SelectorQuery::default(),
    );
    assert_eq!(p.preferences.len(), 1);
    assert_eq!(p.repository_rules.len(), 2);
    let p = resolve(
        &[
            ("a", file("k", &["one"], "")),
            ("b", file("k", &["two"], "")),
        ],
        &[],
        vec![pref(1, "k", "one", None), pref(2, "other", "keep", None)],
        SelectorQuery::default(),
    );
    assert_eq!(p.preferences.len(), 1);
    assert_eq!(p.preferences[0].key, "other");
    assert_eq!(p.contract_resolution["status"], "partial");
    assert_eq!(p.contract_resolution["unresolved_keys"], json!(["k"]));
    // A value the typed grammar cannot carry is withheld, never shown unchecked.
    let p = resolve(
        &[("a", file("k", &["v"], ""))],
        &[],
        vec![pref(1, "k", "multi\nline", None)],
        SelectorQuery::default(),
    );
    assert!(p.preferences.is_empty());
}

#[test]
fn applies_to_narrows_rules_like_preferences() {
    let raw = file(
        "k",
        &[],
        "[applies_to]\npaths = [\"src/\"]\nlanguages = [\"rust\"]\n",
    );
    for (query, applies) in [
        (SelectorQuery::default(), true),
        (
            SelectorQuery {
                path: Some("src/lib.rs".into()),
                language: Some("rust".into()),
                ..Default::default()
            },
            true,
        ),
        (
            SelectorQuery {
                path: Some("docs/a.md".into()),
                ..Default::default()
            },
            false,
        ),
        (
            SelectorQuery {
                language: Some("python".into()),
                ..Default::default()
            },
            false,
        ),
    ] {
        let p = resolve(
            &[("k", raw.clone())],
            &[],
            vec![pref(1, "k", "personal", None)],
            query,
        );
        assert_eq!(p.repository_rules.len(), usize::from(applies));
        assert_eq!(p.preferences.is_empty(), applies);
    }
}

#[test]
fn checks_run_against_the_repository() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join("README.md"), "Run cargo fmt.").unwrap();
    write(
        root,
        "fmt",
        &file(
            "fmt",
            &[],
            "[[check]]\nfile = \"README.md\"\ncontains = \"cargo fmt\"\n[[check]]\nfile = \"rustfmt.toml\"\n",
        ),
    );
    let snapshot = Snapshot::read(root).with_reviewed(all(&["fmt"]));
    let p = project("alpha", vec![], &snapshot, &SelectorQuery::default()).unwrap();
    let c = &p.repository_rules[0]["checks"];
    assert_eq!(c["status"], "failing");
    assert_eq!(c["passed"], 1);
    assert_eq!(c["failure"]["file"], "rustfmt.toml");
    fs::write(root.join("rustfmt.toml"), "").unwrap();
    let snapshot = Snapshot::read(root).with_reviewed(all(&["fmt"]));
    let p = project("alpha", vec![], &snapshot, &SelectorQuery::default()).unwrap();
    assert_eq!(p.repository_rules[0]["checks"]["status"], "verified");
}

#[test]
fn one_check_cache_is_shared_by_all_applying_rules() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join("README.md"), "Run cargo fmt.").unwrap();
    let extra = "[[check]]\nfile = \"README.md\"\ncontains = \"cargo fmt\"\n";
    write(root, "a", &file("a", &[], extra));
    write(root, "b", &file("b", &[], extra));
    let snapshot = Snapshot::read(root).with_reviewed(all(&["a", "b"]));
    let p = project("alpha", vec![], &snapshot, &SelectorQuery::default()).unwrap();
    assert_eq!(p.repository_rules.len(), 2);
    for rule in &p.repository_rules {
        assert_eq!(rule["checks"]["status"], "verified");
    }
    assert_eq!(p.contract_resolution["check_io"]["reads"], 1);
    assert_eq!(p.contract_resolution["check_io"]["cached_files"], 1);
    assert_eq!(p.contract_resolution["check_io"]["exhausted"], false);
}

#[test]
fn aggregate_check_budget_is_reported_across_rules() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let mut ids = Vec::new();
    for rule_index in 0..5 {
        let id = format!("budget-{rule_index}");
        let mut extra = String::new();
        for check_index in 0..16 {
            let file = format!("data-{rule_index}-{check_index}.txt");
            fs::write(root.join(&file), "x").unwrap();
            extra.push_str(&format!("[[check]]\nfile = \"{file}\"\n"));
        }
        write(root, &id, &file(&id, &[], &extra));
        ids.push(id);
    }
    let refs: Vec<_> = ids.iter().map(String::as_str).collect();
    let snapshot = Snapshot::read(root).with_reviewed(all(&refs));
    let p = project("alpha", vec![], &snapshot, &SelectorQuery::default()).unwrap();
    assert_eq!(p.contract_resolution["check_io"]["reads"], MAX_CHECK_READS);
    assert_eq!(p.contract_resolution["check_io"]["exhausted"], true);
    assert_eq!(
        p.repository_rules
            .iter()
            .filter(|rule| rule["checks"]["status"] == "verified")
            .count(),
        4
    );
    let unavailable: Vec<_> = p
        .repository_rules
        .iter()
        .filter(|rule| rule["checks"]["status"] == "unavailable")
        .collect();
    assert_eq!(unavailable.len(), 1);
    assert_eq!(unavailable[0]["checks"]["passed"], 0);
    assert_eq!(unavailable[0]["checks"]["unavailable"], 16);
    assert_eq!(unavailable[0]["checks"]["budget_exhausted"], true);
}

#[test]
fn missing_invalid_and_changed_sources_are_distinct() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let missing = Snapshot::read(root);
    assert_eq!(missing.status(), "not_configured");
    assert!(missing.unchanged(root));
    fs::create_dir_all(root.join(DIR)).unwrap();
    let empty = Snapshot::read(root);
    assert_eq!(empty.status(), "ready");
    write(root, "bad", "malformed PRIVATE_PARSER_DIAGNOSTIC");
    write(root, "good", &file("good", &[], ""));
    assert!(!empty.unchanged(root));
    let invalid = Snapshot::read(root).with_reviewed(all(&["bad", "good"]));
    assert_eq!(invalid.status(), "invalid_source");
    let p = project(
        "alpha",
        vec![pref(1, "unrelated", "keep", None)],
        &invalid,
        &SelectorQuery::default(),
    )
    .unwrap();
    assert!(p.preferences.is_empty());
    assert!(p.repository_rules.is_empty());
    assert_eq!(
        p.contract_resolution["invalid_files"],
        json!([".precept/rules/bad.md"])
    );
    assert!(
        !p.contract_resolution
            .to_string()
            .contains("PRIVATE_PARSER_DIAGNOSTIC")
    );
    // Other files in the directory are ignored.
    fs::remove_file(root.join(DIR).join("bad.md")).unwrap();
    fs::write(root.join(DIR).join("notes.txt"), "x").unwrap();
    let before = Snapshot::read(root);
    assert_eq!(before.status(), "ready");
    write(root, "good", &file("good", &[], "priority = 1\n"));
    assert!(!before.unchanged(root));
    for i in 0..MAX_FILES {
        write(root, &format!("r{i}"), &file("k", &[], ""));
    }
    assert_eq!(Snapshot::read(root).status(), "invalid_source");
}

#[cfg(unix)]
#[test]
fn links_and_unreadable_sources_are_not_absence() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let temp = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join(".precept")).unwrap();
    symlink(elsewhere.path(), temp.path().join(DIR)).unwrap();
    assert_eq!(Snapshot::read(temp.path()).status(), "invalid_source");
    fs::remove_file(temp.path().join(DIR)).unwrap();
    write(temp.path(), "a", &file("a", &[], ""));
    symlink(
        temp.path().join(DIR).join("a.md"),
        temp.path().join(DIR).join("b.md"),
    )
    .unwrap();
    assert_eq!(Snapshot::read(temp.path()).status(), "invalid_source");
    fs::remove_file(temp.path().join(DIR).join("b.md")).unwrap();
    let path = temp.path().join(DIR).join("a.md");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o0)).unwrap();
    let status = Snapshot::read(temp.path()).status();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(status, "source_unavailable");
}

#[test]
fn malformed_queries_never_degrade_to_unrestricted() {
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
fn native_windows_reparse_rule_directory_is_refused_without_following_it() {
    use std::os::windows::fs::MetadataExt;
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("owned-target");
    fs::create_dir(&target).unwrap();
    let root = temp.path().join("root");
    fs::create_dir_all(root.join(".precept")).unwrap();
    // mklink refuses mixed slashes, so build the path segment by segment.
    let link = root.join(".precept").join("rules");
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
