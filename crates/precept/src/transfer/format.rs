use precept_ipc::{Error, Result, operator::*};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{io::Write, path::Path};

pub const FORMAT: &str = "precept-profile-export";
/// The version written. Version 1 files (no rule options) still import, with
/// the defaults (everywhere, priority 0, advisory); version 3 may carry
/// project memory.
pub const VERSION: u32 = 3;
/// The most preferences one export or import carries.
pub const MAX_RECORDS: usize = 10_000;
/// The largest export file read back.
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exported {
    pub scope: Scope,
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub origin: Origin,
    /// The status in the source Profile (`confirmed`, `suggested`, ...).
    pub status: String,
    /// Where it applies, priority, declared enforcement (v2).
    #[serde(default)]
    pub rule: RuleOptions,
    /// Whether it was switched on in the source Profile (v2; informative).
    #[serde(default = "on")]
    pub enabled: bool,
}
fn on() -> bool {
    true
}

/// One project-memory note (v3).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportedNote {
    pub project: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub pinned: bool,
}

/// Which project memory an export carries.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MemoryChoice {
    None,
    /// Cautions, lessons and attempts.
    Notes,
    /// Those and this machine's environment notes.
    WithEnvironment,
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub format: String,
    pub version: u32,
    pub preferences: Vec<Exported>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memory: Vec<ExportedNote>,
    /// Indexes of records in the file whose scope no longer exists
    /// (`environment`: machine facts are project memory now); skipped.
    #[serde(skip)]
    pub dropped: Vec<usize>,
}

/// Writes `export` to a new private file (never overwrites).
pub fn write_new(path: &Path, export: &Export) -> Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => Error::Conflict,
        _ => Error::Unavailable,
    })?;
    let raw = serde_json::to_vec_pretty(export).map_err(|_| Error::Unavailable)?;
    file.write_all(&raw).map_err(|_| Error::Unavailable)?;
    file.sync_all().map_err(|_| Error::Unavailable)
}

/// Reads and validates an export file.
pub fn read(path: &Path) -> Result<Export> {
    let meta = std::fs::metadata(path).map_err(|_| Error::NotFound)?;
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Err(Error::InvalidRequest);
    }
    let raw = std::fs::read(path).map_err(|_| Error::Unavailable)?;
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct File {
        format: String,
        version: u32,
        preferences: Vec<Value>,
        #[serde(default)]
        memory: Vec<ExportedNote>,
    }
    let file: File = serde_json::from_slice(&raw).map_err(|_| Error::InvalidRequest)?;
    if file.format != FORMAT
        || !(1..=VERSION).contains(&file.version)
        || file.preferences.len() > MAX_RECORDS
        || file.memory.len() > MAX_RECORDS
    {
        return Err(Error::Version);
    }
    let mut preferences = Vec::with_capacity(file.preferences.len());
    let mut dropped = vec![];
    for (index, p) in file.preferences.into_iter().enumerate() {
        if p.pointer("/scope/kind") == Some(&json!("environment")) {
            dropped.push(index);
            continue;
        }
        preferences.push(serde_json::from_value(p).map_err(|_| Error::InvalidRequest)?);
    }
    Ok(Export {
        format: file.format,
        version: file.version,
        preferences,
        memory: file.memory,
        dropped,
    })
}

/// Whether `prefix` makes valid operation ids (`<prefix>-<index>`).
pub fn valid_prefix(prefix: &str) -> bool {
    !prefix.is_empty()
        && prefix.len() <= 40
        && prefix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

pub(super) fn file_indexes(export: &Export) -> Vec<usize> {
    let dropped: std::collections::BTreeSet<_> = export.dropped.iter().copied().collect();
    (0..export.preferences.len() + dropped.len())
        .filter(|index| !dropped.contains(index))
        .collect()
}

pub(super) fn validate_export(export: &Export) -> Result<()> {
    if export.format != FORMAT || !(1..=VERSION).contains(&export.version) {
        return Err(Error::Version);
    }
    if export.preferences.len() > MAX_RECORDS || export.memory.len() > MAX_RECORDS {
        return Err(Error::TooLarge);
    }
    Ok(())
}
