//! Real SIGKILL/TerminateProcess tests. Only this parent owns/removes fixture roots.
use acp_rehearsal::recovery::{Capability, Report, Stage, TestRoot};
use acp_store::{Schema, SqliteStore};
use rusqlite::Connection;
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::Duration,
};
struct Worker {
    child: Child,
    lines: Receiver<String>,
}
impl Worker {
    fn start(cap: &Capability, stop: Option<&str>, restore: bool, partial: bool) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_acp-recovery-test-worker"))
            .arg("--synthetic-only")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let output = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                if tx
                    .send(line.unwrap_or_else(|_| "ReadFailure".into()))
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut worker = Self { child, lines };
        let mut stdin = worker.child.stdin.take().unwrap();
        stdin
            .write_all(&cap.request(stop, restore, partial))
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        drop(stdin);
        worker
    }
    fn line(&self) -> String {
        self.lines
            .recv_timeout(Duration::from_secs(25))
            .expect("worker timed out; cleanup guard kills it")
    }
    fn kill_at(mut self, point: &str) -> u32 {
        assert_eq!(self.line(), format!("READY:{point}"));
        let pid = self.child.id();
        self.child.kill().unwrap();
        let status = self.child.wait().unwrap();
        assert!(!status.success());
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(status.signal(), Some(9));
        }
        pid
    }
    fn done(mut self) -> (u32, String) {
        let line = self.line();
        let id = self.child.id();
        assert!(self.child.wait().unwrap().success());
        (id, line)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn recover(cap: &Capability, restore: bool) -> (u32, Report) {
    let (id, line) = Worker::start(cap, None, restore, false).done();
    assert!(line.starts_with("REPORT:"), "worker refused: {line}");
    (
        id,
        serde_json::from_str(line.strip_prefix("REPORT:").unwrap()).unwrap(),
    )
}
fn refused(root: &TestRoot, cap: &Capability, expected: &str) {
    let paths = [
        root.source(),
        root.path().join("backup.sqlite3"),
        root.path().join("output.sqlite3"),
        root.path().join("restored.sqlite3"),
    ];
    let before: Vec<_> = paths.iter().map(|p| fs::read(p).ok()).collect();
    let (_, line) = Worker::start(cap, None, false, false).done();
    assert_eq!(line, format!("ERROR:{expected}"));
    let after: Vec<_> = paths.iter().map(|p| fs::read(p).ok()).collect();
    assert!(before == after, "Safe refusal changed database files");
}
fn assert_complete(root: &TestRoot, report: &Report) {
    assert_eq!(report.stage, Stage::Completed);
    assert_eq!(
        (
            report.source_version,
            report.backup_version,
            report.output_version
        ),
        (0, 0, 1)
    );
    let backup = SqliteStore::open(&root.path().join("backup.sqlite3")).unwrap();
    let output = SqliteStore::open(&root.path().join("output.sqlite3")).unwrap();
    assert_ne!(backup.schema().unwrap(), Schema::Version1);
    assert_eq!(output.schema().unwrap(), Schema::Version1);
    for p in ["alpha", "beta"] {
        assert_eq!(
            serde_json::to_value(backup.read_preferences(p).unwrap()).unwrap(),
            serde_json::to_value(output.read_preferences(p).unwrap()).unwrap()
        );
        assert_eq!(
            serde_json::to_value(backup.read_handoff(p, "shared").unwrap()).unwrap(),
            serde_json::to_value(output.read_handoff(p, "shared").unwrap()).unwrap()
        );
    }
}
fn latest(root: &Path) -> std::path::PathBuf {
    let mut files: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("manifest.")
                && p.extension().is_some_and(|e| e == "json")
        })
        .collect();
    files.sort();
    files.pop().unwrap()
}
#[test]
fn actual_kill_and_new_process_reconcile_all_commit_boundaries() {
    for legacy in [true, false] {
        for wal in [true, false] {
            for point in [
                "before_backup",
                "backup_verified_before_manifest",
                "backup_verified",
                "before_transaction",
                "after_ddl",
                "before_commit",
                "after_commit",
                "commit_recorded",
                "complete",
            ] {
                let root = TestRoot::new(legacy);
                let cap = root.authorize(wal);
                let before = fs::read(root.source()).unwrap();
                let killed = Worker::start(&cap, Some(point), false, false).kill_at(point);
                let (fresh, report) = recover(&cap, false);
                assert_ne!(killed, fresh);
                assert_complete(&root, &report);
                assert_eq!(fs::read(root.source()).unwrap(), before);
                let backup = fs::read(root.path().join("backup.sqlite3")).unwrap();
                let (again, report) = recover(&cap, false);
                assert_ne!(again, fresh);
                assert_complete(&root, &report);
                assert_eq!(
                    fs::read(root.path().join("backup.sqlite3")).unwrap(),
                    backup
                );
                // Manifest stores metadata and hashes only, never raw evidence/source paths.
                let text = fs::read_to_string(latest(root.path())).unwrap();
                for forbidden in [
                    "PENDING_NOT_FOR_AGENT",
                    "BETA_PRIVATE_PREFERENCE",
                    "fixture-new",
                    "log-ref",
                ] {
                    assert!(!text.contains(forbidden));
                }
                assert!(!text.contains(root.path().to_str().unwrap()));
                println!("PASS kill/restart: legacy={legacy} output_wal={wal} point={point}");
            }
        }
    }
}
#[test]
fn incomplete_copies_and_restore_crashes_are_reconciled_or_refused() {
    for point in [
        "backup_copy",
        "output_copy",
        "restore_copy",
        "restore_verified",
    ] {
        let root = TestRoot::new(true);
        let cap = root.authorize(false);
        let source = fs::read(root.source()).unwrap();
        let killed =
            Worker::start(&cap, Some(point), point.starts_with("restore"), false).kill_at(point);
        if point == "restore_verified" {
            let (fresh, report) = recover(&cap, true);
            assert_ne!(fresh, killed);
            assert_eq!(report.stage, Stage::Restored);
            assert_eq!(report.restore_version, Some(0));
            let (_, report) = recover(&cap, true);
            assert_eq!(report.stage, Stage::Restored);
        } else {
            let expected = match point {
                "backup_copy" => "IncompleteBackup",
                "output_copy" => "IncompleteOutput",
                _ => "IncompleteRestore",
            };
            refused(&root, &cap, expected);
        }
        assert_eq!(fs::read(root.source()).unwrap(), source);
        println!("PASS kill/restart: point={point}");
    }
}
#[test]
fn live_wal_writer_and_real_output_lock_survive_worker_death() {
    let root = TestRoot::new(true);
    let mut writer = Connection::open(root.source()).unwrap();
    writer.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; UPDATE preferences SET value='committed' WHERE source='fixture-new';").unwrap();
    let cap = root.authorize(true);
    let before = fs::read(root.source()).unwrap();
    let tx = writer.transaction().unwrap();
    tx.execute(
        "UPDATE preferences SET value='uncommitted' WHERE source='fixture-new'",
        [],
    )
    .unwrap();
    Worker::start(&cap, Some("after_commit"), false, false).kill_at("after_commit");
    let (_, report) = recover(&cap, false);
    assert_complete(&root, &report);
    assert_eq!(fs::read(root.source()).unwrap(), before);
    let backup = SqliteStore::open(&root.path().join("backup.sqlite3")).unwrap();
    assert!(
        backup
            .read_preferences("alpha")
            .unwrap()
            .iter()
            .any(|p| p.value == "committed")
    );
    assert!(
        !backup
            .read_preferences("alpha")
            .unwrap()
            .iter()
            .any(|p| p.value == "uncommitted")
    );
    drop(backup);
    tx.rollback().unwrap();
    drop(writer);
    let root = TestRoot::new(false);
    let cap = root.authorize(false);
    Worker::start(&cap, Some("before_transaction"), false, false).kill_at("before_transaction");
    // A separate process owns the SQLite lock: reading file bytes in the
    // parent must not inadvertently release same-process POSIX fcntl locks.
    let writer = Worker::start(&cap, Some("writer_locked"), false, false);
    assert_eq!(writer.line(), "READY:writer_locked");
    refused(&root, &cap, "Locked");
    drop(writer);
    let (_, report) = recover(&cap, false);
    assert_complete(&root, &report);
}
#[test]
fn backup_missing_changed_corrupt_and_future_outputs_are_refused() {
    for change in ["missing", "changed", "corrupt", "sidecar", "collision"] {
        let root = TestRoot::new(true);
        let cap = root.authorize(false);
        let source = fs::read(root.source()).unwrap();
        Worker::start(&cap, Some("backup_verified"), false, false).kill_at("backup_verified");
        let backup = root.path().join("backup.sqlite3");
        match change {
            "missing" => fs::remove_file(&backup).unwrap(),
            "changed" => {
                let db = Connection::open(&backup).unwrap();
                db.execute("UPDATE preferences SET value='changed'", [])
                    .unwrap();
            }
            "corrupt" => fs::write(&backup, b"corrupt").unwrap(),
            "sidecar" => {
                let path = root.path().join("backup.sqlite3-wal");
                fs::write(&path, b"unexpected").unwrap();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
                }
            }
            _ => {
                fs::write(root.path().join("output.sqlite3"), b"collision").unwrap();
            }
        }
        refused(
            &root,
            &cap,
            match change {
                "missing" => "MissingBackup",
                "changed" | "corrupt" | "sidecar" => "BackupChanged",
                _ => "Collision",
            },
        );
        assert_eq!(fs::read(root.source()).unwrap(), source);
    }
    for ddl in [
        "PRAGMA user_version=2",
        "ALTER TABLE projects ADD COLUMN future TEXT",
        "PRAGMA user_version=1",
    ] {
        let root = TestRoot::new(true);
        let cap = root.authorize(false);
        Worker::start(&cap, Some("before_transaction"), false, false).kill_at("before_transaction");
        let db = Connection::open(root.path().join("output.sqlite3")).unwrap();
        db.execute_batch(ddl).unwrap();
        drop(db);
        refused(&root, &cap, "Database");
    }
}
#[test]
fn missing_truncated_tampered_replayed_or_foreign_manifests_fail_before_writes() {
    for change in ["missing", "truncate", "tamper", "foreign", "gap"] {
        let root = TestRoot::new(false);
        let cap = root.authorize(false);
        let before = fs::read(root.source()).unwrap();
        let path = latest(root.path());
        match change {
            "missing" => fs::remove_file(path).unwrap(),
            "truncate" => fs::write(path, b"{\"body\":").unwrap(),
            "tamper" => {
                let mut value: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                value["body"]["session"] = serde_json::json!("wrong-session");
                fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
            }
            "foreign" => {
                let other = TestRoot::new(false);
                let _other_cap = other.authorize(false);
                fs::write(path, fs::read(latest(other.path())).unwrap()).unwrap();
            }
            _ => fs::rename(path, root.path().join("manifest.00000001.json")).unwrap(),
        }
        refused(
            &root,
            &cap,
            match change {
                "missing" | "truncate" | "gap" => "Manifest",
                _ => "Identity",
            },
        );
        assert!(!root.path().join("backup.sqlite3").exists());
        assert_eq!(fs::read(root.source()).unwrap(), before);
    }
}
#[test]
fn interrupted_failed_manifest_publication_never_falls_back_silently() {
    let root = TestRoot::new(false);
    let cap = root.authorize(false);
    Worker::start(&cap, Some("manifest_flushed"), false, false).kill_at("manifest_flushed");
    refused(&root, &cap, "PendingManifest");
    assert!(!root.path().join("backup.sqlite3").exists());
    let root = TestRoot::new(false);
    let cap = root.authorize(false);
    let (_, line) = Worker::start(&cap, None, false, true).done();
    assert_eq!(line, "ERROR:ManifestWrite");
    refused(&root, &cap, "PendingManifest");
    assert!(!root.path().join("backup.sqlite3").exists());
}
#[test]
fn session_lease_and_unexpected_files_refuse_without_clobbering() {
    let root = TestRoot::new(false);
    let cap = root.authorize(false);
    let worker = Worker::start(&cap, Some("before_backup"), false, false);
    assert_eq!(worker.line(), "READY:before_backup");
    refused(&root, &cap, "Locked");
    drop(worker);
    let (_, report) = recover(&cap, false);
    assert_complete(&root, &report);
    let unexpected = root.path().join("unexpected");
    fs::write(&unexpected, b"preserve").unwrap();
    refused(&root, &cap, "Unsafe");
    assert_eq!(fs::read(unexpected).unwrap(), b"preserve");
}
#[cfg(unix)]
#[test]
fn symlink_hardlink_and_permission_policy_are_enforced() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    for kind in ["symlink", "hardlink", "permissions"] {
        let root = TestRoot::new(false);
        let cap = root.authorize(false);
        let path = latest(root.path());
        match kind {
            "symlink" => {
                fs::remove_file(&path).unwrap();
                symlink(root.source(), &path).unwrap();
            }
            "hardlink" => {
                fs::hard_link(&path, root.path().join("manifest.00000001.json")).unwrap();
            }
            _ => fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap(),
        }
        refused(
            &root,
            &cap,
            if kind == "permissions" {
                "Permissions"
            } else {
                "Unsafe"
            },
        );
        if kind == "permissions" {
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }
    }
}
#[cfg(windows)]
#[test]
fn windows_reparse_and_readonly_permission_policy_are_enforced() {
    for kind in ["symlink", "permissions"] {
        let root = TestRoot::new(false);
        let cap = root.authorize(false);
        let path = latest(root.path());
        if kind == "symlink" {
            fs::remove_file(&path).unwrap();
            std::os::windows::fs::symlink_file(root.source(), &path).unwrap();
        } else {
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_readonly(true);
            fs::set_permissions(&path, perms).unwrap();
        }
        refused(
            &root,
            &cap,
            if kind == "permissions" {
                "Permissions"
            } else {
                "Unsafe"
            },
        );
        if kind == "permissions" {
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_readonly(false);
            fs::set_permissions(path, perms).unwrap();
        }
    }
}

