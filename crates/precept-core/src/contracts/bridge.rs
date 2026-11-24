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
fn served(r: &RuleFile, digest: &str, cache: Option<&mut checks::CheckCache>) -> Value {
    let mut v = json!({
        "id": r.id, "key": r.key, "text": r.body, "tags": r.tags,
        "file": file_of(&r.id), "sha256": digest, "review": "accepted",
        "enforcement": r.enforcement,
        "checks": match cache {
            Some(cache) => checks::evaluate_cached(&r.checks, cache),
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
/// with allowed values keeps those whose value it allows. Uncommitted rule
/// files are listed as pending only.
pub fn project(
    project: &str,
    preferences: Vec<Preference>,
    snapshot: &Snapshot,
    query: &SelectorQuery,
) -> Result<Projection> {
    query.validate()?;
    let candidates: Vec<_> = confirmed_preferences(project, preferences)
        .into_iter()
        .filter(|p| applicable(p, query))
        .collect();
    let mut legacy = legacy_preferences(candidates.clone());
    let Some(rules) = snapshot.rules() else {
        return Ok(if snapshot.status() == "not_configured" {
            Projection {
                preferences: legacy,
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
    let mut cache = snapshot.repository().map(|_| snapshot.check_cache());
    let repository_rules = applying
        .iter()
        .map(|(r, digest)| served(r, digest, cache.as_mut()))
        .collect();
    let mut resolution = json!({"status":if unresolved.is_empty() { "ready" } else { "partial" }, "unresolved_keys":unresolved});
    if let Some(cache) = &cache {
        resolution["check_io"] = cache.stats();
    }
    Ok(Projection {
        preferences: legacy,
        contract_resolution: resolution,
        repository_rules,
        pending_rules,
    })
}

/// Why each rule of `key` reaches this request or not: every Profile rule of
/// the project's scopes (any status) and every repository rule file of that
/// key, each with its outcome and the first reason in resolution order. Uses
/// the same projection agents receive, so it never disagrees with it.
pub fn explain(
    project: &str,
    preferences: Vec<Preference>,
    snapshot: &Snapshot,
    query: &SelectorQuery,
    key: &str,
) -> Result<Value> {
    use crate::Status;
    let projection = self::project(project, preferences.clone(), snapshot, query)?;
    let delivered: BTreeSet<u64> = projection.preferences.iter().map(|p| p.id).collect();
    let served: BTreeSet<String> = projection
        .repository_rules
        .iter()
        .filter_map(|r| r["id"].as_str().map(str::to_owned))
        .collect();
    let unresolved = projection.contract_resolution["unresolved_keys"]
        .as_array()
        .is_some_and(|k| k.iter().any(|k| k == key));
    // Repository rules of this key, committed or not.
    let files: Vec<(RuleFile, bool)> = snapshot
        .rules()
        .unwrap_or_default()
        .into_iter()
        .filter(|(r, _)| r.key == key)
        .map(|(r, _)| {
            let reviewed = snapshot.reviewed(&r.id);
            (r, reviewed)
        })
        .collect();
    let replacing = files
        .iter()
        .find(|(r, reviewed)| *reviewed && r.allowed.is_empty() && applies(&r.applies_to, query))
        .map(|(r, _)| file_of(&r.id));
    let constraining = files
        .iter()
        .find(|(r, reviewed)| *reviewed && !r.allowed.is_empty() && applies(&r.applies_to, query))
        .map(|(r, _)| file_of(&r.id));
    let winner = projection
        .preferences
        .iter()
        .find(|p| p.key == key)
        .map(|p| p.id);
    let mut rules = Vec::new();
    for p in preferences.iter().filter(|p| {
        p.key == key
            && match p.scope {
                Scope::Global => p.project.is_none(),
                Scope::Project => p.project.as_deref() == Some(project),
            }
    }) {
        let (outcome, reason, detail) = if delivered.contains(&p.id) {
            ("delivered", "delivered", String::new())
        } else if p.status != Status::Confirmed {
            (
                "not_delivered",
                "not_confirmed",
                format!(
                    "its status is {}",
                    serde_json::to_value(&p.status)
                        .unwrap_or_default()
                        .as_str()
                        .unwrap_or("?")
                ),
            )
        } else if !p.enabled {
            (
                "not_delivered",
                "switched_off",
                "the developer switched it off".into(),
            )
        } else if !applicable(p, query) {
            (
                "not_delivered",
                "does_not_apply",
                format!("it applies only to {}", crate::where_(&p.rule.applies_to)),
            )
        } else if let Some(file) = &replacing {
            (
                "not_delivered",
                "replaced_by_repository_rule",
                format!("{file} decides this key"),
            )
        } else if let (Some(file), Some(_)) = (&constraining, winner.filter(|_| !unresolved)) {
            (
                "not_delivered",
                "value_not_allowed",
                format!("{file} allows other values"),
            )
        } else if unresolved {
            (
                "not_delivered",
                "conflict",
                "this key has an unresolved conflict with a repository rule".into(),
            )
        } else {
            (
                "not_delivered",
                "outranked",
                match winner {
                    Some(id) => format!(
                        "rule #{id} wins this key (project before personal, then priority, then newest)"
                    ),
                    None => "another rule wins this key".into(),
                },
            )
        };
        rules.push(json!({
            "source": "profile", "id": p.id,
            "scope": if p.scope == Scope::Global { "personal" } else { "project" },
            "value": p.value, "priority": p.rule.priority,
            "outcome": outcome, "reason": reason, "detail": detail,
        }));
    }
    for (r, reviewed) in &files {
        let (outcome, reason, detail) = if served.contains(&r.id) {
            (
                "delivered",
                if r.allowed.is_empty() {
                    "delivered"
                } else {
                    "constrains_values"
                },
                if r.allowed.is_empty() {
                    String::new()
                } else {
                    format!("allows {}", r.allowed.join(", "))
                },
            )
        } else if !reviewed {
            (
                "not_delivered",
                "uncommitted",
                "the file has changes that are not committed".into(),
            )
        } else {
            (
                "not_delivered",
                "does_not_apply",
                format!("it applies only to {}", crate::where_(&r.applies_to)),
            )
        };
        rules.push(json!({
            "source": "repository", "id": r.id, "file": file_of(&r.id),
            "outcome": outcome, "reason": reason, "detail": detail,
        }));
    }
    Ok(json!({
        "key": key,
        "query": query,
        "contract_status": projection.contract_resolution["status"],
        "rules": rules,
    }))
}
