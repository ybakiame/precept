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

fn file_indexes(export: &Export) -> Vec<usize> {
    let dropped: std::collections::BTreeSet<_> = export.dropped.iter().copied().collect();
    (0..export.preferences.len() + dropped.len())
        .filter(|index| !dropped.contains(index))
        .collect()
}

fn validate_export(export: &Export) -> Result<()> {
    if export.format != FORMAT || !(1..=VERSION).contains(&export.version) {
        return Err(Error::Version);
    }
    if export.preferences.len() > MAX_RECORDS || export.memory.len() > MAX_RECORDS {
        return Err(Error::TooLarge);
    }
    Ok(())
}

type PreferenceIdentity = (Scope, String, String, Vec<String>);

#[derive(Clone, PartialEq, Eq)]
struct ExistingNote {
    kind: String,
    title: String,
    body: String,
    pinned: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct Observation {
    binding: Binding,
    write_enabled: bool,
    projects: Vec<String>,
    existing: Vec<PreferenceIdentity>,
    commit_sequence: i64,
    schema: &'static str,
    notes: std::collections::BTreeMap<String, Vec<ExistingNote>>,
    handoffs: Value,
}

async fn observe(location: &Location) -> Result<Observation> {
    let info = control(location, "profile.info", json!({})).await?;
    if info["selected"] != json!(true) {
        return Err(Error::Unavailable);
    }
    let binding: Binding =
        serde_json::from_value(info["binding"].clone()).map_err(|_| Error::Unavailable)?;
    let projects = served(location).await?;
    let mut existing = Vec::new();
    let mut scopes = vec![Scope::Global];
    scopes.extend(
        projects
            .iter()
            .map(|name| Scope::Project { name: name.clone() }),
    );
    for scope in scopes {
        let mut after = 0;
        loop {
            let page = page(location, &scope, after).await?;
            if page.binding != binding {
                return Err(Error::StaleGeneration);
            }
            existing.extend(
                page.records
                    .into_iter()
                    .map(|r| (r.scope, r.key, r.value, r.tags)),
            );
            match page.next_after_id {
                Some(next) if next > after => after = next,
                _ => break,
            }
        }
    }
    // Review count is a read-only way to obtain the Profile-wide commit
    // watermark. It is not an expected revision on the write itself.
    let count: ReviewCount = serde_json::from_value(
        control(
            location,
            "profile.review.count",
            json!({"scope":Scope::Global}),
        )
        .await?,
    )
    .map_err(|_| Error::Unavailable)?;
    if count.binding != binding || count.commit_sequence < 0 {
        return Err(Error::StaleGeneration);
    }
    // These exact native Profile reads validate the one supported schema;
    // there is no version negotiation, migration or downgrade path.
    let schema = "Version1";
    let mut notes = std::collections::BTreeMap::new();
    let mut handoff_projects = Vec::new();
    let mut handoff_listed = 0usize;
    let mut handoffs_complete = true;
    for project in &projects {
        let list = control(location, "memories.list", json!({"project":project})).await?;
        let mut entries = Vec::new();
        for entry in list["entries"].as_array().ok_or(Error::Unavailable)? {
            entries.push(ExistingNote {
                kind: entry["kind"].as_str().ok_or(Error::Unavailable)?.into(),
                title: entry["title"].as_str().ok_or(Error::Unavailable)?.into(),
                body: entry["body"].as_str().ok_or(Error::Unavailable)?.into(),
                pinned: entry["pinned"] == json!(true),
            });
        }
        notes.insert(project.clone(), entries);
        let list = control(location, "profile.handoff.list", json!({"project":project})).await;
        let listed = match list {
            Ok(value) => {
                value["handoffs"]
                    .as_array()
                    .map_or(Err(Error::Unavailable), |entries| {
                        if entries.len() == 50 {
                            handoffs_complete = false;
                        }
                        Ok(Some(entries.len()))
                    })?
            }
            Err(_) => {
                handoffs_complete = false;
                None
            }
        };
        handoff_projects.push(json!({
            "project":project,
            "listed":listed,
            "complete":listed.is_some_and(|n| n < 50)
        }));
        handoff_listed += listed.unwrap_or(0);
    }
    let handoffs = json!({
        "carried":0,
        "reason":"format_never_carries_handoffs",
        "destination_listed":handoff_listed,
        "destination_count_complete":handoffs_complete,
        "served_projects":handoff_projects
    });
    Ok(Observation {
        binding,
        write_enabled: info["write_enabled"] == json!(true),
        projects,
        existing,
        commit_sequence: count.commit_sequence,
        schema,
        notes,
        handoffs,
    })
}

fn project_name_valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

fn preference_identity(p: &Exported) -> Option<PreferenceIdentity> {
    let tags = canonical_tags(&p.tags)?;
    Some((p.scope.clone(), p.key.clone(), p.value.clone(), tags))
}

fn preference_refusal(p: &Exported, projects: &[String]) -> Option<&'static str> {
    if !matches!(p.status.as_str(), "confirmed" | "suggested") {
        return Some("status");
    }
    if let Scope::Project { name } = &p.scope
        && (!project_name_valid(name) || !projects.iter().any(|project| project == name))
    {
        return Some(if project_name_valid(name) {
            "not_served"
        } else {
            "invalid_scope"
        });
    }
    if canonical_tags(&p.tags).is_none() {
        return Some("tags");
    }
    let text_ok = |s: &str, max| {
        !s.contains('\0')
            && s.chars().count() <= max
            && s.chars().any(|c| !c.is_control() && !c.is_whitespace())
    };
    if !text_ok(&p.key, 120) || !text_ok(&p.value, 4096) || !p.rule.valid() {
        return Some("invalid_record");
    }
    None
}

