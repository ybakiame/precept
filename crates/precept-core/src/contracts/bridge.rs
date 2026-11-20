use super::{SOURCE, Snapshot, binding, source};
use crate::{
    Preference, Result, Scope, confirmed_preferences, legacy_preferences, typed_context as typed,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Caller-provided applicability labels, never identity/authentication or roots.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SelectorQuery {
    pub agent_id: Option<String>,
    pub path: Option<String>,
    pub language: Option<String>,
    pub task_tags: Option<Vec<String>>,
}
impl SelectorQuery {
    pub(super) fn input(&self, project: &str, rules: Vec<typed::Rule>) -> typed::ResolutionInputV1 {
        typed::ResolutionInputV1 {
            schema_version: 1,
            registered_projects: vec![binding(project)],
            query: typed::Query {
                binding: binding(project),
                agent_id: self.agent_id.clone(),
                path: self.path.clone(),
                language: self.language.clone(),
                task_tags: self.task_tags.clone(),
                session_id: None,
            },
            rules,
            repository_facts: vec![],
            policy_boundaries: vec![],
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.task_tags.as_ref().is_some_and(|t| t.len() > 64) {
            return Err(crate::Error::InvalidArguments);
        }
        typed::resolve(&self.input("bound", vec![])).map_err(|_| crate::Error::InvalidArguments)?;
        Ok(())
    }
}

pub struct Projection {
    pub preferences: Vec<Preference>,
    pub contract_resolution: Value,
    pub effective_contracts: Vec<Value>,
}
impl Projection {
    pub fn refused(status: &str) -> Self {
        Self {
            preferences: vec![],
            contract_resolution: json!({"status":status}),
            effective_contracts: vec![],
        }
    }
}

/// Adapt exact profile values without carrying private source/decision provenance.
pub fn project(
    project: &str,
    preferences: Vec<Preference>,
    snapshot: &Snapshot,
    query: &SelectorQuery,
) -> Result<Projection> {
    query.validate()?;
    let candidates = confirmed_preferences(project, preferences);
    let mut legacy = legacy_preferences(candidates.clone());
    let Some((contracts, digest)) = snapshot.contracts() else {
        return Ok(if snapshot.status() == "not_configured" {
            Projection {
                preferences: legacy,
                contract_resolution: json!({"status":"not_configured"}),
                effective_contracts: vec![],
            }
        } else {
            Projection::refused(snapshot.status())
        });
    };
    let mut rules: Vec<_> = contracts.iter().map(|c| c.rule(project, &digest)).collect();
    let base = typed::resolve(&query.input(project, rules.clone()))
        .map_err(|_| crate::Error::InvalidArguments)?;
    // Pending and definitely nonmatching contracts do not pull a legacy key into
    // typed validation. Missing query on an accepted rule is genuinely affected.
    let affected: BTreeSet<_> = base
        .explanations
        .iter()
        .filter(|e| {
            e.rule.review == typed::ReviewState::Accepted
                && e.applicability != typed::Applicability::NoMatch
        })
        .map(|e| e.rule.key.clone())
        .collect();
    let contract_ids: BTreeSet<_> = rules.iter().map(|r| r.reference.id.clone()).collect();
    for p in candidates.iter().filter(|p| affected.contains(&p.key)) {
        let mut id = format!("personal-{}", p.id);
        while contract_ids.contains(&id) {
            id.insert(0, '_');
        }
        rules.push(typed::Rule {
            schema_version: 1,
            reference: typed::RuleRef { id, revision: 1 },
            kind: typed::RuleKind::Preference,
            scope: match p.scope {
                Scope::Global => typed::Scope::Global,
                Scope::Project => typed::Scope::Project {
                    binding: binding(project),
                },
            },
            authority: typed::Authority::Personal,
            review: typed::ReviewState::Confirmed,
            selector: typed::Selector::default(),
            key: p.key.clone(),
            allowed_values: vec![p.value.clone()],
            legacy_preference_id: Some(p.id),
            supersedes: vec![],
            source: source("personal-context", "local"),
        });
    }
    let resolved =
        typed::resolve(&query.input(project, rules)).map_err(|_| crate::Error::InvalidArguments)?;
    let selected: BTreeSet<_> = resolved
        .explanations
        .iter()
        .filter(|e| {
            e.rule.kind == typed::RuleKind::Preference && e.outcome == typed::Outcome::Selected
        })
        .filter_map(|e| e.rule.legacy_preference_id)
        .collect();
    legacy.retain(|p| !affected.contains(&p.key) || selected.contains(&p.id));
    let unresolved: BTreeSet<_> = resolved
        .conflicts
        .iter()
        .filter(|c| c.status == typed::ConflictStatus::Unresolved)
        .map(|c| c.key.clone())
        .collect();
    let effective_contracts = resolved.effective.iter().filter_map(|e| {
        let ids: Vec<_> = e.rules.iter().filter(|r| contract_ids.contains(&r.id)).map(|r| &r.id).collect();
        if ids.is_empty() { return None; }
        Some(json!({"key":e.key,"allowed_values":e.allowed_values,"rule_ids":ids,"source":{"path":SOURCE,"sha256":digest}}))
    }).collect();
    Ok(Projection {
        preferences: legacy,
        contract_resolution: json!({"status":if unresolved.is_empty() { "ready" } else { "partial" }, "unresolved_keys":unresolved}),
        effective_contracts,
    })
}
