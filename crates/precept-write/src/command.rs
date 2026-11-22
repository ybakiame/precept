//! Typed synthetic requests; these DTOs convey no operator authorization.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

pub use precept_core::operator::{Command, Expected, Scope, Snapshot};
pub use precept_store::profile::{Binding, Receipt, RecordVersion};
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub binding: Binding,
    pub operation_id: String,
    pub command: Command,
}
// Never derive Debug for content-bearing requests or their command/scope fields.
impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Request(redacted)")
    }
}
impl Request {
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 128 * 1024 {
            return Err(Error::InvalidCommand);
        }
        let request: Self = serde_json::from_slice(bytes).map_err(|_| Error::InvalidCommand)?;
        request.validate()?;
        Ok(request)
    }
    pub(crate) fn validate(&self) -> Result<()> {
        fn hex(s: &str) -> bool {
            s.len() == 32
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }
        // Canonical lowercase SHA-256 hex; the writer byte-compares it later.
        fn digest(s: &str) -> bool {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        }
        if self.version != 2
            || !hex(&self.binding.profile_id)
            || !hex(&self.binding.generation)
            || self.binding.owner_epoch < 0
            || self.operation_id.is_empty()
            || self.operation_id.len() > 64
            || !self
                .operation_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err(Error::InvalidCommand);
        }
        let (expected, scope) = match &self.command {
            Command::RegisterProject {
                name,
                root,
                expected,
            } => {
                if !name_valid(name)
                    || !std::path::Path::new(root).is_absolute()
                    || root.contains('\0')
                    || *expected != Expected::Absent
                {
                    return Err(Error::InvalidCommand);
                }
                (expected, None)
            }
            Command::SuggestPreference {
                scope,
                key,
                value,
                tags,
                rule,
                expected,
                // origin is a closed serde enum; no additional domain check.
                ..
            } => {
                // Tags arrive canonical; the writer never rewrites a proposal.
                if !text(key, 120, true)
                    || !text(value, 4096, true)
                    || !precept_core::operator::tags_canonical(tags)
                    || !rule.valid()
                    || *expected != Expected::Absent
                {
                    return Err(Error::InvalidCommand);
                }
                (expected, Some(scope))
            }
            Command::SetPreferenceEnabled {
                id,
                scope,
                expected,
                ..
            } => {
                if *id <= 0 || !matches!(expected, Expected::Revision(n) if *n>0) {
                    return Err(Error::InvalidCommand);
                }
                (expected, Some(scope))
            }
            Command::ConfirmPreference {
                id,
                scope,
                proposal_digest,
                expected,
                ..
            }
            | Command::RejectPreference {
                id,
                scope,
                proposal_digest,
                expected,
                ..
            }
            | Command::DeprecatePreference {
                id,
                scope,
                proposal_digest,
                expected,
                ..
            }
            | Command::DeletePreference {
                id,
                scope,
                proposal_digest,
                expected,
                ..
            } => {
                if *id <= 0
                    || !digest(proposal_digest)
                    || !matches!(expected, Expected::Revision(n) if *n>0)
                {
                    return Err(Error::InvalidCommand);
                }
                (expected, Some(scope))
            }
            Command::RevisePreference {
                id,
                scope,
                proposal_digest,
                key,
                value,
                tags,
                rule,
                expected,
                ..
            } => {
                if *id <= 0
                    || !digest(proposal_digest)
                    || !matches!(expected, Expected::Revision(n) if *n>0)
                    || !text(key, 120, true)
                    || !text(value, 4096, true)
                    || !precept_core::operator::tags_canonical(tags)
                    || !rule.valid()
                {
                    return Err(Error::InvalidCommand);
                }
                (expected, Some(scope))
            }
            Command::SuggestBatch { items } => {
                if items.is_empty()
                    || items.len() > precept_core::operator::MAX_BATCH
                    || items.iter().any(|i| {
                        !text(&i.key, 120, true)
                            || !text(&i.value, 4096, true)
                            || !precept_core::operator::tags_canonical(&i.tags)
                            || !i.rule.valid()
                            || matches!(&i.scope, Scope::Project { name } if !name_valid(name))
                    })
                {
                    return Err(Error::InvalidCommand);
                }
                (&Expected::Absent, Some(&items[0].scope))
            }
            Command::DecideBatch { items, .. } => {
                let mut ids = std::collections::BTreeSet::new();
                if items.is_empty()
                    || items.len() > precept_core::operator::MAX_BATCH
                    || items.iter().any(|i| {
                        i.id <= 0
                            || !ids.insert(i.id)
                            || !digest(&i.proposal_digest)
                            || !matches!(i.expected, Expected::Revision(n) if n > 0)
                            || matches!(&i.scope, Scope::Project { name } if !name_valid(name))
                    })
                {
                    return Err(Error::InvalidCommand);
                }
                (&items[0].expected, Some(&items[0].scope))
            }
            Command::PutHandoff {
                project,
                task_id,
                objective,
                completed,
                next_action,
                validation_json,
                expected,
                snapshot,
            } => {
                if !precept_store::profile::snapshot_valid(
                    &snapshot.base_sha,
                    &snapshot.branch,
                    &snapshot.state,
                    snapshot.fingerprint.as_deref(),
                ) || !name_valid(project)
                    || !text(task_id, 120, true)
                    || !text(objective, 4000, true)
                    || !text(completed, 8000, false)
                    || !text(next_action, 4000, true)
                    || !validation_valid(validation_json)
                {
                    return Err(Error::InvalidCommand);
                }
                (expected, None)
            }
        };
        if let Some(note) = self.command.note()
            && precept_core::operator::canonical_note(Some(note)) != Some(Some(note.to_owned()))
        {
            return Err(Error::InvalidCommand);
        }
        if matches!(expected, Expected::Revision(n) if *n<=0)
            || matches!(scope, Some(Scope::Project{name}) if !name_valid(name))
        {
            return Err(Error::InvalidCommand);
        }
        if serde_json::to_vec(self)
            .map_err(|_| Error::InvalidCommand)?
            .len()
            > 128 * 1024
        {
            return Err(Error::InvalidCommand);
        }
        Ok(())
    }
}
use precept_store::profile::{name_valid, text, validation_valid};
