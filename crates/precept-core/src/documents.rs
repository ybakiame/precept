//! Live allowlisted documents; bounded reads even if a file grows after stat.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

pub const DOCUMENTS: [&str; 5] = [
    "AGENTS.md",
    "README.md",
    "docs/index.md",
    "docs/README.md",
    "docs/adr/README.md",
];
pub const MAX_DOC_BYTES: u64 = 16384;

pub fn metadata(root: &Path, include_content: bool) -> Vec<Value> {
    let mut result = Vec::new();
    let canonical_root = root.canonicalize();
    for relative in DOCUMENTS {
        let path = root.join(relative);
        if !path.is_file() {
            continue;
        }
        let unavailable = || json!({"path":relative,"status":"unavailable_or_outside_root"});
        let entry = (|| {
            let resolved = path.canonicalize().ok()?;
            if !resolved.starts_with(canonical_root.as_ref().ok()?) { return None; }
            let file = File::open(&resolved).ok()?;
            let size = file.metadata().ok()?.len();
            if size > MAX_DOC_BYTES { return Some(json!({"path":relative,"status":"too_large","bytes":size})); }
            let mut raw = Vec::new();
            file.take(MAX_DOC_BYTES + 1).read_to_end(&mut raw).ok()?;
            if raw.len() as u64 > MAX_DOC_BYTES { return Some(json!({"path":relative,"status":"too_large","bytes":raw.len()})); }
            let mut item = json!({"path":relative,"status":"available","bytes":raw.len(),"sha256":format!("{:x}", Sha256::digest(&raw))});
            if include_content { item["content"] = json!(String::from_utf8_lossy(&raw)); }
            Some(item)
        })().unwrap_or_else(unavailable);
        result.push(entry);
    }
    result
}

/// Optional filesystem adapter. It reads only DOCUMENTS and never scans directories.
/// Canonical checks are best-effort and do not defend against hostile same-user races.
pub struct FileDocuments;
impl crate::DocumentRepository for FileDocuments {
    fn metadata(
        &self,
        project: &crate::Project,
        include_content: bool,
    ) -> crate::Result<Vec<Value>> {
        Ok(metadata(&project.root, include_content))
    }
}
