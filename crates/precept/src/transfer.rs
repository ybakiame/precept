//! Profile export and import: how preferences move between Profiles (a new
//! schema, a new machine) without migration code. Built on the ordinary
//! operator reads and the reviewed write path: import only ever creates
//! suggestions, which the operator confirms again. Handoffs are not carried.
use precept_ipc::{Error, Location, Result, operator::*};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{io::Write, path::Path};

pub const FORMAT: &str = "precept-profile-export";
/// The version written. Version 1 files (no rule options) still import, with
/// the defaults (everywhere, priority 0, advisory).
pub const VERSION: u32 = 2;
/// The most preferences one export or import carries.
pub const MAX_RECORDS: usize = 10_000;
/// The largest export file read back.
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exported {
    pub scope: Scope,
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub origin: Origin,
    /// The status in the source Profile (`confirmed`, `suggested`, ...).
    pub status: String,
    /// Where it applies, priority, declared enforcement (v2).
    #[serde(default)]
    pub rule: RuleOptions,
    /// Whether it was switched on in the source Profile (v2; informative).
    #[serde(default = "on")]
    pub enabled: bool,
}
fn on() -> bool {
    true
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub format: String,
    pub version: u32,
    pub preferences: Vec<Exported>,
}

async fn page(location: &Location, scope: &Scope, after_id: i64) -> Result<PreferencePage> {
    let v = control(
        location,
        "profile.preferences.list",
        json!({"scope":scope,"after_id":after_id}),
    )
    .await?;
    serde_json::from_value(v).map_err(|_| Error::InvalidRequest)
}

/// Every preference of the selected Profile: personal scope and each project
/// the daemon serves.
pub async fn collect(location: &Location) -> Result<Export> {
    let projects = control(location, "project.list", json!({})).await?;
    let mut scopes = vec![Scope::Global];
    for p in projects.as_array().ok_or(Error::Unavailable)? {
        if p.get("state").and_then(Value::as_str) == Some("ready")
            && let Some(name) = p.get("name").and_then(Value::as_str)
        {
            scopes.push(Scope::Project { name: name.into() });
        }
    }
    let mut preferences = Vec::new();
    for scope in scopes {
        let mut after = 0;
        loop {
            let page = page(location, &scope, after).await?;
            for r in page.records {
                if preferences.len() == MAX_RECORDS {
                    return Err(Error::TooLarge);
                }
                preferences.push(Exported {
                    scope: r.scope,
                    key: r.key,
                    value: r.value,
                    tags: r.tags,
                    origin: r.origin,
                    status: r.status,
                    rule: r.rule,
                    enabled: r.enabled,
                });
            }
            match page.next_after_id {
                Some(next) if next > after => after = next,
                _ => break,
            }
        }
    }
    Ok(Export {
        format: FORMAT.into(),
        version: VERSION,
        preferences,
    })
}

/// Writes `export` to a new private file (never overwrites).
pub fn write_new(path: &Path, export: &Export) -> Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => Error::Conflict,
        _ => Error::Unavailable,
    })?;
    let raw = serde_json::to_vec_pretty(export).map_err(|_| Error::Unavailable)?;
    file.write_all(&raw).map_err(|_| Error::Unavailable)?;
    file.sync_all().map_err(|_| Error::Unavailable)
}

/// Reads and validates an export file.
pub fn read(path: &Path) -> Result<Export> {
    let meta = std::fs::metadata(path).map_err(|_| Error::NotFound)?;
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Err(Error::InvalidRequest);
    }
    let raw = std::fs::read(path).map_err(|_| Error::Unavailable)?;
    let export: Export = serde_json::from_slice(&raw).map_err(|_| Error::InvalidRequest)?;
    if export.format != FORMAT
        || !(1..=VERSION).contains(&export.version)
        || export.preferences.len() > MAX_RECORDS
    {
        return Err(Error::Version);
    }
    Ok(export)
}

