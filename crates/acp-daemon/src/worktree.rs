//! Shell-free, bounded on-demand Fingerprint v1 Git inspection.
use acp_core::{
    Project, Worktree, WorktreeInspector,
    fingerprint::{FingerprintV1, MAX_BYTES, MAX_UNTRACKED},
};
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command};
pub struct GitInspector;

fn git_bytes(root: &Path, args: &[&str], cap: usize, timeout: Duration) -> Option<Vec<u8>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?
        .block_on(git_output(root, args, cap, timeout))
}
async fn git_output(root: &Path, args: &[&str], cap: usize, timeout: Duration) -> Option<Vec<u8>> {
    let mut cmd = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            cmd.env_remove(key);
        }
    }
    cmd.env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args([
            "--no-pager",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
            "-C",
        ])
        .arg(root)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd.spawn().ok()?;
    let mut stdout = child.stdout.take()?.take(cap as u64 + 1);
    let mut stderr = child.stderr.take()?;
    let result = tokio::time::timeout(timeout, async {
        let read = async {
            let mut raw = Vec::new();
            stdout.read_to_end(&mut raw).await?;
            if raw.len() > cap {
                return Err(io::Error::other("bounded Git output exceeded"));
            }
            Ok(raw)
        };
        // Drain stderr concurrently without retaining private paths/diff/warnings.
        let mut sink = tokio::io::sink();
        let (raw, _, status) =
            tokio::try_join!(read, tokio::io::copy(&mut stderr, &mut sink), child.wait())?;
        if !status.success() {
            return Err(io::Error::other("Git unavailable"));
        }
        Ok::<_, io::Error>(raw)
    })
    .await
    .ok()
    .and_then(Result::ok);
    if result.is_none() {
        let _ = child.start_kill();
        let _ = child.wait().await;
    }
    result
}
fn git(root: &Path, args: &[&str]) -> Option<String> {
    Some(
        String::from_utf8_lossy(&git_bytes(root, args, 65536, Duration::from_secs(4))?)
            .trim()
            .to_owned(),
    )
}
fn safe_git_config(root: &Path) -> bool {
    // Read only key names, fail closed on config/read/timeout errors. External
    // filters may execute during status; do not run or silently override them.
    if root.join(".gitmodules").exists() {
        return false;
    }
    git(root, &["config", "--includes", "--name-only", "--list"]).is_some_and(|keys| {
        !keys.lines().any(|name| {
            let name = name.to_ascii_lowercase();
            name.starts_with("submodule.")
                || (name.starts_with("filter.")
                    && (name.ends_with(".clean") || name.ends_with(".process")))
        })
    })
}

fn raw_path(raw: &[u8]) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Some(Path::new(std::ffi::OsStr::from_bytes(raw)).to_owned())
    }
    #[cfg(windows)]
    {
        Some(PathBuf::from(std::str::from_utf8(raw).ok()?))
    }
}
fn target_bytes(path: &Path) -> Option<Vec<u8>> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Some(path.as_os_str().as_bytes().to_vec())
    }
    #[cfg(windows)]
    {
        // std::fs::read_link removes the Win32 verbatim prefix, while
        // Python os.readlink preserves the substitute-name spelling.
        let target = path.to_str()?;
        let target = if path.is_absolute() && !target.starts_with(r"\\?\") {
            if let Some(unc) = target.strip_prefix(r"\\") {
                format!(r"\\?\UNC\{unc}")
            } else {
                format!(r"\\?\{target}")
            }
        } else {
            target.to_owned()
        };
        Some(target.into_bytes())
    }
}
fn fingerprint_mode(path: &Path, metadata: &std::fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let _ = path;
        metadata.mode()
    }
    #[cfg(windows)]
    {
        let kind = if metadata.file_type().is_symlink() {
            0o120000
        } else {
            0o100000
        };
        let write = if metadata.permissions().readonly() {
            0
        } else {
            0o222
        };
        use std::os::windows::fs::FileTypeExt;
        let executable = metadata.file_type().is_symlink_dir()
            || path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
                ["exe", "com", "bat", "cmd"]
                    .iter()
                    .any(|ext| s.eq_ignore_ascii_case(ext))
            });
        kind | 0o444 | write | if executable { 0o111 } else { 0 }
    }
}

