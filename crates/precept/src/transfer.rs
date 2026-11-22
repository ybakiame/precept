//! Profile export and import: how preferences move between Profiles (a new
//! schema, a new machine) without migration code. Built on the ordinary
//! operator reads and the reviewed write path: import only ever creates
//! suggestions, which the operator confirms again. Project memory travels only
//! when asked for (this machine's environment notes only when asked for too).
//! Handoffs are not carried.
use precept_ipc::{Error, Location, Result, operator::*};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{io::Write, path::Path};

pub const FORMAT: &str = "precept-profile-export";
/// The version written. Version 1 files (no rule options) still import, with
/// the defaults (everywhere, priority 0, advisory); version 3 may carry
/// project memory.
pub const VERSION: u32 = 3;
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

/// One project-memory note (v3).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportedNote {
    pub project: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub pinned: bool,
}

/// Which project memory an export carries.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MemoryChoice {
    None,
    /// Cautions, lessons and attempts.
    Notes,
    /// Those and this machine's environment notes.
    WithEnvironment,
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub format: String,
    pub version: u32,
    pub preferences: Vec<Exported>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memory: Vec<ExportedNote>,
    /// Indexes of records in the file whose scope no longer exists
    /// (`environment`: machine facts are project memory now); skipped.
    #[serde(skip)]
    pub dropped: Vec<usize>,
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

/// The projects the daemon serves.
async fn served(location: &Location) -> Result<Vec<String>> {
    let projects = control(location, "project.list", json!({})).await?;
    Ok(projects
        .as_array()
        .ok_or(Error::Unavailable)?
        .iter()
        .filter(|p| p.get("state").and_then(Value::as_str) == Some("ready"))
        .filter_map(|p| p.get("name").and_then(Value::as_str).map(str::to_owned))
        .collect())
}

/// Every preference of the selected Profile (personal scope and each project
/// the daemon serves) and, when chosen, those projects' memory.
pub async fn collect(location: &Location, memory: MemoryChoice) -> Result<Export> {
    let projects = served(location).await?;
    let mut scopes = vec![Scope::Global];
    scopes.extend(
        projects
            .iter()
            .map(|name| Scope::Project { name: name.clone() }),
    );
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
    let mut notes = Vec::new();
    if memory != MemoryChoice::None {
        for project in &projects {
            let list = control(location, "memories.list", json!({"project":project})).await?;
            for e in list["entries"].as_array().ok_or(Error::Unavailable)? {
                let kind = e["kind"].as_str().ok_or(Error::Unavailable)?;
                if kind == "environment" && memory != MemoryChoice::WithEnvironment {
                    continue;
                }
                notes.push(ExportedNote {
                    project: project.clone(),
                    kind: kind.into(),
                    title: e["title"].as_str().ok_or(Error::Unavailable)?.into(),
                    body: e["body"].as_str().ok_or(Error::Unavailable)?.into(),
                    pinned: e["pinned"] == json!(true),
                });
            }
        }
    }
    Ok(Export {
        format: FORMAT.into(),
        version: VERSION,
        preferences,
        memory: notes,
        dropped: vec![],
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
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct File {
        format: String,
        version: u32,
        preferences: Vec<Value>,
        #[serde(default)]
        memory: Vec<ExportedNote>,
    }
    let file: File = serde_json::from_slice(&raw).map_err(|_| Error::InvalidRequest)?;
    if file.format != FORMAT
        || !(1..=VERSION).contains(&file.version)
        || file.preferences.len() > MAX_RECORDS
        || file.memory.len() > MAX_RECORDS
    {
        return Err(Error::Version);
    }
    let mut preferences = Vec::with_capacity(file.preferences.len());
    let mut dropped = vec![];
    for (index, p) in file.preferences.into_iter().enumerate() {
        if p.pointer("/scope/kind") == Some(&json!("environment")) {
            dropped.push(index);
            continue;
        }
        preferences.push(serde_json::from_value(p).map_err(|_| Error::InvalidRequest)?);
    }
    Ok(Export {
        format: file.format,
        version: file.version,
        preferences,
        memory: file.memory,
        dropped,
    })
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
    let existing: Vec<(Scope, String, String, Vec<String>)> = collect(location, MemoryChoice::None)
        .await?
        .preferences
        .into_iter()
        .map(|e| (e.scope, e.key, e.value, e.tags))
        .collect();
    let mut suggested = 0;
    let mut skipped: Vec<Value> = export
        .dropped
        .iter()
        .map(|i| json!({"file_index":i,"reason":"environment"}))
        .collect();
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
    // Memory notes go to the projects this daemon serves, grouped per
    // project; a project it does not serve is skipped.
    let projects = served(location).await?;
    let mut memory = serde_json::Map::new();
    let mut by_project: std::collections::BTreeMap<&str, Vec<Value>> = Default::default();
    for n in &export.memory {
        by_project
            .entry(n.project.as_str())
            .or_default()
            .push(json!({"kind":n.kind,"title":n.title,"body":n.body,"pinned":n.pinned}));
    }
    for (project, entries) in by_project {
        let outcome = if !projects.iter().any(|p| p == project) {
            json!("not_served")
        } else {
            match control(
                location,
                "memories.import",
                json!({"project":project,"entries":entries}),
            )
            .await
            {
                Ok(v) => v,
                Err(e) => json!({"error":e}),
            }
        };
        memory.insert(project.to_owned(), outcome);
    }
    Ok(
        json!({"suggested":suggested,"skipped":skipped,"total":export.preferences.len() + export.dropped.len(),"memory":memory}),
    )
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
            memory: vec![ExportedNote {
                project: "alpha".into(),
                kind: "lesson".into(),
                title: "t".into(),
                body: "b".into(),
                pinned: true,
            }],
            dropped: vec![],
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
        // Environment records of older files are skipped, not refused.
        std::fs::write(
            &path,
            r#"{"format":"precept-profile-export","version":2,"preferences":[
              {"scope":{"kind":"environment","name":"alpha"},"key":"shell","value":"WSL",
               "tags":["setup"],"origin":"agent","status":"confirmed"},
              {"scope":{"kind":"global"},"key":"k","value":"v","tags":["style"],
               "origin":"operator","status":"confirmed"}]}"#,
        )
        .unwrap();
        let read_back = read(&path).unwrap();
        assert_eq!(read_back.dropped, vec![0]);
        assert_eq!(read_back.preferences.len(), 1);
        assert_eq!(read_back.preferences[0].key, "k");
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(valid_prefix("move-2026"));
        assert!(!valid_prefix("bad prefix") && !valid_prefix("") && !valid_prefix(&"x".repeat(41)));
    }
}
