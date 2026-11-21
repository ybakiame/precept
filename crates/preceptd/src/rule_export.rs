//! A confirmed project convention written as a repository rule file
//! (`.precept/rules/<id>.md`), only on an explicit Operator action after a
//! preview of the exact text (and, for an existing file, of the change).
//! Updating an existing file keeps what only the file holds (allowed values,
//! checks, guards); a file that does not parse is never touched. Removal
//! takes back only a file still exactly as Precept would write it. Nothing
//! is committed: the operator reviews and commits the file.
use crate::integration::write_atomic;
use precept_core::{
    Preference, Scope, Status,
    contracts::{self, DIR, MAX_BYTES},
    pointer::digest,
};
use precept_ipc::Error;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// The confirmed project convention `id` of `project`, or `NotFound`.
pub fn convention(prefs: Vec<Preference>, project: &str, id: u64) -> Result<Preference, Error> {
    prefs
        .into_iter()
        .find(|p| {
            p.id == id
                && p.status == Status::Confirmed
                && p.scope == Scope::Project
                && p.project.as_deref() == Some(project)
        })
        .ok_or(Error::NotFound)
}

fn rule_id(p: &Preference) -> String {
    contracts::id_for(&p.key)
}
/// The file text Precept writes for `p`: the record's key, title (its
/// text's heading), tags, options and text, over what `existing` (a file
/// already there that parses) alone declares.
pub fn render(p: &Preference, existing: Option<&contracts::RuleFile>) -> String {
    contracts::render(&contracts::RuleFile {
        id: rule_id(p),
        key: p.key.clone(),
        title: contracts::heading_title(&p.value),
        allowed: existing.map(|e| e.allowed.clone()).unwrap_or_default(),
        enforcement: p.rule.enforcement,
        priority: p.rule.priority,
        tags: p.tags.clone(),
        applies_to: p.rule.applies_to.clone(),
        checks: existing.map(|e| e.checks.clone()).unwrap_or_default(),
        guards: existing.map(|e| e.guards.clone()).unwrap_or_default(),
        body: p.value.clone(),
    })
}
/// The parsed file at `p`'s path, when its text parses.
fn parsed(p: &Preference, text: &str) -> Option<contracts::RuleFile> {
    contracts::parse(&rule_id(p), text.as_bytes()).ok()
}
fn file(p: &Preference) -> String {
    contracts::file_of(&rule_id(p))
}

/// The rule file's current text: absent, or UTF-8 text. A link, a directory
/// anywhere on the path or an oversized file is refused.
fn read(root: &Path, relative: &str) -> Result<Option<String>, Error> {
    let mut path = root.to_path_buf();
    let parts: Vec<&str> = relative.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        path.push(part);
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(Error::Unavailable),
        };
        let last = i + 1 == parts.len();
        if meta.file_type().is_symlink() || (!last && !meta.is_dir()) || (last && !meta.is_file()) {
            return Err(Error::UnsafeRuntime);
        }
        if last && meta.len() > MAX_BYTES as u64 {
            return Err(Error::TooLarge);
        }
    }
    let raw = fs::read(&path).map_err(|_| Error::Unavailable)?;
    String::from_utf8(raw)
        .map(Some)
        .map_err(|_| Error::InvalidRequest)
}

/// `.precept/rules` under `root`, created when absent, never through a link.
fn directory(root: &Path) -> Result<PathBuf, Error> {
    let mut dir = root.to_path_buf();
    for part in DIR.split('/') {
        dir.push(part);
        match fs::symlink_metadata(&dir) {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => {}
            Ok(_) => return Err(Error::UnsafeRuntime),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&dir).map_err(|_| Error::Unavailable)?;
            }
            Err(_) => return Err(Error::Unavailable),
        }
    }
    Ok(dir)
}

/// What writing `p` would do: the path, the text, the current text, and
/// whether the file is absent, already the same, different (an update) or
/// invalid (not a rule file Precept can update).
pub fn preview(root: &Path, p: &Preference) -> Result<Value, Error> {
    let path = file(p);
    let current = read(root, &path)?;
    let existing = current.as_deref().and_then(|c| parsed(p, c));
    let text = render(p, existing.as_ref());
    let state = match (&current, &existing) {
        (None, _) => "absent",
        (Some(c), _) if *c == text => "same",
        (Some(_), Some(_)) => "different",
        (Some(_), None) => "invalid",
    };
    // The rendered file must read back as a valid rule.
    let valid = contracts::parse(&rule_id(p), text.as_bytes()).is_ok();
    Ok(json!({
        "path": path,
        "text": text,
        "current": current,
        "state": state,
        "valid": valid,
        "sha256": current.as_ref().map(|c| digest(c.as_bytes())),
    }))
}

