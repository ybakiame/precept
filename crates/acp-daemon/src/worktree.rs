//! Bounded, shell-free best-effort Git inspection; no fingerprint attestation.
use acp_core::{Project, Worktree, WorktreeInspector};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};
pub struct GitInspector;
fn git(root: &Path, args: &[&str]) -> Option<String> {
    // This synchronous inspector runs on the daemon's bounded blocking pool;
    // Tokio drains child output while waiting for the process to finish.
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?
        .block_on(git_output(root, args))
}
async fn git_output(root: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("git");
    // Local Git must not inherit repository selectors or execute fsmonitor helpers.
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
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
            "-C",
        ])
        .arg(root)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = cmd.spawn().ok()?;
    let output = tokio::time::timeout(Duration::from_millis(700), async {
        let mut raw = Vec::new();
        // Drain while Git is running, stopping as soon as the cap is exceeded.
        child
            .stdout
            .take()?
            .take(65537)
            .read_to_end(&mut raw)
            .await
            .ok()?;
        if raw.len() > 65536 || !child.wait().await.ok()?.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&raw).trim().to_owned())
    })
    .await
    .ok()
    .flatten();
    if output.is_none() {
        // Overflow, read failure and deadline all close the pipe and kill/reap.
        let _ = child.kill().await;
    }
    output
}
impl WorktreeInspector for GitInspector {
    fn inspect(&self, p: &Project, _fingerprint: bool) -> acp_core::Result<Worktree> {
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
        let status = git(
            &p.root,
            &["status", "--porcelain=v1", "--untracked-files=normal"],
        );
        Ok(Worktree {
            head,
            branch,
            dirty: status.as_ref().map(|s| !s.is_empty()),
            change_count: status.as_ref().map(|s| s.lines().count() as u64),
            worktree_fingerprint: None,
        })
    }
}
