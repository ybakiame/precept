//! A confirmed project convention written as a repository rule file
//! (`.precept/rules/<id>.md`), only on an explicit Operator action after a
//! preview of the exact text (and, for an existing file, of the change).
//! Updating an existing file keeps what only the file holds (allowed values,
//! checks, guards); a file that does not parse is never touched. Removal
//! takes back only a file still exactly as Precept would write it. Nothing
//! is committed: the operator reviews and commits the file.
use precept_core::{
    Preference, Scope, Status,
    contracts::{self, MAX_BYTES},
    pointer::digest,
    repo_fs::{FileRead, Repository},
};
use precept_ipc::Error;
use serde_json::{Value, json};
#[cfg(test)]
use std::fs;
use std::path::Path;

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
fn read(root: &Repository, relative: &str) -> Result<Option<String>, Error> {
    let raw = match root.read_file(Path::new(relative), MAX_BYTES, MAX_BYTES) {
        Ok(raw) => raw,
        Err(FileRead::Missing) => return Ok(None),
        Err(FileRead::TooLarge) => return Err(Error::TooLarge),
        Err(_) => return Err(Error::UnsafeRuntime),
    };
    String::from_utf8(raw)
        .map(Some)
        .map_err(|_| Error::InvalidRequest)
}

fn write(
    root: &Repository,
    relative: &str,
    text: &str,
    current: Option<&str>,
) -> Result<(), Error> {
    let mode = root
        .open_file(Path::new(relative))
        .ok()
        .and_then(|file| file.metadata().ok())
        .map(|metadata| metadata.permissions());
    root.write_atomic(
        Path::new(relative),
        text.as_bytes(),
        mode,
        current.map(str::as_bytes),
    )
    .map_err(|error| match error {
        precept_core::repo_fs::OpenError::Conflict => Error::Conflict,
        precept_core::repo_fs::OpenError::Unsafe => Error::UnsafeRuntime,
        _ => Error::Unavailable,
    })
}

/// What writing `p` would do: the path, the text, the current text, and
/// whether the file is absent, already the same, different (an update) or
/// invalid (not a rule file Precept can update).
pub fn preview(root: &Repository, p: &Preference) -> Result<Value, Error> {
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
    root: &Repository,
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
            write(root, &path, &text, current.as_deref())?;
            "updated"
        }
        (true, None) => {
            write(root, &path, &text, current.as_deref())?;
            "created"
        }
        (false, None) => "unchanged",
        (false, Some(c)) if c == text => {
            root.remove_file(Path::new(&path))
                .map_err(|_| Error::Unavailable)?;
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
        let repository = Repository::open(root).unwrap();
        let mut p = convention();
        let v = preview(&repository, &p).unwrap();
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
            apply(&repository, &p, true, Some(&"0".repeat(64))),
            Err(Error::Conflict)
        );
        assert_eq!(
            apply(&repository, &p, true, None).unwrap()["action"],
            "created"
        );
        let path = root.join(".precept/rules/commit-style.md");
        let written = fs::read_to_string(&path).unwrap();
        let parsed = contracts::parse("commit-style", written.as_bytes()).unwrap();
        assert_eq!(parsed.key, "Commit style");
        assert_eq!(parsed.enforcement, Enforcement::Verify);
        let same = preview(&repository, &p).unwrap();
        assert_eq!(same["state"], "same");
        let d = same["sha256"].as_str().unwrap().to_owned();
        assert_eq!(
            apply(&repository, &p, true, Some(&d)).unwrap()["action"],
            "unchanged"
        );
        // The operator adds a check by hand; the record's text changes.
        let with_check =
            written.replacen("+++\n\n", "\n[[check]]\nfile = \"README.md\"\n+++\n\n", 1);
        fs::write(&path, &with_check).unwrap();
        p.value = "# Small commits\n\nOne change each, with tests.".into();
        let update = preview(&repository, &p).unwrap();
        assert_eq!(update["state"], "different");
        assert_eq!(update["current"], with_check);
        let d = update["sha256"].as_str().unwrap().to_owned();
        assert_eq!(
            apply(&repository, &p, true, Some(&d)).unwrap()["action"],
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
        let broken = preview(&repository, &p).unwrap();
        assert_eq!(broken["state"], "invalid");
        let d = broken["sha256"].as_str().unwrap().to_owned();
        assert_eq!(apply(&repository, &p, true, Some(&d)), Err(Error::Conflict));
        assert_eq!(
            apply(&repository, &p, false, Some(&d)),
            Err(Error::Conflict)
        );
        // Its own file, exactly as it would write it, is taken back.
        let own = render(&p, None);
        fs::write(&path, &own).unwrap();
        let d = digest(own.as_bytes());
        assert_eq!(
            apply(&repository, &p, false, Some(&d)).unwrap()["action"],
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
        let repository = Repository::open(temp.path()).unwrap();
        assert_eq!(preview(&repository, &p), Err(Error::UnsafeRuntime));
        assert_eq!(
            apply(&repository, &p, true, None),
            Err(Error::UnsafeRuntime)
        );
        assert!(fs::read_dir(elsewhere.path()).unwrap().next().is_none());
    }
}