fn preference_plan(
    export: &Export,
    projects: &[String],
    existing: &[PreferenceIdentity],
) -> Vec<Value> {
    let indexes = file_indexes(export);
    let mut known = existing.to_vec();
    let mut plans = Vec::with_capacity(export.preferences.len() + export.dropped.len());
    for (index, p) in export.preferences.iter().enumerate() {
        let file_index = indexes[index];
        let reason = preference_refusal(p, projects).or_else(|| {
            let identity = preference_identity(p)?;
            if known.contains(&identity) {
                Some(if existing.contains(&identity) {
                    "exists"
                } else {
                    "source_duplicate"
                })
            } else {
                known.push(identity);
                None
            }
        });
        plans.push(json!({
            "file_index":file_index,
            "action":if reason.is_some() { "skip" } else { "suggest" },
            "conflict":matches!(reason, Some("exists" | "source_duplicate")).then_some(reason),
            "skip_reason":reason
        }));
    }
    let mut omitted = export.dropped.to_vec();
    omitted.sort_unstable();
    for file_index in omitted {
        plans.push(json!({
            "file_index":file_index,
            "action":"skip",
            "conflict":null,
            "skip_reason":"environment"
        }));
    }
    plans.sort_by_key(|plan| plan["file_index"].as_u64().unwrap_or(0));
    plans
}

