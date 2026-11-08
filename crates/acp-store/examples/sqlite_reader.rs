//! Synthetic parity harness. Explicit DB path; no default profile discovery.
use acp_core::{ContextResolver, Project, Worktree, WorktreeInspector, documents};
use acp_store::SqliteStore;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, Read},
    path::PathBuf,
};
#[derive(Deserialize)]
struct Input {
    database: PathBuf,
    worktrees: BTreeMap<String, Worktree>,
    requests: Vec<Request>,
}
#[derive(Deserialize)]
struct Request {
    project: String,
    name: String,
    arguments: Value,
}
impl WorktreeInspector for Input {
    fn inspect(&self, project: &Project, _: bool) -> acp_core::Result<Worktree> {
        self.worktrees
            .get(&project.name)
            .cloned()
            .ok_or(acp_core::Error::AdapterUnavailable)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut raw = String::new();
    io::stdin()
        .take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut raw)?;
    if raw.len() > 4 * 1024 * 1024 {
        return Err("input too large".into());
    }
    let input: Input = serde_json::from_str(&raw)?;
    let store = match SqliteStore::open(&input.database) {
        Ok(s) => s,
        Err(e) => {
            println!("{}", json!({"open_error":e.to_string()}));
            return Ok(());
        }
    };
    let answers: Vec<_> = input
        .requests
        .iter()
        .map(|r| {
            match ContextResolver::new(
                &r.project,
                &store,
                &store,
                &store,
                &input,
                &documents::FileDocuments,
            )
            .and_then(|resolver| resolver.read(&r.name, &r.arguments))
            {
                Ok(data) => json!({"data":data}),
                Err(e) => json!({"error":format!("{e}")}),
            }
        })
        .collect();
    println!(
        "{}",
        json!({"schema":format!("{:?}",store.schema()?),"answers":answers})
    );
    Ok(())
}
