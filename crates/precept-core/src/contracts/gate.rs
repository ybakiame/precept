//! The pre-tool gate's decision: does a tool call break a committed hard
//! rule's guard? Pure matching over the rules read in a [`Snapshot`]; the
//! harness hook and the daemon decide whether to ask at all.
use super::{GuardTool, Snapshot, file_of, parser::relative_path};
use crate::operator::Enforcement;

/// The most bytes of a shell command the gate looks at.
pub const MAX_COMMAND: usize = 65_536;
/// The most paths one write call names.
pub const MAX_PATHS: usize = 32;

/// A tool call as the gate sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolCall {
    /// A shell command line.
    Shell(String),
    /// File writes, as repository-relative paths.
    Write(Vec<String>),
}
impl ToolCall {
    pub fn valid(&self) -> bool {
        match self {
            Self::Shell(c) => !c.is_empty() && c.len() <= MAX_COMMAND,
            Self::Write(paths) => {
                !paths.is_empty()
                    && paths.len() <= MAX_PATHS
                    && paths.iter().all(|p| relative_path(p) && !p.ends_with('/'))
            }
        }
    }
}

/// Why a call is denied: the rule and the reason shown to the agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Denial {
    pub id: String,
    pub key: String,
    pub file: String,
    pub reason: String,
}

fn words(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn covers(guard: &str, path: &str) -> bool {
    match guard.strip_suffix('/') {
        Some(dir) => path.starts_with(guard) || path == dir,
        None => path == guard,
    }
}

/// The first committed hard rule whose guard `call` breaks, if any. Rules
/// not committed, not hard, or without a matching guard never deny.
pub fn check(snapshot: &Snapshot, call: &ToolCall) -> Option<Denial> {
    if !call.valid() {
        return None;
    }
    let rules = snapshot.rules()?;
    rules.into_iter().find_map(|(rule, _)| {
        if rule.enforcement != Enforcement::Hard || !snapshot.reviewed(&rule.id) {
            return None;
        }
        let hit = rule.guards.iter().any(|g| match (&g.tool, call) {
            (GuardTool::Shell, ToolCall::Shell(command)) => g
                .deny
                .as_deref()
                .is_some_and(|d| words(command).contains(&words(d))),
            (GuardTool::Write, ToolCall::Write(paths)) => paths
                .iter()
                .any(|p| g.paths.iter().any(|guard| covers(guard, p))),
            _ => false,
        });
        hit.then(|| {
            let summary = crate::contracts::summary(&rule.body);
            Denial {
                reason: format!(
                    "Blocked by the repository rule \"{}\" ({}): {}",
                    rule.title.as_deref().unwrap_or(&rule.key),
                    file_of(&rule.id),
                    summary
                ),
                file: file_of(&rule.id),
                key: rule.key,
                id: rule.id,
            }
        })
    })
}

/// What committed rules claim about `paths` (repository-relative) that does
/// not hold now: one entry per failing check, `{rule, key, file, failure}`.
/// A document whose claims fail may be stale; the agent reading it is told so.
pub fn stale(snapshot: &Snapshot, paths: &[String]) -> Vec<serde_json::Value> {
    let Some(root) = snapshot.root() else {
        return vec![];
    };
    let mut out = vec![];
    let mut cache = super::checks::CheckCache::new(root);
    for (rule, _) in snapshot.rules().unwrap_or_default() {
        if !snapshot.reviewed(&rule.id) {
            continue;
        }
        for check in rule.checks.iter().filter(|c| paths.contains(&c.file)) {
            if let Some(failure) = super::checks::failure_cached(check, &mut cache) {
                out.push(serde_json::json!({
                    "rule": rule.id, "key": rule.key,
                    "file": super::file_of(&rule.id), "failure": failure,
                }));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeSet, fs};

    const HARD: &str = "+++\nkey = \"release\"\ntitle = \"Release safety\"\nenforcement = \"hard\"\n[[guard]]\ntool = \"shell\"\ndeny = \"git push --force\"\n[[guard]]\ntool = \"write\"\npaths = [\"Cargo.lock\", \"vendor/\"]\n+++\n# Release safety\n\nNever rewrite published history.\n";

    fn snapshot(files: &[(&str, &str)], reviewed: &[&str]) -> (tempfile::TempDir, Snapshot) {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join(".precept/rules");
        fs::create_dir_all(&dir).unwrap();
        for (id, raw) in files {
            fs::write(dir.join(format!("{id}.md")), raw).unwrap();
        }
        let reviewed: BTreeSet<String> = reviewed.iter().map(|s| s.to_string()).collect();
        let s = Snapshot::read(temp.path()).with_reviewed(Some(reviewed));
        (temp, s)
    }

    #[test]
    fn hard_committed_guards_deny_matching_calls_only() {
        let (_t, s) = snapshot(&[("release", HARD)], &["release"]);
        let denied = check(
            &s,
            &ToolCall::Shell("git  push   --force origin main".into()),
        )
        .unwrap();
        assert_eq!(denied.key, "release");
        assert!(denied.reason.contains("Release safety"));
        assert!(denied.reason.contains("Never rewrite published history."));
        assert!(check(&s, &ToolCall::Shell("git push origin main".into())).is_none());
        for path in ["Cargo.lock", "vendor/x/y.rs", "vendor"] {
            assert!(
                check(&s, &ToolCall::Write(vec![path.into()])).is_some(),
                "{path}"
            );
        }
        for path in ["src/Cargo.lock", "vendored.rs"] {
            assert!(
                check(&s, &ToolCall::Write(vec![path.into()])).is_none(),
                "{path}"
            );
        }
        // Malformed calls are not judged.
        assert!(check(&s, &ToolCall::Write(vec!["../Cargo.lock".into()])).is_none());
    }

    #[test]
    fn uncommitted_or_softer_rules_never_deny() {
        let (_t, s) = snapshot(&[("release", HARD)], &[]);
        assert!(check(&s, &ToolCall::Shell("git push --force".into())).is_none());
        let verify = HARD.replace("enforcement = \"hard\"\n", "enforcement = \"verify\"\n");
        // A guard requires `hard`: such a file does not parse at all.
        let (_t, s) = snapshot(&[("release", &verify)], &["release"]);
        assert_eq!(s.status(), "invalid_source");
        assert!(check(&s, &ToolCall::Shell("git push --force".into())).is_none());
    }
}
