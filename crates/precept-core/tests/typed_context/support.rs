use precept_core::typed_context::*;

pub(super) fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).into()).collect()
}
pub(super) fn binding(project: &str) -> ProjectBinding {
    ProjectBinding {
        project: project.into(),
        root_id: format!("root-{project}-v1"),
    }
}
pub(super) fn source(id: &str) -> Source {
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
pub(super) fn input() -> ResolutionInputV1 {
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
pub(super) fn contract(id: &str, key: &str, allowed: &[&str]) -> Rule {
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
pub(super) fn preference(id: u64, key: &str, val: &str, project: bool) -> Rule {
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
pub(super) fn constraint(id: &str, key: &str, allowed: &[&str]) -> Rule {
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
pub(super) fn row<'a>(out: &'a EffectiveContextV1, id: &str) -> &'a RuleExplanation {
    out.explanations
        .iter()
        .find(|e| e.rule.reference.id == id)
        .unwrap()
}
pub(super) fn effective<'a>(out: &'a EffectiveContextV1, key: &str) -> Option<&'a EffectiveValue> {
    out.effective.iter().find(|v| v.key == key)
}
pub(super) fn unresolved(out: &EffectiveContextV1, category: ConflictCategory) -> bool {
    out.conflicts
        .iter()
        .any(|c| c.category == category && c.status == ConflictStatus::Unresolved)
}
