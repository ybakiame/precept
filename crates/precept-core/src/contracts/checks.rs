//! One evaluator for a rule's checks against the repository: configuration
//! values and claims in documents alike. Read-only, bounded, inside the root.
use super::parser::Check;
use super::snapshot::read_inside;
use serde_json::{Value, json};
use std::path::Path;

/// The largest file a check reads.
pub const MAX_CHECKED_BYTES: usize = 262_144;

/// The result of every check of one rule: `verified` (all pass), `failing`
/// (with the first failure), `unchecked` (the rule has none) or `unavailable`
/// (a file could not be read).
pub fn evaluate(root: &Path, checks: &[Check]) -> Value {
    if checks.is_empty() {
        return json!({"status":"unchecked","passed":0,"total":0});
    }
    let mut passed = 0;
    let mut failure = None;
    let mut unavailable = false;
    for check in checks {
        match one(root, check) {
            Ok(None) => passed += 1,
            Ok(Some(f)) => {
                failure.get_or_insert(f);
            }
            Err(()) => unavailable = true,
        }
    }
    let status = if failure.is_some() {
        "failing"
    } else if unavailable {
        "unavailable"
    } else {
        "verified"
    };
    let mut out = json!({"status":status,"passed":passed,"total":checks.len()});
    if let Some(f) = failure {
        out["failure"] = f;
    }
    out
}

fn short(s: &str) -> String {
    let mut out: String = s.chars().take(120).collect();
    if s.chars().count() > 120 {
        out.push('…');
    }
    out
}

/// `Ok(None)` when the check passes, `Ok(Some(failure))` when it fails.
fn one(root: &Path, c: &Check) -> Result<Option<Value>, ()> {
    let Some(raw) = read_inside(root, &c.file, MAX_CHECKED_BYTES)? else {
        return Ok(Some(json!({"file":c.file,"kind":"exists"})));
    };
    let text = String::from_utf8_lossy(&raw);
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
            toml::from_str::<toml::Value>(&text)
                .ok()
                .and_then(|v| serde_json::to_value(v).ok()),
            path,
        ),
        (None, Some(path)) => (serde_json::from_str::<Value>(&text).ok(), path),
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
