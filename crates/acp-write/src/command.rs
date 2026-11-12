//! Typed synthetic requests; these DTOs convey no operator authorization.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub profile_id: String,
    pub generation: String,
    pub owner_epoch: i64,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "revision",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Expected {
    Absent,
    Revision(i64),
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Scope {
    Global,
    Project { name: String },
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureRoot {
    Alpha,
    Beta,
    Gamma,
}
impl FixtureRoot {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Alpha => "alpha",
            Self::Beta => "beta",
            Self::Gamma => "gamma",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Snapshot {
    Clean,
    Dirty,
    UnknownFingerprint,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    RegisterProject {
        name: String,
        root: FixtureRoot,
        expected: Expected,
    },
    SuggestPreference {
        scope: Scope,
        key: String,
        value: String,
        source: String,
        expected: Expected,
    },
    ConfirmPreference {
        id: i64,
        scope: Scope,
        expected: Expected,
    },
    RejectPreference {
        id: i64,
        scope: Scope,
        expected: Expected,
    },
    DeprecatePreference {
        id: i64,
        scope: Scope,
        expected: Expected,
    },
    DeletePreference {
        id: i64,
        scope: Scope,
        expected: Expected,
    },
    PutHandoff {
        project: String,
        task_id: String,
        objective: String,
        completed: String,
        next_action: String,
        validation_json: String,
        snapshot: Snapshot,
        expected: Expected,
    },
}
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
        if self.version != 1
            || !hex(&self.binding.profile_id)
            || !hex(&self.binding.generation)
            || self.binding.owner_epoch <= 0
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
            Command::RegisterProject { name, expected, .. } => {
                if !name_valid(name) || *expected != Expected::Absent {
                    return Err(Error::InvalidCommand);
                }
                (expected, None)
            }
            Command::SuggestPreference {
                scope,
                key,
                value,
                source,
                expected,
            } => {
                if !text(key, 120, true)
                    || !text(value, 4096, true)
                    || !text(source, 512, true)
                    || *expected != Expected::Absent
                {
                    return Err(Error::InvalidCommand);
                }
                (expected, Some(scope))
            }
            Command::ConfirmPreference {
                id,
                scope,
                expected,
            }
            | Command::RejectPreference {
                id,
                scope,
                expected,
            }
            | Command::DeprecatePreference {
                id,
                scope,
                expected,
            }
            | Command::DeletePreference {
                id,
                scope,
                expected,
            } => {
                if *id <= 0 || !matches!(expected, Expected::Revision(n) if *n>0) {
                    return Err(Error::InvalidCommand);
                }
                (expected, Some(scope))
            }
            Command::PutHandoff {
                project,
                task_id,
                objective,
                completed,
                next_action,
                validation_json,
                expected,
                ..
            } => {
                if !name_valid(project)
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
pub(crate) fn name_valid(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s != "."
        && s != ".."
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
fn text(s: &str, max: usize, required: bool) -> bool {
    !s.contains('\0')
        && s.chars().count() <= max
        && (!required || s.chars().any(|c| !c.is_whitespace() && !c.is_control()))
}
pub(crate) fn validation_valid(raw: &str) -> bool {
    if !text(raw, 16000, true) {
        return false;
    }
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str(raw) else {
        return false;
    };
    items.iter().all(|item| {
        item.as_object().is_some_and(|obj| {
            obj.get("check").is_some_and(serde_json::Value::is_string)
                && matches!(
                    obj.get("status").and_then(serde_json::Value::as_str),
                    Some("PASS" | "FAIL" | "SKIPPED" | "NOT_RUN")
                )
        })
    })
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordVersion {
    pub kind: String,
    pub project: String,
    pub record_id: String,
    pub revision: i64,
    pub deleted: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub generation: String,
    pub operation_id: String,
    pub commit_sequence: i64,
    pub affected: Vec<RecordVersion>,
}
