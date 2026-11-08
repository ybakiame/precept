//! Synthetic-only backup/migration rehearsal. No default profile or adopted workspace.
//! Write targets must be new files inside this component's owned temporary directory.
use acp_store::{Schema, inspect_schema};
use rusqlite::{
    Connection, OpenFlags, TransactionBehavior,
    backup::{Backup, StepResult},
    types::Value,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    path::{Component, Path, PathBuf},
    time::Duration,
};
use tempfile::TempDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    UnsafePath,
    Collision,
    OpenFailed,
    Schema,
    Corrupt,
    Locked,
    BackupFailed,
    Verification,
    BeforeCommit,
    AfterCommit,
    InjectedBackup,
    InjectedWrite,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    None,
    BackupInterrupted,
    BackupWriteError,
    AfterDdl,
    BeforeCommit,
    AfterCommit,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Migrated,
    AlreadyV1,
}

/// Cannot adopt a caller's home/profile directory. Dropping this object deletes
/// the rehearsal artifacts; retain it for the entire backup/restore experiment.
/// Its path is exposed only to allow the synthetic fixture builder to create input.
pub struct SyntheticWorkspace {
    root: TempDir,
    backups: RefCell<BTreeMap<PathBuf, (Schema, Records)>>,
    outputs: RefCell<BTreeSet<PathBuf>>,
}
impl SyntheticWorkspace {
    pub fn new() -> Result<Self> {
        let root = tempfile::Builder::new()
            .prefix("acp-synthetic-rehearsal-")
            .tempdir()
            .map_err(|_| Error::OpenFailed)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
                .map_err(|_| Error::OpenFailed)?;
        }
        Ok(Self {
            root,
            backups: RefCell::default(),
            outputs: RefCell::default(),
        })
    }
    pub fn path(&self) -> &Path {
        self.root.path()
    }
    fn check_path(&self, path: &Path, existing: bool) -> Result<()> {
        let relative = path
            .strip_prefix(self.path())
            .map_err(|_| Error::UnsafePath)?;
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(Error::UnsafePath);
        }
        let mut current = self.path().to_path_buf();
        for part in relative.components() {
            current.push(part);
            match fs::symlink_metadata(&current) {
                Ok(meta) => {
                    // Also reject Windows junctions/reparse points.
                    #[cfg(windows)]
                    {
                        use std::os::windows::fs::MetadataExt;
                        if meta.file_attributes() & 0x400 != 0 {
                            return Err(Error::UnsafePath);
                        }
                    }
                    if meta.file_type().is_symlink() {
                        return Err(Error::UnsafePath);
                    }
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::MetadataExt;
                        if meta.is_file() && meta.nlink() != 1 {
                            return Err(Error::UnsafePath);
                        }
                    }
                    if current != path && !meta.is_dir() {
                        return Err(Error::UnsafePath);
                    }
                }
                Err(e)
                    if current == path && !existing && e.kind() == std::io::ErrorKind::NotFound => {
                }
                Err(_) => return Err(Error::UnsafePath),
            }
        }
        if existing && !path.is_file() {
            return Err(Error::UnsafePath);
        }
        for suffix in ["-wal", "-shm", "-journal"] {
            let mut name = path.as_os_str().to_os_string();
            name.push(suffix);
            let sidecar = PathBuf::from(name);
            if fs::symlink_metadata(&sidecar).is_ok() {
                if !existing {
                    return Err(Error::Collision);
                }
                // Check the sidecar too, without recursively checking its sidecars.
                let meta = fs::symlink_metadata(&sidecar).map_err(|_| Error::UnsafePath)?;
                if !meta.is_file() || meta.file_type().is_symlink() {
                    return Err(Error::UnsafePath);
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if meta.file_attributes() & 0x400 != 0 {
                        return Err(Error::UnsafePath);
                    }
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if meta.nlink() != 1 {
                        return Err(Error::UnsafePath);
                    }
                }
            }
        }
        Ok(())
    }
    fn reserve(&self, path: &Path) -> Result<()> {
        self.check_path(path, false)?;
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        opts.open(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Error::Collision
            } else {
                Error::OpenFailed
            }
        })?;
        Ok(())
    }
    /// Pinned source read snapshot + SQLite backup API; never opens source writable.
    pub fn backup(&self, source: &Path, backup: &Path, fault: Fault) -> Result<()> {
        self.check_path(source, true)?;
        self.check_path(backup, false)?;
        let source_db = open(source, false)?;
        source_db
            .execute_batch("BEGIN")
            .map_err(|_| Error::Locked)?;
        let shape = verify_schema(&source_db)?;
        integrity(&source_db)?;
        let before = records(&source_db)?;
        if let Some((saved_shape, saved_records)) = self.backups.borrow().get(source)
            && (*saved_shape != shape || *saved_records != before)
        {
            return Err(Error::Verification);
        }
        self.reserve(backup)?;
        let result = (|| {
            let mut dest = open(backup, true)?;
            if fault == Fault::BackupWriteError {
                return Err(Error::InjectedWrite);
            }
            {
                let copy = Backup::new(&source_db, &mut dest).map_err(|_| Error::BackupFailed)?;
                let mut done = false;
                for _ in 0..1024 {
                    let pages = if fault == Fault::BackupInterrupted {
                        1
                    } else {
                        16
                    };
                    let step = copy.step(pages).map_err(|_| Error::BackupFailed)?;
                    if fault == Fault::BackupInterrupted {
                        return Err(Error::InjectedBackup);
                    }
                    match step {
                        StepResult::Done => {
                            done = true;
                            break;
                        }
                        StepResult::More => (),
                        StepResult::Busy | StepResult::Locked => return Err(Error::Locked),
                        _ => return Err(Error::BackupFailed),
                    }
                }
                if !done {
                    return Err(Error::BackupFailed);
                }
            }
            drop(dest);
            let independent = open(backup, false)?;
            integrity(&independent)?;
            if verify_schema(&independent)? != verify_schema(&source_db)?
                || records(&independent)? != before
            {
                return Err(Error::Verification);
            }
            Ok(())
        })();
        // Only this call's exclusively reserved file can be removed on failure.
        // Never clean caller files, backups, source files or active sidecars.
        if result.is_err() {
            fs::remove_file(backup).map_err(|_| Error::BackupFailed)?;
        }
        if result.is_ok() {
            self.backups
                .borrow_mut()
                .insert(backup.to_path_buf(), (shape, before));
        }
        result
    }
    /// Normal workflow: verified untouched backup, then migrate a NEW output copy.
    /// A precommit failure keeps a rolled-back v0 output for inspection; a
    /// postcommit error explicitly leaves v1 output and a verified v0 backup.
    pub fn rehearse(
        &self,
        source: &Path,
        backup: &Path,
        output: &Path,
        fault: Fault,
    ) -> Result<Outcome> {
        self.check_path(output, false)?;
        if output.exists() || source == output || backup == output {
            return Err(Error::Collision);
        }
        self.backup(source, backup, fault)?;
        self.backup(backup, output, Fault::None)?;
        self.backups.borrow_mut().remove(output);
        self.outputs.borrow_mut().insert(output.to_path_buf());
        let expected = self
            .backups
            .borrow()
            .get(backup)
            .ok_or(Error::Verification)?
            .1
            .clone();
        self.migrate_output(output, fault, &expected)
    }
    /// Retry an output already created inside this rehearsal. Never accepts a
    /// caller-created source: require this workspace's registered output and backup
    /// with matching logical records. No public arbitrary-path mutation method.
    pub fn retry(&self, backup: &Path, output: &Path, fault: Fault) -> Result<Outcome> {
        if !self.backups.borrow().contains_key(backup) || !self.outputs.borrow().contains(output) {
            return Err(Error::UnsafePath);
        }
        self.check_path(backup, true)?;
        self.check_path(output, true)?;
        if backup == output {
            return Err(Error::Collision);
        }
        let b = open(backup, false)?;
        let o = open(output, false)?;
        verify_schema(&b)?;
        verify_schema(&o)?;
        integrity(&b)?;
        integrity(&o)?;
        let expected = self
            .backups
            .borrow()
            .get(backup)
            .ok_or(Error::Verification)?
            .clone();
        if verify_schema(&b)? != expected.0
            || records(&b)? != expected.1
            || records(&o)? != expected.1
        {
            return Err(Error::Verification);
        }
        drop(o);
        drop(b);
        self.migrate_output(output, fault, &expected.1)
    }
    fn migrate_output(&self, output: &Path, fault: Fault, expected: &Records) -> Result<Outcome> {
        self.check_path(output, true)?;
        let mut db = open(output, true)?;
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| Error::Locked)?;
        let schema = verify_schema(&tx)?;
        integrity(&tx)?;
        let before = records(&tx)?;
        if &before != expected {
            return Err(Error::Verification);
        }
        if schema == Schema::Version1 {
            return Ok(Outcome::AlreadyV1);
        }
        if schema == Schema::Legacy {
            tx.execute_batch("ALTER TABLE handoffs ADD COLUMN worktree_fingerprint TEXT")
                .map_err(|_| Error::Schema)?;
        }
        if fault == Fault::AfterDdl {
            return Err(Error::BeforeCommit);
        }
        tx.pragma_update(None, "user_version", 1)
            .map_err(|_| Error::Schema)?;
        if fault == Fault::BeforeCommit {
            return Err(Error::BeforeCommit);
        }
        if verify_schema(&tx)? != Schema::Version1 || records(&tx)? != before {
            return Err(Error::Verification);
        }
        tx.commit().map_err(|_| Error::Locked)?;
        if fault == Fault::AfterCommit {
            return Err(Error::AfterCommit);
        }
        integrity(&db)?;
        if records(&db)? != before {
            return Err(Error::Verification);
        }
        Ok(Outcome::Migrated)
    }
    /// Recovery always creates a separate destination with the original version.
    pub fn restore(&self, backup: &Path, restored: &Path) -> Result<()> {
        if !self.backups.borrow().contains_key(backup) {
            return Err(Error::UnsafePath);
        }
        self.backup(backup, restored, Fault::None)
    }
}
fn open(path: &Path, writable: bool) -> Result<Connection> {
    let flags = if writable {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    } else {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    };
    let db = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|_| Error::OpenFailed)?;
    db.busy_timeout(Duration::ZERO)
        .map_err(|_| Error::OpenFailed)?;
    db.pragma_update(None, "trusted_schema", false)
        .map_err(|_| Error::OpenFailed)?;
    if !writable {
        db.pragma_update(None, "query_only", true)
            .map_err(|_| Error::OpenFailed)?;
    }
    Ok(db)
}
fn integrity(db: &Connection) -> Result<()> {
    let mut stmt = db
        .prepare("PRAGMA integrity_check")
        .map_err(|_| Error::Corrupt)?;
    let values = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|_| Error::Corrupt)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| Error::Corrupt)?;
    if values != ["ok"] {
        return Err(Error::Corrupt);
    }
    let violations: i64 = db
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .map_err(|_| Error::Corrupt)?;
    if violations != 0 {
        return Err(Error::Corrupt);
    }
    Ok(())
}
fn verify_schema(db: &Connection) -> Result<Schema> {
    let shape = inspect_schema(db).map_err(|_| Error::Schema)?;
    // Migration accepts the exact P0 nullability and index contract, in addition
    // to the read adapter's names/types/PK checks. CHECK SQL is verified below.
    crate::schema::validate(db)?;
    Ok(shape)
}
type Records = Vec<Vec<Vec<Value>>>;
fn records(db: &Connection) -> Result<Records> {
    let shape = inspect_schema(db).map_err(|_| Error::Schema)?;
    let fp = if shape == Schema::Legacy {
        "NULL"
    } else {
        "worktree_fingerprint"
    };
    let queries=["SELECT name,root,created_at FROM projects ORDER BY name".to_owned(),
        "SELECT id,scope,project,pref_key,value,status,source,created_at,updated_at FROM preferences ORDER BY id".to_owned(),
        format!("SELECT project,task_id,objective,base_sha,branch,worktree_state,{fp},completed,next_action,validation_json,updated_at FROM handoffs ORDER BY project,task_id"),
        "SELECT name,seq FROM sqlite_sequence ORDER BY name".to_owned()];
    queries
        .iter()
        .map(|query| {
            let mut stmt = db.prepare(query).map_err(|_| Error::Verification)?;
            let n = stmt.column_count();
            stmt.query_map([], |row| {
                (0..n)
                    .map(|i| row.get::<_, Value>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(|_| Error::Verification)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|_| Error::Verification)
        })
        .collect()
}
mod schema;
