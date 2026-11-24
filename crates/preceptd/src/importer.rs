//! Reads only the fixed agent-rule paths of one already registered project.
//! Every file is bounded and opened through the same no-follow adapter as
//! worktree fingerprints. Nothing here writes or follows repository links.
use precept_core::{
    import::{self, Candidate, DIRECTORIES, DirectorySpec, FILES, Scan, Skipped, SourceSpec},
    repo_fs::{FileRead, OpenError, Repository, RootIdentity},
};
use std::path::Path;

const MAX_FILE: u64 = 256 * 1024;
const MAX_TOTAL_BYTES: u64 = 1024 * 1024;
const MAX_DIR_ENTRIES: usize = 64;
const MAX_DIR_FILES: usize = 16;

#[derive(Debug, PartialEq, Eq)]
enum FileState {
    Text(String),
    Missing,
    Unsafe,
    Budget,
}

fn read_file(repository: &Repository, source: &str, remaining: &mut u64) -> FileState {
    let limit = (*remaining).min(MAX_FILE) as usize;
    match repository.read_file(Path::new(source), MAX_FILE as usize, limit) {
        Ok(bytes) => {
            *remaining -= bytes.len() as u64;
            match String::from_utf8(bytes) {
                Ok(text) => FileState::Text(text),
                Err(_) => FileState::Unsafe,
            }
        }
        Err(FileRead::Missing) => FileState::Missing,
        Err(FileRead::BudgetExceeded) => FileState::Budget,
        Err(FileRead::Unsafe | FileRead::TooLarge | FileRead::Unavailable) => FileState::Unsafe,
    }
}

fn skip(scan: &mut Scan, source: &str, title: &str, reason: &'static str) {
    scan.skip(Skipped {
        source: source.into(),
        title: title.into(),
        reason,
        detail: None,
    });
}

fn read_source(
    root: &Repository,
    source: &str,
    spec: SourceSpec,
    scan: &mut Scan,
    remaining: &mut u64,
    listed: bool,
) -> bool {
    match read_file(root, source, remaining) {
        FileState::Text(text) => import::parse(source, spec, &text, scan),
        FileState::Missing if listed => skip(scan, source, "", "unreadable"),
        FileState::Missing => {}
        FileState::Unsafe => skip(scan, source, "", "unreadable"),
        FileState::Budget => {
            skip(scan, source, "", "budget_exhausted");
            return false;
        }
    }
    if *remaining == 0 {
        skip(scan, source, "", "budget_exhausted");
        return false;
    }
    !scan.exhausted()
}

fn matching_files(root: &Repository, spec: DirectorySpec, scan: &mut Scan) -> Option<Vec<String>> {
    let entries = match root.entries(
        Path::new(spec.directory),
        MAX_DIR_ENTRIES,
        MAX_DIR_ENTRIES,
        |_| true,
    ) {
        Ok(entries) => entries,
        Err(OpenError::Missing) => return None,
        Err(OpenError::TooMany) => {
            skip(scan, spec.directory, "", "budget_exhausted");
            return None;
        }
        Err(_) => {
            skip(scan, spec.directory, "", "unreadable");
            return None;
        }
    };
    let mut names = Vec::new();
    for entry in entries {
        let Some(name) = entry.name.to_str() else {
            skip(scan, spec.directory, "", "unreadable");
            return None;
        };
        if name.chars().any(char::is_control) || name.contains('\\') {
            skip(scan, spec.directory, "", "unreadable");
            return None;
        }
        if name.ends_with(&format!(".{}", spec.suffix)) {
            names.push(name.to_owned());
        }
    }
    names.sort();
    if names.len() > MAX_DIR_FILES {
        skip(scan, spec.directory, "", "budget_exhausted");
        names.truncate(MAX_DIR_FILES);
    }
    Some(
        names
            .into_iter()
            .map(|name| format!("{}/{}", spec.directory, name))
            .collect(),
    )
}

