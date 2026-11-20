//! Explicit-path, read-only adapter. No default user state, migration or write API.
use acp_core::{
    Handoff, HandoffRepository, Preference, PreferenceRepository, Project, ProjectRegistry, Scope,
    Status,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
#[doc(hidden)]
pub mod native;
mod operator;
#[doc(hidden)]
pub mod profile;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schema {
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
    PreferenceNotFound,
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

/// Strict fresh Profile v1 reader. The connection is always SQLite read-only.
pub struct SqliteStore {
    db: Connection,
}
impl SqliteStore {
    /// Verify the actual SQLite file against a daemon-owned native guard.
    pub fn check_open_file(&self, guard: &std::fs::File) -> Result<()> {
        native::check(&self.db, guard)
    }
    pub fn open(path: &Path) -> Result<Self> {
        if !path.is_absolute() || !path.is_file() {
            return Err(Error::OpenFailed);
        }
        let db = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| Error::OpenFailed)?;
        db.busy_timeout(Duration::from_millis(100))
            .map_err(|_| Error::OpenFailed)?;
        db.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH, 256 * 1024)
            .map_err(|_| Error::OpenFailed)?;
        for (name, value) in [
            ("query_only", true),
            ("foreign_keys", true),
            ("trusted_schema", false),
        ] {
            db.pragma_update(None, name, value)
                .map_err(|_| Error::OpenFailed)?;
        }
        let store = Self { db };
        store.metadata()?;
        Ok(store)
    }
    fn snapshot<T>(&self, read: impl FnOnce(profile::Metadata) -> Result<T>) -> Result<T> {
        let tx = self
            .db
            .unchecked_transaction()
            .map_err(|_| Error::ReadFailed)?;
        let metadata = profile::verify(&tx)?;
        let result = read(metadata);
        drop(tx);
        result
    }
    pub fn metadata(&self) -> Result<profile::Metadata> {
        self.snapshot(Ok)
    }
    pub fn schema(&self) -> Result<Schema> {
        self.snapshot(|_| Ok(Schema::Version1))
    }
    pub fn project_revision(&self, name: &str) -> Result<i64> {
        self.snapshot(|_| {
            self.db
                .query_row("SELECT revision FROM projects WHERE name=?1", [name], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(|_| Error::InvalidRecord)?
                .ok_or(Error::UnknownProject)
        })
    }
    pub fn preference_revision(&self, id: i64) -> Result<i64> {
        self.snapshot(|_| {
            self.db
                .query_row("SELECT revision FROM preferences WHERE id=?1", [id], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(|_| Error::InvalidRecord)?
                .ok_or(Error::InvalidRecord)
        })
    }
    pub fn handoff_revision(&self, project: &str, task: &str) -> Result<i64> {
        self.snapshot(|_| {
            self.db
                .query_row(
                    "SELECT revision FROM handoffs WHERE project=?1 AND task_id=?2",
                    (project, task),
                    |r| r.get(0),
                )
                .optional()
                .map_err(|_| Error::InvalidRecord)?
                .ok_or(Error::HandoffNotFound)
        })
    }
    pub fn read_project(&self, name: &str) -> Result<Project> {
        self.snapshot(|_| {
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
        })
    }
    /// The registered project whose root is exactly `root`, if any (a project
    /// the daemon's set no longer lists keeps its Profile record).
    pub fn project_at_root(&self, root: &std::path::Path) -> Result<Option<String>> {
        let Some(root) = root.to_str() else {
            return Ok(None);
        };
        self.snapshot(|_| {
            self.db
                .query_row("SELECT name FROM projects WHERE root=?1", [root], |r| {
                    r.get::<_, String>(0)
                })
                .optional()
                .map_err(|_| Error::InvalidRecord)
        })
    }
    /// Returns only structurally scoped records; the core applies confirmed-only
    /// filtering, precedence and ID ordering. Invalid review states fail closed.
    pub fn read_preferences(&self, project: &str) -> Result<Vec<Preference>> {
        self.snapshot(|_| {
        project_record(&self.db, project)?;
        let mut stmt = self.db.prepare("SELECT id,scope,project,pref_key,value,status,tags FROM preferences WHERE (scope='global' AND project IS NULL) OR (scope='project' AND project=?1) ORDER BY pref_key,CASE scope WHEN 'project' THEN 0 ELSE 1 END,id DESC")
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
            let (id, scope, project, key, value, status, tags) =
                row.map_err(|_| Error::InvalidRecord)?;
            let tags = profile::stored_tags(&tags).ok_or(Error::InvalidRecord)?;
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
                tags,
            });
        }
        Ok(result)

        })
    }
    pub fn read_handoff(&self, project: &str, task: &str) -> Result<Handoff> {
        self.snapshot(|_| {
        project_record(&self.db, project)?;
        let sql = "SELECT project,task_id,objective,base_sha,branch,worktree_state,worktree_fingerprint,completed,next_action,validation_json,updated_at FROM handoffs WHERE project=?1 AND task_id=?2";
        self.db
            .query_row(sql, [project, task], |r| {
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

        })
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

fn project_record(db: &Connection, name: &str) -> Result<()> {
    db.query_row("SELECT 1 FROM projects WHERE name=?1", [name], |_| Ok(()))
        .optional()
        .map_err(|_| Error::InvalidRecord)?
        .ok_or(Error::UnknownProject)
}
#[cfg(test)]
extern crate self as acp_store;
#[cfg(test)]
mod read_cost;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn read_only_mode_denies_sql_even_when_query_only_is_disabled() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("fresh.sqlite3");
        {
            let db = Connection::open(&p).unwrap();
            for ddl in profile::DDL {
                db.execute_batch(ddl).unwrap()
            }
            db.execute_batch("PRAGMA user_version=1;INSERT INTO acp_profile VALUES(1,'00000000000000000000000000000000','11111111111111111111111111111111',0,1,0,1)").unwrap();
        }
        let s = SqliteStore::open(&p).unwrap();
        s.db.pragma_update(None, "query_only", false).unwrap();
        assert_eq!(
            s.db.execute("DELETE FROM acp_profile", [])
                .unwrap_err()
                .sqlite_error_code(),
            Some(rusqlite::ErrorCode::ReadOnly)
        );
    }
}
