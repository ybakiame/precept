use super::support::*;
use precept_core::typed_context::*;

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

pub(super) fn review_unresolved_contracts_cannot_claim_a_resolved_preference_mask() {
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

pub(super) fn review_unresolved_constraints_cannot_claim_a_resolved_preference_mask() {
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

pub(super) fn review_drift_or_disagreeing_facts_cannot_claim_a_resolved_preference_mask() {
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
