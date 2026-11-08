//! Explicit-path, read-only adapter. No default user state, migration or write API.
use acp_core::{
    Handoff, HandoffRepository, Preference, PreferenceRepository, Project, ProjectRegistry, Scope,
    Status,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schema {
    Legacy,
    Fingerprinted,
    Version1,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    OpenFailed,
    UnsupportedVersion,
    IncompatibleSchema,
    ReadFailed,
    InvalidRecord,
    UnknownProject,
    HandoffNotFound,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Deliberately exclude SQLite messages, paths, SQL and record values.
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
fn core_error(e: Error) -> acp_core::Error {
    match e {
        Error::UnknownProject => acp_core::Error::UnknownProject,
        Error::HandoffNotFound => acp_core::Error::HandoffNotFound,
        Error::InvalidRecord => acp_core::Error::InvalidRecord,
        _ => acp_core::Error::AdapterUnavailable,
    }
}

/// Owns a private read-only connection, not the database. Python remains the writer.
/// Open another adapter per reader/thread; never use immutable mode for live WAL data.
pub struct SqliteStore {
    db: Connection,
}
impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Err(Error::OpenFailed);
        }
        let db = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| Error::OpenFailed)?;
        db.busy_timeout(Duration::from_secs(5))
            .map_err(|_| Error::OpenFailed)?;
        // Connection-local protections; no journal mode, version, DDL or checkpoint writes.
        db.pragma_update(None, "query_only", true)
            .map_err(|_| Error::OpenFailed)?;
        db.pragma_update(None, "trusted_schema", false)
            .map_err(|_| Error::OpenFailed)?;
        let store = Self { db };
        store.schema()?;
        Ok(store)
    }
    pub fn schema(&self) -> Result<Schema> {
        inspect_schema(&self.db)
    }
    pub fn read_project(&self, name: &str) -> Result<Project> {
        self.schema()?;
        let record = self
            .db
            .query_row(
                "SELECT name, root FROM projects WHERE name=?1",
                [name],
                |r| {
                    Ok(Project {
                        name: r.get(0)?,
                        root: PathBuf::from(r.get::<_, String>(1)?),
                    })
                },
            )
            .optional()
            .map_err(|_| Error::InvalidRecord)?
            .ok_or(Error::UnknownProject)?;
        if record.name.is_empty() || !record.root.is_absolute() {
            return Err(Error::InvalidRecord);
        }
        Ok(record)
    }
    /// Returns only structurally scoped records; the core applies confirmed-only
    /// filtering, precedence and ID ordering. Invalid review states fail closed.
    pub fn read_preferences(&self, project: &str) -> Result<Vec<Preference>> {
        self.read_project(project)?;
        let mut stmt = self.db.prepare("SELECT id,scope,project,pref_key,value,status,source FROM preferences WHERE (scope='global' AND project IS NULL) OR (scope='project' AND project=?1) ORDER BY pref_key,CASE scope WHEN 'project' THEN 0 ELSE 1 END,id DESC")
            .map_err(|_| Error::ReadFailed)?;
        let rows = stmt
            .query_map([project], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                ))
            })
            .map_err(|_| Error::ReadFailed)?;
        let mut result = Vec::new();
        for row in rows {
            let (id, scope, project, key, value, status, source) =
                row.map_err(|_| Error::InvalidRecord)?;
            let scope = match scope.as_str() {
                "global" => Scope::Global,
                "project" => Scope::Project,
                _ => return Err(Error::InvalidRecord),
            };
            let status = match status.as_str() {
                "suggested" => Status::Suggested,
                "confirmed" => Status::Confirmed,
                "rejected" => Status::Rejected,
                "deprecated" => Status::Deprecated,
                _ => return Err(Error::InvalidRecord),
            };
            let id = u64::try_from(id).map_err(|_| Error::InvalidRecord)?;
            result.push(Preference {
                id,
                scope,
                project,
                key,
                value,
                status,
                source,
            });
        }
        Ok(result)
    }
    pub fn read_handoff(&self, project: &str, task: &str) -> Result<Handoff> {
        self.read_project(project)?;
        let fingerprint = match self.schema()? {
            Schema::Legacy => "NULL",
            Schema::Fingerprinted | Schema::Version1 => "worktree_fingerprint",
        };
        let sql = format!(
            "SELECT project,task_id,objective,base_sha,branch,worktree_state,{fingerprint},completed,next_action,validation_json,updated_at FROM handoffs WHERE project=?1 AND task_id=?2"
        );
        self.db
            .query_row(&sql, [project, task], |r| {
                let validation_json: String = r.get(9)?;
                let validation = serde_json::from_str(&validation_json)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                Ok(Handoff {
                    project: r.get(0)?,
                    task_id: r.get(1)?,
                    objective: r.get(2)?,
                    base_sha: r.get(3)?,
                    branch: r.get(4)?,
                    worktree_state: r.get(5)?,
                    worktree_fingerprint: r.get(6)?,
                    completed: r.get(7)?,
                    next_action: r.get(8)?,
                    validation,
                    updated_at: r.get(10)?,
                })
            })
            .optional()
            .map_err(|_| Error::InvalidRecord)?
            .ok_or(Error::HandoffNotFound)
    }
}
impl ProjectRegistry for SqliteStore {
    fn project(&self, name: &str) -> acp_core::Result<Project> {
        self.read_project(name).map_err(core_error)
    }
}
impl PreferenceRepository for SqliteStore {
    fn preferences(&self, project: &str) -> acp_core::Result<Vec<Preference>> {
        self.read_preferences(project).map_err(core_error)
    }
}
impl HandoffRepository for SqliteStore {
    fn handoff(&self, project: &str, task: &str) -> acp_core::Result<Handoff> {
        self.read_handoff(project, task).map_err(core_error)
    }
}

