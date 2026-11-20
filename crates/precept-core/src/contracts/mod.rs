//! Bounded repository claims and a privacy-preserving adapter to frozen typed v1.
//! No writer, authorization, discovery, or prose interpretation lives here.
mod bridge;
mod parser;
mod snapshot;

pub use bridge::{Projection, SelectorQuery, project};
pub use parser::{InvalidSource, MAX_BYTES, SOURCE, parse};
pub use snapshot::Snapshot;

use crate::typed_context as typed;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    id: String,
    revision: u64,
    review: RepositoryReview,
    key: String,
    allowed_values: Vec<String>,
    #[serde(default)]
    selector: RepositorySelector,
    #[serde(default)]
    supersedes: Vec<typed::RuleRef>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RepositoryReview {
    Accepted,
    Unreviewed,
}
#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositorySelector {
    agents: Option<Vec<String>>,
    paths: Option<Vec<String>>,
    languages: Option<Vec<String>>,
    task_tags: Option<Vec<String>>,
}

fn binding(project: &str) -> typed::ProjectBinding {
    typed::ProjectBinding {
        project: project.into(),
        root_id: "authorized-request-root".into(),
    }
}
fn source(id: &str, digest: &str) -> typed::Source {
    typed::Source {
        reference: typed::SourceRef {
            id: id.into(),
            version: format!("sha256:{digest}"),
            digest: Some(digest.into()),
        },
        state: typed::SourceState::Fresh,
        provenance: typed::Provenance {
            origin: typed::Origin::DaemonObserved,
            references: vec![],
            historical_validation: vec![],
        },
    }
}
impl Contract {
    fn rule(&self, project: &str, digest: &str) -> typed::Rule {
        typed::Rule {
            schema_version: 1,
            reference: typed::RuleRef {
                id: self.id.clone(),
                revision: self.revision,
            },
            kind: typed::RuleKind::Contract,
            scope: typed::Scope::Project {
                binding: binding(project),
            },
            authority: typed::Authority::Repository,
            review: match self.review {
                RepositoryReview::Accepted => typed::ReviewState::Accepted,
                RepositoryReview::Unreviewed => typed::ReviewState::Unreviewed,
            },
            selector: typed::Selector {
                agents: self.selector.agents.clone(),
                paths: self.selector.paths.clone(),
                languages: self.selector.languages.clone(),
                task_tags: self.selector.task_tags.clone(),
                projects: None,
            },
            key: self.key.clone(),
            allowed_values: self.allowed_values.clone(),
            legacy_preference_id: None,
            supersedes: self.supersedes.clone(),
            source: source(SOURCE, digest),
        }
    }
}