#[test]
fn replaying_only_an_old_valid_prefix_cannot_reset_existing_output() {
    let root = TestRoot::new(false);
    let cap = root.authorize(false);
    let (_, report) = recover(&cap, false);
    assert_complete(&root, &report);
    for entry in fs::read_dir(root.path()).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap();
        if name.starts_with("manifest.")
            && name.ends_with(".json")
            && name != "manifest.00000000.json"
        {
            fs::remove_file(path).unwrap();
        }
    }
    refused(&root, &cap, "Collision");
}

#[test]
fn python_full_snapshot_matches_after_kill_new_process_and_restore() {
    for legacy in [true, false] {
        let root = TestRoot::new(legacy);
        let cap = root.authorize(true);
        Worker::start(&cap, Some("after_commit"), false, false).kill_at("after_commit");
        let (_, report) = recover(&cap, true);
        assert_eq!(report.stage, Stage::Restored);
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output =
            Command::new(std::env::var("ACP_TEST_PYTHON").unwrap_or_else(|_| "python".into()))
                .arg(repo.join("tools/check_recovered_snapshot.py"))
                .arg("--synthetic-only")
                .arg(root.path())
                .env("PYTHONPATH", repo.join("src"))
                .output()
                .unwrap();
        assert!(
            output.status.success(),
            "Python full-record recovery parity failed"
        );
    }
}

#[test]
fn unrecorded_v0_schema_mutation_and_unverified_backup_shape_are_refused() {
    for point in ["before_transaction", "backup_verified_before_manifest"] {
        let root = TestRoot::new(true);
        let cap = root.authorize(false);
        Worker::start(&cap, Some(point), false, false).kill_at(point);
        let name = if point == "before_transaction" {
            "output.sqlite3"
        } else {
            "backup.sqlite3"
        };
        let db = Connection::open(root.path().join(name)).unwrap();
        db.execute_batch("ALTER TABLE handoffs ADD COLUMN worktree_fingerprint TEXT")
            .unwrap();
        drop(db);
        refused(
            &root,
            &cap,
            if point == "before_transaction" {
                "Ambiguous"
            } else {
                "BackupChanged"
            },
        );
    }
}
