//! Integrations Precept writes into a registered repository, only on an
//! explicit Operator action: the AGENTS.md pointer block. A request names the
//! digest of the file the operator previewed (or none for an absent file); a
//! file that changed since is refused, never overwritten. Writes are atomic
//! (temporary file + rename in the same directory) and keep the file's mode.
use precept_core::pointer::{self, Malformed};
use precept_ipc::Error;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

fn path(root: &Path) -> PathBuf {
    root.join(pointer::FILE)
}

/// The current file: absent, or its UTF-8 text. Links, directories, special
/// files and oversized files are refused.
fn read(path: &Path) -> Result<Option<String>, Error> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::Unavailable),
    };
    if !meta.file_type().is_file() {
        return Err(Error::UnsafeRuntime);
    }
    if meta.len() > pointer::MAX_BYTES as u64 {
        return Err(Error::TooLarge);
    }
    let raw = fs::read(path).map_err(|_| Error::Unavailable)?;
    String::from_utf8(raw)
        .map(Some)
        .map_err(|_| Error::InvalidRequest)
}

fn digest(current: &Option<String>) -> Value {
    current
        .as_ref()
        .map_or(Value::Null, |t| json!(pointer::digest(t.as_bytes())))
}

/// What the pointer looks like now and what applying would do.
pub fn status(root: &Path, project: &str) -> Result<Value, Error> {
    let current = read(&path(root))?;
    let state = pointer::state(current.as_deref(), project);
    Ok(json!({
        "path": pointer::FILE,
        "exists": current.is_some(),
        "sha256": digest(&current),
        "state": state.ok(),
        "malformed": state.is_err(),
        "block": pointer::block(project),
    }))
}

fn write_atomic(path: &Path, text: &str) -> Result<(), Error> {
    let dir = path.parent().ok_or(Error::UnsafeRuntime)?;
    let mut nonce = [0u8; 8];
    getrandom::fill(&mut nonce).map_err(|_| Error::Unavailable)?;
    let name: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
    let temp = dir.join(format!(".{}.precept-{name}", pointer::FILE));
    let mode = fs::metadata(path).ok().map(|m| m.permissions());
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|_| Error::Unavailable)?;
        file.write_all(text.as_bytes())
            .map_err(|_| Error::Unavailable)?;
        file.sync_all().map_err(|_| Error::Unavailable)?;
        if let Some(mode) = mode {
            fs::set_permissions(&temp, mode).map_err(|_| Error::Unavailable)?;
        }
        fs::rename(&temp, path).map_err(|_| Error::Unavailable)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Adds (`enabled`) or removes the block, if the file is still the one the
/// operator previewed (`expected` digest, `None` for absent).
pub fn apply(
    root: &Path,
    project: &str,
    enabled: bool,
    expected: Option<&str>,
) -> Result<Value, Error> {
    let path = path(root);
    let current = read(&path)?;
    let actual = current.as_ref().map(|t| pointer::digest(t.as_bytes()));
    if actual.as_deref() != expected {
        return Err(Error::Conflict);
    }
    let malformed = |_: Malformed| Error::InvalidRequest;
    let action = if enabled {
        let next = pointer::upsert(current.as_deref(), project).map_err(malformed)?;
        if current.as_deref() == Some(next.as_str()) {
            "unchanged"
        } else {
            write_atomic(&path, &next)?;
            if current.is_some() {
                "updated"
            } else {
                "created"
            }
        }
    } else {
        match current.as_deref() {
            None => "unchanged",
            Some(text) => match pointer::remove(text).map_err(malformed)? {
                None => {
                    fs::remove_file(&path).map_err(|_| Error::Unavailable)?;
                    "deleted"
                }
                Some(next) if next == text => "unchanged",
                Some(next) => {
                    write_atomic(&path, &next)?;
                    "removed"
                }
            },
        }
    };
    let mut out = status(root, project)?;
    out["action"] = json!(action);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previewed_digest_guards_every_write_and_links_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let s = status(root, "alpha").unwrap();
        assert_eq!(s["state"], "absent");
        assert_eq!(s["sha256"], Value::Null);
        let created = apply(root, "alpha", true, None).unwrap();
        assert_eq!(created["action"], "created");
        assert_eq!(created["state"], "current");
        let digest = created["sha256"].as_str().unwrap().to_owned();
        assert_eq!(apply(root, "alpha", true, None), Err(Error::Conflict));
        assert_eq!(
            apply(root, "alpha", true, Some(&digest)).unwrap()["action"],
            "unchanged"
        );
        let removed = apply(root, "alpha", false, Some(&digest)).unwrap();
        assert_eq!(removed["action"], "deleted");
        assert!(!root.join("AGENTS.md").exists());

        fs::write(root.join("AGENTS.md"), "# Rules\n").unwrap();
        let d = pointer::digest(b"# Rules\n");
        let added = apply(root, "alpha", true, Some(&d)).unwrap();
        assert_eq!(added["action"], "updated");
        let text = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(text.starts_with("# Rules\n\n<!-- precept:begin v1 -->"));
        let d = added["sha256"].as_str().unwrap().to_owned();
        assert_eq!(
            apply(root, "alpha", false, Some(&d)).unwrap()["action"],
            "removed"
        );
        assert_eq!(
            fs::read_to_string(root.join("AGENTS.md")).unwrap(),
            "# Rules\n"
        );
        assert!(
            fs::read_dir(root).unwrap().count() == 1,
            "no temporary file left"
        );

        #[cfg(unix)]
        {
            fs::remove_file(root.join("AGENTS.md")).unwrap();
            std::os::unix::fs::symlink("/etc/hostname", root.join("AGENTS.md")).unwrap();
            assert_eq!(status(root, "alpha"), Err(Error::UnsafeRuntime));
            assert_eq!(apply(root, "alpha", true, None), Err(Error::UnsafeRuntime));
        }
    }
}
