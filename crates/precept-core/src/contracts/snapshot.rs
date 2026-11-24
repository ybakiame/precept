//! One bounded read of the repository's rule files, and the checked reader
//! that rule checks share. Links are never followed; everything stays inside
//! the served root.
use super::parser::{DIR, MAX_BYTES, MAX_FILES, RuleFile, parse};
use sha2::{Digest, Sha256};
use std::{
    cell::OnceCell,
    collections::BTreeSet,
    fs,
    io::Read as _,
    path::{Path, PathBuf},
};

#[derive(Clone, PartialEq, Eq)]
enum Read {
    Absent,
    /// `(id, bytes)` per `*.md` file, sorted by id.
    Files(Vec<(String, Vec<u8>)>),
    Invalid,
    Unavailable,
}
pub struct Snapshot {
    root: Option<PathBuf>,
    read: Read,
    /// Ids whose file Git tracks with no uncommitted change; `None` when Git
    /// could not tell (every rule is then unreviewed).
    reviewed: Option<BTreeSet<String>>,
    parsed: OnceCell<Vec<Option<(RuleFile, String)>>>,
}
impl Snapshot {
    pub fn read(root: &Path) -> Self {
        let canonical = root.canonicalize().ok();
        let read = match &canonical {
            None => Read::Unavailable,
            Some(c) => read(c),
        };
        Self {
            root: canonical,
            read,
            reviewed: None,
            parsed: OnceCell::new(),
        }
    }
    /// Record which rule ids Git reports as committed and unchanged.
    pub fn with_reviewed(mut self, reviewed: Option<BTreeSet<String>>) -> Self {
        self.reviewed = reviewed;
        self
    }
    pub fn status(&self) -> &'static str {
        match &self.read {
            Read::Absent => "not_configured",
            Read::Files(_) if self.invalid().is_empty() => "ready",
            Read::Files(_) | Read::Invalid => "invalid_source",
            Read::Unavailable => "source_unavailable",
        }
    }
    /// Ids of rule files that do not parse (named so the operator can fix them).
    pub fn invalid(&self) -> Vec<String> {
        match &self.read {
            Read::Files(files) => files
                .iter()
                .zip(self.parsed())
                .filter(|(_, parsed)| parsed.is_none())
                .map(|(id, _)| id.0.clone())
                .collect(),
            _ => vec![],
        }
    }
    fn parsed(&self) -> &[Option<(RuleFile, String)>] {
        self.parsed.get_or_init(|| match &self.read {
            Read::Files(files) => files
                .iter()
                .map(|(id, raw)| {
                    parse(id, raw)
                        .ok()
                        .map(|r| (r, format!("{:x}", Sha256::digest(raw))))
                })
                .collect(),
            _ => vec![],
        })
    }
    /// Every rule with its file digest, only when all files parse.
    pub(super) fn rules(&self) -> Option<Vec<(RuleFile, String)>> {
        match &self.read {
            Read::Files(_) => self.parsed().iter().cloned().collect::<Option<Vec<_>>>(),
            _ => None,
        }
    }
    pub(super) fn reviewed(&self, id: &str) -> bool {
        self.reviewed.as_ref().is_some_and(|s| s.contains(id))
    }
    pub(super) fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }
    pub fn unchanged(&self, root: &Path) -> bool {
        let current = Self::read(root);
        self.root.is_some() && self.root == current.root && self.read == current.read
    }
}

/// The file names under [`DIR`] that are rule files (`<id>.md`).
fn read(root: &Path) -> Read {
    let mut dir = root.to_path_buf();
    for part in DIR.split('/') {
        dir.push(part);
        match fs::symlink_metadata(&dir) {
            Ok(m) if m.is_dir() && !link(&m) => {}
            Ok(_) => return Read::Invalid,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Read::Absent,
            Err(_) => return Read::Unavailable,
        }
    }
    let Ok(entries) = fs::read_dir(&dir) else {
        return Read::Unavailable;
    };
    let mut files = vec![];
    for entry in entries {
        let Ok(entry) = entry else {
            return Read::Unavailable;
        };
        let name = entry.file_name();
        let Some(id) = name.to_str().and_then(|n| n.strip_suffix(".md")) else {
            continue;
        };
        if files.len() >= MAX_FILES {
            return Read::Invalid;
        }
        match read_file(&dir.join(&name), MAX_BYTES) {
            Ok(Some(raw)) => files.push((id.to_owned(), raw)),
            // A link or oversized file: kept empty, so it is named as invalid.
            Ok(None) => files.push((id.to_owned(), vec![])),
            Err(()) => return Read::Unavailable,
        }
    }
    files.sort();
    Read::Files(files)
}

fn link(m: &fs::Metadata) -> bool {
    if m.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

/// A regular, unlinked file of at most `max` bytes: `Ok(None)` when it is
/// not one (a link, a directory, too large), `Err` when it cannot be read.
fn read_file(path: &Path, max: usize) -> Result<Option<Vec<u8>>, ()> {
    let meta = fs::symlink_metadata(path).map_err(|_| ())?;
    if !meta.is_file() || link(&meta) || meta.len() > max as u64 {
        return Ok(None);
    }
    let file = fs::File::open(path).map_err(|_| ())?;
    let mut raw = vec![];
    file.take(max as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|_| ())?;
    Ok((raw.len() <= max).then_some(raw))
}

pub(crate) enum InsideRead {
    Bytes(Vec<u8>),
    Absent,
    BudgetExceeded,
}

/// A checked read that cannot consume more than the remaining aggregate byte
/// budget. Metadata races may read one byte beyond it to detect growth.
pub(crate) fn read_inside_budget(
    root: &Path,
    relative: &str,
    max: usize,
    remaining: usize,
) -> Result<InsideRead, ()> {
    let mut path = root.to_path_buf();
    let parts: Vec<&str> = relative.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        path.push(part);
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(InsideRead::Absent),
            Err(_) => return Err(()),
        };
        if link(&meta) {
            return Ok(InsideRead::Absent);
        }
        if i + 1 < parts.len() && !meta.is_dir() {
            return Ok(InsideRead::Absent);
        }
    }
    let meta = fs::symlink_metadata(&path).map_err(|_| ())?;
    if !meta.is_file() || link(&meta) || meta.len() > max as u64 {
        return Ok(InsideRead::Absent);
    }
    if meta.len() > remaining as u64 {
        return Ok(InsideRead::BudgetExceeded);
    }
    let file = fs::File::open(path).map_err(|_| ())?;
    let mut raw = vec![];
    file.take(remaining as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|_| ())?;
    if raw.len() > remaining || raw.len() > max {
        return Ok(InsideRead::BudgetExceeded);
    }
    Ok(InsideRead::Bytes(raw))
}