fn memory_text(raw: &str, max: usize) -> Option<String> {
    let text = raw.trim();
    (!text.is_empty()
        && text.chars().count() <= max
        && !text
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t'))
    .then(|| text.to_owned())
}

fn normalized_title(title: &str) -> String {
    title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn memory_plan(export: &Export, observation: &Observation) -> Value {
    let mut simulated = observation.notes.clone();
    let mut plans = Vec::with_capacity(export.memory.len());
    let mut counts: std::collections::BTreeMap<&str, serde_json::Map<String, Value>> =
        Default::default();
    let mut unserved = std::collections::BTreeSet::new();
    for (index, note) in export.memory.iter().enumerate() {
        let project_ok = project_name_valid(&note.project);
        let served = project_ok && observation.projects.iter().any(|p| p == &note.project);
        if project_ok && !served {
            unserved.insert(note.project.as_str());
        }
        let kind_ok = matches!(
            note.kind.as_str(),
            "caution" | "lesson" | "attempt" | "environment"
        );
        let title = memory_text(&note.title, 120);
        let body = memory_text(&note.body, 2000);
        let reason = if !project_ok {
            Some("invalid_project")
        } else if !served {
            Some("not_served")
        } else if !kind_ok {
            Some("invalid_kind")
        } else if title.is_none() || body.is_none() {
            Some("invalid_record")
        } else {
            None
        };
        let mut action = "skip";
        let mut pin_change = false;
        if reason.is_none() {
            let title = title.as_deref().unwrap_or_default();
            let body = body.as_deref().unwrap_or_default();
            let entries = simulated.entry(note.project.clone()).or_default();
            let same = entries.iter().position(|entry| {
                entry.kind == note.kind && normalized_title(&entry.title) == normalized_title(title)
            });
            let position = match same {
                Some(position) => {
                    action = if entries[position].body == body {
                        "reaffirm"
                    } else {
                        entries[position].body = body.to_owned();
                        "correct"
                    };
                    position
                }
                None => {
                    entries.push(ExistingNote {
                        kind: note.kind.clone(),
                        title: title.to_owned(),
                        body: body.to_owned(),
                        pinned: false,
                    });
                    action = "add";
                    entries.len() - 1
                }
            };
            pin_change = note.pinned && !entries[position].pinned;
            if note.pinned {
                entries[position].pinned = true;
            }
            let count = counts.entry(note.project.as_str()).or_default();
            let n = count.get(action).and_then(Value::as_u64).unwrap_or(0);
            count.insert(action.into(), json!(n + 1));
            if pin_change {
                let n = count.get("pin").and_then(Value::as_u64).unwrap_or(0);
                count.insert("pin".into(), json!(n + 1));
            }
        }
        plans.push(json!({
            "memory_index":index,
            "project":if project_ok { Some(note.project.as_str()) } else { None },
            "action":action,
            "conflict":null,
            "skip_reason":reason,
            "pin_change":pin_change
        }));
    }
    let projects: serde_json::Map<_, _> = counts
        .iter()
        .map(|(project, count)| {
            (
                (*project).to_owned(),
                json!({"action":"import","planned":count}),
            )
        })
        .collect();
    let environment = export
        .memory
        .iter()
        .filter(|note| note.kind == "environment")
        .count();
    json!({
        "notes":plans,
        "projects":projects,
        "unserved_projects":unserved,
        "environment":{
            "included":environment > 0,
            "included_count":environment,
            "omitted":environment == 0,
            "reason":if environment == 0 { Some("not_carried_by_source_export") } else { None }
        }
    })
}

async fn preflight_inner(location: &Location, export: &Export) -> Result<(Value, Observation)> {
    validate_export(export)?;
    let observation = observe(location).await?;
    let preferences = preference_plan(export, &observation.projects, &observation.existing);
    let omitted_environment: Vec<_> = preferences
        .iter()
        .filter(|plan| plan["skip_reason"] == json!("environment"))
        .filter_map(|plan| plan["file_index"].as_u64())
        .collect();
    let conflicts: Vec<_> = preferences
        .iter()
        .filter(|plan| plan["action"] == json!("skip"))
        .map(|plan| json!({"file_index":plan["file_index"],"reason":plan["skip_reason"]}))
        .collect();
    let mut unserved = std::collections::BTreeSet::new();
    for p in &export.preferences {
        if let Scope::Project { name } = &p.scope
            && project_name_valid(name)
            && !observation.projects.contains(name)
        {
            unserved.insert(name.as_str());
        }
    }
    let report = json!({
        "source":{"format":export.format,"version":export.version},
        "destination":{
            "selected":true,
            "write_enabled":observation.write_enabled,
            "schema":observation.schema,
            "binding":{
                "profile_id":observation.binding.profile_id,
                "generation":observation.binding.generation,
                "owner_epoch":observation.binding.owner_epoch
            }
        },
        "consistency":{
            "watermark":observation.commit_sequence,
            "strategy":"binding_and_profile_commit_sequence",
            "preflight_rechecked_before_apply":false,
            "limitation":"Preflight is not a transaction; each reviewed write rechecks the binding and exact absence. Operation-id receipts are not readable by preflight. Apply-time authorization, capacity and storage refusals remain authoritative and are reported per file_index."
        },
        "preferences":preferences,
        "conflicts":conflicts,
        "unserved_projects":unserved,
        "omitted":{
            "environment_scope":{"file_indexes":omitted_environment},
            "handoffs":observation.handoffs
        },
        "memory":memory_plan(export, &observation)
    });
    Ok((report, observation))
}

/// Read-only, redacted import plan. It never returns preference or memory
/// contents, Handoff bodies, paths, credentials or raw errors.
pub async fn preflight(location: &Location, export: &Export) -> Result<Value> {
    preflight_inner(location, export)
        .await
        .map(|(report, _)| report)
}

fn same_destination(a: &Observation, b: &Observation) -> bool {
    a.binding == b.binding
        && a.write_enabled == b.write_enabled
        && a.projects == b.projects
        && a.existing == b.existing
        && a.commit_sequence == b.commit_sequence
        && a.schema == b.schema
        && a.notes == b.notes
}

/// Suggests every confirmed or pending preference of `export` into the selected
/// Profile, one reviewed write each (`<prefix>-<file_index>`, so an uncertain
/// write can be retried exactly). Apply reruns preflight and rechecks its
/// destination snapshot before the first write. Nothing is confirmed here.
pub async fn import(location: &Location, export: &Export, prefix: &str) -> Result<Value> {
    if !valid_prefix(prefix) {
        return Err(Error::InvalidRequest);
    }
    let (mut report, before) = preflight_inner(location, export).await?;
    // Writes must be on: refuse up front rather than skip every record.
    if !before.write_enabled {
        return Err(Error::Unauthorized);
    }
    let after = observe(location).await?;
    if !same_destination(&before, &after) {
        return Err(Error::Conflict);
    }
    report["consistency"]["preflight_rechecked_before_apply"] = json!(true);
    let indexes = file_indexes(export);
    let plans = report["preferences"]
        .as_array()
        .ok_or(Error::Unavailable)?
        .clone();
    let mut suggested = 0;
    let mut skipped = Vec::new();
    for plan in &plans {
        if plan["action"] != json!("suggest") {
            skipped.push(json!({"file_index":plan["file_index"],"reason":plan["skip_reason"]}));
            continue;
        }
        let file_index = plan["file_index"].as_u64().ok_or(Error::Unavailable)? as usize;
        let index = indexes
            .iter()
            .position(|candidate| *candidate == file_index)
            .ok_or(Error::Unavailable)?;
        let p = &export.preferences[index];
        let tags = canonical_tags(&p.tags).ok_or(Error::InvalidRequest)?;
        let result = mutate(
            location,
            &Mutation {
                version: 2,
                binding: before.binding.clone(),
                operation_id: format!("{prefix}-{file_index}"),
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
            Err(e) => skipped.push(json!({"file_index":file_index,"reason":e})),
        }
    }
    let mut memory = serde_json::Map::new();
    let mut by_project: std::collections::BTreeMap<&str, Vec<Value>> = Default::default();
    let memory_plans = report["memory"]["notes"]
        .as_array()
        .ok_or(Error::Unavailable)?;
    for plan in memory_plans {
        if plan["action"] == json!("skip") {
            continue;
        }
        let index = plan["memory_index"].as_u64().ok_or(Error::Unavailable)? as usize;
        let note = &export.memory[index];
        by_project.entry(note.project.as_str()).or_default().push(
            json!({"kind":note.kind,"title":note.title,"body":note.body,"pinned":note.pinned}),
        );
    }
    if let Some(projects) = report["memory"]["unserved_projects"].as_array() {
        for project in projects {
            if let Some(project) = project.as_str() {
                memory.insert(project.to_owned(), json!("not_served"));
            }
        }
    }
    for (project, entries) in by_project {
        let outcome = if !before.projects.iter().any(|p| p == project) {
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
    Ok(json!({
        "suggested":suggested,
        "skipped":skipped,
        "total":export.preferences.len() + export.dropped.len(),
        "memory":memory,
        "preflight":report
    }))
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

    fn preference(status: &str, key: &str, scope: Scope) -> Exported {
        Exported {
            scope,
            key: key.into(),
            value: format!("private-value-{key}"),
            tags: vec!["test".into()],
            origin: Origin::Operator,
            status: status.into(),
            rule: Default::default(),
            enabled: true,
        }
    }

    #[test]
    fn preference_plans_keep_original_indexes_and_name_every_refusal() {
        let mut duplicate = preference("confirmed", "duplicate", Scope::Global);
        duplicate.value = "private-value-duplicate".into();
        let mut bad_tags = preference("confirmed", "bad-tags", Scope::Global);
        bad_tags.tags = vec!["bad/tag".into()];
        let mut bad_rule = preference("suggested", "bad-rule", Scope::Global);
        bad_rule.rule.priority = 101;
        let export = Export {
            format: FORMAT.into(),
            version: VERSION,
            preferences: vec![
                duplicate.clone(),
                preference("suggested", "new", Scope::Global),
                preference("confirmed", "new", Scope::Global),
                preference("rejected", "rejected", Scope::Global),
                preference("deprecated", "deprecated", Scope::Global),
                preference(
                    "confirmed",
                    "remote",
                    Scope::Project {
                        name: "gamma".into(),
                    },
                ),
                bad_tags,
                bad_rule,
            ],
            memory: vec![],
            dropped: vec![0, 3],
        };
        let projects = vec!["alpha".to_owned()];
        let existing = vec![preference_identity(&duplicate).unwrap()];
        let plans = preference_plan(&export, &projects, &existing);
        assert_eq!(file_indexes(&export), vec![1, 2, 4, 5, 6, 7, 8, 9]);
        let summarized: Vec<_> = plans
            .iter()
            .map(|plan| {
                (
                    plan["file_index"].as_u64().unwrap(),
                    plan["action"].as_str().unwrap(),
                    plan["conflict"].as_str(),
                    plan["skip_reason"].as_str(),
                )
            })
            .collect();
        assert_eq!(
            summarized,
            vec![
                (0, "skip", None, Some("environment")),
                (1, "skip", Some("exists"), Some("exists")),
                (2, "suggest", None, None),
                (3, "skip", None, Some("environment")),
                (
                    4,
                    "skip",
                    Some("source_duplicate"),
                    Some("source_duplicate")
                ),
                (5, "skip", None, Some("status")),
                (6, "skip", None, Some("status")),
                (7, "skip", None, Some("not_served")),
                (8, "skip", None, Some("tags")),
                (9, "skip", None, Some("invalid_record")),
            ]
        );
        let redacted = serde_json::to_string(&plans).unwrap();
        assert!(!redacted.contains("private-value"));
        assert!(!redacted.contains("bad-rule"));
        assert!(!redacted.contains("remote"));
    }

    #[test]
    fn memory_plans_are_selective_sequential_and_redacted() {
        let export = Export {
            format: FORMAT.into(),
            version: VERSION,
            preferences: vec![],
            memory: vec![
                ExportedNote {
                    project: "alpha".into(),
                    kind: "lesson".into(),
                    title: " Private title ".into(),
                    body: "private-body".into(),
                    pinned: true,
                },
                ExportedNote {
                    project: "alpha".into(),
                    kind: "lesson".into(),
                    title: "private title".into(),
                    body: "private-body".into(),
                    pinned: false,
                },
                ExportedNote {
                    project: "alpha".into(),
                    kind: "environment".into(),
                    title: "runtime".into(),
                    body: "private-environment".into(),
                    pinned: false,
                },
                ExportedNote {
                    project: "gamma".into(),
                    kind: "lesson".into(),
                    title: "remote".into(),
                    body: "private-remote".into(),
                    pinned: false,
                },
            ],
            dropped: vec![],
        };
        let observation = Observation {
            binding: Binding {
                profile_id: "0".repeat(32),
                generation: "1".repeat(32),
                owner_epoch: 1,
            },
            write_enabled: false,
            projects: vec!["alpha".into()],
            existing: vec![],
            commit_sequence: 0,
            schema: "Version1",
            notes: [(
                "alpha".into(),
                vec![ExistingNote {
                    kind: "lesson".into(),
                    title: "Private title".into(),
                    body: "old-private-body".into(),
                    pinned: false,
                }],
            )]
            .into(),
            handoffs: json!({}),
        };
        let plan = memory_plan(&export, &observation);
        assert_eq!(plan["notes"][0]["action"], "correct");
        assert_eq!(plan["notes"][0]["pin_change"], true);
        assert_eq!(plan["notes"][1]["action"], "reaffirm");
        assert_eq!(plan["notes"][2]["action"], "add");
        assert_eq!(plan["notes"][3]["skip_reason"], "not_served");
        assert_eq!(plan["environment"]["included_count"], 1);
        assert_eq!(plan["projects"]["alpha"]["planned"]["correct"], 1);
        assert_eq!(
            plan["projects"]["alpha"]["planned"]["reaffirmed"],
            json!(null)
        );
        let redacted = serde_json::to_string(&plan).unwrap();
        assert!(!redacted.contains("private-"));
        assert!(!redacted.contains("Private title"));
        let without_environment = Export {
            memory: vec![export.memory[0].clone()],
            ..export
        };
        let plan = memory_plan(
            &without_environment,
            &Observation {
                notes: Default::default(),
                ..observation
            },
        );
        assert_eq!(plan["environment"]["omitted"], true);
        assert_eq!(plan["notes"][0]["action"], "add");
    }

    #[test]
    fn preflight_validation_refuses_other_schemas_without_fallback() {
        let export = Export {
            format: FORMAT.into(),
            version: VERSION + 1,
            preferences: vec![],
            memory: vec![],
            dropped: vec![],
        };
        assert_eq!(validate_export(&export), Err(Error::Version));
        assert_eq!(
            validate_export(&Export {
                format: "other".into(),
                ..export
            }),
            Err(Error::Version)
        );
    }
}