/// Whether `prefix` makes valid operation ids (`<prefix>-<index>`).
pub fn valid_prefix(prefix: &str) -> bool {
    !prefix.is_empty()
        && prefix.len() <= 40
        && prefix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

/// Suggests every confirmed or pending preference of `export` into the selected
/// Profile, one reviewed write each (`<prefix>-<index>`, so an uncertain write
/// can be retried exactly). Skipped: other statuses (rejected, deprecated),
/// records the Profile already holds (same scope, key, value and tags, so a
/// repeat adds nothing), and project records whose project it does not serve.
pub async fn import(location: &Location, export: &Export, prefix: &str) -> Result<Value> {
    if !valid_prefix(prefix) {
        return Err(Error::InvalidRequest);
    }
    let info = control(location, "profile.info", json!({})).await?;
    // Writes must be on: refuse up front rather than skip every record.
    if info["write_enabled"] != json!(true) {
        return Err(Error::Unauthorized);
    }
    let binding: Binding =
        serde_json::from_value(info["binding"].clone()).map_err(|_| Error::Unavailable)?;
    // What the Profile already holds (any status): never suggested twice.
    let existing: Vec<(Scope, String, String, Vec<String>)> = collect(location)
        .await?
        .preferences
        .into_iter()
        .map(|e| (e.scope, e.key, e.value, e.tags))
        .collect();
    let mut suggested = 0;
    let mut skipped = Vec::new();
    for (index, p) in export.preferences.iter().enumerate() {
        if !matches!(p.status.as_str(), "confirmed" | "suggested") {
            skipped.push(json!({"index":index,"reason":"status"}));
            continue;
        }
        if existing
            .iter()
            .any(|(s, k, v, t)| *s == p.scope && *k == p.key && *v == p.value && *t == p.tags)
        {
            skipped.push(json!({"index":index,"reason":"exists"}));
            continue;
        }
        let Some(tags) = canonical_tags(&p.tags) else {
            skipped.push(json!({"index":index,"reason":"tags"}));
            continue;
        };
        let result = mutate(
            location,
            &Mutation {
                version: 2,
                binding: binding.clone(),
                operation_id: format!("{prefix}-{index}"),
                command: Command::SuggestPreference {
                    rule: p.rule.clone(),
                    scope: p.scope.clone(),
                    key: p.key.clone(),
                    value: p.value.clone(),
                    tags,
                    origin: p.origin,
                    expected: Expected::Absent,
                },
            },
        )
        .await;
        match result {
            Ok(_) => suggested += 1,
            Err(e) => skipped.push(json!({"index":index,"reason":e})),
        }
    }
    Ok(json!({"suggested":suggested,"skipped":skipped,"total":export.preferences.len()}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_new_private_versioned_and_bounded() {
        let dir = std::env::temp_dir().join(format!("precept-transfer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("export.json");
        let _ = std::fs::remove_file(&path);
        let export = Export {
            format: FORMAT.into(),
            version: VERSION,
            preferences: vec![Exported {
                scope: Scope::Global,
                key: "k".into(),
                value: "v".into(),
                tags: vec!["style".into()],
                origin: Origin::Operator,
                status: "confirmed".into(),
                rule: Default::default(),
                enabled: true,
            }],
        };
        write_new(&path, &export).unwrap();
        assert_eq!(write_new(&path, &export), Err(Error::Conflict));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert!(read(&path).ok() == Some(export));
        std::fs::write(&path, r#"{"format":"other","version":1,"preferences":[]}"#).unwrap();
        assert!(matches!(read(&path), Err(Error::Version)));
        std::fs::write(
            &path,
            r#"{"format":"precept-profile-export","version":1,"preferences":[],"x":1}"#,
        )
        .unwrap();
        assert!(matches!(read(&path), Err(Error::InvalidRequest)));
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(valid_prefix("move-2026"));
        assert!(!valid_prefix("bad prefix") && !valid_prefix("") && !valid_prefix(&"x".repeat(41)));
    }
}
