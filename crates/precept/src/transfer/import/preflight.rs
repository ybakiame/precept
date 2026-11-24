use super::report::build_report;
use crate::transfer::export::{page, served};
use crate::transfer::format::{Export, Exported, file_indexes, validate_export};
use precept_ipc::{Error, Location, Result, operator::*};
use serde_json::{Value, json};

pub(in crate::transfer) type PreferenceIdentity = (Scope, String, String, Vec<String>);

#[derive(Clone, PartialEq, Eq)]
pub(in crate::transfer) struct ExistingNote {
    pub(in crate::transfer) kind: String,
    pub(in crate::transfer) title: String,
    pub(in crate::transfer) body: String,
    pub(in crate::transfer) pinned: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(in crate::transfer) struct Observation {
    pub(in crate::transfer) binding: Binding,
    pub(in crate::transfer) write_enabled: bool,
    pub(in crate::transfer) projects: Vec<String>,
    pub(in crate::transfer) existing: Vec<PreferenceIdentity>,
    pub(in crate::transfer) commit_sequence: i64,
    pub(in crate::transfer) schema: &'static str,
    pub(in crate::transfer) notes: std::collections::BTreeMap<String, Vec<ExistingNote>>,
    pub(in crate::transfer) handoffs: Value,
}

pub(super) async fn observe(location: &Location) -> Result<Observation> {
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

pub(super) fn project_name_valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

pub(in crate::transfer) fn preference_identity(p: &Exported) -> Option<PreferenceIdentity> {
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

pub(in crate::transfer) fn preference_plan(
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

pub(in crate::transfer) fn memory_plan(export: &Export, observation: &Observation) -> Value {
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

pub(super) async fn preflight_inner(
    location: &Location,
    export: &Export,
) -> Result<(Value, Observation)> {
    validate_export(export)?;
    let observation = observe(location).await?;
    let report = build_report(export, &observation);
    Ok((report, observation))
}

/// Read-only, redacted import plan. It never returns preference or memory
/// contents, Handoff bodies, paths, credentials or raw errors.
pub async fn preflight(location: &Location, export: &Export) -> Result<Value> {
    preflight_inner(location, export)
        .await
        .map(|(report, _)| report)
}

pub(super) fn same_destination(a: &Observation, b: &Observation) -> bool {
    a.binding == b.binding
        && a.write_enabled == b.write_enabled
        && a.projects == b.projects
        && a.existing == b.existing
        && a.commit_sequence == b.commit_sequence
        && a.schema == b.schema
        && a.notes == b.notes
}