#[cfg(unix)]
fn confined_file(root: &Path, relative: &Path) -> Option<std::fs::File> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::ffi::OsStrExt,
        },
    };
    let mut directory = std::fs::File::open(root).ok()?;
    let components: Vec<_> = relative.components().collect();
    for (index, component) in components.iter().enumerate() {
        let std::path::Component::Normal(name) = component else {
            return None;
        };
        let name = CString::new(name.as_bytes()).ok()?;
        let last = index + 1 == components.len();
        // Descriptor-relative, no-follow parent traversal prevents a concurrent
        // directory swap from redirecting a content read outside the root.
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | if last {
                libc::O_NONBLOCK
            } else {
                libc::O_DIRECTORY
            };
        let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return None;
        }
        directory = unsafe { std::fs::File::from_raw_fd(fd) };
    }
    Some(directory)
}
#[cfg(windows)]
fn confined_file(root: &Path, relative: &Path) -> Option<std::fs::File> {
    use std::os::windows::{ffi::OsStringExt, fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_NAME_NORMALIZED, GetFinalPathNameByHandleW,
    };
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(root.join(relative))
        .ok()?;
    let mut name = vec![0u16; 32768];
    let len = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle(),
            name.as_mut_ptr(),
            name.len() as u32,
            FILE_NAME_NORMALIZED,
        )
    } as usize;
    if len == 0 || len >= name.len() {
        return None;
    }
    let final_path = PathBuf::from(std::ffi::OsString::from_wide(&name[..len]));
    if !final_path.starts_with(root.canonicalize().ok()?) {
        return None;
    }
    Some(file)
}

