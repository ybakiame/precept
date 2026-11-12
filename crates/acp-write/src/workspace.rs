//! Fresh owned fixtures and provenance; none of these constructors can adopt a path.
use crate::{Binding, Error, Fault, FixtureRoot, Result, Writer, schema};
use rusqlite::{
    Connection, OpenFlags, TransactionBehavior,
    backup::{Backup, StepResult},
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};
use tempfile::TempDir;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixtureShape {
    LegacyV0,
    FingerprintedV0,
    Version1,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity(u64, u64);
#[derive(Clone, Copy)]
pub(crate) enum Role {
    Source,
    Backup,
    Candidate,
}
impl Role {
    fn directory(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Backup => "backup",
            Self::Candidate => "candidate",
        }
    }
}

/// Cannot be serialized, cloned or reconstructed from caller-supplied locations.
/// Borrowed writers cannot outlive the parent that deletes its disposable root.
pub struct SyntheticWorkspace {
    root: TempDir,
    root_identity: Identity,
    source_identity: Identity,
    backup_identity: Option<Identity>,
    candidate_identity: Option<Identity>,
    pub(crate) binding: Binding,
    original: Option<schema::Records>,
    backup_digest: Option<[u8; 32]>,
    verified: bool,
}
impl std::fmt::Debug for SyntheticWorkspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SyntheticWorkspace(redacted)")
    }
}
impl SyntheticWorkspace {
    pub fn new(shape: FixtureShape) -> Result<Self> {
        Self::build(
            tempfile::Builder::new()
                .prefix("acp-m1-synthetic-")
                .tempdir()
                .map_err(|_| Error::IoFailure)?,
            shape,
        )
    }
    #[cfg(test)]
    pub(crate) fn for_test_parent(parent: &Path, shape: FixtureShape) -> Result<Self> {
        let canonical = parent.canonicalize().map_err(|_| Error::UnsafeWorkspace)?;
        if canonical.parent()
            != Some(
                std::env::temp_dir()
                    .canonicalize()
                    .map_err(|_| Error::UnsafeWorkspace)?
                    .as_path(),
            )
            || !canonical
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("acp-m1-parent-"))
        {
            return Err(Error::UnsafeWorkspace);
        }
        identity(&canonical, true)?;
        Self::build(
            tempfile::Builder::new()
                .prefix("acp-m1-child-")
                .tempdir_in(canonical)
                .map_err(|_| Error::IoFailure)?,
            shape,
        )
    }
    fn build(root: TempDir, shape: FixtureShape) -> Result<Self> {
        private_dir(root.path())?;
        let root_identity = identity(root.path(), true)?;
        for dir in [
            "source",
            "backup",
            "candidate",
            "projects",
            "projects/alpha",
            "projects/beta",
            "projects/gamma",
        ] {
            fs::create_dir(root.path().join(dir)).map_err(|_| Error::IoFailure)?;
            private_dir(&root.path().join(dir))?;
        }
        let binding = Binding {
            profile_id: random_id()?,
            generation: random_id()?,
            owner_epoch: 1,
        };
        let source = root.path().join("source/profile.sqlite3");
        reserve_file(&source)?;
        let db = connect(&source, false)?;
        schema::create_source(
            &db,
            match shape {
                FixtureShape::LegacyV0 => schema::Shape::Legacy,
                FixtureShape::FingerprintedV0 => schema::Shape::Fingerprinted,
                FixtureShape::Version1 => schema::Shape::V1,
            },
        )?;
        for name in ["alpha", "beta"] {
            let project_root = root
                .path()
                .canonicalize()
                .map_err(|_| Error::UnsafeWorkspace)?
                .join("projects")
                .join(name);
            db.execute(
                "INSERT INTO projects VALUES(?1,?2,'2024-01-02T03:04:05Z')",
                (name, project_root.to_str().ok_or(Error::UnsafeWorkspace)?),
            )
            .map_err(crate::sql_error)?;
        }
        // These stored deprecated fixtures model historical rows reachable through
        // permissive Python transitions; migration never replays/judges that history.
        for (id, scope, project, status) in [
            (1, "global", None, "suggested"),
            (2, "global", None, "confirmed"),
            (3, "global", None, "rejected"),
            (4, "global", None, "deprecated"),
            (5, "project", Some("alpha"), "suggested"),
            (6, "project", Some("alpha"), "confirmed"),
            (7, "project", Some("alpha"), "confirmed"),
            (8, "project", Some("alpha"), "rejected"),
            (9, "project", Some("alpha"), "deprecated"),
            (10, "project", Some("beta"), "confirmed"),
        ] {
            db.execute("INSERT INTO preferences VALUES(?1,?2,?3,'formatter',?4,?5,'legacy-unknown','original-created','original-updated')",rusqlite::params![id,scope,project,format!("synthetic-value-{id}"),status]).map_err(crate::sql_error)?;
        }
        db.execute(
            "UPDATE sqlite_sequence SET seq=1000 WHERE name='preferences'",
            [],
        )
        .map_err(crate::sql_error)?;
        for project in ["alpha", "beta"] {
            let raw = "[ {\"check\":\"synthetic\", \"status\":\"PASS\", \"evidence\":{\"id\":\"opaque\",\"updated_at\":\"old\",\"nested\":[null,true,2,\"合成\"]}}, {\"check\":\"unrun\",\"status\":\"NOT_RUN\"} ]";
            db.execute("INSERT INTO handoffs(project,task_id,objective,base_sha,branch,worktree_state,completed,next_action,validation_json,updated_at) VALUES(?1,'shared','synthetic objective','base','main','clean','done','inspect',?2,'historical-time')",(project,raw)).map_err(crate::sql_error)?;
        }
        if shape != FixtureShape::LegacyV0 {
            db.execute(
                "UPDATE handoffs SET worktree_fingerprint='saved' WHERE project='alpha'",
                [],
            )
            .map_err(crate::sql_error)?;
        }
        drop(db);
        let source_identity = identity(&source, false)?;
        Ok(Self {
            root,
            root_identity,
            source_identity,
            backup_identity: None,
            candidate_identity: None,
            binding,
            original: None,
            backup_digest: None,
            verified: false,
        })
    }
    pub fn binding(&self) -> Binding {
        self.binding.clone()
    }
    pub fn prepare(&mut self) -> Result<()> {
        self.prepare_inner(Fault::None)
    }
    pub(crate) fn path(&self, role: Role) -> PathBuf {
        self.root
            .path()
            .join(role.directory())
            .join("profile.sqlite3")
    }
    pub(crate) fn project_root(&self, slot: FixtureRoot) -> Result<String> {
        self.check()?;
        self.root
            .path()
            .canonicalize()
            .map_err(|_| Error::UnsafeWorkspace)?
            .join("projects")
            .join(slot.name())
            .to_str()
            .map(str::to_owned)
            .ok_or(Error::UnsafeWorkspace)
    }
    pub(crate) fn check(&self) -> Result<()> {
        if identity(self.root.path(), true)? != self.root_identity {
            return Err(Error::UnsafeWorkspace);
        }
        for dir in [
            "source",
            "backup",
            "candidate",
            "projects",
            "projects/alpha",
            "projects/beta",
            "projects/gamma",
        ] {
            identity(&self.root.path().join(dir), true)?;
        }
        for entry in fs::read_dir(self.root.path()).map_err(|_| Error::UnsafeWorkspace)? {
            let name = entry.map_err(|_| Error::UnsafeWorkspace)?.file_name();
            if !["source", "backup", "candidate", "projects"]
                .iter()
                .any(|s| name == *s)
            {
                return Err(Error::UnsafeWorkspace);
            }
        }
        for entry in
            fs::read_dir(self.root.path().join("projects")).map_err(|_| Error::UnsafeWorkspace)?
        {
            let entry = entry.map_err(|_| Error::UnsafeWorkspace)?;
            if !["alpha", "beta", "gamma"]
                .iter()
                .any(|n| entry.file_name() == *n)
                || fs::read_dir(entry.path())
                    .map_err(|_| Error::UnsafeWorkspace)?
                    .next()
                    .is_some()
            {
                return Err(Error::UnsafeWorkspace);
            }
        }
        for role in [Role::Source, Role::Backup, Role::Candidate] {
            for entry in fs::read_dir(self.root.path().join(role.directory()))
                .map_err(|_| Error::UnsafeWorkspace)?
            {
                let entry = entry.map_err(|_| Error::UnsafeWorkspace)?;
                if ![
                    "profile.sqlite3",
                    "profile.sqlite3-wal",
                    "profile.sqlite3-shm",
                    "profile.sqlite3-journal",
                ]
                .iter()
                .any(|n| entry.file_name() == *n)
                {
                    return Err(Error::UnsafeWorkspace);
                }
                identity(&entry.path(), false)?;
            }
            let expected = match role {
                Role::Source => Some(self.source_identity),
                Role::Backup => self.backup_identity,
                Role::Candidate => self.candidate_identity,
            };
            if let Some(expected) = expected
                && identity(&self.path(role), false)? != expected
            {
                return Err(Error::UnsafeWorkspace);
            }
        }
        Ok(())
    }
    fn reserve(&self, role: Role) -> Result<Identity> {
        self.check()?;
        let path = self.path(role);
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let name = PathBuf::from(format!(
                "{}{suffix}",
                path.to_str().ok_or(Error::UnsafeWorkspace)?
            ));
            if fs::symlink_metadata(name).is_ok() {
                return Err(Error::Collision);
            }
        }
        reserve_file(&path)?;
        identity(&path, false)
    }
    fn check_roots(&self, db: &Connection) -> Result<()> {
        for row in &schema::records(db)?[0] {
            let rusqlite::types::Value::Text(root) = &row[1] else {
                return Err(Error::InvalidRecord);
            };
            if ![FixtureRoot::Alpha, FixtureRoot::Beta, FixtureRoot::Gamma]
                .iter()
                .any(|slot| self.project_root(*slot).is_ok_and(|p| p == *root))
            {
                return Err(Error::UnsafeWorkspace);
            }
        }
        Ok(())
    }
    pub(crate) fn prepare_inner(&mut self, _fault: Fault) -> Result<()> {
        self.check()?;
        if self.backup_identity.is_some() || self.candidate_identity.is_some() {
            return Err(Error::Collision);
        }
        let source = connect(&self.path(Role::Source), true)?;
        source.execute_batch("BEGIN").map_err(crate::sql_error)?;
        if schema::inspect(&source)? == schema::Shape::V2 {
            return Err(Error::UnsupportedVersion);
        }
        schema::validate_records(&source)?;
        self.check_roots(&source)?;
        let original = schema::records(&source)?;
        self.backup_identity = Some(self.reserve(Role::Backup)?);
        #[cfg(test)]
        if _fault == Fault::StorageFull {
            return Err(Error::StorageFull);
        }
        backup(&source, &self.path(Role::Backup), _fault)?;
        let backup_db = connect(&self.path(Role::Backup), true)?;
        schema::validate_records(&backup_db)?;
        if schema::inspect(&backup_db)? != schema::inspect(&source)?
            || schema::records(&backup_db)? != original
        {
            return Err(Error::Verification);
        }
        drop(source);
        self.backup_digest = Some(file_digest(&self.path(Role::Backup))?);
        self.original = Some(original);
        self.candidate_identity = Some(self.reserve(Role::Candidate)?);
        backup(&backup_db, &self.path(Role::Candidate), Fault::None)?;
        drop(backup_db);
        let mut output = connect(&self.path(Role::Candidate), false)?;
        let tx = output
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(crate::sql_error)?;
        schema::validate_records(&tx)?;
        if Some(&schema::records(&tx)?) != self.original.as_ref() {
            return Err(Error::Verification);
        }
        schema::migrate(&tx)?;
        tx.execute(
            "INSERT INTO acp_profile VALUES(1,?1,?2,?3,0,4096)",
            (
                &self.binding.profile_id,
                &self.binding.generation,
                self.binding.owner_epoch,
            ),
        )
        .map_err(crate::sql_error)?;
        tx.execute_batch("INSERT INTO acp_revisions SELECT 'project','',name,1,0 FROM projects; INSERT INTO acp_revisions SELECT 'preference','',cast(id AS TEXT),1,0 FROM preferences; INSERT INTO acp_revisions SELECT 'handoff',project,task_id,1,0 FROM handoffs;").map_err(crate::sql_error)?;
        #[cfg(test)]
        if _fault == Fault::BeforeCommit {
            return Err(Error::BeforeCommit);
        }
        tx.commit().map_err(|_| Error::OutcomeUnknown)?;
        drop(output);
        #[cfg(test)]
        if _fault == Fault::AfterCommit {
            return Err(Error::OutcomeUnknown);
        }
        self.verify_candidate()
    }
    /// Explicit reconciliation of this workspace's candidate, never adoption.
    pub fn verify_candidate(&mut self) -> Result<()> {
        self.verified = false;
        self.check()?;
        if self.candidate_identity.is_none() || self.original.is_none() {
            return Err(Error::Verification);
        }
        self.verify_prior()?;
        let candidate = connect(&self.path(Role::Candidate), true)?;
        candidate.execute_batch("BEGIN").map_err(crate::sql_error)?;
        crate::writer::verify(&candidate, &self.binding)?;
        let (sequence,limit,changed):(i64,i64,i64)=candidate.query_row(
            "SELECT commit_sequence,receipt_limit,(SELECT count(*) FROM acp_revisions WHERE revision<>1 OR deleted<>0) FROM acp_profile WHERE singleton=1",
            [],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))
        ).map_err(|_|Error::Verification)?;
        if sequence != 0 || limit != 4096 || changed != 0 {
            return Err(Error::Verification);
        }
        self.check_roots(&candidate)?;
        if Some(&schema::records(&candidate)?) != self.original.as_ref() {
            return Err(Error::Verification);
        }
        self.verified = true;
        Ok(())
    }
    pub(crate) fn verify_prior(&self) -> Result<()> {
        self.check()?;
        if Some(file_digest(&self.path(Role::Backup))?) != self.backup_digest {
            return Err(Error::Verification);
        }
        for role in [Role::Source, Role::Backup] {
            let db = connect(&self.path(role), true)?;
            schema::validate_records(&db)?;
            if Some(&schema::records(&db)?) != self.original.as_ref() {
                return Err(Error::Verification);
            }
        }
        Ok(())
    }
    pub fn writer(&self) -> Result<Writer<'_>> {
        if !self.verified {
            return Err(Error::Verification);
        }
        self.check()?;
        let read = connect(&self.path(Role::Candidate), true)?;
        read.execute_batch("BEGIN").map_err(crate::sql_error)?;
        crate::writer::verify(&read, &self.binding)?;
        drop(read);
        let db = connect(&self.path(Role::Candidate), false)?;
        Ok(Writer {
            db,
            workspace: self,
        })
    }
}
fn random_id() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::IoFailure)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn private_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| Error::IoFailure)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
fn reserve_file(path: &Path) -> Result<()> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|_| Error::Collision)?;
    Ok(())
}
fn identity(path: &Path, directory: bool) -> Result<Identity> {
    let meta = fs::symlink_metadata(path).map_err(|_| Error::UnsafeWorkspace)?;
    if meta.file_type().is_symlink()
        || (if directory {
            !meta.is_dir()
        } else {
            !meta.is_file()
        })
    {
        return Err(Error::UnsafeWorkspace);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if (!directory && meta.nlink() != 1)
            || meta.permissions().mode() & 0o777 != if directory { 0o700 } else { 0o600 }
        {
            return Err(Error::UnsafeWorkspace);
        }
        Ok(Identity(meta.dev(), meta.ino()))
    }
    #[cfg(windows)]
    {
        use std::os::windows::{
            fs::{MetadataExt, OpenOptionsExt},
            io::AsRawHandle,
        };
        if meta.file_attributes() & 0x400 != 0 || meta.permissions().readonly() {
            return Err(Error::UnsafeWorkspace);
        }
        #[repr(C)]
        struct Info {
            attributes: u32,
            creation: [u32; 2],
            access: [u32; 2],
            write: [u32; 2],
            volume: u32,
            size_high: u32,
            size_low: u32,
            links: u32,
            index_high: u32,
            index_low: u32,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetFileInformationByHandle(handle: *mut std::ffi::c_void, info: *mut Info) -> i32;
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(if directory { 0x02000000 } else { 0 })
            .open(path)
            .map_err(|_| Error::UnsafeWorkspace)?;
        let mut info = std::mem::MaybeUninit::<Info>::uninit();
        // SAFETY: valid owned file handle; Info matches BY_HANDLE_FILE_INFORMATION.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
            return Err(Error::UnsafeWorkspace);
        }
        // SAFETY: the successful call initialized every field of Info.
        let info = unsafe { info.assume_init() };
        if !directory && info.links != 1 {
            return Err(Error::UnsafeWorkspace);
        }
        Ok(Identity(
            u64::from(info.volume),
            (u64::from(info.index_high) << 32) | u64::from(info.index_low),
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(Error::UnsafeWorkspace)
    }
}
pub(crate) fn connect(path: &Path, readonly: bool) -> Result<Connection> {
    let flags = if readonly {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };
    let db = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(crate::sql_error)?;
    db.busy_timeout(Duration::from_millis(100))
        .map_err(crate::sql_error)?;
    db.pragma_update(None, "trusted_schema", false)
        .map_err(crate::sql_error)?;
    db.pragma_update(None, "foreign_keys", true)
        .map_err(crate::sql_error)?;
    if readonly {
        db.pragma_update(None, "query_only", true)
            .map_err(crate::sql_error)?;
    } else {
        db.pragma_update(None, "synchronous", "FULL")
            .map_err(crate::sql_error)?;
    }
    Ok(db)
}
fn backup(source: &Connection, path: &Path, _fault: Fault) -> Result<()> {
    let mut destination = connect(path, false)?;
    {
        let copy = Backup::new(source, &mut destination).map_err(crate::sql_error)?;
        let mut done = false;
        for _ in 0..1024 {
            let result = copy.step(16).map_err(crate::sql_error)?;
            #[cfg(test)]
            if _fault == Fault::BackupPartial {
                return Err(Error::BeforeCommit);
            }
            match result {
                StepResult::Done => {
                    done = true;
                    break;
                }
                StepResult::More => (),
                StepResult::Busy | StepResult::Locked => return Err(Error::Busy),
                _ => return Err(Error::IoFailure),
            }
        }
        if !done {
            return Err(Error::Capacity);
        }
    }
    destination
        .pragma_update(None, "journal_mode", "DELETE")
        .map_err(crate::sql_error)?;
    Ok(())
}
fn file_digest(path: &Path) -> Result<[u8; 32]> {
    let mut file = File::open(path).map_err(|_| Error::IoFailure)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|_| Error::IoFailure)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    Ok(digest.finalize().into())
}