/// Every candidate and skipped source in the registered project's fixed inventory.
#[cfg(test)]
pub fn scan(root: &Path) -> (Vec<Candidate>, Vec<Skipped>) {
    let Ok(repository) = Repository::open(root) else {
        return Scan::default().into_parts();
    };
    scan_repository(&repository)
}

pub fn scan_expected(
    root: &Path,
    identity: &RootIdentity,
) -> Option<(Vec<Candidate>, Vec<Skipped>)> {
    let repository = Repository::open_expected(root, identity).ok()?;
    Some(scan_repository(&repository))
}

fn scan_repository(repository: &Repository) -> (Vec<Candidate>, Vec<Skipped>) {
    let mut scan = Scan::default();
    let mut remaining = MAX_TOTAL_BYTES;
    for spec in FILES {
        if scan.exhausted() || remaining == 0 {
            break;
        }
        if !read_source(
            repository,
            spec.source,
            spec,
            &mut scan,
            &mut remaining,
            false,
        ) {
            break;
        }
    }
    for directory_spec in DIRECTORIES {
        if scan.exhausted() || remaining == 0 {
            break;
        }
        let Some(files) = matching_files(repository, directory_spec, &mut scan) else {
            continue;
        };
        for source in files {
            if scan.exhausted() || remaining == 0 {
                break;
            }
            let spec = SourceSpec {
                source: directory_spec.directory,
                kind: directory_spec.kind,
                sections: false,
                frontmatter: Some(directory_spec.frontmatter),
            };
            if !read_source(repository, &source, spec, &mut scan, &mut remaining, true) {
                break;
            }
        }
    }
    scan.into_parts()
}

#[cfg(test)]
mod tests {
    use super::*;
    use precept_core::import::SourceKind;
    use std::fs;

    fn write(root: &Path, source: &str, text: &str) {
        let path = root.join(source);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn reads_every_fixed_source_and_ignores_unknown_or_nested_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "AGENTS.md", "## Agents\n\nShared instructions.\n");
        write(root, "CLAUDE.md", "## Claude\n\nClaude instructions.\n");
        write(
            root,
            ".claude/CLAUDE.md",
            "## Claude Project\n\nClaude project instructions.\n",
        );
        write(root, ".cursorrules", "Legacy Cursor instructions.\n");
        write(
            root,
            ".cursor/rules/style.mdc",
            "---\nalwaysApply: true\n---\nCursor style.\n",
        );
        write(root, ".continue/rules/test.md", "Continue tests.\n");
        write(
            root,
            ".github/copilot-instructions.md",
            "Copilot instructions.\n",
        );
        write(
            root,
            ".github/instructions/api.instructions.md",
            "---\napplyTo: src/\n---\nCopilot API.\n",
        );
        write(
            root,
            ".claude/rules/rust.md",
            "---\npaths: crates/\n---\nClaude Rust.\n",
        );
        write(root, ".cursor/rules/nested/ignored.mdc", "Ignored.\n");
        write(root, ".cursor/rules/ignored.txt", "Ignored.\n");
        write(root, ".precept/rules/ignored.md", "Ignored.\n");
        write(root, "nested/AGENTS.md", "## Ignored\n\nIgnored.\n");

