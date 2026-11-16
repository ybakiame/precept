use super::{Contract, MAX_BYTES, SOURCE, parse};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read as _,
    path::{Path, PathBuf},
};

#[derive(Clone, PartialEq, Eq)]
enum Read {
    Absent,
    Bytes(Vec<u8>),
    Invalid,
    Unavailable,
}
/// Exact bounded source snapshot. Fields intentionally have no Debug/raw projection.
pub struct Snapshot {
    root: Option<PathBuf>,
    read: Read,
}
impl Snapshot {
    pub fn read(root: &Path) -> Self {
        let canonical = root.canonicalize().ok();
        let read = if canonical.is_none() {
            Read::Unavailable
        } else {
            read(root)
        };
        Self {
            root: canonical,
            read,
        }
    }
    pub fn status(&self) -> &'static str {
        match &self.read {
            Read::Absent => "not_configured",
            Read::Bytes(raw) if parse(raw).is_ok() => "ready",
            Read::Bytes(_) | Read::Invalid => "invalid_source",
            Read::Unavailable => "source_unavailable",
        }
    }
    pub(super) fn contracts(&self) -> Option<(Vec<Contract>, String)> {
        match &self.read {
            Read::Bytes(raw) => parse(raw)
                .ok()
                .map(|v| (v, format!("{:x}", Sha256::digest(raw)))),
            _ => None,
        }
    }
    /// No old-source fallback or retry. Compares exact bytes, presence and root.
    pub fn unchanged(&self, root: &Path) -> bool {
        let current = Self::read(root);
        self.root.is_some() && self.root == current.root && self.read == current.read
    }
}
fn read(root: &Path) -> Read {
    let path = root.join(SOURCE);
    let meta = match fs::symlink_metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Read::Absent,
        Err(_) => return Read::Unavailable,
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Read::Invalid;
        }
    }
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_BYTES as u64 {
        return Read::Invalid;
    }
    let Ok(canonical_root) = root.canonicalize() else {
        return Read::Unavailable;
    };
    let Ok(resolved) = path.canonicalize() else {
        return Read::Unavailable;
    };
    if resolved.parent() != Some(canonical_root.as_path()) {
        return Read::Invalid;
    }
    let Ok(file) = fs::File::open(&path) else {
        return Read::Unavailable;
    };
    if !file
        .metadata()
        .is_ok_and(|m| m.is_file() && m.len() <= MAX_BYTES as u64)
    {
        return Read::Invalid;
    }
    let mut raw = vec![];
    if file
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut raw)
        .is_err()
    {
        return Read::Unavailable;
    }
    if raw.len() > MAX_BYTES {
        return Read::Invalid;
    }
    Read::Bytes(raw)
}
