//! Exact fresh Rust-native Profile v1; verification never mutates database data.
use crate::{Error, Result};
use rusqlite::{Connection, types::Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// Thread-local and test-only: no runtime metric, cache or validation bypass.
#[cfg(test)]
thread_local! {
    pub(crate) static VERIFY_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub const PROFILE: &str = r#"CREATE TABLE acp_profile (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    profile_id TEXT NOT NULL
      CHECK (length(profile_id) = 32 AND profile_id NOT GLOB '*[^0-9a-f]*'),
    generation TEXT NOT NULL
      CHECK (length(generation) = 32 AND generation NOT GLOB '*[^0-9a-f]*'),
    owner_epoch INTEGER NOT NULL CHECK (owner_epoch >= 0),
    next_preference_id INTEGER NOT NULL CHECK (next_preference_id > 0),
    commit_sequence INTEGER NOT NULL CHECK (commit_sequence >= 0)
)"#;
pub const PROJECTS: &str = r#"CREATE TABLE projects (
    name TEXT NOT NULL PRIMARY KEY CHECK (length(name) BETWEEN 1 AND 64),
    root TEXT NOT NULL UNIQUE,
    revision INTEGER NOT NULL CHECK (revision > 0),
    created_at TEXT NOT NULL
)"#;
pub const PREFERENCES: &str = r#"CREATE TABLE preferences (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    scope TEXT NOT NULL CHECK (scope IN ('global', 'project')),
    project TEXT REFERENCES projects(name) ON DELETE RESTRICT,
    pref_key TEXT NOT NULL CHECK (length(pref_key) BETWEEN 1 AND 120),
    value TEXT NOT NULL CHECK (length(value) BETWEEN 1 AND 4096),
    status TEXT NOT NULL DEFAULT 'suggested'
      CHECK (status IN ('suggested', 'confirmed', 'rejected', 'deprecated')),
    source TEXT NOT NULL CHECK (length(source) BETWEEN 1 AND 512),
    revision INTEGER NOT NULL CHECK (revision > 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (
      (scope = 'global' AND project IS NULL) OR
      (scope = 'project' AND project IS NOT NULL)
    )
)"#;
pub const PREFS_LOOKUP: &str = r#"CREATE INDEX prefs_lookup
    ON preferences(scope, project, status, pref_key)"#;
