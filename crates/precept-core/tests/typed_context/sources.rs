use super::support::*;
use precept_core::typed_context::*;

pub(super) fn sources_unknown_and_invalid_categories_block_only_affected_key() {
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

pub(super) fn invalid_versions_shapes_sources_and_duplicate_identity_are_explained() {
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

pub(super) fn only_explicit_eligible_same_domain_supersession_can_retire_old_revision() {
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

pub(super) fn project_registration_and_scope_cannot_choose_new_roots_or_disclose_foreign_sources() {
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
