use super::preflight::{observe, preflight_inner, same_destination};
use crate::transfer::format::{Export, file_indexes, valid_prefix};
use precept_ipc::{Error, Location, Result, operator::*};
use serde_json::{Value, json};

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
