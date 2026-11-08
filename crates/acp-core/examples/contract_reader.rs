//! Synthetic JSON adapter for parity testing, not a replacement CLI or MCP server.
use acp_core::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, Read},
};

#[derive(Deserialize)]
struct Dataset {
    projects: Vec<Project>,
    preferences: Vec<Preference>,
    handoffs: Vec<Handoff>,
    worktrees: BTreeMap<String, Worktree>,
    requests: Vec<Request>,
}
#[derive(Deserialize)]
struct Request {
    project: String,
    name: String,
    arguments: Value,
}
impl ProjectRegistry for Dataset {
    fn project(&self, name: &str) -> Result<Project> {
        self.projects
            .iter()
            .find(|p| p.name == name)
            .cloned()
            .ok_or(Error::UnknownProject)
    }
}
impl PreferenceRepository for Dataset {
    fn preferences(&self, _: &str) -> Result<Vec<Preference>> {
        Ok(self.preferences.clone())
    }
}
impl HandoffRepository for Dataset {
    fn handoff(&self, project: &str, task: &str) -> Result<Handoff> {
        self.handoffs
            .iter()
            .find(|h| h.project == project && h.task_id == task)
            .cloned()
            .ok_or(Error::HandoffNotFound)
    }
}
impl WorktreeInspector for Dataset {
    fn inspect(&self, project: &Project, _: bool) -> Result<Worktree> {
        self.worktrees
            .get(&project.name)
            .cloned()
            .ok_or(Error::AdapterUnavailable)
    }
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin()
        .take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut input)?;
    if input.len() > 4 * 1024 * 1024 {
        return Err("synthetic input too large".into());
    }
    let data: Dataset = serde_json::from_str(&input)?;
    let answers: Vec<_> = data
        .requests
        .iter()
        .map(|r| {
            match ContextResolver::new(
                &r.project,
                &data,
                &data,
                &data,
                &data,
                &documents::FileDocuments,
            )
            .and_then(|resolver| resolver.read(&r.name, &r.arguments))
            {
                Ok(value) => json!({"data": value}),
                Err(_) => json!({"error": true}),
            }
        })
        .collect();
    println!("{}", serde_json::to_string(&answers)?);
    Ok(())
}
