//! One evaluator for a rule's checks against the repository: configuration
//! values and claims in documents alike. Read-only, bounded, inside the root.
use super::parser::{Check, relative_path};
use super::snapshot::{InsideRead, read_inside_budget};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    cell::OnceCell,
    collections::HashMap,
    path::{Path, PathBuf},
};

/// The largest file a check reads.
pub const MAX_CHECKED_BYTES: usize = 262_144;
/// The most distinct checked files one resolution reads.
pub const MAX_CHECK_READS: usize = 64;
/// The most checked-file content one resolution reads in aggregate.
pub const MAX_CHECK_BYTES: usize = 1_048_576;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CheckIo {
    pub reads: usize,
    pub bytes: usize,
    pub exhausted: bool,
}

struct CheckedFile {
    text: String,
    toml: OnceCell<Option<Value>>,
    json: OnceCell<Option<Value>>,
}
enum Entry {
    Absent,
    Unavailable,
    File(CheckedFile),
}

#[derive(Clone, Copy)]
enum ReadError {
    Unavailable,
    Budget,
}

/// One resolution's checked-file cache. Paths are parser-validated and every
/// file is read once; parsed TOML/JSON is reused by all checks of that file.
pub struct CheckCache {
    root: PathBuf,
    entries: HashMap<String, Entry>,
    io: CheckIo,
}
impl CheckCache {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            entries: HashMap::new(),
            io: CheckIo::default(),
        }
    }
    pub fn stats(&self) -> Value {
        json!({
            "reads":self.io.reads,"bytes":self.io.bytes,"exhausted":self.io.exhausted,
            "cached_files":self.entries.len(),
            "max_reads":MAX_CHECK_READS,"max_bytes":MAX_CHECK_BYTES
        })
    }
    fn entry(&mut self, relative: &str) -> Result<&Entry, ReadError> {
        if self.entries.contains_key(relative) {
            return self.entries.get(relative).ok_or(ReadError::Unavailable);
        }
        if !relative_path(relative) || relative.ends_with('/') {
            let entry = Entry::Unavailable;
            self.entries.insert(relative.to_owned(), entry);
            return self.entries.get(relative).ok_or(ReadError::Unavailable);
        }
        if self.io.reads >= MAX_CHECK_READS || self.io.bytes >= MAX_CHECK_BYTES {
            self.io.exhausted = true;
            return Err(ReadError::Budget);
        }
        let remaining = MAX_CHECK_BYTES - self.io.bytes;
        let entry = match read_inside_budget(&self.root, relative, MAX_CHECKED_BYTES, remaining) {
            Ok(InsideRead::Absent) => Entry::Absent,
            Ok(InsideRead::Bytes(raw)) => {
                self.io.reads += 1;
                self.io.bytes += raw.len();
                Entry::File(CheckedFile {
                    text: String::from_utf8_lossy(&raw).into_owned(),
                    toml: OnceCell::new(),
                    json: OnceCell::new(),
                })
            }
            Ok(InsideRead::BudgetExceeded) => {
                self.io.exhausted = true;
                return Err(ReadError::Budget);
            }
            Err(()) => Entry::Unavailable,
        };
        self.entries.insert(relative.to_owned(), entry);
        self.entries.get(relative).ok_or(ReadError::Unavailable)
    }
}

/// The result of every check of one rule: `verified` (all pass), `failing`
/// (with the first failure), `unchecked` (the rule has none) or `unavailable`
/// (a file could not be read or the aggregate budget was exhausted).
pub fn evaluate(root: &Path, checks: &[Check]) -> Value {
    evaluate_cached(checks, &mut CheckCache::new(root))
}

