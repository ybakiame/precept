use super::support::*;
use precept_core::typed_context::*;

pub(super) fn reviewed_project_fallback_and_legacy_newest_id_are_preserved() {
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

pub(super) fn repository_contract_masks_incompatible_preference_with_both_source_refs() {
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

pub(super) fn equal_authority_total_intersection_conflicts_have_no_winner() {
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

pub(super) fn session_constraints_only_narrow_contract_and_supplied_policy() {
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

pub(super) fn live_repository_drift_remains_unresolved_and_claims_are_not_promoted() {
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
