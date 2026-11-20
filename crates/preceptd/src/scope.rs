//! Which directory may become a project root. One daemon-side validator for
//! `project.check` and for adding a project; clients only propose a path.
//!
//! Accepted only when, after canonicalizing: inside the daemon user's home,
//! not the home itself, owned by that user, on the home's filesystem, a Git
//! repository root, and neither containing nor inside the runtime, the state
//! directory, the selected Profile's directory or another project's root. It
//! reads the path and its ancestors' metadata only; it never lists a directory.
use std::path::{Path, PathBuf};

/// A fixed refusal; the code crosses IPC and the bridge verbatim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NotAbsolute,
    NotFound,
    NotDirectory,
    NoHome,
    OutsideHome,
    IsHome,
    // Built by the Unix-only checks; the codes stay stable on every platform.
    #[cfg_attr(not(unix), allow(dead_code))]
    SystemPath,
    #[cfg_attr(not(unix), allow(dead_code))]
    OtherFilesystem,
    #[cfg_attr(not(unix), allow(dead_code))]
    NotOwner,
    NotGitRoot,
    OverlapsDaemon,
    OverlapsProfile,
    OverlapsProject,
}
impl Refusal {
    pub fn code(self) -> &'static str {
        match self {
            Self::NotAbsolute => "not_absolute",
            Self::NotFound => "not_found",
            Self::NotDirectory => "not_directory",
            Self::NoHome => "no_home",
            Self::OutsideHome => "outside_home",
            Self::IsHome => "is_home",
            Self::SystemPath => "system_path",
            Self::OtherFilesystem => "other_filesystem",
            Self::NotOwner => "not_owner",
            Self::NotGitRoot => "not_git_root",
            Self::OverlapsDaemon => "overlaps_daemon",
            Self::OverlapsProfile => "overlaps_profile",
            Self::OverlapsProject => "overlaps_project",
        }
    }
}

/// What a candidate must stay clear of.
pub struct Context<'a> {
    pub home: Option<PathBuf>,
    /// Runtime and state directories.
    pub daemon: &'a [&'a Path],
    /// The selected Profile's directory, if any.
    pub profile: Option<&'a Path>,
    /// Other projects' roots (canonical).
    pub projects: &'a [PathBuf],
}

/// The daemon user's home, canonical (`$HOME`; `%USERPROFILE%` on Windows).
pub fn home() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .and_then(|p| p.canonicalize().ok())
}