pub fn evaluate_cached(checks: &[Check], cache: &mut CheckCache) -> Value {
    if checks.is_empty() {
        return json!({"status":"unchecked","passed":0,"total":0});
    }
    let mut passed = 0;
    let mut failure = None;
    let mut unavailable = 0;
    let mut budget_exhausted = false;
    for check in checks {
        match cache.entry(&check.file).and_then(|entry| one(check, entry)) {
            Ok(None) => passed += 1,
            Ok(Some(f)) => {
                failure.get_or_insert(f);
            }
            Err(ReadError::Budget) => {
                unavailable += 1;
                budget_exhausted = true;
            }
            Err(ReadError::Unavailable) => unavailable += 1,
        }
    }
    let status = if failure.is_some() {
        "failing"
    } else if unavailable > 0 {
        "unavailable"
    } else {
        "verified"
    };
    let mut out = json!({"status":status,"passed":passed,"total":checks.len()});
    if let Some(f) = failure {
        out["failure"] = f;
    }
    if unavailable > 0 {
        out["unavailable"] = json!(unavailable);
    }
    if budget_exhausted {
        out["budget_exhausted"] = json!(true);
    }
    out
}

/// The failure of one check, `None` when it passes (an unreadable file is
/// not a failure: nothing is claimed about it).
pub(super) fn failure_cached(check: &Check, cache: &mut CheckCache) -> Option<Value> {
    cache
        .entry(&check.file)
        .ok()
        .and_then(|entry| one(check, entry).ok().flatten())
}

fn short(s: &str) -> String {
    let mut out: String = s.chars().take(120).collect();
    if s.chars().count() > 120 {
        out.push('…');
    }
    out
}

/// `Ok(None)` when the check passes, `Ok(Some(failure))` when it fails.
fn one(c: &Check, entry: &Entry) -> Result<Option<Value>, ReadError> {
    let file = match entry {
        Entry::Absent => return Ok(Some(json!({"file":c.file,"kind":"exists"}))),
        Entry::Unavailable => return Err(ReadError::Unavailable),
        Entry::File(file) => file,
    };
    let text = &file.text;
    if let Some(needle) = &c.contains {
        return Ok((!text.contains(needle.as_str()))
            .then(|| json!({"file":c.file,"kind":"contains","expected":short(needle)})));
    }
    if let Some(needle) = &c.absent {
        return Ok(text
            .contains(needle.as_str())
            .then(|| json!({"file":c.file,"kind":"absent","expected":short(needle)})));
    }
    let (doc, path) = match (&c.toml, &c.json) {
        (Some(path), _) => (
            file.toml.get_or_init(|| {
                toml::from_str::<toml::Value>(text)
                    .ok()
                    .and_then(|v| serde_json::to_value(v).ok())
            }),
            path,
        ),
        (None, Some(path)) => (
            file.json
                .get_or_init(|| serde_json::from_str::<Value>(text).ok()),
            path,
        ),
        (None, None) => return Ok(None),
    };
    let found = doc.as_ref().and_then(|d| lookup(d, path));
    let expected = c.equals.clone().unwrap_or(Value::Null);
    Ok((found != Some(&expected)).then(|| {
        json!({"file":c.file,"kind":"equals","path":path,
            "expected":short(&expected.to_string()),
            "found":found.map(|v| short(&v.to_string()))})
    }))
}