/// Inspect supported schema metadata on a borrowed connection; never mutates it.
/// V0 has two shapes; v1 requires the nullable fingerprint column.
pub fn inspect_schema(db: &Connection) -> Result<Schema> {
    SchemaInspector { db }.schema()
}
struct SchemaInspector<'a> {
    db: &'a Connection,
}
impl SchemaInspector<'_> {
    /// Both Python P0 variants are unversioned (user_version=0); v1 is additive. Recheck before
    /// every read so an external DDL/version change is not silently accepted.
    pub fn schema(&self) -> Result<Schema> {
        let version: i64 = self
            .db
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(|_| Error::IncompatibleSchema)?;
        if !matches!(version, 0 | 1) {
            return Err(Error::UnsupportedVersion);
        }
        let mut stmt = self.db.prepare("SELECT name, type FROM sqlite_schema WHERE name NOT GLOB 'sqlite_*' AND type != 'index'")
            .map_err(|_| Error::IncompatibleSchema)?;
        let objects: BTreeMap<String, String> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|_| Error::IncompatibleSchema)?
            .collect::<rusqlite::Result<_>>()
            .map_err(|_| Error::IncompatibleSchema)?;
        let expected = ["projects", "preferences", "handoffs"]
            .into_iter()
            .map(|n| (n.to_owned(), "table".to_owned()))
            .collect();
        if objects != expected {
            return Err(Error::IncompatibleSchema);
        }
        self.check_columns("projects", "name:TEXT:1,root:TEXT:0,created_at:TEXT:0")?;
        self.check_columns("preferences", "id:INTEGER:1,scope:TEXT:0,project:TEXT:0,pref_key:TEXT:0,value:TEXT:0,status:TEXT:0,source:TEXT:0,created_at:TEXT:0,updated_at:TEXT:0")?;
        let columns = self.columns("handoffs")?;
        let schema = if columns.contains_key("worktree_fingerprint") {
            Schema::Fingerprinted
        } else {
            Schema::Legacy
        };
        let base = "project:TEXT:1,task_id:TEXT:2,objective:TEXT:0,base_sha:TEXT:0,branch:TEXT:0,worktree_state:TEXT:0,completed:TEXT:0,next_action:TEXT:0,validation_json:TEXT:0,updated_at:TEXT:0";
        let spec = if schema == Schema::Fingerprinted {
            format!("{base},worktree_fingerprint:TEXT:0")
        } else {
            base.to_owned()
        };
        self.check_columns("handoffs", &spec)?;
        if version == 1 {
            if schema != Schema::Fingerprinted {
                return Err(Error::IncompatibleSchema);
            }
            return Ok(Schema::Version1);
        }
        Ok(schema)
    }
    fn columns(&self, table: &str) -> Result<BTreeMap<String, (String, i64)>> {
        let mut stmt = self
            .db
            .prepare("SELECT name, type, pk, hidden FROM pragma_table_xinfo(?1)")
            .map_err(|_| Error::IncompatibleSchema)?;
        let rows = stmt
            .query_map([table], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .map_err(|_| Error::IncompatibleSchema)?;
        let mut result = BTreeMap::new();
        for row in rows {
            let (name, ty, pk, hidden) = row.map_err(|_| Error::IncompatibleSchema)?;
            if hidden != 0 {
                return Err(Error::IncompatibleSchema);
            }
            result.insert(name, (ty.to_ascii_uppercase(), pk));
        }
        Ok(result)
    }
    fn check_columns(&self, table: &str, spec: &str) -> Result<()> {
        let expected = spec
            .split(',')
            .map(|part| {
                let mut fields = part.split(':');
                let name = fields.next().unwrap().to_owned();
                let ty = fields.next().unwrap().to_owned();
                let pk = fields.next().unwrap().parse::<i64>().unwrap();
                (name, (ty, pk))
            })
            .collect();
        if self.columns(table)? != expected {
            return Err(Error::IncompatibleSchema);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn read_only_flags_deny_writes_even_if_query_only_is_disabled() {
        let temp = tempfile::tempdir().unwrap();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = std::process::Command::new(
            std::env::var("ACP_TEST_PYTHON").unwrap_or_else(|_| "python".into()),
        )
        .arg(repo.join("tools/sqlite_fixture.py"))
        .arg(temp.path())
        .env("PYTHONPATH", repo.join("src"))
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "synthetic fixture generation failed"
        );
        let store = SqliteStore::open(&temp.path().join("state/context.sqlite3")).unwrap();
        store.db.pragma_update(None, "query_only", false).unwrap();
        let error = store
            .db
            .execute("UPDATE preferences SET value='after'", [])
            .unwrap_err();
        assert_eq!(
            error.sqlite_error_code(),
            Some(rusqlite::ErrorCode::ReadOnly)
        );
        assert_eq!(
            store
                .db
                .query_row(
                    "SELECT value FROM preferences WHERE source='fixture-new'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "newest"
        );
    }
}
