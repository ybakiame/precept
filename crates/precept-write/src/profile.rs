use crate::{Binding, Error, Fault, Result, Writer, files};
use precept_store::{SqliteStore, profile};
use rusqlite::{Connection, OpenFlags};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
pub struct Profile {
    pub(crate) path: PathBuf,
    pub(crate) identity: files::Identity,
    pub(crate) parent_identity: files::Identity,
    pub(crate) binding: Binding,
}
impl std::fmt::Debug for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Profile(redacted)")
    }
}
impl Profile {
    /// Explicit fresh creation only. No existing-profile writer adoption API.
    pub fn create(destination: &Path) -> Result<Self> {
        Self::create_inner(destination, Fault::None)
    }
    pub(crate) fn create_inner(destination: &Path, _fault: Fault) -> Result<Self> {
        let (path, parent_identity) = files::destination(destination)?;
        let parent = path.parent().ok_or(Error::UnsafeWorkspace)?;
        let stage = parent.join(format!(".precept-create-{}", random_id()?));
        let file = files::create_private(&stage)?;
        drop(file);
        let result = (|| {
            let db = connect(&stage, false)?;
            db.execute_batch("BEGIN IMMEDIATE")
                .map_err(crate::sql_error)?;
            for ddl in profile::DDL {
                db.execute_batch(ddl).map_err(crate::sql_error)?;
            }
            db.pragma_update(None, "user_version", 1)
                .map_err(crate::sql_error)?;
            let binding = Binding {
                profile_id: random_id()?,
                generation: random_id()?,
                owner_epoch: 0,
            };
            db.execute(
                "INSERT INTO acp_profile VALUES(1,?1,?2,0,1,0,1)",
                (&binding.profile_id, &binding.generation),
            )
            .map_err(crate::sql_error)?;
            profile::verify(&db)?;
            #[cfg(test)]
            if _fault == Fault::BeforeCommit {
                return Err(Error::BeforeCommit);
            }
            db.execute_batch("COMMIT").map_err(crate::sql_error)?;
            db.close().map_err(|_| Error::IoFailure)?;
            let reader = SqliteStore::open(&stage)?;
            if reader.metadata()?.binding != binding {
                return Err(Error::Verification);
            }
            drop(reader);
            files::sidecars(&stage)?;
            files::check_file(&stage)?;
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&stage)
                .map_err(|_| Error::IoFailure)?
                .sync_all()
                .map_err(|_| Error::IoFailure)?;
            // Check the selected parent and target namespace again before publish.
            if files::destination(&path)?.1 != parent_identity {
                return Err(Error::UnsafeWorkspace);
            }
            let identity = files::check_file(&stage)?;
            files::publish(&stage, &path)?;
            // Publication is the irreversible boundary: never remove the target.
            let finish = (|| {
                #[cfg(test)]
                if _fault == Fault::AfterCommit {
                    return Err(Error::Verification);
                }
                files::flush_parent(parent)?;
                if files::check_file(&path)? != identity {
                    return Err(Error::Verification);
                }
                let reader = SqliteStore::open(&path)?;
                if reader.metadata()?.binding != binding {
                    return Err(Error::Verification);
                }
                Ok(Self {
                    path: path.clone(),
                    identity,
                    parent_identity,
                    binding,
                })
            })();
            finish.map_err(|_| Error::OutcomeUnknown)
        })();
        // Only our randomized, unpublished staging file can be removed here.
        if stage.exists() {
            let _ = fs::remove_file(&stage);
        }
        result
    }
    pub fn binding(&self) -> Binding {
        self.binding.clone()
    }
    pub(crate) fn check(&self) -> Result<()> {
        if files::check_parent(self.path.parent().ok_or(Error::UnsafeWorkspace)?)?
            != self.parent_identity
            || files::check_file(&self.path)? != self.identity
        {
            return Err(Error::UnsafeWorkspace);
        }
        files::live_sidecars(&self.path)?;
        Ok(())
    }
    pub fn writer(&self) -> Result<Writer<'_>> {
        self.check()?;
        let db = connect(&self.path, false)?;
        {
            let tx = db.unchecked_transaction().map_err(crate::sql_error)?;
            let metadata = profile::verify(&tx)?;
            if metadata.binding != self.binding {
                return Err(Error::OwnerMismatch);
            }
        }
        Ok(Writer { db, profile: self })
    }
}
pub(crate) fn connect(path: &Path, read_only: bool) -> Result<Connection> {
    let mode = if read_only {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };
    let db = Connection::open_with_flags(path, mode | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(crate::sql_error)?;
    db.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH, 256 * 1024)
        .map_err(crate::sql_error)?;
    db.busy_timeout(Duration::from_millis(100))
        .map_err(crate::sql_error)?;
    db.pragma_update(None, "foreign_keys", true)
        .map_err(crate::sql_error)?;
    db.pragma_update(None, "trusted_schema", false)
        .map_err(crate::sql_error)?;
    if read_only {
        db.pragma_update(None, "query_only", true)
            .map_err(crate::sql_error)?;
    } else {
        db.pragma_update(None, "synchronous", "FULL")
            .map_err(crate::sql_error)?;
    }
    Ok(db)
}
pub(crate) fn random_id() -> Result<String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|_| Error::IoFailure)?;
    Ok(b.iter().map(|v| format!("{v:02x}")).collect())
}