fn fingerprint(root: &Path) -> Option<String> {
    let timeout = Duration::from_secs(6);
    let status = git_bytes(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        MAX_BYTES,
        timeout,
    )?;
    let diff = git_bytes(
        root,
        &[
            "diff",
            "--binary",
            "--no-ext-diff",
            "--no-textconv",
            "HEAD",
            "--",
        ],
        MAX_BYTES - status.len(),
        timeout,
    )?;
    let mut hash = FingerprintV1::new(&status, &diff)?;
    // Preserve v1's exact NUL-record/prefix parsing (including rename records).
    let mut names: Vec<_> = status
        .split(|b| *b == 0)
        .filter_map(|v| v.strip_prefix(b"?? "))
        .collect();
    if names.len() > MAX_UNTRACKED {
        return None;
    }
    names.sort_unstable();
    let root = root.canonicalize().ok()?;
    for raw in names {
        let relative = raw_path(raw)?;
        if relative.is_absolute()
            || relative
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return None;
        }
        let file = root.join(&relative);
        if !file.parent()?.canonicalize().ok()?.starts_with(&root) {
            return None;
        }
        let metadata = file.symlink_metadata().ok()?;
        hash.untracked(raw, fingerprint_mode(&file, &metadata))?;
        if metadata.file_type().is_symlink() {
            hash.symlink_target(&target_bytes(&std::fs::read_link(&file).ok()?)?);
        } else if metadata.is_file() {
            if metadata.len() > hash.remaining() as u64 {
                return None;
            }
            let mut opened = confined_file(&root, &relative)?;
            if !opened.metadata().ok()?.is_file() {
                return None;
            }
            let mut chunk = [0u8; 65536];
            loop {
                let read = opened.read(&mut chunk).ok()?;
                if read == 0 {
                    break;
                }
                hash.regular_chunk(&chunk[..read])?;
            }
        } else {
            return None;
        }
    }
    Some(hash.finish())
}
impl WorktreeInspector for GitInspector {
    fn inspect(&self, p: &Project, include_fingerprint: bool) -> acp_core::Result<Worktree> {
        if !p.root.join(".git").exists() {
            return Ok(Worktree::default());
        }
        let Some(top) = git(&p.root, &["rev-parse", "--show-toplevel"]) else {
            return Ok(Worktree::default());
        };
        if Path::new(&top).canonicalize().ok() != p.root.canonicalize().ok() {
            return Ok(Worktree::default());
        }
        let head = git(&p.root, &["rev-parse", "HEAD"]);
        let branch = git(&p.root, &["branch", "--show-current"]);
        let safe = safe_git_config(&p.root);
        let status = if safe {
            git(
                &p.root,
                &["status", "--porcelain=v1", "--untracked-files=normal"],
            )
        } else {
            None
        };
        let worktree_fingerprint = if safe && include_fingerprint && head.is_some() {
            fingerprint(&p.root)
        } else {
            None
        };
        Ok(Worktree {
            head,
            branch,
            dirty: status.as_ref().map(|s| !s.is_empty()),
            change_count: status.as_ref().map(|s| s.lines().count() as u64),
            worktree_fingerprint,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command};
    fn git_test(root: &Path, args: &[&str]) {
        let result = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(result.status.success(), "synthetic Git setup failed");
    }
    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git_test(dir.path(), &["init", "-q"]);
        git_test(dir.path(), &["config", "core.autocrlf", "false"]);
        git_test(dir.path(), &["config", "user.name", "Synthetic"]);
        git_test(
            dir.path(),
            &["config", "user.email", "synthetic@example.invalid"],
        );
        fs::write(dir.path().join("tracked.txt"), b"initial\n").unwrap();
        fs::write(dir.path().join("binary.bin"), [0, 1, 255, 0]).unwrap();
        git_test(dir.path(), &["add", "."]);
        git_test(dir.path(), &["commit", "-qm", "synthetic"]);
        dir
    }
    fn project(root: &Path) -> Project {
        Project {
            name: "synthetic".into(),
            root: root.to_owned(),
        }
    }
    fn observed(root: &Path) -> Option<String> {
        GitInspector
            .inspect(&project(root), true)
            .unwrap()
            .worktree_fingerprint
    }
    #[test]
    fn v1_clean_staged_unstaged_binary_rename_unicode_and_head_branch() {
        let dir = repo();
        let root = dir.path();
        let clean = observed(root).unwrap();
        assert_eq!(
            clean,
            "869b578b87029f73beda1c4cf4a76a19645cf0b93a1d4f5aa324c4e5b49c6e1b"
        );
        let snapshot = GitInspector.inspect(&project(root), false).unwrap();
        assert!(snapshot.worktree_fingerprint.is_none());
        fs::write(root.join("tracked.txt"), b"unstaged\n").unwrap();
        let first = observed(root).unwrap();
        assert_ne!(clean, first);
        git_test(root, &["add", "tracked.txt"]);
        let staged = observed(root).unwrap();
        assert_ne!(first, staged);
        fs::write(root.join("tracked.txt"), b"different unstaged\n").unwrap();
        assert_ne!(first, observed(root).unwrap());
        let unstaged = observed(root).unwrap();
        fs::write(root.join("binary.bin"), [0, 5, 255, 3, 0]).unwrap();
        let binary = observed(root).unwrap();
        assert_ne!(unstaged, binary);
        git_test(root, &["mv", "tracked.txt", "renamed.txt"]);
        let renamed = observed(root).unwrap();
        assert_ne!(binary, renamed);
        fs::write(root.join("中文-🚀.txt"), "合成内容").unwrap();
        assert_ne!(renamed, observed(root).unwrap());
        git_test(root, &["add", "."]);
        git_test(root, &["commit", "-qm", "changed"]);
        assert_eq!(clean, observed(root).unwrap());
        git_test(root, &["checkout", "-qb", "synthetic-other"]);
        assert_eq!(clean, observed(root).unwrap());
        git_test(root, &["checkout", "--detach", "-q"]);
        assert_eq!(clean, observed(root).unwrap());
    }
    #[test]
    fn v1_untracked_same_name_count_changed_bytes_and_modes() {
        let dir = repo();
        let root = dir.path();
        let file = root.join("new.txt");
        fs::write(&file, b"first bytes").unwrap();
        let first = observed(root).unwrap();
        fs::write(&file, b"other bytes").unwrap();
        assert_ne!(first, observed(root).unwrap());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
            let executable = observed(root).unwrap();
            fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
            assert_ne!(executable, observed(root).unwrap());
        }
        for name in ["tool.exe", "UPPER.CMD", "plain"] {
            let before = observed(root).unwrap();
            fs::write(root.join(name), b"synthetic").unwrap();
            assert_ne!(before, observed(root).unwrap());
        }
        let writable = observed(root).unwrap();
        let mut permissions = fs::metadata(&file).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&file, permissions).unwrap();
        assert_ne!(writable, observed(root).unwrap());
        #[cfg(windows)]
        {
            let mut permissions = fs::metadata(&file).unwrap().permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            // Windows-only restoration of this owned fixture.
            permissions.set_readonly(false);
            fs::set_permissions(&file, permissions).unwrap();
        }
    }
    #[test]
    fn v1_aggregate_file_count_limits_and_missing_head() {
        let dir = repo();
        let root = dir.path();
        let status = git_bytes(
            root,
            &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            MAX_BYTES,
            Duration::from_secs(6),
        )
        .unwrap();
        assert!(status.is_empty());
        fs::write(root.join("large"), vec![b'x'; MAX_BYTES]).unwrap();
        assert_eq!(observed(root), None);
        fs::remove_file(root.join("large")).unwrap();
        for i in 0..501 {
            fs::write(root.join(format!("file-{i:03}")), b"x").unwrap();
        }
        assert_eq!(observed(root), None);
        fs::remove_file(root.join("file-500")).unwrap();
        observed(root).unwrap();
        let empty = tempfile::tempdir().unwrap();
        git_test(empty.path(), &["init", "-q"]);
        assert_eq!(observed(empty.path()), None);
        let no_git = tempfile::tempdir().unwrap();
        assert_eq!(observed(no_git.path()), None);
    }
    #[cfg(unix)]
    #[test]
    fn v1_non_utf8_symlink_targets_and_unsafe_parent_do_not_read_outside() {
        use std::os::unix::{ffi::OsStrExt, fs::symlink};
        let dir = repo();
        let root = dir.path();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("private"), b"OUTSIDE_MARKER").unwrap();
        symlink(outside.path().join("private"), root.join("link")).unwrap();
        let link = observed(root).unwrap();
        fs::write(outside.path().join("private"), b"CHANGED_OUTSIDE_MARKER").unwrap();
        assert_eq!(link, observed(root).unwrap());
        fs::write(
            root.join(std::ffi::OsStr::from_bytes(b"invalid-\xff")),
            b"raw bytes",
        )
        .unwrap();
        observed(root).unwrap();
        symlink(
            std::ffi::OsStr::from_bytes(b"target-\xfe"),
            root.join("raw-link"),
        )
        .unwrap();
        observed(root).unwrap();
        symlink(outside.path(), root.join("unsafe-parent")).unwrap();
        assert!(confined_file(root, Path::new("unsafe-parent/private")).is_none());
        assert!(confined_file(root, Path::new("../private")).is_none());
    }
    #[cfg(windows)]
    #[test]
    fn v1_windows_symlink_targets_modes_and_outside_parent() {
        use std::os::windows::fs::{symlink_dir, symlink_file};
        let dir = repo();
        let root = dir.path();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("private"), b"OUTSIDE_MARKER").unwrap();
        symlink_file(outside.path().join("private"), root.join("link")).unwrap();
        observed(root).unwrap();
        symlink_file("tracked.txt", root.join("relative.exe")).unwrap();
        observed(root).unwrap();
        symlink_dir(outside.path(), root.join("unsafe-parent")).unwrap();
        assert!(confined_file(root, Path::new("unsafe-parent/private")).is_none());
        observed(root).unwrap();
    }
    #[test]
    fn configured_external_filter_is_refused_without_execution() {
        let dir = repo();
        let root = dir.path();
        git_test(
            root,
            &[
                "config",
                "filter.synthetic.clean",
                "UNTRUSTED_EXECUTABLE_MUST_NOT_RUN",
            ],
        );
        assert!(!safe_git_config(root));
        let snapshot = GitInspector.inspect(&project(root), true).unwrap();
        assert!(snapshot.worktree_fingerprint.is_none() && snapshot.dirty.is_none());
        git_test(root, &["config", "--unset", "filter.synthetic.clean"]);
        fs::write(
            root.join(".gitmodules"),
            b"synthetic unsupported submodules",
        )
        .unwrap();
        assert!(!safe_git_config(root));
        fs::remove_file(root.join(".gitmodules")).unwrap();
        git_test(root, &["config", "submodule.synthetic.active", "true"]);
        assert!(!safe_git_config(root));
    }
    #[test]
    fn git_output_overflow_deadline_and_stderr_are_bounded_and_reaped() {
        let dir = repo();
        let root = dir.path();
        assert!(git_bytes(root, &["log", "--oneline"], 1, Duration::from_secs(4)).is_none());
        // Tokio may poll an already-ready child before its expired timer.
        // A zero budget may therefore return the real result; it must never be corrupt.
        let expected = git_bytes(root, &["status"], 65536, Duration::from_secs(4)).unwrap();
        let immediate = git_bytes(root, &["status"], 65536, Duration::ZERO);
        assert!(immediate.is_none_or(|raw| raw == expected));
        // An endless built-in read exercises actual timeout/kill/reap without a
        // scheduler race, shell, executable fixture or repository mutation.
        #[cfg(unix)]
        assert!(
            git_bytes(
                root,
                &["hash-object", "/dev/zero"],
                65536,
                Duration::from_millis(30)
            )
            .is_none()
        );
        assert!(git_bytes(root, &["not-a-command"], 65536, Duration::from_secs(4)).is_none());
        assert!(git_bytes(root, &["rev-parse", "HEAD"], 65536, Duration::from_secs(4)).is_some());
    }
}