/// Write (`enabled`) or take back the rule file of `p`, if the file is still
/// the one the operator previewed (`expected` digest, `None` for absent).
pub fn apply(
    root: &Path,
    p: &Preference,
    enabled: bool,
    expected: Option<&str>,
) -> Result<Value, Error> {
    let path = file(p);
    let current = read(root, &path)?;
    if current.as_ref().map(|c| digest(c.as_bytes())).as_deref() != expected {
        return Err(Error::Conflict);
    }
    let existing = current.as_deref().and_then(|c| parsed(p, c));
    let text = render(p, existing.as_ref());
    if contracts::parse(&rule_id(p), text.as_bytes()).is_err() {
        return Err(Error::InvalidRequest);
    }
    let action = match (enabled, current.as_deref()) {
        (true, Some(c)) if c == text => "unchanged",
        // Not a rule file Precept can merge into: left to the operator.
        (true, Some(_)) if existing.is_none() => return Err(Error::Conflict),
        (true, Some(_)) => {
            let dir = directory(root)?;
            write_atomic(&dir.join(format!("{}.md", rule_id(p))), &text)?;
            "updated"
        }
        (true, None) => {
            let dir = directory(root)?;
            write_atomic(&dir.join(format!("{}.md", rule_id(p))), &text)?;
            "created"
        }
        (false, None) => "unchanged",
        (false, Some(c)) if c == text => {
            fs::remove_file(root.join(&path)).map_err(|_| Error::Unavailable)?;
            "deleted"
        }
        // Edited since Precept wrote it: the operator's file now.
        (false, Some(_)) => return Err(Error::Conflict),
    };
    Ok(json!({"action":action,"path":path}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use precept_core::operator::Enforcement;

    fn convention() -> Preference {
        let mut p = Preference {
            rule: Default::default(),
            enabled: true,
            id: 7,
            key: "Commit style".into(),
            value: "# Small commits\n\nOne change each.".into(),
            scope: Scope::Project,
            project: Some("alpha".into()),
            status: Status::Confirmed,
            tags: vec!["git".into()],
        };
        p.rule.enforcement = Enforcement::Verify;
        p
    }

    #[test]
    fn writes_once_previewed_updates_keeping_hand_written_parts_and_takes_back_its_own() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let mut p = convention();
        let v = preview(root, &p).unwrap();
        assert_eq!(v["state"], "absent");
        assert_eq!(v["path"], ".precept/rules/commit-style.md");
        assert_eq!(v["valid"], true);
        assert!(
            v["text"]
                .as_str()
                .unwrap()
                .contains("title = \"Small commits\"")
        );
        // A stale preview (the file appeared since) is refused.
        assert_eq!(
            apply(root, &p, true, Some(&"0".repeat(64))),
            Err(Error::Conflict)
        );
        assert_eq!(apply(root, &p, true, None).unwrap()["action"], "created");
        let path = root.join(".precept/rules/commit-style.md");
        let written = fs::read_to_string(&path).unwrap();
        let parsed = contracts::parse("commit-style", written.as_bytes()).unwrap();
        assert_eq!(parsed.key, "Commit style");
        assert_eq!(parsed.enforcement, Enforcement::Verify);
        let same = preview(root, &p).unwrap();
        assert_eq!(same["state"], "same");
        let d = same["sha256"].as_str().unwrap().to_owned();
        assert_eq!(
            apply(root, &p, true, Some(&d)).unwrap()["action"],
            "unchanged"
        );
        // The operator adds a check by hand; the record's text changes.
        let with_check =
            written.replacen("+++\n\n", "\n[[check]]\nfile = \"README.md\"\n+++\n\n", 1);
        fs::write(&path, &with_check).unwrap();
        p.value = "# Small commits\n\nOne change each, with tests.".into();
        let update = preview(root, &p).unwrap();
        assert_eq!(update["state"], "different");
        assert_eq!(update["current"], with_check);
        let d = update["sha256"].as_str().unwrap().to_owned();
        assert_eq!(
            apply(root, &p, true, Some(&d)).unwrap()["action"],
            "updated"
        );
        let updated = contracts::parse(
            "commit-style",
            fs::read_to_string(&path).unwrap().as_bytes(),
        )
        .unwrap();
        assert_eq!(
            updated.body,
            "# Small commits\n\nOne change each, with tests."
        );
        assert_eq!(updated.checks.len(), 1, "the hand-written check stays");
        // A file that does not parse is neither updated nor removed.
        fs::write(&path, "not a rule").unwrap();
        let broken = preview(root, &p).unwrap();
        assert_eq!(broken["state"], "invalid");
        let d = broken["sha256"].as_str().unwrap().to_owned();
        assert_eq!(apply(root, &p, true, Some(&d)), Err(Error::Conflict));
        assert_eq!(apply(root, &p, false, Some(&d)), Err(Error::Conflict));
        // Its own file, exactly as it would write it, is taken back.
        let own = render(&p, None);
        fs::write(&path, &own).unwrap();
        let d = digest(own.as_bytes());
        assert_eq!(
            apply(root, &p, false, Some(&d)).unwrap()["action"],
            "deleted"
        );
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn links_are_refused() {
        let temp = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), temp.path().join(".precept")).unwrap();
        let p = convention();
        assert_eq!(preview(temp.path(), &p), Err(Error::UnsafeRuntime));
        assert_eq!(
            apply(temp.path(), &p, true, None),
            Err(Error::UnsafeRuntime)
        );
        assert!(fs::read_dir(elsewhere.path()).unwrap().next().is_none());
    }
}
