//! Exact schema allowlist frozen in docs/p1/profile-v2-schema-and-writer.md.
use crate::{Error, Result};
use rusqlite::{Connection, types::Value};
use std::collections::BTreeMap;

pub(crate) const PROJECTS: &str = r#"CREATE TABLE projects (
    name TEXT PRIMARY KEY,
    root TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL
)"#;
pub(crate) const PREFERENCES: &str = r#"CREATE TABLE preferences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT NOT NULL CHECK (scope IN ('global','project')),
    project TEXT REFERENCES projects(name) ON DELETE CASCADE,
    pref_key TEXT NOT NULL,
    value TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'suggested'
      CHECK (status IN ('suggested','confirmed','rejected','deprecated')),
    source TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK ((scope = 'global' AND project IS NULL) OR
           (scope = 'project' AND project IS NOT NULL))
)"#;
pub(crate) const PREFS_LOOKUP: &str =
    r#"CREATE INDEX prefs_lookup ON preferences(scope, project, status, pref_key)"#;
pub(crate) const HANDOFFS: &str = r#"CREATE TABLE handoffs (
    project TEXT NOT NULL REFERENCES projects(name) ON DELETE CASCADE,
    task_id TEXT NOT NULL,
    objective TEXT NOT NULL,
    base_sha TEXT NOT NULL,
    branch TEXT NOT NULL,
    worktree_state TEXT NOT NULL,
    worktree_fingerprint TEXT,
    completed TEXT NOT NULL,
    next_action TEXT NOT NULL,
    validation_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (project, task_id)
)"#;
pub(crate) const PROFILE: &str = r#"CREATE TABLE acp_profile (
    singleton INTEGER PRIMARY KEY CHECK (singleton=1),
    profile_id TEXT NOT NULL CHECK (length(profile_id)=32),
    generation TEXT NOT NULL CHECK (length(generation)=32),
    owner_epoch INTEGER NOT NULL CHECK (owner_epoch>0),
    commit_sequence INTEGER NOT NULL CHECK (commit_sequence>=0),
    receipt_limit INTEGER NOT NULL CHECK (receipt_limit BETWEEN 1 AND 4096)
)"#;
pub(crate) const REVISIONS: &str = r#"CREATE TABLE acp_revisions (
    kind TEXT NOT NULL CHECK (kind IN ('project','preference','handoff')),
    project TEXT NOT NULL,
    record_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision>0),
    deleted INTEGER NOT NULL CHECK (deleted IN (0,1)),
    PRIMARY KEY (kind,project,record_id),
    CHECK ((kind='project' AND project='' AND deleted=0) OR
           (kind='preference' AND project='') OR
           (kind='handoff' AND project<>'' AND deleted=0))
)"#;
pub(crate) const RECEIPTS: &str = r#"CREATE TABLE acp_receipts (
    operation_id TEXT PRIMARY KEY CHECK (length(operation_id) BETWEEN 1 AND 64),
    generation TEXT NOT NULL CHECK (length(generation)=32),
    request_digest BLOB NOT NULL CHECK (length(request_digest)=32),
    commit_sequence INTEGER NOT NULL UNIQUE CHECK (commit_sequence>0),
    result_json TEXT NOT NULL CHECK (length(result_json) BETWEEN 1 AND 65536)
)"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    Legacy,
    Fingerprinted,
    V1,
    V2,
}
fn normalize(sql: &str) -> String {
    let mut literal = false;
    sql.trim_end_matches(';')
        .chars()
        .filter_map(|c| {
            if c == '\'' {
                literal = !literal;
                return Some(c);
            }
            if literal {
                Some(c)
            } else if c.is_whitespace() || c == '"' {
                None
            } else {
                Some(c.to_ascii_lowercase())
            }
        })
        .collect()
}
fn handoffs() -> [String; 3] {
    let legacy = HANDOFFS.replace("    worktree_fingerprint TEXT,\n", "");
    let appended = legacy.replace(
        "    PRIMARY KEY",
        "    worktree_fingerprint TEXT,\n    PRIMARY KEY",
    );
    [legacy, HANDOFFS.to_owned(), appended]
}
pub(crate) fn inspect(db: &Connection) -> Result<Shape> {
    let version: i64 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|_| Error::IncompatibleSchema)?;
    if !matches!(version, 0..=2) {
        return Err(Error::UnsupportedVersion);
    }
    let mut stmt = db
        .prepare(
            "SELECT name,type,sql FROM sqlite_schema WHERE name NOT GLOB 'sqlite_*' ORDER BY name",
        )
        .map_err(|_| Error::IncompatibleSchema)?;
    let objects = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, String>(1)?, r.get::<_, String>(2)?),
            ))
        })
        .map_err(|_| Error::IncompatibleSchema)?
        .collect::<rusqlite::Result<BTreeMap<_, _>>>()
        .map_err(|_| Error::IncompatibleSchema)?;
    let variants = handoffs();
    let (_, handoff_sql) = objects.get("handoffs").ok_or(Error::IncompatibleSchema)?;
    let matched = variants
        .iter()
        .position(|ddl| normalize(ddl) == normalize(handoff_sql))
        .ok_or(Error::IncompatibleSchema)?;
    if version > 0 && matched == 0 {
        return Err(Error::IncompatibleSchema);
    }
    let mut expected = BTreeMap::from([
        ("projects".to_owned(), PROJECTS.to_owned()),
        ("preferences".to_owned(), PREFERENCES.to_owned()),
        ("prefs_lookup".to_owned(), PREFS_LOOKUP.to_owned()),
        ("handoffs".to_owned(), variants[matched].clone()),
    ]);
    if version == 2 {
        expected.extend([
            ("acp_profile".into(), PROFILE.into()),
            ("acp_revisions".into(), REVISIONS.into()),
            ("acp_receipts".into(), RECEIPTS.into()),
        ]);
    }
    if objects.len() != expected.len()
        || expected.iter().any(|(name, ddl)| {
            objects.get(name).is_none_or(|(kind, sql)| {
                kind != if name == "prefs_lookup" {
                    "index"
                } else {
                    "table"
                } || normalize(sql) != normalize(ddl)
            })
        })
    {
        return Err(Error::IncompatibleSchema);
    }
    Ok(match (version, matched) {
        (2, _) => Shape::V2,
        (1, _) => Shape::V1,
        (0, 0) => Shape::Legacy,
        _ => Shape::Fingerprinted,
    })
}
pub(crate) fn create_source(db: &Connection, shape: Shape) -> Result<()> {
    for ddl in [PROJECTS, PREFERENCES, PREFS_LOOKUP] {
        db.execute_batch(ddl).map_err(crate::sql_error)?;
    }
    let variants = handoffs();
    db.execute_batch(if shape == Shape::Legacy {
        &variants[0]
    } else {
        HANDOFFS
    })
    .map_err(crate::sql_error)?;
    db.pragma_update(None, "user_version", if shape == Shape::V1 { 1 } else { 0 })
        .map_err(crate::sql_error)
}
pub(crate) fn migrate(db: &Connection) -> Result<()> {
    match inspect(db)? {
        Shape::Legacy => db
            .execute_batch("ALTER TABLE handoffs ADD COLUMN worktree_fingerprint TEXT")
            .map_err(crate::sql_error)?,
        Shape::V2 => return Err(Error::UnsupportedVersion),
        _ => (),
    }
    for ddl in [PROFILE, REVISIONS, RECEIPTS] {
        db.execute_batch(ddl).map_err(crate::sql_error)?;
    }
    db.pragma_update(None, "user_version", 2)
        .map_err(crate::sql_error)
}
pub(crate) fn integrity(db: &Connection) -> Result<()> {
    let mut stmt = db
        .prepare("PRAGMA integrity_check")
        .map_err(|_| Error::InvalidRecord)?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|_| Error::InvalidRecord)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| Error::InvalidRecord)?;
    let violations: i64 = db
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .map_err(|_| Error::InvalidRecord)?;
    if rows != ["ok"] || violations != 0 {
        return Err(Error::InvalidRecord);
    }
    Ok(())
}
pub(crate) type Records = Vec<Vec<Vec<Value>>>;
pub(crate) fn records(db: &Connection) -> Result<Records> {
    let fp = if inspect(db)? == Shape::Legacy {
        "NULL"
    } else {
        "worktree_fingerprint"
    };
    [
        "SELECT name,root,created_at FROM projects ORDER BY name".to_owned(),
        "SELECT id,scope,project,pref_key,value,status,source,created_at,updated_at FROM preferences ORDER BY id".to_owned(),
        format!("SELECT project,task_id,objective,base_sha,branch,worktree_state,{fp},completed,next_action,validation_json,updated_at FROM handoffs ORDER BY project,task_id"),
        "SELECT name,seq FROM sqlite_sequence ORDER BY name".to_owned()
    ].iter().map(|q| {
        let mut stmt=db.prepare(q).map_err(|_|Error::InvalidRecord)?;
        let count=stmt.column_count();
        stmt.query_map([],|row|(0..count).map(|i|row.get::<_,Value>(i)).collect::<rusqlite::Result<Vec<_>>>())
            .map_err(|_|Error::InvalidRecord)?.collect::<rusqlite::Result<_>>().map_err(|_|Error::InvalidRecord)
    }).collect()
}
pub(crate) fn validate_records(db: &Connection) -> Result<()> {
    integrity(db)?;
    let rows = records(db)?;
    for row in &rows[0] {
        if !matches!((&row[0],&row[1],&row[2]),(Value::Text(name),Value::Text(root),Value::Text(_)) if !name.is_empty() && std::path::Path::new(root).is_absolute())
        {
            return Err(Error::InvalidRecord);
        }
    }
    for row in &rows[1] {
        if !matches!(row[0],Value::Integer(n) if n>0)
            || !matches!((&row[1],&row[2]),(Value::Text(s),Value::Null) if s=="global")
                && !matches!((&row[1],&row[2]),(Value::Text(s),Value::Text(p)) if s=="project" && !p.is_empty())
            || [3, 4, 5, 6, 7, 8]
                .iter()
                .any(|i| !matches!(row[*i], Value::Text(_)))
            || !matches!(&row[5],Value::Text(s) if ["suggested","confirmed","rejected","deprecated"].contains(&s.as_str()))
        {
            return Err(Error::InvalidRecord);
        }
    }
    for row in &rows[2] {
        if (0..11)
            .filter(|i| *i != 6)
            .any(|i| !matches!(row[i], Value::Text(_)))
            || !matches!(row[6], Value::Null | Value::Text(_))
            || !matches!(&row[9],Value::Text(raw) if crate::command::validation_valid(raw))
        {
            return Err(Error::InvalidRecord);
        }
    }
    let seq: Option<i64> = db
        .query_row(
            "SELECT seq FROM sqlite_sequence WHERE name='preferences'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| Error::InvalidRecord)?;
    let max: i64 = db
        .query_row("SELECT coalesce(max(id),0) FROM preferences", [], |r| {
            r.get(0)
        })
        .map_err(|_| Error::InvalidRecord)?;
    if seq.is_some_and(|n| n < max) || (max > 0 && seq.is_none()) {
        return Err(Error::InvalidRecord);
    }
    Ok(())
}
use rusqlite::OptionalExtension;
