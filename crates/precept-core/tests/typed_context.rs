use precept_core::typed_context::*;
use serde_json::{Value, json};

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).into()).collect()
}
fn binding(project: &str) -> ProjectBinding {
    ProjectBinding {
        project: project.into(),
        root_id: format!("root-{project}-v1"),
    }
}
fn source(id: &str) -> Source {
    Source {
        reference: SourceRef {
            id: id.into(),
            version: "1".into(),
            digest: Some("synthetic-digest".into()),
        },
        state: SourceState::Fresh,
        provenance: Provenance {
            origin: Origin::DaemonObserved,
            references: strings(&["synthetic-observation"]),
            historical_validation: vec![],
        },
    }
}
fn input() -> ResolutionInputV1 {
    ResolutionInputV1 {
        schema_version: 1,
        registered_projects: vec![binding("alpha"), binding("beta")],
        query: Query {
            binding: binding("alpha"),
            agent_id: None,
            path: None,
            language: None,
            task_tags: None,
            session_id: Some("session-1".into()),
        },
        rules: vec![],
        repository_facts: vec![],
        policy_boundaries: vec![],
    }
}
fn contract(id: &str, key: &str, allowed: &[&str]) -> Rule {
    Rule {
        schema_version: 1,
        reference: RuleRef {
            id: id.into(),
            revision: 1,
        },
        kind: RuleKind::Contract,
        scope: Scope::Project {
            binding: binding("alpha"),
        },
        authority: Authority::Repository,
        review: ReviewState::Accepted,
        selector: Selector::default(),
        key: key.into(),
        allowed_values: strings(allowed),
        legacy_preference_id: None,
        supersedes: vec![],
        source: source(&format!("source-{id}")),
    }
}
fn preference(id: u64, key: &str, val: &str, project: bool) -> Rule {
    Rule {
        reference: RuleRef {
            id: format!("preference-{id}"),
            revision: 1,
        },
        kind: RuleKind::Preference,
        scope: if project {
            Scope::Project {
                binding: binding("alpha"),
            }
        } else {
            Scope::Global
        },
        authority: Authority::Personal,
        review: ReviewState::Confirmed,
        legacy_preference_id: Some(id),
        ..contract("temporary", key, &[val])
    }
}
fn constraint(id: &str, key: &str, allowed: &[&str]) -> Rule {
    Rule {
        kind: RuleKind::Constraint,
        scope: Scope::Session {
            binding: binding("alpha"),
            session_id: "session-1".into(),
        },
        authority: Authority::SessionNarrowing,
        review: ReviewState::Confirmed,
        ..contract(id, key, allowed)
    }
}
fn row<'a>(out: &'a EffectiveContextV1, id: &str) -> &'a RuleExplanation {
    out.explanations
        .iter()
        .find(|e| e.rule.reference.id == id)
        .unwrap()
}
fn effective<'a>(out: &'a EffectiveContextV1, key: &str) -> Option<&'a EffectiveValue> {
    out.effective.iter().find(|v| v.key == key)
}
fn unresolved(out: &EffectiveContextV1, category: ConflictCategory) -> bool {
    out.conflicts
        .iter()
        .any(|c| c.category == category && c.status == ConflictStatus::Unresolved)
}

