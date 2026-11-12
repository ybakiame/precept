//! Daemon-only normal dependency. Selection is RO; attachment always acquires
//! the native lease and advances epoch. No arbitrary existing-path Writer API.
use crate::{Binding, Error, Fault, Profile, Receipt, Request, Result, files, profile::connect};
use acp_store::{SqliteStore, profile};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    path::{Path, PathBuf},
};

pub struct Selected {
    profile: Profile,
    guard: File,
}
impl Selected {
    pub fn select(path: &Path) -> Result<Self> {
        files::validate_name(path)?;
        let parent = path.parent().ok_or(Error::UnsafeWorkspace)?;
        let parent_identity = files::check_parent(parent)?;
        let guard = files::open_guard(path, false)?;
        let (a, b) = acp_store::native::identity(&guard)?;
        let path = path.canonicalize().map_err(|_| Error::UnsafeWorkspace)?;
        let reader = SqliteStore::open(&path)?;
        reader.check_open_file(&guard)?;
        let selected = Self {
            profile: Profile {
                path,
                identity: files::Identity(a, b),
                parent_identity,
                binding: reader.metadata()?.binding,
            },
            guard,
        };
        selected.reader()?;
        Ok(selected)
    }
    pub fn binding(&self) -> Binding {
        self.profile.binding()
    }
    pub fn path(&self) -> &Path {
        &self.profile.path
    }
    pub fn reader(&self) -> Result<SqliteStore> {
        self.profile.check()?;
        let reader = SqliteStore::open(&self.profile.path)?;
        reader.check_open_file(&self.guard)?;
        let binding = reader.metadata()?.binding;
        if binding.profile_id != self.profile.binding.profile_id
            || binding.generation != self.profile.binding.generation
        {
            return Err(Error::OwnerMismatch);
        }
        self.profile.check()?;
        reader.check_open_file(&self.guard)?;
        Ok(reader)
    }
    pub fn acquire(&self) -> Result<Owner> {
        self.reader()?;
        let id = self.profile.identity;
        let name = format!(
            ".acp-writer-{:x}.lock",
            Sha256::digest([id.0.to_le_bytes(), id.1.to_le_bytes()].concat())
        );
        let lock_path = self
            .profile
            .path
            .parent()
            .ok_or(Error::UnsafeWorkspace)?
            .join(name);
        let lock = match files::create_private(&lock_path) {
            Ok(f) => f,
            Err(Error::Collision) => files::open_guard(&lock_path, true)?,
            Err(e) => return Err(e),
        };
        lock.try_lock().map_err(|e| match e {
            std::fs::TryLockError::WouldBlock => Error::Busy,
            _ => Error::UnsafeWorkspace,
        })?;
        let lock_identity = files::check_file(&lock_path)?;
        if acp_store::native::identity(&lock)? != (lock_identity.0, lock_identity.1) {
            return Err(Error::UnsafeWorkspace);
        }
        self.reader()?;
        let mut db = connect(&self.profile.path, false)?;
        acp_store::native::check(&db, &self.guard)?;
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(crate::sql_error)?;
        let mut binding = profile::verify(&tx)?.binding;
        if binding.profile_id != self.profile.binding.profile_id
            || binding.generation != self.profile.binding.generation
        {
            return Err(Error::OwnerMismatch);
        }
        binding.owner_epoch = binding.owner_epoch.checked_add(1).ok_or(Error::Capacity)?;
        tx.execute(
            "UPDATE acp_profile SET owner_epoch=?1 WHERE singleton=1",
            [binding.owner_epoch],
        )
        .map_err(crate::sql_error)?;
        tx.commit().map_err(|_| Error::OutcomeUnknown)?;
        let owner = Owner {
            profile: Profile {
                path: self.profile.path.clone(),
                identity: id,
                parent_identity: self.profile.parent_identity,
                binding,
            },
            guard: self.guard.try_clone().map_err(|_| Error::IoFailure)?,
            lock,
            lock_path,
            lock_identity,
        };
        owner.check()?;
        Ok(owner)
    }
}
pub struct Owner {
    profile: Profile,
    guard: File,
    lock: File,
    lock_path: PathBuf,
    lock_identity: files::Identity,
}
impl Owner {
    pub fn binding(&self) -> Binding {
        self.profile.binding()
    }
    fn check(&self) -> Result<()> {
        self.profile.check()?;
        if files::check_file(&self.lock_path)? != self.lock_identity
            || acp_store::native::identity(&self.lock)?
                != (self.lock_identity.0, self.lock_identity.1)
        {
            return Err(Error::UnsafeWorkspace);
        }
        Ok(())
    }
    pub fn execute(&self, request: &Request, admit: &dyn Fn() -> Result<()>) -> Result<Receipt> {
        admit()?;
        self.check()?;
        let mut writer = self.profile.writer()?;
        acp_store::native::check(&writer.db, &self.guard)?;
        writer.execute_checked(request, Fault::None, &|| {
            self.check()?;
            admit()
        })
    }
}
