use super::{Command, Mutation, Operator, Origin, Scope};
use crate::Model;
use serde_json::{Value, json};

/// The most rules one import files (the writer's batch bound).
pub const MAX_BATCH_UI: usize = precept_core::operator::MAX_BATCH;

/// One rule an import scan found (`import.scan`).
#[derive(Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportCandidate {
    pub source: String,
    pub kind: precept_core::import::SourceKind,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub rule: precept_core::operator::RuleOptions,
    pub scope: Scope,
    /// Already in the Profile, suggested or confirmed, exactly as found.
    pub exists: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportSkipped {
    pub source: String,
    pub title: String,
    pub reason: String,
    #[serde(default)]
    pub detail: Option<String>,
}
/// A scan of one project's files.
#[derive(Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportScan {
    #[serde(skip)]
    pub project: String,
    pub candidates: Vec<ImportCandidate>,
    pub skipped: Vec<ImportSkipped>,
}

fn known_directory_file(source: &str, spec: &precept_core::import::DirectorySpec) -> bool {
    source
        .strip_prefix(&format!("{}/", spec.directory))
        .is_some_and(|name| {
            !name.is_empty()
                && !name.contains('/')
                && name.ends_with(&format!(".{}", spec.suffix))
                && !name.chars().any(char::is_control)
                && !name.contains('\\')
        })
}

fn known_import_source(source: &str, kind: precept_core::import::SourceKind) -> bool {
    if precept_core::import::FILES
        .iter()
        .any(|spec| spec.source == source && spec.kind == kind)
    {
        return true;
    }
    precept_core::import::DIRECTORIES
        .iter()
        .any(|spec| spec.kind == kind && known_directory_file(source, spec))
}

fn known_skip_source(source: &str) -> bool {
    precept_core::import::FILES
        .iter()
        .any(|spec| spec.source == source)
        || precept_core::import::DIRECTORIES
            .iter()
            .any(|spec| spec.directory == source || known_directory_file(source, spec))
}

impl ImportScan {
    /// Parses a daemon reply for `project`; new rules start picked.
    pub fn parse(project: &str, v: serde_json::Value) -> Option<(Self, Vec<usize>)> {
        let Ok(bytes) = serde_json::to_vec(&v) else {
            return None;
        };
        if bytes.len() > precept_core::import::MAX_REPLY_BYTES {
            return None;
        }
        let mut scan: Self = serde_json::from_value(v).ok()?;
        if scan.candidates.len() > precept_core::import::MAX_CANDIDATES
            || scan.skipped.len() > precept_core::import::MAX_SKIPPED
        {
            return None;
        }
        let scope = Scope::Project {
            name: project.to_owned(),
        };
        let mut keys = std::collections::HashSet::new();
        let mut semantics = std::collections::HashSet::new();
        for candidate in &scan.candidates {
            if !known_import_source(&candidate.source, candidate.kind)
                || candidate.scope != scope
                || candidate.key.is_empty()
                || candidate.key.chars().count() > 120
                || !keys.insert(candidate.key.as_str())
                || !semantics.insert((candidate.value.clone(), candidate.rule.clone()))
                || candidate.value.is_empty()
                || candidate.value.len() > precept_core::import::MAX_VALUE
                || candidate.title.is_empty()
                || candidate.title.chars().count() > 256
                || candidate
                    .description
                    .as_ref()
                    .is_some_and(|d| d.chars().count() > 256 || d.chars().any(char::is_control))
                || !candidate.rule.valid()
                || precept_core::operator::canonical_tags(&candidate.tags).as_ref()
                    != Some(&candidate.tags)
            {
                return None;
            }
        }
        for skipped in &scan.skipped {
            if !known_skip_source(&skipped.source)
                || skipped.title.chars().count() > 256
                || !matches!(
                    skipped.reason.as_str(),
                    "empty"
                        | "too_long"
                        | "unreadable"
                        | "malformed_frontmatter"
                        | "unsupported_metadata"
                        | "unsupported_applicability"
                        | "duplicate"
                        | "budget_exhausted"
                )
                || skipped.detail.as_ref().is_some_and(|d| {
                    d.is_empty() || d.chars().count() > 256 || d.chars().any(char::is_control)
                })
            {
                return None;
            }
        }
        scan.project = project.to_owned();
        let picked = scan
            .candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.exists)
            .map(|(i, _)| i)
            .take(precept_core::operator::MAX_BATCH)
            .collect();
        Some((scan, picked))
    }
}

