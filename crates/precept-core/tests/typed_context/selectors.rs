use super::support::*;
use precept_core::typed_context::*;

pub(super) fn selectors_are_and_across_or_within_exact_dimensions() {
    let mut base = input();
    base.query.agent_id = Some("codex".into());
    base.query.path = Some("src/lib.rs".into());
    base.query.language = Some("rust".into());
    base.query.task_tags = Some(strings(&["review", "security"]));
    let selector = Selector {
        projects: Some(vec![binding("beta"), binding("alpha")]),
        agents: Some(strings(&["kimi", "codex"])),
        paths: Some(strings(&["src/main.rs", "src/lib.rs"])),
        languages: Some(strings(&["python", "rust"])),
        task_tags: Some(strings(&["security", "test"])),
    };
    let mut r = contract("rule", "formatter", &["rustfmt"]);
    r.selector = selector;
    base.rules.push(r);
    let out = resolve(&base).unwrap();
    assert_eq!(row(&out, "rule").applicability, Applicability::Match);
    assert!(effective(&out, "formatter").is_some());
    for dimension in ["project", "agent", "path", "language", "tag"] {
        let mut i = base.clone();
        match dimension {
            "project" => i.rules[0].selector.projects = Some(vec![binding("beta")]),
            "agent" => i.query.agent_id = Some("another".into()),
            "path" => i.query.path = Some("src/lib.rs/child".into()),
            "language" => i.query.language = Some("Rust".into()),
            _ => i.query.task_tags = Some(vec![]),
        }
        let out = resolve(&i).unwrap();
        assert_eq!(
            row(&out, "rule").applicability,
            Applicability::NoMatch,
            "{dimension}"
        );
        assert!(out.effective.is_empty());
        assert!(out.conflicts.is_empty());
    }
    let mut case = base.clone();
    case.query.path = Some("src/Lib.rs".into());
    assert_eq!(
        resolve(&case).unwrap().explanations[0].applicability,
        Applicability::NoMatch
    );
}

pub(super) fn absent_missing_known_empty_and_false_unknown_have_distinct_meanings() {
    let mut i = input();
    i.rules.push(contract("r", "k", &["v"]));
    assert_eq!(
        resolve(&i).unwrap().explanations[0].applicability,
        Applicability::Match
    );
    for dimension in ["agent", "path", "language", "tags", "session"] {
        let mut i = i.clone();
        match dimension {
            "agent" => i.rules[0].selector.agents = Some(strings(&["codex"])),
            "path" => i.rules[0].selector.paths = Some(strings(&["src/lib.rs"])),
            "language" => i.rules[0].selector.languages = Some(strings(&["rust"])),
            "tags" => i.rules[0].selector.task_tags = Some(strings(&["review"])),
            _ => {
                i.rules[0] = constraint("r", "k", &["v"]);
                i.query.session_id = None;
            }
        }
        let out = resolve(&i).unwrap();
        assert_eq!(row(&out, "r").outcome, Outcome::Unknown, "{dimension}");
        assert!(unresolved(&out, ConflictCategory::UnknownApplicability));
        assert!(out.effective.is_empty());
    }
    i.rules[0].selector.agents = Some(strings(&["codex"]));
    i.rules[0].selector.languages = Some(strings(&["rust"]));
    i.query.language = Some("python".into());
    assert_eq!(
        resolve(&i).unwrap().explanations[0].applicability,
        Applicability::NoMatch
    );
    i.rules[0].selector.agents = Some(vec![]);
    assert_eq!(
        resolve(&i).unwrap().explanations[0].applicability,
        Applicability::Invalid
    );
}

pub(super) fn malformed_selectors_paths_and_queries_never_broaden_applicability() {
    for p in [
        "",
        "/src/lib.rs",
        "../file",
        "src/../file",
        "src/./file",
        "src//file",
        "src/",
        "C:/file",
        "C:\\file",
        "\\\\host\\share",
        "**/*.rs",
        "a?b",
        "[a]",
        "{a}",
        "a\0b",
        " src/file",
    ] {
        let mut i = input();
        let mut r = contract("r", "k", &["v"]);
        r.selector.paths = Some(strings(&[p]));
        i.rules.push(r);
        let out = resolve(&i).unwrap();
        assert_eq!(
            out.explanations[0].applicability,
            Applicability::Invalid,
            "{p:?}"
        );
        assert!(unresolved(&out, ConflictCategory::InvalidSourceSchema));
        assert!(out.effective.is_empty());
        i.rules[0].selector = Selector::default();
        i.query.path = Some(p.into());
        assert_eq!(resolve(&i), Err(InputError::InvalidQuery), "{p:?}");
    }
    for selector in [
        Selector {
            agents: Some(vec![]),
            ..Default::default()
        },
        Selector {
            languages: Some(strings(&[""])),
            ..Default::default()
        },
        Selector {
            task_tags: Some(strings(&["x\ny"])),
            ..Default::default()
        },
        Selector {
            projects: Some(vec![binding("unregistered")]),
            ..Default::default()
        },
    ] {
        let mut i = input();
        i.rules.push(Rule {
            selector,
            ..contract("r", "k", &["v"])
        });
        assert_eq!(
            resolve(&i).unwrap().explanations[0].applicability,
            Applicability::Invalid
        );
    }
}
