//! Repository rules (`.precept/rules/*.md`): bounded parsing, checks against
//! the repository, and their projection beside the Profile's preferences.
//! No writer, authorization or discovery lives here.
mod bridge;
mod checks;
mod parser;
mod snapshot;

pub use bridge::{Projection, SelectorQuery, project};
pub use checks::{MAX_CHECKED_BYTES, evaluate};
pub use parser::{
    Check, DIR, Guard, GuardTool, InvalidSource, MAX_BYTES, MAX_FILES, RuleFile, id_for, parse,
    relative_path, render, valid_id, valid_key,
};
pub use snapshot::Snapshot;

use crate::typed_context as typed;

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
/// The repository path of rule `id`.
pub fn file_of(id: &str) -> String {
    format!("{DIR}/{id}.md")
}
impl RuleFile {
    /// The typed contract of a rule with allowed values (applicability is
    /// decided before resolution, so the typed selector matches everything).
    fn typed(&self, project: &str, digest: &str) -> typed::Rule {
        typed::Rule {
            schema_version: 1,
            reference: typed::RuleRef {
                id: self.id.clone(),
                revision: 1,
            },
            kind: typed::RuleKind::Contract,
            scope: typed::Scope::Project {
                binding: binding(project),
            },
            authority: typed::Authority::Repository,
            review: typed::ReviewState::Accepted,
            selector: typed::Selector::default(),
            key: self.key.clone(),
            allowed_values: self.allowed.clone(),
            legacy_preference_id: None,
            supersedes: vec![],
            source: source(&file_of(&self.id), digest),
        }
    }
}
