//! Reads the project's AGENTS.md for an import: a regular file only (never a
//! link), bounded in size; anything else is reported as unreadable, never
//! followed. Nothing is written.
use precept_core::import::{self, Candidate, Skipped};
use std::{fs, path::Path};

/// The largest file read.
const MAX_FILE: u64 = 256 * 1024;

/// Every candidate and skipped section of `root`'s AGENTS.md.
pub fn scan(root: &Path) -> (Vec<Candidate>, Vec<Skipped>) {
    let mut out: (Vec<Candidate>, Vec<Skipped>) = Default::default();
    let path = root.join(import::FILE);
    let Ok(meta) = fs::symlink_metadata(&path) else {
        return out; // No AGENTS.md: nothing to import.
    };
    let text = (meta.file_type().is_file() && meta.len() <= MAX_FILE)
        .then(|| fs::read(&path).ok())
        .flatten()
        .and_then(|raw| String::from_utf8(raw).ok());
    match text {
        Some(text) => import::agents_md(import::FILE, &text, &mut out),
        None => out.1.push(Skipped {
            source: import::FILE.into(),
            title: String::new(),
            reason: "unreadable",
        }),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_agents_md_only_and_never_follows_a_link() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert_eq!(scan(root), (vec![], vec![]), "no AGENTS.md, nothing");
        fs::write(root.join("AGENTS.md"), "# A\n\n## Style\n\nShort lines.\n").unwrap();
        // Other files, and Precept's own rule folder, are never read.
        fs::write(root.join("CLAUDE.md"), "## Not read\n\nx\n").unwrap();
        fs::create_dir_all(root.join(".precept/rules")).unwrap();
        fs::write(root.join(".precept/rules/a.md"), "Not read.\n").unwrap();
        let (c, s) = scan(root);
        let keys: Vec<&str> = c.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["agents-style"]);
        assert!(s.is_empty());
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            fs::write(outside.path().join("AGENTS.md"), "## Secret\n\nx\n").unwrap();
            fs::remove_file(root.join("AGENTS.md")).unwrap();
            std::os::unix::fs::symlink(outside.path().join("AGENTS.md"), root.join("AGENTS.md"))
                .unwrap();
            let (c, s) = scan(root);
            assert!(c.is_empty());
            assert_eq!(s[0].reason, "unreadable");
        }
    }
}