pub const HANDOFFS: &str = r#"CREATE TABLE handoffs (
    project TEXT NOT NULL REFERENCES projects(name) ON DELETE RESTRICT,
    task_id TEXT NOT NULL CHECK (length(task_id) BETWEEN 1 AND 120),
    objective TEXT NOT NULL CHECK (length(objective) BETWEEN 1 AND 4000),
    base_sha TEXT NOT NULL,
    branch TEXT NOT NULL,
    worktree_state TEXT NOT NULL,
    worktree_fingerprint TEXT,
    completed TEXT NOT NULL CHECK (length(completed) <= 8000),
    next_action TEXT NOT NULL CHECK (length(next_action) BETWEEN 1 AND 4000),
    validation_json TEXT NOT NULL CHECK (length(validation_json) <= 16000),
    revision INTEGER NOT NULL CHECK (revision > 0),
    updated_at TEXT NOT NULL,
    PRIMARY KEY (project, task_id)
)"#;
pub const RECEIPTS: &str = r#"CREATE TABLE acp_receipts (
    operation_id TEXT NOT NULL PRIMARY KEY
      CHECK (length(operation_id) BETWEEN 1 AND 64),
    generation TEXT NOT NULL
      CHECK (length(generation) = 32 AND generation NOT GLOB '*[^0-9a-f]*'),
    request_digest BLOB NOT NULL CHECK (length(request_digest) = 32),
    commit_sequence INTEGER NOT NULL UNIQUE CHECK (commit_sequence > 0),
    result_json TEXT NOT NULL
      CHECK (length(result_json) BETWEEN 1 AND 65536)
)"#;
pub const DDL: [&str; 6] = [
    PROFILE,
    PROJECTS,
    PREFERENCES,
    PREFS_LOOKUP,
    HANDOFFS,
    RECEIPTS,
];
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub profile_id: String,
    pub generation: String,
    pub owner_epoch: i64,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordVersion {
    pub kind: String,
    pub project: String,
    pub record_id: String,
    pub revision: i64,
    pub deleted: bool,
}
impl RecordVersion {
    pub fn key(&self) -> (String, String, String) {
        (
            self.kind.clone(),
            self.project.clone(),
            self.record_id.clone(),
        )
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub generation: String,
    pub operation_id: String,
    pub commit_sequence: i64,
    pub affected: Vec<RecordVersion>,
}
impl std::fmt::Debug for Receipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Receipt(redacted)")
    }
}
impl std::fmt::Debug for Binding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Binding(redacted)")
    }
}
#[derive(Clone)]
pub struct Metadata {
    pub binding: Binding,
    pub next_preference_id: i64,
    pub commit_sequence: i64,
}
pub fn name_valid(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s != "."
        && s != ".."
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
pub fn text(s: &str, max: usize, required: bool) -> bool {
    !s.contains('\0')
        && s.chars().count() <= max
        && (!required || s.chars().any(|c| !c.is_control() && !c.is_whitespace()))
}
pub fn hex(s: &str, n: usize) -> bool {
    s.len() == n
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn operation_valid(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
pub fn validation_valid(s: &str) -> bool {
    fn safe(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::String(s) => !s.contains('\0'),
            serde_json::Value::Array(a) => a.iter().all(safe),
            serde_json::Value::Object(o) => o.iter().all(|(k, v)| !k.contains('\0') && safe(v)),
            _ => true,
        }
    }
    text(s, 16000, true)
        && serde_json::from_str::<serde_json::Value>(s)
            .ok()
            .is_some_and(|v| {
                safe(&v)
                    && v.as_array().is_some_and(|a| {
                        a.iter().all(|v| {
                            v.as_object().is_some_and(|o| {
                                o.get("check").is_some_and(serde_json::Value::is_string)
                                    && matches!(
                                        o.get("status").and_then(serde_json::Value::as_str),
                                        Some("PASS" | "FAIL" | "SKIPPED" | "NOT_RUN")
                                    )
                            })
                        })
                    })
            })
}
pub fn snapshot_valid(sha: &str, branch: &str, state: &str, fp: Option<&str>) -> bool {
    (sha.is_empty() || hex(sha, 40) || hex(sha, 64))
        && text(branch, 1024, false)
        && ["clean", "dirty", "unknown"].contains(&state)
        && fp.is_none_or(|s| hex(s, 64))
}
/// Rust-generated UTC timestamp; no SQLite clock or external process.
pub fn timestamp() -> Result<String> {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::InvalidRecord)?
        .as_secs();
    format_utc(n).ok_or(Error::InvalidRecord)
}
fn format_utc(n: u64) -> Option<String> {
    let days = i64::try_from(n / 86400).ok()?;
    let z = days.checked_add(719468)?;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += i64::from(m <= 2);
    if !(1970..=9999).contains(&y) {
        return None;
    }
    Some(format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        n % 86400 / 3600,
        n % 3600 / 60,
        n % 60
    ))
}
fn timestamp_valid(s: &str) -> bool {
    if s.len() != 20
        || !s.is_ascii()
        || &s[4..5] != "-"
        || &s[7..8] != "-"
        || &s[10..11] != "T"
        || &s[13..14] != ":"
        || &s[16..17] != ":"
        || &s[19..] != "Z"
    {
        return false;
    }
    let fields = [
        &s[..4],
        &s[5..7],
        &s[8..10],
        &s[11..13],
        &s[14..16],
        &s[17..19],
    ];
    if fields
        .iter()
        .any(|s| !s.bytes().all(|b| b.is_ascii_digit()))
    {
        return false;
    }
    let v: Vec<u32> = fields
        .iter()
        .map(|s| s.parse().unwrap_or(u32::MAX))
        .collect();
    let (y, m, d) = (v[0], v[1], v[2]);
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let max = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    (1970..=9999).contains(&y) && d > 0 && d <= max && v[3] < 24 && v[4] < 60 && v[5] < 60
}
fn normalize(s: &str) -> String {
    let mut quote = false;
    let mut out = String::new();
    for c in s.chars() {
        if c == '\'' {
            quote = !quote;
            out.push(c)
        } else if quote {
            out.push(c)
        } else if !c.is_whitespace() {
            out.push(c.to_ascii_lowercase())
        }
    }
    out
}
pub fn inspect(db: &Connection) -> Result<()> {
    let version: i64 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|_| Error::IncompatibleSchema)?;
    if version != 1 {
        return Err(Error::UnsupportedVersion);
    }
    let mut expected: BTreeMap<String, (String, Option<String>)> = DDL
        .iter()
        .map(|ddl| {
            let mut words = ddl.split_whitespace();
            words.next();
            let kind = words.next().unwrap().to_ascii_lowercase();
            let name = words.next().unwrap().to_owned();
            (name, (kind, Some(normalize(ddl))))
        })
        .collect();
    for n in [
        "sqlite_autoindex_projects_1",
        "sqlite_autoindex_projects_2",
        "sqlite_autoindex_handoffs_1",
        "sqlite_autoindex_acp_receipts_1",
        "sqlite_autoindex_acp_receipts_2",
    ] {
        expected.insert(n.into(), ("index".into(), None));
    }
    let mut stmt = db
        .prepare("SELECT name,type,sql FROM sqlite_schema")
        .map_err(|_| Error::IncompatibleSchema)?;
    let actual = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?.map(|s| normalize(&s)),
                ),
            ))
        })
        .map_err(|_| Error::IncompatibleSchema)?
        .collect::<rusqlite::Result<BTreeMap<_, _>>>()
        .map_err(|_| Error::IncompatibleSchema)?;
    if actual != expected {
        return Err(Error::IncompatibleSchema);
    }
    Ok(())
}
fn rows(db: &Connection, sql: &str) -> Result<Vec<Vec<Value>>> {
    let mut stmt = db.prepare(sql).map_err(|_| Error::InvalidRecord)?;
    let n = stmt.column_count();
    let mut result = Vec::new();
    let mut cursor = stmt.query([]).map_err(|_| Error::InvalidRecord)?;
    while let Some(row) = cursor.next().map_err(|_| Error::InvalidRecord)? {
        if result.len() >= 4096 {
            return Err(Error::InvalidRecord);
        }
        result.push(
            (0..n)
                .map(|i| row.get(i))
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|_| Error::InvalidRecord)?,
        );
    }
    Ok(result)
}
fn string(v: &Value) -> Result<&str> {
    if let Value::Text(s) = v {
        Ok(s)
    } else {
        Err(Error::InvalidRecord)
    }
}
fn integer(v: &Value) -> Result<i64> {
    if let Value::Integer(n) = v {
        Ok(*n)
    } else {
        Err(Error::InvalidRecord)
    }
}
fn positive(v: &Value) -> Result<i64> {
    let n = integer(v)?;
    if n <= 0 {
        Err(Error::InvalidRecord)
    } else {
        Ok(n)
    }
}
fn valid(ok: bool) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::InvalidRecord)
    }
}
fn str_valid(v: &Value, max: usize, required: bool) -> Result<&str> {
    let s = string(v)?;
    valid(text(s, max, required))?;
    Ok(s)
}
/// Caller holds a pinned read transaction or BEGIN IMMEDIATE. No database writes.
pub fn verify(db: &Connection) -> Result<Metadata> {
    #[cfg(test)]
    VERIFY_CALLS.with(|n| n.set(n.get() + 1));
    inspect(db)?;
    valid(rows(db, "PRAGMA integrity_check")? == vec![vec![Value::Text("ok".into())]])?;
    valid(rows(db, "PRAGMA foreign_key_check")?.is_empty())?;
    let meta = rows(
        db,
        "SELECT singleton,profile_id,generation,owner_epoch,next_preference_id,commit_sequence FROM acp_profile",
    )?;
    valid(meta.len() == 1)?;
    let m = &meta[0];
    valid(integer(&m[0])? == 1)?;
    let id = string(&m[1])?;
    let generation = string(&m[2])?;
    valid(hex(id, 32) && hex(generation, 32))?;
    let epoch = integer(&m[3])?;
    let next = positive(&m[4])?;
    let seq = integer(&m[5])?;
    valid(epoch >= 0 && (0..=4096).contains(&seq))?;
    let mut live = BTreeMap::new();
    let mut max_id = 0;
    for r in rows(db, "SELECT name,root,revision,created_at FROM projects")? {
        let name = string(&r[0])?;
        valid(name_valid(name))?;
        valid(std::path::Path::new(str_valid(&r[1], 128 * 1024, true)?).is_absolute())?;
        valid(timestamp_valid(string(&r[3])?))?;
        live.insert(
            ("project".into(), String::new(), name.into()),
            positive(&r[2])?,
        );
    }
    for r in rows(
        db,
        "SELECT id,scope,project,pref_key,value,status,source,revision,created_at,updated_at FROM preferences",
    )? {
        let id = positive(&r[0])?;
        max_id = max_id.max(id);
        let scope = string(&r[1])?;
        valid(
            (scope == "global" && r[2] == Value::Null)
                || (scope == "project" && name_valid(string(&r[2])?)),
        )?;
        str_valid(&r[3], 120, true)?;
        str_valid(&r[4], 4096, true)?;
        str_valid(&r[6], 512, true)?;
        valid(["suggested", "confirmed", "rejected", "deprecated"].contains(&string(&r[5])?))?;
        valid(timestamp_valid(string(&r[8])?) && timestamp_valid(string(&r[9])?))?;
        live.insert(
            ("preference".into(), String::new(), id.to_string()),
            positive(&r[7])?,
        );
    }
    for r in rows(
        db,
        "SELECT project,task_id,objective,base_sha,branch,worktree_state,worktree_fingerprint,completed,next_action,validation_json,revision,updated_at FROM handoffs",
    )? {
        let project = string(&r[0])?;
        valid(name_valid(project))?;
        let task = str_valid(&r[1], 120, true)?;
        str_valid(&r[2], 4000, true)?;
        str_valid(&r[7], 8000, false)?;
        str_valid(&r[8], 4000, true)?;
        let fp = if r[6] == Value::Null {
            None
        } else {
            Some(string(&r[6])?)
        };
        valid(
            snapshot_valid(string(&r[3])?, string(&r[4])?, string(&r[5])?, fp)
                && validation_valid(string(&r[9])?)
                && timestamp_valid(string(&r[11])?),
        )?;
        live.insert(
            ("handoff".into(), project.into(), task.into()),
            positive(&r[10])?,
        );
    }
    let mut history: BTreeMap<(String, String, String), RecordVersion> = BTreeMap::new();
    let receipts = rows(
        db,
        "SELECT operation_id,generation,request_digest,commit_sequence,result_json FROM acp_receipts ORDER BY commit_sequence",
    )?;
    valid(receipts.len() as i64 == seq)?;
    for (i, r) in receipts.iter().enumerate() {
        let operation = string(&r[0])?;
        valid(
            operation_valid(operation)
                && string(&r[1])? == generation
                && matches!(&r[2],Value::Blob(b) if b.len()==32)
                && positive(&r[3])? == i as i64 + 1,
        )?;
        let raw = string(&r[4])?;
        valid(raw.len() <= 65536)?;
        let receipt: Receipt = serde_json::from_str(raw).map_err(|_| Error::InvalidRecord)?;
        valid(
            receipt.generation == generation
                && receipt.operation_id == operation
                && receipt.commit_sequence == i as i64 + 1
                && !receipt.affected.is_empty()
                && receipt.affected.len() <= 1024,
        )?;
        let mut previous = None;
        for version in receipt.affected {
            let key = version.key();
            valid(previous.as_ref().is_none_or(|p| p < &key))?;
            previous = Some(key.clone());
            match version.kind.as_str() {
                "project" => valid(
                    version.project.is_empty()
                        && name_valid(&version.record_id)
                        && !version.deleted,
                )?,
                "preference" => {
                    let id = version
                        .record_id
                        .parse::<i64>()
                        .map_err(|_| Error::InvalidRecord)?;
                    valid(
                        id > 0 && id.to_string() == version.record_id && version.project.is_empty(),
                    )?;
                    max_id = max_id.max(id);
                }
                "handoff" => valid(
                    name_valid(&version.project)
                        && text(&version.record_id, 120, true)
                        && !version.deleted,
                )?,
                _ => return Err(Error::InvalidRecord),
            }
            let wanted = if let Some(last) = history.get(&key) {
                valid(!last.deleted)?;
                last.revision.checked_add(1).ok_or(Error::InvalidRecord)?
            } else {
                valid(!version.deleted)?;
                1
            };
            valid(version.revision == wanted)?;
            history.insert(key, version);
        }
    }
    for (key, version) in history {
        if version.deleted {
            valid(!live.contains_key(&key))?;
        } else {
            valid(live.remove(&key) == Some(version.revision))?;
        }
    }
    valid(live.is_empty() && next > max_id && (seq != 0 || next == 1))?;
    Ok(Metadata {
        binding: Binding {
            profile_id: id.into(),
            generation: generation.into(),
            owner_epoch: epoch,
        },
        next_preference_id: next,
        commit_sequence: seq,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rust_utc_calendar_vectors_and_invalid_dates() {
        for (seconds, expected) in [
            (0, "1970-01-01T00:00:00Z"),
            (951782400, "2000-02-29T00:00:00Z"),
            (1709251199, "2024-02-29T23:59:59Z"),
        ] {
            assert_eq!(format_utc(seconds).as_deref(), Some(expected));
            assert!(timestamp_valid(expected));
        }
        for invalid in [
            "2026-02-30T00:00:00Z",
            "2100-02-29T00:00:00Z",
            "2026-01-01T24:00:00Z",
            "2026-01-01T00:00:00+00:00",
            "2026-01-01T00:00:60Z",
        ] {
            assert!(!timestamp_valid(invalid));
        }
        assert!(format_utc(u64::MAX).is_none());
    }
}
