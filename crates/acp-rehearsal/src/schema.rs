//! Deliberately narrow migration allowlist, not a general SQL schema recognizer.
use crate::{Error, Result};
use rusqlite::Connection;

fn normalize(sql: &str) -> String {
    let mut literal = false;
    sql.chars()
        .filter_map(|c| {
            if c == '\'' {
                literal = !literal;
                return Some(c);
            }
            if literal {
                return Some(c);
            }
            if c.is_whitespace() || c == '"' {
                None
            } else {
                Some(c.to_ascii_lowercase())
            }
        })
        .collect()
}
pub(crate) fn validate(db: &Connection) -> Result<()> {
    let projects = "CREATE TABLE projects (name TEXT PRIMARY KEY,root TEXT NOT NULL UNIQUE,created_at TEXT NOT NULL)";
    let preferences = "CREATE TABLE preferences (id INTEGER PRIMARY KEY AUTOINCREMENT,scope TEXT NOT NULL CHECK (scope IN ('global','project')),project TEXT REFERENCES projects(name) ON DELETE CASCADE,pref_key TEXT NOT NULL,value TEXT NOT NULL,status TEXT NOT NULL DEFAULT 'suggested' CHECK (status IN ('suggested','confirmed','rejected','deprecated')),source TEXT NOT NULL,created_at TEXT NOT NULL,updated_at TEXT NOT NULL,CHECK ((scope = 'global' AND project IS NULL) OR (scope = 'project' AND project IS NOT NULL)))";
    let prefix = "CREATE TABLE handoffs (project TEXT NOT NULL REFERENCES projects(name) ON DELETE CASCADE,task_id TEXT NOT NULL,objective TEXT NOT NULL,base_sha TEXT NOT NULL,branch TEXT NOT NULL,worktree_state TEXT NOT NULL,";
    let suffix = "completed TEXT NOT NULL,next_action TEXT NOT NULL,validation_json TEXT NOT NULL,updated_at TEXT NOT NULL";
    // SQLite appends ADD COLUMN after table constraints, while the Python current
    // initializer declares fingerprint before completed. Both are authoritative.
    let handoffs = [
        format!("{prefix}{suffix},PRIMARY KEY (project, task_id))"),
        format!("{prefix}worktree_fingerprint TEXT,{suffix},PRIMARY KEY (project, task_id))"),
        format!("{prefix}{suffix},worktree_fingerprint TEXT,PRIMARY KEY (project, task_id))"),
    ];
    for (name, allowed) in [
        ("projects", vec![projects.to_owned()]),
        ("preferences", vec![preferences.to_owned()]),
        ("handoffs", handoffs.to_vec()),
    ] {
        let sql: String = db
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type='table' AND name=?1",
                [name],
                |r| r.get(0),
            )
            .map_err(|_| Error::Schema)?;
        if !allowed
            .iter()
            .any(|known| normalize(known) == normalize(&sql))
        {
            return Err(Error::Schema);
        }
    }
    let mut stmt=db.prepare("SELECT name,sql FROM sqlite_schema WHERE type='index' AND sql IS NOT NULL ORDER BY name").map_err(|_|Error::Schema)?;
    let indexes = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|_| Error::Schema)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| Error::Schema)?;
    if indexes.len() != 1
        || indexes[0].0 != "prefs_lookup"
        || normalize(&indexes[0].1)
            != normalize(
                "CREATE INDEX prefs_lookup ON preferences(scope, project, status, pref_key)",
            )
    {
        return Err(Error::Schema);
    }
    Ok(())
}