fn lookup<'a>(doc: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(doc, |v, part| match v {
        Value::Object(map) => map.get(part),
        Value::Array(items) => part.parse::<usize>().ok().and_then(|i| items.get(i)),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn check(toml_src: &str) -> Check {
        #[derive(serde::Deserialize)]
        struct W {
            check: Check,
        }
        toml::from_str::<W>(&format!("[check]\n{toml_src}"))
            .unwrap()
            .check
    }

    #[test]
    fn each_kind_passes_and_fails() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nedition = \"2024\"\n[workspace]\nmembers = [\"a\"]\n",
        )
        .unwrap();
        fs::create_dir(root.join("docs")).unwrap();
        fs::write(root.join("docs/guide.md"), "Run cargo fmt first.").unwrap();
        fs::write(root.join("package.json"), r#"{"engines":{"node":">=20"}}"#).unwrap();
        let pass = [
            "file = \"Cargo.toml\"",
            "file = \"docs/guide.md\"\ncontains = \"cargo fmt\"",
            "file = \"docs/guide.md\"\nabsent = \"npm\"",
            "file = \"Cargo.toml\"\ntoml = \"package.edition\"\nequals = \"2024\"",
            "file = \"Cargo.toml\"\ntoml = \"workspace.members.0\"\nequals = \"a\"",
            "file = \"package.json\"\njson = \"engines.node\"\nequals = \">=20\"",
        ];
        let checks: Vec<_> = pass.iter().map(|s| check(s)).collect();
        assert_eq!(evaluate(root, &checks)["status"], "verified", "{pass:?}");
        for (src, kind) in [
            ("file = \"missing.md\"", "exists"),
            (
                "file = \"docs/guide.md\"\ncontains = \"clippy\"",
                "contains",
            ),
            ("file = \"docs/guide.md\"\nabsent = \"cargo\"", "absent"),
            (
                "file = \"Cargo.toml\"\ntoml = \"package.edition\"\nequals = \"2021\"",
                "equals",
            ),
        ] {
            let v = evaluate(root, &[check(src)]);
            assert_eq!(v["status"], "failing", "{src}");
            assert_eq!(v["failure"]["kind"], kind);
        }
        assert_eq!(evaluate(root, &[])["status"], "unchecked");
    }

    #[test]
    fn repeated_files_are_read_once_and_missing_files_are_cached() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let raw = "[package]\nedition = \"2024\"\n";
        fs::write(root.join("Cargo.toml"), raw).unwrap();
        let checks = vec![
            check("file = \"Cargo.toml\"\ntoml = \"package.edition\"\nequals = \"2024\""),
            check("file = \"Cargo.toml\"\ncontains = \"edition\""),
            check("file = \"missing.md\""),
            check("file = \"missing.md\""),
        ];
        let mut cache = CheckCache::new(root);
        let first = evaluate_cached(&checks, &mut cache);
        assert_eq!(first["status"], "failing");
        assert_eq!(first["passed"], 2);
        let stats = cache.stats();
        assert_eq!(stats["reads"], 1);
        assert_eq!(stats["cached_files"], 2);
        assert_eq!(stats["bytes"], raw.len());
        assert_eq!(stats["exhausted"], false);
        let second = evaluate_cached(&checks[..2], &mut cache);
        assert_eq!(second["status"], "verified");
        assert_eq!(cache.stats(), stats);
    }

    #[test]
    fn aggregate_read_count_exhaustion_is_unavailable_not_verified() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut checks = Vec::new();
        for index in 0..MAX_CHECK_READS + 1 {
            let file = format!("file-{index}.txt");
            fs::write(root.join(&file), "x").unwrap();
            checks.push(check(&format!("file = \"{file}\"")));
        }
        let mut cache = CheckCache::new(root);
        let result = evaluate_cached(&checks, &mut cache);
        assert_eq!(result["status"], "unavailable");
        assert_eq!(result["passed"], MAX_CHECK_READS);
        assert_eq!(result["unavailable"], 1);
        assert_eq!(result["budget_exhausted"], true);
        let stats = cache.stats();
        assert_eq!(stats["reads"], MAX_CHECK_READS);
        assert_eq!(stats["exhausted"], true);
    }

    #[test]
    fn aggregate_byte_exhaustion_never_claims_the_remaining_checks_passed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut checks = Vec::new();
        for index in 0..5 {
            let file = format!("large-{index}.txt");
            fs::write(root.join(&file), vec![b'x'; MAX_CHECKED_BYTES]).unwrap();
            checks.push(check(&format!("file = \"{file}\"")));
        }
        let mut cache = CheckCache::new(root);
        let result = evaluate_cached(&checks, &mut cache);
        assert_eq!(result["status"], "unavailable");
        assert_eq!(result["passed"], 4);
        assert_eq!(result["unavailable"], 1);
        assert_eq!(result["budget_exhausted"], true);
        let stats = cache.stats();
        assert_eq!(stats["reads"], 4);
        assert_eq!(stats["bytes"], MAX_CHECK_BYTES);
        assert_eq!(stats["exhausted"], true);
    }

    #[cfg(unix)]
    #[test]
    fn links_are_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret"), "token").unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("out")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret"), dir.path().join("s")).unwrap();
        for file in ["out/secret", "s"] {
            let v = evaluate(
                dir.path(),
                &[check(&format!("file = \"{file}\"\ncontains = \"token\""))],
            );
            assert_eq!(v["failure"]["kind"], "exists", "{file}");
        }
    }
}