fn overlaps(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

/// The canonical root, or why not.
pub fn check(path: &Path, cx: &Context) -> Result<PathBuf, Refusal> {
    if !path.is_absolute()
        || path
            .to_str()
            .is_none_or(|p| p.len() > 4096 || p.contains('\0'))
    {
        return Err(Refusal::NotAbsolute);
    }
    let home = cx.home.clone().ok_or(Refusal::NoHome)?;
    let root = path.canonicalize().map_err(|_| Refusal::NotFound)?;
    let meta = std::fs::metadata(&root).map_err(|_| Refusal::NotFound)?;
    if !meta.is_dir() {
        return Err(Refusal::NotDirectory);
    }
    #[cfg(unix)]
    if ["/mnt", "/proc", "/sys", "/dev", "/run", "/media"]
        .iter()
        .any(|p| root.starts_with(p))
    {
        return Err(Refusal::SystemPath);
    }
    // Symlinks resolve above: a link that leaves home is outside home.
    if !root.starts_with(&home) {
        return Err(Refusal::OutsideHome);
    }
    if root == home {
        return Err(Refusal::IsHome);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let home_meta = std::fs::metadata(&home).map_err(|_| Refusal::NoHome)?;
        if meta.dev() != home_meta.dev() {
            return Err(Refusal::OtherFilesystem);
        }
        // The daemon's own uid (`/proc/self` belongs to the running process).
        let me = std::fs::metadata("/proc/self")
            .map(|m| m.uid())
            .unwrap_or(u32::MAX);
        if meta.uid() != me {
            return Err(Refusal::NotOwner);
        }
    }
    if !root.join(".git").exists() {
        return Err(Refusal::NotGitRoot);
    }
    if cx.daemon.iter().any(|d| overlaps(&root, d)) {
        return Err(Refusal::OverlapsDaemon);
    }
    if cx.profile.is_some_and(|p| overlaps(&root, p)) {
        return Err(Refusal::OverlapsProfile);
    }
    if cx.projects.iter().any(|p| overlaps(&root, p)) {
        return Err(Refusal::OverlapsProject);
    }
    Ok(root)
}

/// The folder name as a project name, if it is a valid one.
pub fn suggested_name(root: &Path) -> Option<String> {
    let name: String = root
        .file_name()?
        .to_str()?
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-_.".contains(c) {
                c
            } else {
                '-'
            }
        })
        .take(64)
        .collect();
    precept_ipc::valid_project(&name).then_some(name)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    struct Fixture {
        _temp: tempfile::TempDir,
        home: PathBuf,
    }
    fn fixture() -> Fixture {
        let temp = tempfile::Builder::new()
            .prefix("precept-scope-")
            .tempdir()
            .unwrap();
        let home = temp.path().canonicalize().unwrap().join("home");
        fs::create_dir_all(home.join("code/repo/.git")).unwrap();
        fs::create_dir_all(home.join("code/plain")).unwrap();
        fs::create_dir_all(home.join(".local/state/precept/runtime")).unwrap();
        fs::write(home.join("file"), "x").unwrap();
        Fixture { _temp: temp, home }
    }
    fn cx<'a>(f: &Fixture, daemon: &'a [&'a Path], projects: &'a [PathBuf]) -> Context<'a> {
        Context {
            home: Some(f.home.clone()),
            daemon,
            profile: None,
            projects,
        }
    }
    #[test]
    fn a_git_root_inside_home_is_accepted_with_a_suggested_name() {
        let f = fixture();
        let root = check(&f.home.join("code/repo"), &cx(&f, &[], &[])).unwrap();
        assert_eq!(root, f.home.join("code/repo"));
        assert_eq!(suggested_name(&root).as_deref(), Some("repo"));
        // A trailing component and `.` resolve to the same canonical root.
        assert_eq!(
            check(&f.home.join("code/./repo/"), &cx(&f, &[], &[])).unwrap(),
            root
        );
    }
    #[test]
    fn every_scope_rule_refuses_with_its_code() {
        let f = fixture();
        let state = f.home.join(".local/state/precept");
        let daemon = [state.as_path()];
        let repo = f.home.join("code/repo");
        let c = cx(&f, &[], &[]);
        assert_eq!(check(Path::new("code/repo"), &c), Err(Refusal::NotAbsolute));
        assert_eq!(check(&f.home.join("missing"), &c), Err(Refusal::NotFound));
        assert_eq!(check(&f.home.join("file"), &c), Err(Refusal::NotDirectory));
        assert_eq!(check(&f.home, &c), Err(Refusal::IsHome));
        assert_eq!(check(Path::new("/tmp"), &c), Err(Refusal::OutsideHome));
        assert_eq!(check(Path::new("/proc"), &c), Err(Refusal::SystemPath));
        assert_eq!(
            check(&f.home.join("code/plain"), &c),
            Err(Refusal::NotGitRoot)
        );
        // Inside a repository is not its root.
        fs::create_dir_all(repo.join("src")).unwrap();
        assert_eq!(check(&repo.join("src"), &c), Err(Refusal::NotGitRoot));
        // A symlink that leaves home is outside home after resolution.
        std::os::unix::fs::symlink("/tmp", f.home.join("escape")).unwrap();
        assert_eq!(check(&f.home.join("escape"), &c), Err(Refusal::OutsideHome));
        // The daemon's own directories, the Profile and other projects.
        fs::create_dir_all(state.join(".git")).unwrap();
        assert_eq!(
            check(&state, &cx(&f, &daemon, &[])),
            Err(Refusal::OverlapsDaemon)
        );
        let profile = repo.clone();
        let with_profile = Context {
            profile: Some(&profile),
            ..cx(&f, &[], &[])
        };
        assert_eq!(check(&repo, &with_profile), Err(Refusal::OverlapsProfile));
        let others = [f.home.join("code")];
        assert_eq!(
            check(&repo, &cx(&f, &[], &others)),
            Err(Refusal::OverlapsProject)
        );
        let no_home = Context {
            home: None,
            ..cx(&f, &[], &[])
        };
        assert_eq!(check(&repo, &no_home), Err(Refusal::NoHome));
    }
    #[test]
    fn suggested_names_are_valid_project_names_or_none() {
        assert_eq!(
            suggested_name(Path::new("/h/my repo")).as_deref(),
            Some("my-repo")
        );
        assert_eq!(
            suggested_name(Path::new("/h/precept.v2")).as_deref(),
            Some("precept.v2")
        );
        assert_eq!(suggested_name(Path::new("/")), None);
    }
}
