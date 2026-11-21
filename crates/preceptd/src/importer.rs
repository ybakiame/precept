//! Reads the files an import looks at in a served project's root: the exact
//! instruction files and the rule folders of [`precept_core::import`]. Only
//! regular files (never links), bounded in size and number; anything else is
//! reported as unreadable, never followed. Nothing is written.
use precept_core::import::{self, Candidate, Format, Skipped};
use std::{fs, path::Path};

/// The largest file read.
const MAX_FILE: u64 = 256 * 1024;
/// The most files read from one rule folder.
const MAX_FOLDER: usize = 100;

fn read(path: &Path) -> Option<String> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_file() || meta.len() > MAX_FILE {
        return None;
    }
    String::from_utf8(fs::read(path).ok()?).ok()
}

/// Every candidate and skipped part in `root`'s instruction files and rule
/// folders, in a stable order.
pub fn scan(root: &Path) -> (Vec<Candidate>, Vec<Skipped>) {
    let mut out: (Vec<Candidate>, Vec<Skipped>) = Default::default();
    let file = |relative: &str, format: Format, out: &mut (Vec<Candidate>, Vec<Skipped>)| {
        let path = root.join(relative);
        if fs::symlink_metadata(&path).is_err() {
            return;
        }
        match read(&path) {
            Some(text) => import::parse(relative, format, &text, out),
            None => out.1.push(Skipped {
                source: relative.into(),
                title: String::new(),
                reason: "unreadable",
            }),
        }
    };
    for (relative, format) in import::FILES {
        file(relative, format, &mut out);
    }
    for (folder, extension, format) in import::DIRECTORIES {
        let dir = root.join(folder);
        // A real directory only: a link is never followed.
        if !fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_dir()) {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| {
                Path::new(n).extension().and_then(|e| e.to_str()) == Some(extension)
                    && !n.starts_with('.')
            })
            .collect();
        names.sort();
        for name in names.into_iter().take(MAX_FOLDER) {
            file(&format!("{folder}/{name}"), format, &mut out);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_known_files_and_folders_and_never_follows_links() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("AGENTS.md"), "# A\n\n## Style\n\nShort lines.\n").unwrap();
        fs::create_dir_all(root.join(".cursor/rules")).unwrap();
        fs::write(root.join(".cursor/rules/b.mdc"), "Use tabs.\n").unwrap();
        fs::write(root.join(".cursor/rules/a.mdc"), "Use spaces.\n").unwrap();
        fs::write(root.join(".cursor/rules/notes.txt"), "ignored").unwrap();
        fs::write(root.join("README.md"), "## Not read\n\nx\n").unwrap();
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            fs::write(outside.path().join("CLAUDE.md"), "## Secret\n\nx\n").unwrap();
            std::os::unix::fs::symlink(outside.path().join("CLAUDE.md"), root.join("CLAUDE.md"))
                .unwrap();
        }
        let (c, s) = scan(root);
        let keys: Vec<&str> = c.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["agents-style", "cursor-a", "cursor-b"]);
        #[cfg(unix)]
        assert_eq!(s[0].source, "CLAUDE.md");
        #[cfg(unix)]
        assert_eq!(s[0].reason, "unreadable");
    }
}
