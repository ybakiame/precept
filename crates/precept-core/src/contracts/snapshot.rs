//! One bounded read of the repository's rule files, and the checked reader
//! that rule checks share. Observed links are refused; path checks are not a
//! same-user rename/reparse-point race sandbox.
use super::parser::{DIR, MAX_BYTES, MAX_FILES, RuleFile, parse};
use crate::repo_fs::{FileRead, OpenError, Repository, RootIdentity};
use sha2::{Digest, Sha256};
use std::{cell::OnceCell, collections::BTreeSet, path::Path, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileIssue {
    InvalidName,
    UnsafePath,
    TooLarge,
}

impl FileIssue {
    fn code(self) -> &'static str {
        match self {
            Self::InvalidName => "id.invalid",
            Self::UnsafePath => "file.unsafe_path",
            Self::TooLarge => "file.too_large",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceIssue {
    UnsafePath,
    TooManyFiles,
}

impl SourceIssue {
    fn code(self) -> &'static str {
        match self {
            Self::UnsafePath => "source.unsafe_path",
            Self::TooManyFiles => "source.too_many_files",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct SourceFile {
    id: String,
    raw: Vec<u8>,
    /// Unsafe and oversized entries stay empty so parsing still refuses them.
    issue: Option<FileIssue>,
}

#[derive(Clone, PartialEq, Eq)]
enum Read {
    Absent,
    /// Rule entries sorted by id, including sanitized invalid-file reasons.
    Files(Vec<SourceFile>),
    Invalid(SourceIssue),
    Unavailable,
}
pub struct Snapshot {
    repository: Option<Arc<Repository>>,
    read: Read,
    /// Ids whose file Git tracks with no uncommitted change; `None` when Git
    /// could not tell (every rule is then unreviewed).
    reviewed: Option<BTreeSet<String>>,
    parsed: OnceCell<Vec<Option<(RuleFile, String)>>>,
}
impl Snapshot {
    pub fn read(root: &Path) -> Self {
        Self::open(Repository::open(root))
    }

    pub fn read_expected(root: &Path, identity: &RootIdentity) -> Self {
        Self::open(Repository::open_expected(root, identity))
    }

    pub fn from_repository(repository: Repository) -> Self {
        Self::open(Ok(repository))
    }

    fn open(repository: Result<Repository, OpenError>) -> Self {
        let (repository, read) = match repository {
            Ok(repository) => {
                let read = read(&repository);
                (Some(Arc::new(repository)), read)
            }
            Err(_) => (None, Read::Unavailable),
        };
        Self {
            repository,
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
    /// Lint the bounded inventory captured by this snapshot. Selector probes
    /// inspect current metadata only and never follow links or reparse points.
    pub fn lint(&self) -> super::lint::LintReport {
        use super::lint::{LintStatus, lint_files, report, source_report};
        match &self.read {
            Read::Absent => report(LintStatus::NotConfigured, vec![]),
            Read::Unavailable => source_report(LintStatus::Unavailable, "source.unavailable"),
            Read::Invalid(issue) => source_report(LintStatus::Issues, issue.code()),
            Read::Files(files) => lint_files(
                files.iter().map(|file| {
                    (
                        file.id.as_str(),
                        file.raw.as_slice(),
                        file.issue.map(FileIssue::code),
                    )
                }),
                self.repository.as_deref(),
            ),
        }
    }
    pub fn status(&self) -> &'static str {
        match &self.read {
            Read::Absent => "not_configured",
            Read::Files(_) if self.invalid().is_empty() => "ready",
            Read::Files(_) | Read::Invalid(_) => "invalid_source",
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
                .map(|(file, _)| file.id.clone())
                .collect(),
            _ => vec![],
        }
    }
    fn parsed(&self) -> &[Option<(RuleFile, String)>] {
        self.parsed.get_or_init(|| match &self.read {
            Read::Files(files) => files
                .iter()
                .map(|file| {
                    parse(&file.id, &file.raw)
                        .ok()
                        .map(|r| (r, format!("{:x}", Sha256::digest(&file.raw))))
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
    pub(super) fn repository(&self) -> Option<&Arc<Repository>> {
        self.repository.as_ref()
    }
    pub(super) fn check_cache(&self) -> super::checks::CheckCache {
        super::checks::CheckCache::with_repository(self.repository.clone())
    }
    pub fn unchanged(&self, root: &Path) -> bool {
        let Some(repository) = &self.repository else {
            return false;
        };
        if !repository.is_current() {
            return false;
        }
        let current = Self::open(Repository::open_expected(root, repository.root_identity()));
        current.repository.is_some() && self.read == current.read
    }
}

fn source_id(name: &std::ffi::OsStr) -> Option<(String, Option<FileIssue>)> {
    if !name.as_encoded_bytes().ends_with(b".md") {
        return None;
    }
    if let Some(id) = name.to_str().and_then(|name| name.strip_suffix(".md")) {
        return Some((id.to_owned(), None));
    }
    let hash = format!("{:x}", Sha256::digest(name.as_encoded_bytes()));
    Some((format!("invalid-{hash}"), Some(FileIssue::InvalidName)))
}

/// The file names under [`DIR`] that are rule files (`<id>.md`).
fn read(repository: &Repository) -> Read {
    let directory = match repository.open_dir(Path::new(DIR)) {
        Ok(directory) => directory,
        Err(OpenError::Missing) => return Read::Absent,
        Err(OpenError::Unsafe) => return Read::Invalid(SourceIssue::UnsafePath),
        Err(_) => return Read::Unavailable,
    };
    let entries = match directory.entries(MAX_FILES * 16, MAX_FILES, |name| {
        name.as_encoded_bytes().ends_with(b".md")
    }) {
        Ok(entries) => entries,
        Err(OpenError::TooMany) => return Read::Invalid(SourceIssue::TooManyFiles),
        Err(OpenError::Unsafe) => return Read::Invalid(SourceIssue::UnsafePath),
        Err(_) => return Read::Unavailable,
    };
    let mut files = vec![];
    for entry in entries {
        let name = entry.name;
        let Some((id, name_issue)) = source_id(&name) else {
            continue;
        };
        let (raw, issue) = if let Some(issue) = name_issue {
            (vec![], Some(issue))
        } else {
            let relative = Path::new(DIR).join(&name);
            match repository.read_file(&relative, MAX_BYTES, MAX_BYTES) {
                Ok(raw) => (raw, None),
                Err(FileRead::Missing | FileRead::Unsafe) => (vec![], Some(FileIssue::UnsafePath)),
                Err(FileRead::TooLarge) => (vec![], Some(FileIssue::TooLarge)),
                Err(FileRead::BudgetExceeded | FileRead::Unavailable) => return Read::Unavailable,
            }
        };
        files.push(SourceFile {
            id: id.to_owned(),
            raw,
            issue,
        });
    }
    files.sort_by(|a, b| a.id.cmp(&b.id));
    Read::Files(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{LintStatus, Severity};
    use std::{fs, path::PathBuf};

    const RULE: &str = "+++\nkey = \"x\"\n+++\nbody\n";

    fn rules_dir(root: &Path) -> PathBuf {
        root.join(DIR)
    }

    fn write_rule(root: &Path, id: &str) {
        fs::write(rules_dir(root).join(format!("{id}.md")), RULE).unwrap();
    }

    #[cfg(unix)]
    fn make_link(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn make_link(target: &Path, link: &Path) -> std::io::Result<()> {
        if target.is_dir() {
            let status = std::process::Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "& { param([string]$link,[string]$target) New-Item -ItemType Junction -Path $link -Target $target | Out-Null }",
                    &link.to_string_lossy(),
                    &target.to_string_lossy(),
                ])
                .status()?;
            if status.success() {
                Ok(())
            } else {
                Err(std::io::Error::other("junction creation failed"))
            }
        } else {
            std::os::windows::fs::symlink_file(target, link)
        }
    }

    #[test]
    fn invalid_files_keep_legacy_snapshot_behavior_and_lint_reasons() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(rules_dir(temp.path())).unwrap();
        fs::create_dir(rules_dir(temp.path()).join("directory.md")).unwrap();
        fs::write(
            rules_dir(temp.path()).join("large.md"),
            vec![b'a'; MAX_BYTES + 1],
        )
        .unwrap();
        write_rule(temp.path(), "valid");

        let snapshot = Snapshot::read(temp.path());
        assert_eq!(snapshot.status(), "invalid_source");
        assert_eq!(snapshot.invalid(), ["directory", "large"]);
        assert!(snapshot.rules().is_none());
        assert!(snapshot.unchanged(temp.path()));
        let report = snapshot.lint();
        assert_eq!(report.status, LintStatus::Issues);
        assert_eq!(report.summary.errors, 2);
        let directory = report
            .files
            .iter()
            .find(|file| file.file.ends_with("/directory.md"))
            .unwrap();
        assert_eq!(directory.diagnostics[0].code, "file.unsafe_path");
        assert_eq!(directory.diagnostics[0].severity, Severity::Error);
        let large = report
            .files
            .iter()
            .find(|file| file.file.ends_with("/large.md"))
            .unwrap();
        assert_eq!(large.diagnostics[0].code, "file.too_large");
    }

    #[test]
    fn non_unicode_rule_names_are_reported_without_exposing_the_name() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(rules_dir(temp.path())).unwrap();
        #[cfg(unix)]
        let name = {
            use std::os::unix::ffi::OsStringExt;
            std::ffi::OsString::from_vec(vec![b'x', 0xff, b'.', b'm', b'd'])
        };
        #[cfg(windows)]
        let name = {
            use std::os::windows::ffi::OsStringExt;
            std::ffi::OsString::from_wide(&[
                b'x' as u16,
                0xd800,
                b'.' as u16,
                b'm' as u16,
                b'd' as u16,
            ])
        };
        #[cfg(not(any(unix, windows)))]
        let name = std::ffi::OsString::from("x.md");
        if let Err(error) = fs::write(rules_dir(temp.path()).join(&name), RULE) {
            panic!("non-Unicode filename fixture failed: {error}");
        }
        let snapshot = Snapshot::read(temp.path());
        assert_eq!(snapshot.status(), "invalid_source");
        assert!(snapshot.invalid()[0].starts_with("invalid-"));
        let report = snapshot.lint();
        assert_eq!(report.summary.errors, 1);
        assert_eq!(report.files[0].diagnostics[0].code, "id.invalid");
        assert!(report.files[0].file.starts_with(".precept/rules/invalid-"));
        assert!(!serde_json::to_string(&report).unwrap().contains('\u{fffd}'));
    }

    #[test]
    fn unsafe_source_and_file_budget_failures_are_sanitized() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join(".precept"), "not a directory").unwrap();
        let report = Snapshot::read(temp.path()).lint();
        assert_eq!(report.status, LintStatus::Issues);
        assert_eq!(report.files[0].diagnostics[0].code, "source.unsafe_path");
        assert!(
            !serde_json::to_string(&report)
                .unwrap()
                .contains("not a directory")
        );

        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(rules_dir(temp.path())).unwrap();
        for index in 0..=MAX_FILES {
            write_rule(temp.path(), &format!("rule-{index:02}"));
        }
        let snapshot = Snapshot::read(temp.path());
        assert_eq!(snapshot.status(), "invalid_source");
        assert!(snapshot.invalid().is_empty());
        let report = snapshot.lint();
        assert_eq!(report.status, LintStatus::Issues);
        assert_eq!(report.files[0].diagnostics[0].code, "source.too_many_files");
        assert_eq!(report.summary.files, 1);
    }

    #[test]
    fn rule_file_links_are_never_followed() {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("target.md");
        fs::write(&target, RULE).unwrap();
        fs::create_dir_all(rules_dir(temp.path())).unwrap();
        if let Err(error) = make_link(&target, &rules_dir(temp.path()).join("linked.md")) {
            panic!("rule-file link fixture failed: {error}");
        }
        let snapshot = Snapshot::read(temp.path());
        assert_eq!(snapshot.status(), "invalid_source");
        assert_eq!(snapshot.invalid(), ["linked"]);
        let report = snapshot.lint();
        assert_eq!(report.files[0].diagnostics[0].code, "file.unsafe_path");
        assert!(
            !serde_json::to_string(&report)
                .unwrap()
                .contains("target.md")
        );
    }

    #[test]
    fn source_directory_links_are_never_followed() {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir_all(rules_dir(outside.path())).unwrap();
        fs::write(rules_dir(outside.path()).join("secret.md"), RULE).unwrap();
        fs::create_dir(temp.path().join(".precept")).unwrap();
        if let Err(error) = make_link(&rules_dir(outside.path()), &rules_dir(temp.path())) {
            panic!("rule-directory link fixture failed: {error}");
        }
        let report = Snapshot::read(temp.path()).lint();
        assert_eq!(report.status, LintStatus::Issues);
        assert_eq!(report.files[0].diagnostics[0].code, "source.unsafe_path");
        assert!(
            !serde_json::to_string(&report)
                .unwrap()
                .contains("secret.md")
        );
    }

    #[test]
    fn linked_selector_targets_are_ambiguous_not_missing() {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("target.txt");
        fs::write(&target, "present").unwrap();
        let linked = temp.path().join("linked.txt");
        if let Err(error) = make_link(&target, &linked) {
            panic!("selector-link fixture failed: {error}");
        }
        fs::create_dir_all(rules_dir(temp.path())).unwrap();
        let raw = "+++\nkey = \"x\"\n[applies_to]\npaths = [\"linked.txt\"]\n+++\nbody\n";
        fs::write(rules_dir(temp.path()).join("x.md"), raw).unwrap();
        let report = Snapshot::read(temp.path()).lint();
        assert_eq!(report.status, LintStatus::Ok);
        assert!(report.files[0].diagnostics.is_empty());
    }

    #[test]
    fn selector_probe_distinguishes_missing_from_unavailable() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("dir")).unwrap();
        fs::write(temp.path().join("file"), "x").unwrap();
        let repository = Repository::open(temp.path()).unwrap();
        assert_eq!(repository.selector_missing("missing"), Some(true));
        assert_eq!(repository.selector_missing("dir/"), Some(false));
        assert_eq!(repository.selector_missing("file"), Some(false));
        assert_eq!(repository.selector_missing("file/"), Some(true));
        assert_eq!(repository.selector_missing("file/child"), Some(true));
        assert_eq!(
            Repository::open(&temp.path().join("absent-root"))
                .ok()
                .and_then(|repository| repository.selector_missing("missing")),
            None
        );
    }
}