#[test]
fn selectors_are_and_across_or_within_exact_dimensions() {
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

#[test]
fn absent_missing_known_empty_and_false_unknown_have_distinct_meanings() {
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

#[test]
fn malformed_selectors_paths_and_queries_never_broaden_applicability() {
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

#[test]
fn reviewed_project_fallback_and_legacy_newest_id_are_preserved() {
    let mut i = input();
    i.rules = vec![
        preference(1, "formatter", "global", false),
        preference(90, "formatter", "new-global", false),
        preference(2, "formatter", "old-project", true),
        preference(3, "formatter", "new-project", true),
    ];
    for (id, status) in [
        (100, ReviewState::Suggested),
        (101, ReviewState::Rejected),
        (102, ReviewState::Deprecated),
    ] {
        let mut r = preference(id, "formatter", "NEVER_EFFECTIVE", true);
        r.review = status;
        i.rules.push(r);
    }
    let mut unaccepted = contract("unreviewed", "unreviewed-key", &["NEVER_EFFECTIVE"]);
    unaccepted.review = ReviewState::Unreviewed;
    i.rules.push(unaccepted);
    let out = resolve(&i).unwrap();
    assert_eq!(
        effective(&out, "formatter").unwrap().allowed_values,
        strings(&["new-project"])
    );
    assert_eq!(
        row(&out, "preference-90").reasons,
        vec![Reason::ProjectPreference]
    );
    assert_eq!(
        row(&out, "preference-2").reasons,
        vec![Reason::NewerPreference]
    );
    assert!(effective(&out, "unreviewed-key").is_none());
    assert!(out.conflicts.is_empty());
    for id in [
        "preference-100",
        "preference-101",
        "preference-102",
        "unreviewed",
    ] {
        assert_eq!(row(&out, id).outcome, Outcome::Excluded);
    }
    i.rules
        .retain(|r| !matches!(r.scope, Scope::Project { .. }));
    assert_eq!(
        resolve(&i).unwrap().effective[0].allowed_values,
        strings(&["new-global"])
    );
}

#[test]
fn repository_contract_masks_incompatible_preference_with_both_source_refs() {
    let mut i = input();
    i.rules = vec![
        preference(7, "formatter", "personal", true),
        contract("contract-fmt", "formatter", &["cargo-fmt"]),
    ];
    let out = resolve(&i).unwrap();
    assert_eq!(
        effective(&out, "formatter").unwrap().allowed_values,
        strings(&["cargo-fmt"])
    );
    assert_eq!(row(&out, "preference-7").outcome, Outcome::Masked);
    let conflict = out
        .conflicts
        .iter()
        .find(|c| c.category == ConflictCategory::PreferenceVsContract)
        .unwrap();
    assert_eq!(conflict.status, ConflictStatus::ResolvedByAuthority);
    assert_eq!(conflict.rules.len(), 2);
    assert_eq!(conflict.sources.len(), 2);
    assert_eq!(conflict.next_action, ReviewAction::InspectPreference);
    i.rules[0].allowed_values = strings(&["cargo-fmt"]);
    let compatible = resolve(&i).unwrap();
    assert!(compatible.conflicts.is_empty());
    assert_eq!(compatible.effective[0].rules.len(), 2);
}

#[test]
fn equal_authority_total_intersection_conflicts_have_no_winner() {
    let mut i = input();
    i.query.path = Some("src/lib.rs".into());
    i.rules = vec![
        contract("a", "mode", &["one", "two"]),
        contract("b", "mode", &["two", "three"]),
        contract("c", "mode", &["one", "three"]),
        contract("unaffected", "language", &["rust"]),
    ];
    i.rules[2].selector.paths = Some(strings(&["src/lib.rs"]));
    let out = resolve(&i).unwrap();
    assert!(effective(&out, "mode").is_none());
    assert!(effective(&out, "language").is_some());
    assert!(unresolved(&out, ConflictCategory::RuleVsRule));
    for id in ["a", "b", "c"] {
        assert_eq!(row(&out, id).outcome, Outcome::Conflicted);
    }
    let mut id_collision = preference(1, "k", "a", true);
    let mut other = id_collision.clone();
    other.reference.id = "another".into();
    other.allowed_values = strings(&["b"]);
    i.rules = vec![id_collision.clone(), other];
    assert!(unresolved(
        &resolve(&i).unwrap(),
        ConflictCategory::RuleVsRule
    ));
    id_collision.allowed_values = strings(&["b"]);
    i.rules[0] = id_collision;
    assert_eq!(
        resolve(&i).unwrap().effective[0].allowed_values,
        strings(&["b"])
    );
}

#[test]
fn session_constraints_only_narrow_contract_and_supplied_policy() {
    let mut i = input();
    i.rules = vec![
        contract("repo", "operation", &["read", "write"]),
        constraint("session", "operation", &["read"]),
        preference(1, "operation", "write", true),
    ];
    i.policy_boundaries.push(PolicyBoundary {
        binding: binding("alpha"),
        key: "operation".into(),
        allowed_values: strings(&["read"]),
        source: source("policy"),
    });
    let out = resolve(&i).unwrap();
    assert_eq!(out.effective[0].allowed_values, strings(&["read"]));
    assert_eq!(row(&out, "preference-1").outcome, Outcome::Masked);
    for (allowed, category) in [
        (
            strings(&["read", "write"]),
            ConflictCategory::SessionVsPolicy,
        ),
        (strings(&["admin"]), ConflictCategory::SessionVsContract),
    ] {
        let mut i = i.clone();
        i.rules[1].allowed_values = allowed;
        let out = resolve(&i).unwrap();
        assert!(unresolved(&out, category));
        assert!(out.effective.is_empty());
        assert_eq!(row(&out, "repo").outcome, Outcome::Conflicted);
    }
    i.rules.push(constraint("other", "operation", &["write"]));
    assert!(unresolved(
        &resolve(&i).unwrap(),
        ConflictCategory::RuleVsRule
    ));
    let serialized = serde_json::to_value(resolve(&i).unwrap()).unwrap();
    assert!(serialized.get("approved").is_none());
    assert!(serialized.get("capability").is_none());
}

#[test]
fn live_repository_drift_remains_unresolved_and_claims_are_not_promoted() {
    let mut i = input();
    i.rules = vec![contract("repo", "formatter", &["rustfmt"])];
    i.repository_facts.push(RepositoryFact {
        binding: binding("alpha"),
        key: "formatter".into(),
        value: "other".into(),
        source: source("current-config"),
    });
    let out = resolve(&i).unwrap();
    assert!(unresolved(&out, ConflictCategory::RuleVsRepository));
    assert!(out.effective.is_empty());
    assert_eq!(out.repository_facts[0].source.reference.version, "1");
    for origin in [Origin::AgentReported, Origin::LegacyUnknown] {
        let mut i = i.clone();
        i.repository_facts[0].source.provenance.origin = origin;
        let out = resolve(&i).unwrap();
        assert!(unresolved(&out, ConflictCategory::SourceUnknown));
        assert!(!unresolved(&out, ConflictCategory::RuleVsRepository));
        assert_eq!(out.repository_facts[0].source.provenance.origin, origin);
    }
    i.rules = vec![preference(1, "formatter", "personal", false)];
    let out = resolve(&i).unwrap();
    assert_eq!(out.explanations[0].outcome, Outcome::Masked);
    assert!(out.effective.is_empty());
    assert_eq!(out.conflicts[0].status, ConflictStatus::ResolvedByAuthority);
}

#[test]
fn sources_unknown_and_invalid_categories_block_only_affected_key() {
    for (state, category, reason) in [
        (
            SourceState::Stale,
            ConflictCategory::StaleSourceSnapshot,
            Reason::StaleSource,
        ),
        (
            SourceState::Unavailable,
            ConflictCategory::SourceUnavailable,
            Reason::UnavailableSource,
        ),
        (
            SourceState::Unknown,
            ConflictCategory::SourceUnknown,
            Reason::UnknownSource,
        ),
        (
            SourceState::Malformed,
            ConflictCategory::InvalidSourceSchema,
            Reason::InvalidSource,
        ),
        (
            SourceState::UnsupportedSchema,
            ConflictCategory::InvalidSourceSchema,
            Reason::InvalidSource,
        ),
    ] {
        let mut i = input();
        i.rules = vec![
            contract("bad", "affected", &["v"]),
            contract("good", "unaffected", &["v"]),
        ];
        i.rules[0].source.state = state;
        let out = resolve(&i).unwrap();
        assert!(unresolved(&out, category));
        assert!(row(&out, "bad").reasons.contains(&reason));
        assert!(effective(&out, "affected").is_none());
        assert!(effective(&out, "unaffected").is_some());
    }
    let mut i = input();
    i.rules = vec![
        preference(1, "k", "global", false),
        preference(2, "k", "project", true),
    ];
    i.rules[1].selector.languages = Some(strings(&["rust"]));
    let out = resolve(&i).unwrap();
    assert!(unresolved(&out, ConflictCategory::UnknownApplicability));
    assert!(out.effective.is_empty());
    i.rules[1].source.state = SourceState::Stale;
    let out = resolve(&i).unwrap();
    assert!(unresolved(&out, ConflictCategory::UnknownApplicability));
    assert!(unresolved(&out, ConflictCategory::StaleSourceSnapshot));
    assert!(
        row(&out, "preference-2")
            .reasons
            .contains(&Reason::MissingQuery)
    );
    assert!(
        row(&out, "preference-2")
            .reasons
            .contains(&Reason::StaleSource)
    );
    i.rules[1].review = ReviewState::Suggested;
    assert_eq!(
        resolve(&i).unwrap().effective[0].allowed_values,
        strings(&["global"])
    );
}

#[test]
fn invalid_versions_shapes_sources_and_duplicate_identity_are_explained() {
    let base = contract("r", "k", &["v"]);
    let mut variants = vec![];
    let mut r = base.clone();
    r.schema_version = 2;
    variants.push((r, Reason::InvalidSchema));
    let mut r = base.clone();
    r.reference.revision = 0;
    variants.push((r, Reason::InvalidIdentity));
    let mut r = base.clone();
    r.authority = Authority::Personal;
    variants.push((r, Reason::InvalidRuleShape));
    let mut r = base.clone();
    r.allowed_values.clear();
    variants.push((r, Reason::InvalidRuleShape));
    let mut r = base.clone();
    r.allowed_values = (0..65).map(|i| i.to_string()).collect();
    variants.push((r, Reason::InvalidRuleShape));
    let mut r = base.clone();
    r.source.reference.version.clear();
    variants.push((r, Reason::InvalidSource));
    let mut r = base.clone();
    r.source.reference.digest = Some("\n".into());
    variants.push((r, Reason::InvalidSource));
    for (r, reason) in variants {
        let mut i = input();
        i.rules = vec![r];
        let out = resolve(&i).unwrap();
        assert!(out.effective.is_empty());
        assert!(out.explanations[0].reasons.contains(&reason));
        assert!(unresolved(&out, ConflictCategory::InvalidSourceSchema));
    }
    let mut i = input();
    i.rules = vec![base.clone(), base.clone()];
    assert_eq!(resolve(&i).unwrap().explanations.len(), 1);
    i.rules[1].allowed_values = strings(&["different"]);
    let out = resolve(&i).unwrap();
    assert_eq!(out.explanations.len(), 2);
    assert!(out.effective.is_empty());
    assert!(unresolved(&out, ConflictCategory::InvalidSourceSchema));
    let mut i = input();
    i.schema_version = 2;
    assert_eq!(resolve(&i), Err(InputError::UnsupportedSchema));
}

#[test]
fn only_explicit_eligible_same_domain_supersession_can_retire_old_revision() {
    let mut i = input();
    let old = contract("r", "k", &["old"]);
    let mut new = old.clone();
    new.reference.revision = 2;
    new.allowed_values = strings(&["new"]);
    new.source.reference.version = "2".into();
    i.rules = vec![old.clone(), new.clone()];
    assert!(unresolved(
        &resolve(&i).unwrap(),
        ConflictCategory::RuleVsRule
    ));
    new.supersedes = vec![old.reference.clone()];
    i.rules[1] = new.clone();
    let out = resolve(&i).unwrap();
    assert_eq!(out.effective[0].allowed_values, strings(&["new"]));
    assert!(
        out.explanations
            .iter()
            .any(|e| e.rule.reference.revision == 1 && e.reasons == vec![Reason::Superseded])
    );
    for status in [
        ReviewState::Unreviewed,
        ReviewState::Rejected,
        ReviewState::Deprecated,
    ] {
        let mut i = i.clone();
        i.rules[1].review = status;
        assert_eq!(
            resolve(&i).unwrap().effective[0].allowed_values,
            strings(&["old"])
        );
    }
    let mut stale = i.clone();
    stale.rules[1].source.state = SourceState::Stale;
    let out = resolve(&stale).unwrap();
    assert!(
        !out.explanations
            .iter()
            .any(|r| r.reasons.contains(&Reason::Superseded))
    );
    assert!(out.effective.is_empty());
    for change in ["key", "source", "kind", "forward"] {
        let mut i = i.clone();
        match change {
            "key" => i.rules[1].key = "changed".into(),
            "source" => i.rules[1].source.reference.id = "changed".into(),
            "kind" => {
                i.rules[1].kind = RuleKind::Preference;
                i.rules[1].authority = Authority::Personal;
                i.rules[1].review = ReviewState::Confirmed;
                i.rules[1].legacy_preference_id = Some(1);
            }
            _ => i.rules[1].supersedes[0].revision = 3,
        }
        let out = resolve(&i).unwrap();
        assert!(
            unresolved(&out, ConflictCategory::InvalidSourceSchema),
            "{change}"
        );
        assert!(
            !out.explanations
                .iter()
                .any(|r| r.reasons.contains(&Reason::Superseded))
        );
    }
}

#[test]
fn project_registration_and_scope_cannot_choose_new_roots_or_disclose_foreign_sources() {
    let mut i = input();
    i.query.binding = binding("unknown");
    assert_eq!(resolve(&i), Err(InputError::UnregisteredProject));
    i.query.binding = binding("alpha");
    i.query.binding.root_id = "other-root".into();
    assert_eq!(resolve(&i), Err(InputError::UnregisteredProject));
    i = input();
    i.registered_projects.push(ProjectBinding {
        project: "alpha".into(),
        root_id: "other".into(),
    });
    assert_eq!(resolve(&i), Err(InputError::InvalidRegistration));
    i = input();
    let mut foreign = contract("FOREIGN_PRIVATE", "FOREIGN_KEY", &["FOREIGN_VALUE"]);
    foreign.scope = Scope::Project {
        binding: binding("beta"),
    };
    foreign.source = source("FOREIGN_SOURCE");
    let mut wrong_root = foreign.clone();
    wrong_root.scope = Scope::Project {
        binding: ProjectBinding {
            project: "alpha".into(),
            root_id: "FOREIGN_ROOT".into(),
        },
    };
    let mut other_session = constraint("FOREIGN_SESSION", "k", &["FOREIGN_VALUE"]);
    other_session.scope = Scope::Session {
        binding: binding("alpha"),
        session_id: "FOREIGN_SESSION".into(),
    };
    i.rules = vec![
        foreign,
        wrong_root,
        other_session,
        contract("local", "k", &["v"]),
    ];
    i.repository_facts.push(RepositoryFact {
        binding: binding("beta"),
        key: "FOREIGN_FACT".into(),
        value: "FOREIGN_VALUE".into(),
        source: source("FOREIGN_SOURCE"),
    });
    i.policy_boundaries.push(PolicyBoundary {
        binding: binding("beta"),
        key: "FOREIGN_POLICY".into(),
        allowed_values: strings(&["FOREIGN_VALUE"]),
        source: source("FOREIGN_SOURCE"),
    });
    let out = resolve(&i).unwrap();
    assert!(!serde_json::to_string(&out).unwrap().contains("FOREIGN"));
    assert_eq!(out.explanations.len(), 1);
    i.rules[3].selector.projects = Some(vec![ProjectBinding {
        project: "alpha".into(),
        root_id: "new-root".into(),
    }]);
    assert_eq!(
        row(&resolve(&i).unwrap(), "local").applicability,
        Applicability::Invalid
    );
}

#[test]
fn normalization_and_input_permutations_produce_identical_serialized_output() {
    let mut i = input();
    i.query.path = Some("src/lib.rs".into());
    i.query.language = Some("rust".into());
    i.rules = vec![
        contract("b", "k", &["b", "a", "a"]),
        contract("a", "k", &["a", "b"]),
        preference(1, "k", "a", true),
        contract("z", "other", &["x"]),
        contract("conflict-a", "blocked", &["one"]),
        contract("conflict-b", "blocked", &["two"]),
        contract("conflict-c", "blocked-2", &["one"]),
        contract("conflict-d", "blocked-2", &["two"]),
    ];
    for r in &mut i.rules {
        r.selector.paths = Some(strings(&["src/main.rs", "src/lib.rs", "src/lib.rs"]));
        r.selector.languages = Some(strings(&["rust", "python"]));
        r.source.provenance.references = strings(&["z", "a", "a"]);
        r.source.provenance.historical_validation = vec![HistoricalValidation {
            reference: "claim".into(),
            origin: Origin::AgentReported,
            result: HistoricalResult::PASS,
            revision: Some("old-revision".into()),
        }];
    }
    i.policy_boundaries.push(PolicyBoundary {
        binding: binding("alpha"),
        key: "operation".into(),
        allowed_values: strings(&["write", "read", "read"]),
        source: source("policy"),
    });
    i.repository_facts.push(RepositoryFact {
        binding: binding("alpha"),
        key: "other".into(),
        value: "x".into(),
        source: source("fact"),
    });
    i.repository_facts.push(RepositoryFact {
        binding: binding("alpha"),
        key: "k".into(),
        value: "a".into(),
        source: source("fact-2"),
    });
    i.policy_boundaries.push(PolicyBoundary {
        binding: binding("alpha"),
        key: "another-policy".into(),
        allowed_values: strings(&["v"]),
        source: source("policy-2"),
    });
    let expected = serde_json::to_string(&resolve(&i).unwrap()).unwrap();
    for shift in 0..i.rules.len() {
        let mut p = i.clone();
        p.rules.rotate_left(shift);
        p.registered_projects.reverse();
        p.rules.reverse();
        p.rules.push(p.rules[0].clone());
        p.repository_facts.reverse();
        p.policy_boundaries.reverse();
        for r in &mut p.rules {
            r.allowed_values.reverse();
            r.source.provenance.references.reverse();
            r.selector.paths.as_mut().unwrap().reverse();
            r.selector.languages.as_mut().unwrap().reverse();
        }
        assert_eq!(
            serde_json::to_string(&resolve(&p).unwrap()).unwrap(),
            expected
        );
    }
    let out = resolve(&i).unwrap();
    assert_eq!(
        out.explanations
            .iter()
            .map(|r| r.rule.key.as_str())
            .collect::<Vec<_>>(),
        vec![
            "blocked",
            "blocked",
            "blocked-2",
            "blocked-2",
            "k",
            "k",
            "k",
            "other"
        ]
    );
    let claim = &out.explanations[0]
        .rule
        .source
        .provenance
        .historical_validation[0];
    assert_eq!(claim.origin, Origin::AgentReported);
    assert_eq!(claim.result, HistoricalResult::PASS);
    assert_eq!(claim.revision.as_deref(), Some("old-revision"));
    assert_eq!(
        serde_json::from_str::<EffectiveContextV1>(&expected).unwrap(),
        out
    );
}

#[test]
fn malformed_serialized_input_and_unknown_fields_are_not_silently_dropped() {
    let mut i = input();
    i.rules.push(contract("r", "k", &["v"]));
    let raw = serde_json::to_value(&i).unwrap();
    for field in ["selector", "source", "reference", "scope"] {
        let mut v = raw.clone();
        v["rules"][0][field]["unrecognized"] = json!(true);
        assert!(
            serde_json::from_value::<ResolutionInputV1>(v).is_err(),
            "{field}"
        );
    }
    for replacement in [Value::Null, json!([]), json!("src/lib.rs"), json!([1])] {
        let mut v = raw.clone();
        v["rules"][0]["selector"] = replacement;
        assert!(serde_json::from_value::<ResolutionInputV1>(v).is_err());
    }
    let mut v = raw.clone();
    v["rules"][0]["selector"]["paths"] = json!([1]);
    assert!(serde_json::from_value::<ResolutionInputV1>(v).is_err());
    let mut v = raw.clone();
    v["rules"][0]["kind"] = json!("unknown");
    assert!(serde_json::from_value::<ResolutionInputV1>(v).is_err());
    let mut v = raw.clone();
    v["rules"][0]["source"]["provenance"]["historical_validation"] =
        json!([{"reference":"claim","origin":"agent_reported","result":"BLOCKED","revision":null}]);
    assert!(serde_json::from_value::<ResolutionInputV1>(v).is_err());
    let mut v = raw;
    v["schema_version"] = json!(2);
    let i = serde_json::from_value(v).unwrap();
    assert_eq!(resolve(&i), Err(InputError::UnsupportedSchema));
}

#[test]
fn no_match_invalid_metadata_does_not_block_and_unmatched_observation_issues_stay_visible() {
    let mut i = input();
    let mut mismatch = contract("unrelated", "k", &[]);
    mismatch.selector.languages = Some(strings(&["rust"]));
    i.query.language = Some("python".into());
    i.rules = vec![contract("active", "k", &["v"]), mismatch];
    i.repository_facts.push(RepositoryFact {
        binding: binding("alpha"),
        key: "unmatched-fact".into(),
        value: "v".into(),
        source: Source {
            state: SourceState::Stale,
            ..source("stale-source")
        },
    });
    i.policy_boundaries.push(PolicyBoundary {
        binding: binding("alpha"),
        key: "unmatched-policy".into(),
        allowed_values: vec![],
        source: source("bad-policy"),
    });
    let out = resolve(&i).unwrap();
    assert!(effective(&out, "k").is_some());
    assert!(out.conflicts.iter().all(|c| c.key != "k"));
    assert!(out.conflicts.iter().any(|c| c.key == "unmatched-fact"
        && c.category == ConflictCategory::StaleSourceSnapshot
        && c.rules.is_empty()));
    assert!(out.conflicts.iter().any(|c| c.key == "unmatched-policy"
        && c.category == ConflictCategory::InvalidSourceSchema
        && c.rules.is_empty()));
    assert_eq!(row(&out, "unrelated").outcome, Outcome::Excluded);
}

fn assert_review_case(input: &ResolutionInputV1, category: ConflictCategory) {
    let expected = resolve(input).unwrap();
    assert!(unresolved(&expected, category));
    let pref = row(&expected, "preference-7");
    assert_eq!(pref.outcome, Outcome::Conflicted);
    assert!(pref.reasons.contains(&Reason::UnresolvedKey));
    assert!(!pref.reasons.iter().any(|r| matches!(
        r,
        Reason::RepositoryContract | Reason::SessionConstraint | Reason::RepositoryObservation
    )));
    assert!(effective(&expected, "affected").is_none());
    assert!(effective(&expected, "unaffected").is_some());
    assert!(
        !expected
            .conflicts
            .iter()
            .any(|c| c.key == "affected" && c.status == ConflictStatus::ResolvedByAuthority)
    );
    for shift in 0..input.rules.len() {
        let mut permuted = input.clone();
        permuted.rules.rotate_left(shift);
        permuted.rules.reverse();
        permuted.repository_facts.reverse();
        permuted.policy_boundaries.reverse();
        assert_eq!(resolve(&permuted).unwrap(), expected);
    }
}

#[test]
fn review_unresolved_contracts_cannot_claim_a_resolved_preference_mask() {
    for cause in ["empty", "revisions", "stale", "missing-query"] {
        let mut i = input();
        i.rules = vec![
            preference(7, "affected", "rustfmt", true),
            contract("a", "affected", &["prettier"]),
            contract("unrelated", "unaffected", &["v"]),
        ];
        let category = match cause {
            "empty" => {
                i.rules.push(contract("b", "affected", &["rustfmt"]));
                ConflictCategory::RuleVsRule
            }
            "revisions" => {
                let mut newer = i.rules[1].clone();
                newer.reference.revision = 2;
                i.rules.push(newer);
                ConflictCategory::RuleVsRule
            }
            "stale" => {
                let mut stale = contract("stale", "affected", &["rustfmt"]);
                stale.source.state = SourceState::Stale;
                i.rules.push(stale);
                ConflictCategory::StaleSourceSnapshot
            }
            _ => {
                let mut unknown = contract("unknown", "affected", &["rustfmt"]);
                unknown.selector.languages = Some(strings(&["rust"]));
                i.rules.push(unknown);
                ConflictCategory::UnknownApplicability
            }
        };
        assert_review_case(&i, category);
    }
}

#[test]
fn review_unresolved_constraints_cannot_claim_a_resolved_preference_mask() {
    for cause in ["empty", "contract", "policy", "unknown-policy", "revisions"] {
        let mut i = input();
        i.rules = vec![
            preference(7, "affected", "read", true),
            constraint("session", "affected", &["write"]),
            contract("unrelated", "unaffected", &["v"]),
        ];
        let category = match cause {
            "empty" => {
                i.rules.push(constraint("other", "affected", &["read"]));
                ConflictCategory::RuleVsRule
            }
            "contract" => {
                i.rules.push(contract("repo", "affected", &["read"]));
                ConflictCategory::SessionVsContract
            }
            "policy" | "unknown-policy" => {
                let mut p = PolicyBoundary {
                    binding: binding("alpha"),
                    key: "affected".into(),
                    allowed_values: strings(&["read"]),
                    source: source("policy"),
                };
                if cause == "unknown-policy" {
                    p.source.state = SourceState::Unknown;
                }
                i.policy_boundaries.push(p);
                if cause == "policy" {
                    ConflictCategory::SessionVsPolicy
                } else {
                    ConflictCategory::SourceUnknown
                }
            }
            _ => {
                let mut newer = i.rules[1].clone();
                newer.reference.revision = 2;
                i.rules.push(newer);
                ConflictCategory::RuleVsRule
            }
        };
        assert_review_case(&i, category);
    }
}

#[test]
fn review_drift_or_disagreeing_facts_cannot_claim_a_resolved_preference_mask() {
    for cause in [
        "contract-drift",
        "constraint-drift",
        "disagreement",
        "unknown-fact",
    ] {
        let mut i = input();
        i.rules = vec![
            preference(7, "affected", "read", true),
            contract("unrelated", "unaffected", &["v"]),
        ];
        i.repository_facts.push(RepositoryFact {
            binding: binding("alpha"),
            key: "affected".into(),
            value: "write".into(),
            source: source("observed-write"),
        });
        let category = match cause {
            "contract-drift" => {
                i.rules.push(contract("repo", "affected", &["read"]));
                ConflictCategory::RuleVsRepository
            }
            "constraint-drift" => {
                i.rules.push(constraint("session", "affected", &["read"]));
                ConflictCategory::RuleVsRepository
            }
            "disagreement" | "unknown-fact" => {
                let mut fact = RepositoryFact {
                    binding: binding("alpha"),
                    key: "affected".into(),
                    value: "read".into(),
                    source: source("observed-read"),
                };
                if cause == "unknown-fact" {
                    fact.source.state = SourceState::Unknown;
                }
                i.repository_facts.push(fact);
                if cause == "disagreement" {
                    ConflictCategory::RuleVsRepository
                } else {
                    ConflictCategory::SourceUnknown
                }
            }
            _ => unreachable!(),
        };
        assert_review_case(&i, category);
    }
    // Consistent trusted observations may still mask a preference; retain every source.
    let mut i = input();
    i.rules = vec![
        preference(7, "affected", "read", true),
        contract("repo", "affected", &["read", "write"]),
    ];
    for id in ["observer-a", "observer-b"] {
        i.repository_facts.push(RepositoryFact {
            binding: binding("alpha"),
            key: "affected".into(),
            value: "write".into(),
            source: source(id),
        });
    }
    let out = resolve(&i).unwrap();
    assert_eq!(row(&out, "preference-7").outcome, Outcome::Masked);
    assert_eq!(
        row(&out, "preference-7").reasons,
        vec![Reason::RepositoryObservation]
    );
    let conflict = out
        .conflicts
        .iter()
        .find(|c| c.category == ConflictCategory::RuleVsRepository)
        .unwrap();
    assert_eq!(conflict.status, ConflictStatus::ResolvedByAuthority);
    assert_eq!(conflict.sources.len(), 3);
    i.repository_facts.reverse();
    i.rules.reverse();
    assert_eq!(resolve(&i).unwrap(), out);
}
