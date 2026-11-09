//! Feature-gated subprocess test protocol. Capability travels only over stdin.
//! A trusted parent owns the temporary root across worker death; never adopt profiles.
use crate::{integrity, open, records, verify_schema};
use acp_store::Schema;
use hmac::{Hmac, Mac};
use rusqlite::{
    Connection, TransactionBehavior,
    backup::{Backup, StepResult},
    types::Value,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, Read, Write},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use tempfile::TempDir;

type Result<T> = std::result::Result<T, Failure>;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Failure {
    Unsafe,
    Permissions,
    Identity,
    Manifest,
    PendingManifest,
    ManifestWrite,
    Durability,
    Locked,
    Collision,
    SourceChanged,
    MissingBackup,
    BackupChanged,
    IncompleteBackup,
    IncompleteOutput,
    IncompleteRestore,
    Database,
    Ambiguous,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    Created,
    BackingUp,
    BackupVerified,
    PreparingOutput,
    OutputReady,
    Migrating,
    Committed,
    Completed,
    Restoring,
    Restored,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: u32,
    session: String,
    roles: [String; 4],
    source_version: u32,
    source_shape: u8,
    desired_version: u32,
    records_hash: String,
    source_file_hash: String,
    backup_file_hash: Option<String>,
    restored_file_hash: Option<String>,
    sequence: u32,
    previous_hash: String,
    previous_stage: Stage,
    verified_stage: Stage,
    stage: Stage,
    desired: Stage,
    last_error: Option<String>,
    directory_synced: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    body: Manifest,
    mac: Vec<u8>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Permit {
    root: PathBuf,
    session: String,
    key: [u8; 32],
    records_hash: String,
    source_file_hash: String,
    output_wal: bool,
    source_shape: u8,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    permit: Permit,
    stop: Option<String>,
    restore: bool,
    partial_manifest: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub stage: Stage,
    pub source_version: u32,
    pub backup_version: u32,
    pub output_version: u32,
    pub restore_version: Option<u32>,
}
const ROLES: [&str; 4] = [
    "fixture/state/context.sqlite3",
    "backup.sqlite3",
    "output.sqlite3",
    "restored.sqlite3",
];

/// Only constructor creates its own root and invokes the synthetic builder. The
/// key is never persisted. Temp ancestry and key possession are not an OS sandbox.
pub struct TestRoot {
    dir: TempDir,
}
#[derive(Clone)]
pub struct Capability {
    permit: Permit,
}
impl TestRoot {
    pub fn new(legacy: bool) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("acp-synthetic-recovery-")
            .tempdir()
            .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        }
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut cmd =
            Command::new(std::env::var("ACP_TEST_PYTHON").unwrap_or_else(|_| "python".into()));
        cmd.arg(repo.join("tools/sqlite_fixture.py"))
            .arg(dir.path().join("fixture"))
            .env("PYTHONPATH", repo.join("src"));
        if legacy {
            cmd.arg("--legacy");
        }
        assert!(
            cmd.output().unwrap().status.success(),
            "SyntheticBuilderFailed"
        );
        Self { dir }
    }
    pub fn path(&self) -> &Path {
        self.dir.path()
    }
    pub fn source(&self) -> PathBuf {
        self.path().join(ROLES[0])
    }
    /// Seal after any parent-controlled WAL setup, before launching workers.
    pub fn authorize(&self, output_wal: bool) -> Capability {
        let mut key = [0; 32];
        getrandom::fill(&mut key).unwrap();
        let mut id = [0; 16];
        getrandom::fill(&mut id).unwrap();
        let root = self.path().canonicalize().unwrap();
        let db = open(&self.source(), false).unwrap();
        let shape = verify_schema(&db).unwrap();
        assert_ne!(shape, Schema::Version1);
        let records_hash = logical_hash(&db).unwrap();
        let source_file_hash = file_hash(&self.source()).unwrap();
        let permit = Permit {
            root,
            session: hex(&id),
            key,
            records_hash,
            source_file_hash,
            output_wal,
            source_shape: if shape == Schema::Legacy { 0 } else { 1 },
        };
        let lock = private_new(&permit.root.join("session.lock")).unwrap();
        lock.sync_all().unwrap();
        let mut session = Session {
            permit: permit.clone(),
            stop: None,
            partial_manifest: false,
            current: None,
            current_hash: String::new(),
        };
        session.publish(Stage::Created, false).unwrap();
        Capability { permit }
    }
}
impl Capability {
    pub fn request(&self, stop: Option<&str>, restore: bool, partial_manifest: bool) -> Vec<u8> {
        serde_json::to_vec(&Request {
            permit: self.permit.clone(),
            stop: stop.map(str::to_owned),
            restore,
            partial_manifest,
        })
        .unwrap()
    }
    pub fn source_digest(&self) -> &str {
        &self.permit.records_hash
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn file_hash(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(|_| Failure::Unsafe)?;
    let mut hash = Sha256::new();
    let mut buf = [0; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|_| Failure::Unsafe)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(hex(&hash.finalize()))
}
fn logical_hash(db: &Connection) -> Result<String> {
    let data = records(db).map_err(|_| Failure::Database)?;
    let mut hash = Sha256::new();
    for table in data {
        hash.update((table.len() as u64).to_le_bytes());
        for row in table {
            hash.update((row.len() as u64).to_le_bytes());
            for cell in row {
                match cell {
                    Value::Null => hash.update([0]),
                    Value::Integer(i) => {
                        hash.update([1]);
                        hash.update(i.to_le_bytes());
                    }
                    Value::Real(f) => {
                        hash.update([2]);
                        hash.update(f.to_bits().to_le_bytes());
                    }
                    Value::Text(t) => {
                        hash.update([3]);
                        hash.update((t.len() as u64).to_le_bytes());
                        hash.update(t.as_bytes());
                    }
                    Value::Blob(b) => {
                        hash.update([4]);
                        hash.update((b.len() as u64).to_le_bytes());
                        hash.update(b);
                    }
                }
            }
        }
    }
    Ok(hex(&hash.finalize()))
}
fn private_new(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).read(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|_| Failure::Collision)
}
fn safe_meta(path: &Path, private: bool) -> Result<()> {
    let meta = fs::symlink_metadata(path).map_err(|_| Failure::Unsafe)?;
    if meta.file_type().is_symlink() || (!meta.is_file() && !meta.is_dir()) {
        return Err(Failure::Unsafe);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(Failure::Unsafe);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if meta.is_file() && meta.nlink() != 1 {
            return Err(Failure::Unsafe);
        }
        if private && meta.permissions().mode() & 0o777 != if meta.is_dir() { 0o700 } else { 0o600 }
        {
            return Err(Failure::Permissions);
        }
    }
    if private && meta.permissions().readonly() {
        return Err(Failure::Permissions);
    }
    Ok(())
}
fn manifests(root: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    fn visit(root: &Path, relative: &Path, names: &mut Vec<String>) -> Result<()> {
        for item in fs::read_dir(root.join(relative)).map_err(|_| Failure::Unsafe)? {
            let item = item.map_err(|_| Failure::Unsafe)?;
            let rel = relative.join(item.file_name());
            let name = rel.to_str().ok_or(Failure::Unsafe)?.replace('\\', "/");
            let role = ROLES.iter().any(|r| {
                name == *r
                    || ["-wal", "-shm", "-journal"]
                        .iter()
                        .any(|s| name == format!("{r}{s}"))
            });
            let manifest =
                name.starts_with("manifest.") && name.ends_with(".json") && name.len() == 22;
            let fixture = matches!(
                name.as_str(),
                "fixture"
                    | "fixture/state"
                    | "fixture/alpha"
                    | "fixture/beta"
                    | "fixture/alpha/AGENTS.md"
                    | "fixture/beta/AGENTS.md"
            );
            if name == "manifest.pending" {
                return Err(Failure::PendingManifest);
            }
            if !role && !manifest && !fixture && name != "session.lock" {
                return Err(Failure::Unsafe);
            }
            safe_meta(&item.path(), role || manifest || name == "session.lock")?;
            if item.path().is_dir() {
                visit(root, &rel, names)?;
            }
            if manifest {
                names.push(name);
            }
        }
        Ok(())
    }
    visit(root, Path::new(""), &mut names)?;
    names.sort();
    if names.is_empty() || names.len() > 64 {
        return Err(Failure::Manifest);
    }
    Ok(names)
}
struct Session {
    permit: Permit,
    stop: Option<String>,
    partial_manifest: bool,
    current: Option<Manifest>,
    current_hash: String,
}
impl Session {
    fn original_shape(&self) -> Schema {
        if self.permit.source_shape == 0 {
            Schema::Legacy
        } else {
            Schema::Fingerprinted
        }
    }
    fn point(&self, name: &str) {
        if self.stop.as_deref() == Some(name) {
            println!("READY:{name}");
            std::io::stdout().flush().unwrap();
            loop {
                std::thread::park();
            }
        }
    }
    fn signature(&self, body: &Manifest) -> Result<Vec<u8>> {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&self.permit.key).map_err(|_| Failure::Manifest)?;
        mac.update(self.permit.root.to_string_lossy().as_bytes());
        mac.update(&serde_json::to_vec(body).map_err(|_| Failure::Manifest)?);
        Ok(mac.finalize().into_bytes().to_vec())
    }
    fn load(&mut self) -> Result<()> {
        let root = &self.permit.root;
        if !root.is_absolute()
            || root.parent()
                != Some(
                    std::env::temp_dir()
                        .canonicalize()
                        .map_err(|_| Failure::Unsafe)?
                        .as_path(),
                )
            || !root
                .file_name()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.starts_with("acp-synthetic-recovery-"))
            || root.canonicalize().map_err(|_| Failure::Unsafe)? != *root
        {
            return Err(Failure::Unsafe);
        }
        safe_meta(root, true)?;
        let names = manifests(root)?;
        let mut previous = String::new();
        let mut previous_stage = Stage::Created;
        for (i, name) in names.iter().enumerate() {
            if name != &format!("manifest.{i:08}.json") {
                return Err(Failure::Manifest);
            }
            let raw = fs::read(root.join(name)).map_err(|_| Failure::Manifest)?;
            if raw.len() > 16384 {
                return Err(Failure::Manifest);
            }
            let envelope: Envelope = serde_json::from_slice(&raw).map_err(|_| Failure::Manifest)?;
            let m = &envelope.body;
            let mut mac =
                Hmac::<Sha256>::new_from_slice(&self.permit.key).map_err(|_| Failure::Manifest)?;
            mac.update(self.permit.root.to_string_lossy().as_bytes());
            mac.update(&serde_json::to_vec(m).map_err(|_| Failure::Manifest)?);
            mac.verify_slice(&envelope.mac)
                .map_err(|_| Failure::Identity)?;
            if m.format != 1
                || m.session != self.permit.session
                || m.roles != ROLES.map(str::to_owned)
                || m.records_hash != self.permit.records_hash
                || m.source_file_hash != self.permit.source_file_hash
                || m.source_shape != self.permit.source_shape
                || m.source_shape > 1
                || m.source_version != 0
                || m.desired_version != 1
                || m.sequence != i as u32
                || m.previous_hash != previous
                || m.previous_stage != previous_stage
                || m.directory_synced != cfg!(unix)
            {
                return Err(Failure::Identity);
            }
            previous = hex(&Sha256::digest(&raw));
            previous_stage = m.stage;
            self.current = Some(envelope.body);
        }
        self.current_hash = previous;
        Ok(())
    }
    fn publish(&mut self, stage: Stage, reconciled: bool) -> Result<()> {
        let mut next = self.current.clone().unwrap_or(Manifest {
            format: 1,
            session: self.permit.session.clone(),
            roles: ROLES.map(str::to_owned),
            source_version: 0,
            source_shape: self.permit.source_shape,
            desired_version: 1,
            records_hash: self.permit.records_hash.clone(),
            source_file_hash: self.permit.source_file_hash.clone(),
            backup_file_hash: None,
            restored_file_hash: None,
            sequence: 0,
            previous_hash: String::new(),
            previous_stage: Stage::Created,
            verified_stage: Stage::Created,
            stage: Stage::Created,
            desired: Stage::Completed,
            last_error: None,
            directory_synced: cfg!(unix),
        });
        if let Some(current) = &self.current {
            next.sequence = current.sequence + 1;
            next.previous_stage = current.stage;
            next.previous_hash = self.current_hash.clone();
        }
        if next.sequence >= 64 {
            return Err(Failure::Manifest);
        }
        next.stage = stage;
        if matches!(
            stage,
            Stage::BackupVerified | Stage::OutputReady | Stage::Completed | Stage::Restored
        ) {
            next.verified_stage = stage;
        }
        next.desired = if matches!(stage, Stage::Restoring | Stage::Restored) {
            Stage::Restored
        } else {
            Stage::Completed
        };
        next.last_error = if reconciled {
            Some("ReconciledFromDisk".into())
        } else {
            None
        };
        if matches!(
            stage,
            Stage::BackupVerified
                | Stage::PreparingOutput
                | Stage::OutputReady
                | Stage::Migrating
                | Stage::Committed
                | Stage::Completed
                | Stage::Restoring
                | Stage::Restored
        ) {
            next.backup_file_hash = Some(file_hash(&self.permit.root.join(ROLES[1]))?);
        }
        if stage == Stage::Restored {
            next.restored_file_hash = Some(file_hash(&self.permit.root.join(ROLES[3]))?);
        }
        let raw = serde_json::to_vec(&Envelope {
            mac: self.signature(&next)?,
            body: next.clone(),
        })
        .map_err(|_| Failure::ManifestWrite)?;
        let pending = self.permit.root.join("manifest.pending");
        let mut file = private_new(&pending).map_err(|_| Failure::ManifestWrite)?;
        if self.partial_manifest {
            file.write_all(&raw[..raw.len() / 2])
                .map_err(|_| Failure::ManifestWrite)?;
            file.sync_all().map_err(|_| Failure::ManifestWrite)?;
            return Err(Failure::ManifestWrite);
        }
        file.write_all(&raw).map_err(|_| Failure::ManifestWrite)?;
        file.sync_all().map_err(|_| Failure::Durability)?;
        drop(file);
        self.point("manifest_flushed");
        let path = self
            .permit
            .root
            .join(format!("manifest.{:08}.json", next.sequence));
        if path.exists() {
            return Err(Failure::Collision);
        }
        fs::rename(&pending, path).map_err(|_| Failure::ManifestWrite)?;
        #[cfg(unix)]
        {
            File::open(&self.permit.root)
                .and_then(|f| f.sync_all())
                .map_err(|_| Failure::Durability)?;
        }
        self.current = Some(next);
        self.current_hash = hex(&Sha256::digest(raw));
        Ok(())
    }
    fn inspect(&self, path: &Path, writable: bool) -> Result<Schema> {
        safe_meta(path, true)?;
        if [ROLES[1], ROLES[3]]
            .iter()
            .any(|role| path == self.permit.root.join(role))
        {
            for suffix in ["-wal", "-shm", "-journal"] {
                let mut sidecar = path.as_os_str().to_os_string();
                sidecar.push(suffix);
                if fs::symlink_metadata(PathBuf::from(sidecar)).is_ok() {
                    return Err(Failure::Database);
                }
            }
        }
        let db = open(path, writable).map_err(|_| Failure::Database)?;
        integrity(&db).map_err(|_| Failure::Database)?;
        let schema = verify_schema(&db).map_err(|_| Failure::Database)?;
        if logical_hash(&db)? != self.permit.records_hash {
            return Err(Failure::Database);
        }
        Ok(schema)
    }
    fn copy(&self, source: &Path, dest: &Path, point: &str) -> Result<()> {
        private_new(dest)?;
        let source = open(source, false).map_err(|_| Failure::Database)?;
        source.execute_batch("BEGIN").map_err(|_| Failure::Locked)?;
        if logical_hash(&source)? != self.permit.records_hash {
            return Err(Failure::SourceChanged);
        }
        let mut target = open(dest, true).map_err(|_| Failure::Database)?;
        target
            .pragma_update(None, "synchronous", "FULL")
            .map_err(|_| Failure::Database)?;
        {
            let backup = Backup::new(&source, &mut target).map_err(|_| Failure::Database)?;
            let mut done = false;
            for _ in 0..16384 {
                let step = backup.step(1).map_err(|_| Failure::Database)?;
                self.point(point);
                match step {
                    StepResult::Done => {
                        done = true;
                        break;
                    }
                    StepResult::More => (),
                    StepResult::Busy | StepResult::Locked => return Err(Failure::Locked),
                    _ => return Err(Failure::Database),
                }
            }
            if !done {
                return Err(Failure::Database);
            }
        }
        // Freeze new backup/restore copies as independent single-file artifacts.
        target
            .pragma_update(None, "journal_mode", "DELETE")
            .map_err(|_| Failure::Database)?;
        drop(target);
        OpenOptions::new()
            .write(true)
            .open(dest)
            .and_then(|f| f.sync_all())
            .map_err(|_| Failure::Durability)?;
        if self.inspect(dest, false)? != self.original_shape() {
            return Err(Failure::Database);
        }
        Ok(())
    }
    fn run(&mut self, restore: bool) -> Result<Report> {
        self.load()?;
        let source = self.permit.root.join(ROLES[0]);
        let backup = self.permit.root.join(ROLES[1]);
        let output = self.permit.root.join(ROLES[2]);
        let restored = self.permit.root.join(ROLES[3]);
        if file_hash(&source)? != self.permit.source_file_hash
            || self.inspect(&source, false)? != self.original_shape()
        {
            return Err(Failure::SourceChanged);
        }
        let mut stage = self.current.as_ref().unwrap().stage;
        if stage == Stage::Created {
            if backup.exists() || output.exists() || restored.exists() {
                return Err(Failure::Collision);
            }
            self.publish(Stage::BackingUp, true)?;
            stage = Stage::BackingUp;
        }
        if stage == Stage::BackingUp {
            self.point("before_backup");
            if !backup.exists() {
                self.copy(&source, &backup, "backup_copy")?;
            }
            if self
                .inspect(&backup, false)
                .map_err(|_| Failure::IncompleteBackup)?
                != self.original_shape()
            {
                return Err(Failure::BackupChanged);
            }
            self.point("backup_verified_before_manifest");
            self.publish(Stage::BackupVerified, true)?;
            stage = Stage::BackupVerified;
        }
        if !backup.is_file() {
            return Err(Failure::MissingBackup);
        }
        if self
            .inspect(&backup, false)
            .map_err(|_| Failure::BackupChanged)?
            != self.original_shape()
            || self.current.as_ref().unwrap().backup_file_hash.as_deref()
                != Some(file_hash(&backup)?.as_str())
        {
            return Err(Failure::BackupChanged);
        }
        if stage == Stage::BackupVerified {
            if output.exists() || restored.exists() {
                return Err(Failure::Collision);
            }
            self.point("backup_verified");
            self.publish(Stage::PreparingOutput, true)?;
            stage = Stage::PreparingOutput;
        }
        if stage == Stage::PreparingOutput {
            if !output.exists() {
                self.copy(&backup, &output, "output_copy")?;
            }
            if self
                .inspect(&output, true)
                .map_err(|_| Failure::IncompleteOutput)?
                != self.original_shape()
            {
                return Err(Failure::Ambiguous);
            }
            if self.permit.output_wal {
                let db = open(&output, true).map_err(|_| Failure::Database)?;
                db.pragma_update(None, "journal_mode", "WAL")
                    .map_err(|_| Failure::Database)?;
            }
            self.publish(Stage::OutputReady, true)?;
            stage = Stage::OutputReady;
        }
        let observed = self.inspect(&output, true)?;
        if observed != Schema::Version1 {
            if observed != self.original_shape() {
                return Err(Failure::Ambiguous);
            }
            if !matches!(stage, Stage::OutputReady | Stage::Migrating) {
                return Err(Failure::Ambiguous);
            }
            self.point("before_transaction");
            if stage != Stage::Migrating {
                self.publish(Stage::Migrating, true)?;
            }
            let mut db = open(&output, true).map_err(|_| Failure::Database)?;
            db.pragma_update(None, "synchronous", "FULL")
                .map_err(|_| Failure::Database)?;
            let tx = db
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| Failure::Locked)?;
            let shape = verify_schema(&tx).map_err(|_| Failure::Database)?;
            if logical_hash(&tx)? != self.permit.records_hash || shape != self.original_shape() {
                return Err(Failure::Ambiguous);
            }
            if shape == Schema::Legacy {
                tx.execute_batch("ALTER TABLE handoffs ADD COLUMN worktree_fingerprint TEXT")
                    .map_err(|_| Failure::Database)?;
            }
            self.point("after_ddl");
            tx.pragma_update(None, "user_version", 1)
                .map_err(|_| Failure::Database)?;
            self.point("before_commit");
            tx.commit().map_err(|_| Failure::Locked)?;
            self.point("after_commit");
            self.publish(Stage::Committed, true)?;
            self.point("commit_recorded");
            stage = Stage::Committed;
        } else if !matches!(
            stage,
            Stage::Migrating
                | Stage::Committed
                | Stage::Completed
                | Stage::Restoring
                | Stage::Restored
        ) {
            return Err(Failure::Ambiguous);
        }
        if matches!(stage, Stage::Migrating | Stage::Committed) {
            if self.inspect(&output, true)? != Schema::Version1 {
                return Err(Failure::Ambiguous);
            }
            self.publish(Stage::Completed, true)?;
            stage = Stage::Completed;
            self.point("complete");
        }
        if restore || matches!(stage, Stage::Restoring | Stage::Restored) {
            if stage == Stage::Completed {
                if restored.exists() {
                    return Err(Failure::Collision);
                }
                self.publish(Stage::Restoring, true)?;
                stage = Stage::Restoring;
            }
            if stage == Stage::Restoring {
                if !restored.exists() {
                    self.copy(&backup, &restored, "restore_copy")?;
                }
                if self
                    .inspect(&restored, false)
                    .map_err(|_| Failure::IncompleteRestore)?
                    != self.original_shape()
                {
                    return Err(Failure::IncompleteRestore);
                }
                self.point("restore_verified");
                self.publish(Stage::Restored, true)?;
                stage = Stage::Restored;
            }
            if self.current.as_ref().unwrap().restored_file_hash.as_deref()
                != Some(file_hash(&restored)?.as_str())
                || self.inspect(&restored, false)? != self.original_shape()
            {
                return Err(Failure::IncompleteRestore);
            }
        } else if restored.exists() {
            return Err(Failure::Collision);
        }
        Ok(Report {
            stage,
            source_version: 0,
            backup_version: 0,
            output_version: 1,
            restore_version: if stage == Stage::Restored {
                Some(0)
            } else {
                None
            },
        })
    }
}
/// Fresh worker entry. No path/capability arguments or environment discovery.
pub fn worker_main() {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(45));
        std::process::exit(124);
    });
    let mut line = String::new();
    if std::io::stdin()
        .lock()
        .take(16385)
        .read_line(&mut line)
        .is_err()
        || line.len() > 16384
    {
        println!("ERROR:Manifest");
        return;
    }
    let Ok(request) = serde_json::from_str::<Request>(&line) else {
        println!("ERROR:Manifest");
        return;
    };
    // Validate all inventory before acquiring the known session lease or opening DBs.
    let mut session = Session {
        permit: request.permit,
        stop: request.stop,
        partial_manifest: request.partial_manifest,
        current: None,
        current_hash: String::new(),
    };
    let result = (|| {
        session.load()?;
        if session.stop.as_deref() == Some("writer_locked") {
            let output = session.permit.root.join(ROLES[2]);
            session.inspect(&output, true)?;
            let mut writer = open(&output, true).map_err(|_| Failure::Database)?;
            let _tx = writer
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| Failure::Locked)?;
            session.point("writer_locked");
            return Err(Failure::Locked);
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(session.permit.root.join("session.lock"))
            .map_err(|_| Failure::Locked)?;
        lock.try_lock().map_err(|_| Failure::Locked)?;
        session.run(request.restore)
    })();
    match result {
        Ok(report) => println!("REPORT:{}", serde_json::to_string(&report).unwrap()),
        Err(error) => println!("ERROR:{error:?}"),
    }
}
