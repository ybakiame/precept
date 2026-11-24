use super::format::{Export, Exported, ExportedNote, FORMAT, MAX_RECORDS, MemoryChoice, VERSION};
use precept_ipc::{Error, Location, Result, operator::*};
use serde_json::{Value, json};

pub(super) async fn page(
    location: &Location,
    scope: &Scope,
    after_id: i64,
) -> Result<PreferencePage> {
    let v = control(
        location,
        "profile.preferences.list",
        json!({"scope":scope,"after_id":after_id}),
    )
    .await?;
    serde_json::from_value(v).map_err(|_| Error::InvalidRequest)
}

/// The projects the daemon serves.
pub(super) async fn served(location: &Location) -> Result<Vec<String>> {
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