        let (candidates, skipped) = scan(root);
        let sources: Vec<&str> = candidates.iter().map(|c| c.source.as_str()).collect();
        assert_eq!(
            sources,
            [
                "AGENTS.md",
                "CLAUDE.md",
                ".claude/CLAUDE.md",
                ".cursorrules",
                ".github/copilot-instructions.md",
                ".cursor/rules/style.mdc",
                ".continue/rules/test.md",
                ".github/instructions/api.instructions.md",
                ".claude/rules/rust.md",
            ]
        );
        assert!(skipped.is_empty(), "{skipped:?}");
        assert_eq!(candidates[5].kind, SourceKind::Cursor);
        assert_eq!(candidates[6].kind, SourceKind::Continue);
        assert_eq!(candidates[7].kind, SourceKind::Copilot);
        assert_eq!(candidates[8].kind, SourceKind::Claude);
    }

    #[test]
    fn directory_order_is_deterministic_and_extensions_are_exact() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, ".claude/rules/z.md", "Zed.\n");
        write(root, ".claude/rules/a.md", "Aye.\n");
        write(root, ".claude/rules/b.MD", "Wrong case.\n");
        let first = scan(root);
        let second = scan(root);
        assert_eq!(first, second);
        let sources: Vec<&str> = first.0.iter().map(|c| c.source.as_str()).collect();
        assert_eq!(sources, [".claude/rules/a.md", ".claude/rules/z.md"]);
    }

    #[cfg(unix)]
    #[test]
    fn unix_file_and_parent_links_are_never_followed() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(outside.path(), "secret.md", "Private outside content.\n");
        fs::create_dir_all(root.join(".claude")).unwrap();
        symlink(outside.path(), root.join(".claude/rules")).unwrap();
        symlink(outside.path().join("secret.md"), root.join("CLAUDE.md")).unwrap();
        let (candidates, skipped) = scan(root);
        assert!(candidates.is_empty());
        assert_eq!(skipped.len(), 2);
        assert!(skipped.iter().all(|s| s.reason == "unreadable"));
        assert!(!format!("{skipped:?}").contains("Private outside content"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_file_and_directory_links_are_never_followed() {
        use std::os::windows::fs::{symlink_dir, symlink_file};
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(outside.path(), "secret.md", "Private outside content.\n");
        fs::create_dir_all(root.join(".claude")).unwrap();
        symlink_dir(outside.path(), root.join(".claude/rules")).unwrap();
        symlink_file(outside.path().join("secret.md"), root.join("CLAUDE.md")).unwrap();
        let (candidates, skipped) = scan(root);
        assert!(candidates.is_empty());
        assert_eq!(skipped.len(), 2);
        assert!(skipped.iter().all(|s| s.reason == "unreadable"));
    }

    #[test]
    fn oversized_non_utf8_and_non_regular_files_are_unreadable() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("AGENTS.md"), vec![b'x'; MAX_FILE as usize + 1]).unwrap();
        fs::write(root.join("CLAUDE.md"), vec![0xff, 0xfe]).unwrap();
        fs::create_dir_all(root.join(".cursor/rules/directory.mdc")).unwrap();
        let (candidates, skipped) = scan(root);
        assert!(candidates.is_empty());
        assert_eq!(skipped.len(), 3);
        assert!(skipped.iter().all(|s| s.reason == "unreadable"));
    }

    #[test]
    fn directory_entry_and_file_count_budgets_are_disclosed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".claude/rules")).unwrap();
        for index in 0..=MAX_DIR_ENTRIES {
            fs::write(root.join(format!(".claude/rules/{index}.md")), "Body.\n").unwrap();
        }
        let (candidates, skipped) = scan(root);
        assert!(candidates.is_empty());
        assert!(skipped.iter().any(|s| s.reason == "budget_exhausted"));

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for index in 0..=MAX_DIR_FILES {
            write(
                root,
                &format!(".claude/rules/{index}.md"),
                &format!("Body {index}.\n"),
            );
        }
        let (candidates, skipped) = scan(root);
        assert_eq!(candidates.len(), MAX_DIR_FILES);
        assert!(skipped.iter().any(|s| s.reason == "budget_exhausted"));
    }

    #[test]
    fn aggregate_bytes_stop_further_reads_without_leaking_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for index in 0..5 {
            write(
                root,
                &format!(".claude/rules/{index}.md"),
                &format!("{index}:{}\n", "x".repeat(MAX_FILE as usize - 32)),
            );
        }
        let (candidates, skipped) = scan(root);
        assert_eq!(skipped.iter().filter(|s| s.reason == "too_long").count(), 4);
        assert!(skipped.iter().any(|s| s.reason == "budget_exhausted"));
        let rendered = format!("{candidates:?}{skipped:?}");
        assert!(!rendered.contains(root.to_str().unwrap()));
        assert!(!rendered.contains("x".repeat(100).as_str()));
    }
}