pub(super) fn prepare(
    model: &Model,
    operation_id: &str,
) -> Option<(&'static str, Value, Option<Mutation>)> {
    let info = model.operator.info.as_ref()?;
    if !info.write_enabled {
        return None;
    }
    let binding = info.binding.clone()?;
    let scan = model.operator.import.as_ref()?;
    if model.selected_project.as_deref() != Some(scan.project.as_str()) {
        return None;
    }
    let items: Vec<precept_core::operator::SuggestItem> = model
        .operator
        .import_picked
        .iter()
        .filter_map(|i| scan.candidates.get(*i))
        .filter(|c| !c.exists)
        .map(|c| precept_core::operator::SuggestItem {
            scope: Scope::Project {
                name: scan.project.clone(),
            },
            key: c.key.clone(),
            value: c.value.clone(),
            tags: c.tags.clone(),
            origin: Origin::Operator,
            rule: c.rule.clone(),
        })
        .collect();
    if items.is_empty() || items.len() > precept_core::operator::MAX_BATCH {
        return None;
    }
    let mutation = Mutation {
        version: 2,
        binding,
        operation_id: operation_id.into(),
        command: Command::SuggestBatch { items },
    };
    Some(("profile.mutate", json!({}), Some(mutation)))
}

impl Operator {
    pub(super) fn clear_import(&mut self) {
        self.import = None;
        self.import_picked.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operator::tests::support::model;

    fn import_reply() -> Value {
        json!({
            "candidates":[{
                "source":".cursor/rules/style.mdc",
                "kind":"cursor",
                "title":"Style",
                "description":"Cursor style",
                "key":"cursor-cursor-rules-style-mdc",
                "value":"Use the shared style.",
                "tags":["cursor","imported"],
                "rule":{},
                "scope":{"kind":"project","name":"alpha"},
                "exists":false
            }],
            "skipped":[{
                "source":".cursor/rules/glob.mdc",
                "title":"Glob",
                "reason":"unsupported_applicability",
                "detail":"src/**/*.rs"
            }]
        })
    }

    #[test]
    fn import_reply_validates_sources_scope_and_suggestion_binding() {
        let (scan, picked) = ImportScan::parse("alpha", import_reply()).unwrap();
        assert_eq!(scan.project, "alpha");
        assert_eq!(picked, [0]);
        assert_eq!(
            scan.candidates[0].kind,
            precept_core::import::SourceKind::Cursor
        );
        assert_eq!(scan.skipped[0].detail.as_deref(), Some("src/**/*.rs"));
        assert!(
            ImportScan::parse(
                "alpha",
                Value::String("x".repeat(precept_core::import::MAX_REPLY_BYTES + 1)),
            )
            .is_none()
        );

        let mut wrong_scope = import_reply();
        wrong_scope["candidates"][0]["scope"] = json!({"kind":"project","name":"beta"});
        assert!(ImportScan::parse("alpha", wrong_scope).is_none());
        let mut wrong_kind = import_reply();
        wrong_kind["candidates"][0]["kind"] = json!("claude");
        assert!(ImportScan::parse("alpha", wrong_kind).is_none());
        let mut unknown_source = import_reply();
        unknown_source["candidates"][0]["source"] = json!("nested/AGENTS.md");
        assert!(ImportScan::parse("alpha", unknown_source).is_none());
        let mut unknown_reason = import_reply();
        unknown_reason["skipped"][0]["reason"] = json!("future_reason");
        assert!(ImportScan::parse("alpha", unknown_reason).is_none());
        let mut extra = import_reply();
        extra["candidates"][0]["root"] = json!("/synthetic/private");
        assert!(ImportScan::parse("alpha", extra).is_none());

        let mut duplicate = import_reply();
        duplicate["candidates"] = json!([
            duplicate["candidates"][0].clone(),
            duplicate["candidates"][0].clone()
        ]);
        assert!(ImportScan::parse("alpha", duplicate).is_none());
        let mut semantic_duplicate = import_reply();
        semantic_duplicate["candidates"] = json!([
            semantic_duplicate["candidates"][0].clone(),
            semantic_duplicate["candidates"][0].clone()
        ]);
        semantic_duplicate["candidates"][1]["key"] = json!("cursor-another-key");
        assert!(ImportScan::parse("alpha", semantic_duplicate).is_none());
        let mut too_many = import_reply();
        let mut candidates = Vec::new();
        for index in 0..=precept_core::import::MAX_CANDIDATES {
            let mut candidate = too_many["candidates"][0].clone();
            candidate["key"] = json!(format!("cursor-{index}"));
            candidates.push(candidate);
        }
        too_many["candidates"] = Value::Array(candidates);
        assert!(ImportScan::parse("alpha", too_many).is_none());

        let mut m = model();
        m.selected_project = Some("alpha".into());
        m.operator.import = Some(scan);
        m.operator.import_picked = vec![0];
        assert!(m.operator_review("import", "import-op"));
        let request = m.operator_confirm().unwrap();
        let Command::SuggestBatch { items } = &request.mutation.as_ref().unwrap().command else {
            panic!("expected SuggestBatch");
        };
        assert_eq!(items.len(), 1);
        assert!(
            items[0].scope
                == Scope::Project {
                    name: "alpha".into()
                }
        );
        assert_eq!(items[0].origin, Origin::Operator);
        assert_eq!(items[0].rule, Default::default());
    }
}
