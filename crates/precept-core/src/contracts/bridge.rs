use super::{RuleFile, Snapshot, binding, checks, file_of, source};
use crate::{
    Preference, Result, Scope, applicable, applies, confirmed_preferences, legacy_preferences,
    typed_context as typed,
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
    /// Committed, unchanged rule files that apply to the request, each with
    /// its text and the result of its checks.
    pub repository_rules: Vec<Value>,
    /// Rule files with uncommitted changes or not tracked (ids and keys only;
    /// their text is not served until it is committed).
    pub pending_rules: Vec<Value>,
}
impl Projection {
    pub fn refused(status: &str, invalid: Vec<String>) -> Self {
        let mut resolution = json!({"status":status});
        if !invalid.is_empty() {
            resolution["invalid_files"] =
                json!(invalid.iter().map(|id| file_of(id)).collect::<Vec<_>>());
        }
        Self {
            preferences: vec![],
            contract_resolution: resolution,
            repository_rules: vec![],
            pending_rules: vec![],
        }
    }
}

/// One served rule: what agents read, never private Profile provenance.
fn served(r: &RuleFile, digest: &str, root: Option<&std::path::Path>) -> Value {
    let mut v = json!({
        "id": r.id, "key": r.key, "text": r.body, "tags": r.tags,
        "file": file_of(&r.id), "sha256": digest, "review": "accepted",
        "enforcement": r.enforcement,
        "checks": match root {
            Some(root) => checks::evaluate(root, &r.checks),
            None => json!({"status":"unavailable","passed":0,"total":r.checks.len()}),
        },
    });
    if let Some(t) = &r.title {
        v["title"] = json!(t);
    }
    if !r.allowed.is_empty() {
        v["allowed_values"] = json!(r.allowed);
    }
    if r.priority != 0 {
        v["priority"] = json!(r.priority);
    }
    if !r.applies_to.is_empty() {
        v["applies_to"] = json!(r.applies_to);
    }
    if !r.guards.is_empty() {
        v["guards"] = json!(r.guards.len());
    }
    v
}

/// Resolve the Profile's confirmed rules against the repository's rules.
/// A committed rule that applies masks Profile preferences of its key; one
/// with allowed values keeps those whose value it allows. Environment notes
/// never compete. Uncommitted rule files are listed as pending only.
pub fn project(
    project: &str,
    preferences: Vec<Preference>,
    snapshot: &Snapshot,
    query: &SelectorQuery,
) -> Result<Projection> {
    query.validate()?;
    let (environment, candidates): (Vec<_>, Vec<_>) = confirmed_preferences(project, preferences)
        .into_iter()
        .filter(|p| applicable(p, query))
        .partition(|p| p.scope == Scope::Environment);
    let mut legacy = legacy_preferences(candidates.clone());
    let Some(rules) = snapshot.rules() else {
        return Ok(if snapshot.status() == "not_configured" {
            Projection {
                preferences: legacy.into_iter().chain(environment).collect(),
                contract_resolution: json!({"status":"not_configured"}),
                repository_rules: vec![],
                pending_rules: vec![],
            }
        } else {
            Projection::refused(snapshot.status(), snapshot.invalid())
        });
    };
    let (accepted, pending): (Vec<_>, Vec<_>) = rules
        .into_iter()
        .partition(|(r, _)| snapshot.reviewed(&r.id));
    let pending_rules = pending
        .iter()
        .map(|(r, _)| json!({"id":r.id,"key":r.key,"file":file_of(&r.id),"review":"unreviewed"}))
        .collect();
    let applying: Vec<_> = accepted
        .into_iter()
        .filter(|(r, _)| applies(&r.applies_to, query))
        .collect();
    // Rules without allowed values replace the Profile's rules of their key.
    let masked: BTreeSet<_> = applying
        .iter()
        .filter(|(r, _)| r.allowed.is_empty())
        .map(|(r, _)| r.key.clone())
        .collect();
    legacy.retain(|p| !masked.contains(&p.key));
    let mut typed_rules: Vec<_> = applying
        .iter()
        .filter(|(r, _)| !r.allowed.is_empty())
        .map(|(r, digest)| r.typed(project, digest))
        .collect();
    let mut unresolved = BTreeSet::new();
    if !typed_rules.is_empty() {
        let affected: BTreeSet<_> = typed_rules.iter().map(|r| r.key.clone()).collect();
        let contract_ids: BTreeSet<_> =
            typed_rules.iter().map(|r| r.reference.id.clone()).collect();
        for p in candidates
            .iter()
            .filter(|p| affected.contains(&p.key) && !masked.contains(&p.key))
        {
            let mut id = format!("personal-{}", p.id);
            while contract_ids.contains(&id) {
                id.insert(0, '_');
            }
            typed_rules.push(typed::Rule {
                schema_version: 1,
                reference: typed::RuleRef { id, revision: 1 },
                kind: typed::RuleKind::Preference,
                scope: match p.scope {
                    Scope::Global => typed::Scope::Global,
                    Scope::Project | Scope::Environment => typed::Scope::Project {
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
        let resolved = typed::resolve(&query.input(project, typed_rules))
            .map_err(|_| crate::Error::InvalidArguments)?;
        let selected: BTreeSet<_> = resolved
            .explanations
            .iter()
            .filter(|e| {
                e.rule.kind == typed::RuleKind::Preference && e.outcome == typed::Outcome::Selected
            })
            .filter_map(|e| e.rule.legacy_preference_id)
            .collect();
        legacy.retain(|p| !affected.contains(&p.key) || selected.contains(&p.id));
        unresolved = resolved
            .conflicts
            .iter()
            .filter(|c| c.status == typed::ConflictStatus::Unresolved)
            .map(|c| c.key.clone())
            .collect();
    }
    let repository_rules = applying
        .iter()
        .map(|(r, digest)| served(r, digest, snapshot.root()))
        .collect();
    Ok(Projection {
        preferences: legacy.into_iter().chain(environment).collect(),
        contract_resolution: json!({"status":if unresolved.is_empty() { "ready" } else { "partial" }, "unresolved_keys":unresolved}),
        repository_rules,
        pending_rules,
    })
}
