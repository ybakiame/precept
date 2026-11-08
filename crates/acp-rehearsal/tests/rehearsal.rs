use acp_rehearsal::{Error, Fault, Outcome, SyntheticWorkspace};
use acp_store::{Schema, SqliteStore};
use rusqlite::{Connection, OpenFlags};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn fixture(legacy: bool) -> (SyntheticWorkspace, PathBuf) {
    let ws = SyntheticWorkspace::new().unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cmd =
        Command::new(std::env::var("ACP_TEST_PYTHON").unwrap_or_else(|_| "python".into()));
    cmd.arg(repo.join("tools/sqlite_fixture.py"))
        .arg(ws.path().join("fixture"))
        .env("PYTHONPATH", repo.join("src"));
    if legacy {
        cmd.arg("--legacy");
    }
    assert!(
        cmd.output().unwrap().status.success(),
        "synthetic builder failed"
    );
    let path = ws.path().join("fixture/state/context.sqlite3");
    (ws, path)
}
fn version(path: &Path) -> i64 {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    db.pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}
#[test]
fn both_shapes_migrate_restore_and_retry_with_no_record_loss() {
    for legacy in [true, false] {
        let (ws, source) = fixture(legacy);
        let backup = ws.path().join("backup.sqlite3");
        let output = ws.path().join("output.sqlite3");
        let before = fs::read(&source).unwrap();
        assert_eq!(
            ws.rehearse(&source, &backup, &output, Fault::None),
            Ok(Outcome::Migrated)
        );
        assert_eq!(fs::read(&source).unwrap(), before);
        assert_eq!(version(&backup), 0);
        assert_eq!(version(&output), 1);
        let backup_before = fs::read(&backup).unwrap();
        let reader = SqliteStore::open(&output).unwrap();
        assert_eq!(reader.schema().unwrap(), Schema::Version1);
        let original = SqliteStore::open(&backup).unwrap();
        for project in ["alpha", "beta"] {
            assert_eq!(
                serde_json::to_value(reader.read_preferences(project).unwrap()).unwrap(),
                serde_json::to_value(original.read_preferences(project).unwrap()).unwrap()
            );
            assert_eq!(
                serde_json::to_value(reader.read_handoff(project, "shared").unwrap()).unwrap(),
                serde_json::to_value(original.read_handoff(project, "shared").unwrap()).unwrap()
            );
        }
        drop(reader);
        drop(original);
        let output_before = fs::read(&output).unwrap();
        assert_eq!(
            ws.retry(&backup, &output, Fault::None),
            Ok(Outcome::AlreadyV1)
        );
        assert_eq!(fs::read(&output).unwrap(), output_before);
        let restored = ws.path().join("restored.sqlite3");
        ws.restore(&backup, &restored).unwrap();
        assert_eq!(version(&restored), 0);
        assert_eq!(
            SqliteStore::open(&restored).unwrap().schema().unwrap(),
            if legacy {
                Schema::Legacy
            } else {
                Schema::Fingerprinted
            }
        );
        assert_eq!(fs::read(backup).unwrap(), backup_before);
    }
}
#[test]
fn backup_includes_committed_wal_but_not_uncommitted_updates() {
    let (ws, source) = fixture(true);
    let mut writer = Connection::open(&source).unwrap();
    writer.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; UPDATE preferences SET value='committed' WHERE source='fixture-new';").unwrap();
    let tx = writer.transaction().unwrap();
    tx.execute(
        "UPDATE preferences SET value='uncommitted' WHERE source='fixture-new'",
        [],
    )
    .unwrap();
    let backup = ws.path().join("backup.sqlite3");
    ws.backup(&source, &backup, Fault::None).unwrap();
    let reader = SqliteStore::open(&backup).unwrap();
    assert!(
        reader
            .read_preferences("alpha")
            .unwrap()
            .iter()
            .any(|p| p.value == "committed")
    );
    assert!(
        !reader
            .read_preferences("alpha")
            .unwrap()
            .iter()
            .any(|p| p.value == "uncommitted")
    );
    tx.rollback().unwrap();
    drop(reader);
    drop(writer);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(backup).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(ws.path()).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}
#[test]
fn failed_backups_leave_no_published_backup_and_retry_succeeds() {
    for fault in [Fault::BackupInterrupted, Fault::BackupWriteError] {
        let (ws, source) = fixture(false);
        let before = fs::read(&source).unwrap();
        let backup = ws.path().join("backup.sqlite3");
        assert!(ws.backup(&source, &backup, fault).is_err());
        assert!(!backup.exists());
        assert_eq!(fs::read(&source).unwrap(), before);
        ws.backup(&source, &backup, Fault::None).unwrap();
        assert_eq!(version(&backup), 0);
    }
}
#[test]
fn transaction_faults_rollback_both_ddl_and_version_then_retry() {
    for fault in [Fault::AfterDdl, Fault::BeforeCommit] {
        let (ws, source) = fixture(true);
        let backup = ws.path().join("backup.sqlite3");
        let output = ws.path().join("output.sqlite3");
        assert_eq!(
            ws.rehearse(&source, &backup, &output, fault),
            Err(Error::BeforeCommit)
        );
        assert_eq!(version(&output), 0);
        assert_eq!(
            SqliteStore::open(&output).unwrap().schema().unwrap(),
            Schema::Legacy
        );
        let backup_before = fs::read(&backup).unwrap();
        // All migration connections have closed; retry reopens and revalidates.
        assert_eq!(
            ws.retry(&backup, &output, Fault::None),
            Ok(Outcome::Migrated)
        );
        assert_eq!(fs::read(&backup).unwrap(), backup_before);
    }
}
#[test]
fn postcommit_error_has_explicit_v1_state_and_v0_restore() {
    let (ws, source) = fixture(false);
    let backup = ws.path().join("backup.sqlite3");
    let output = ws.path().join("output.sqlite3");
    assert_eq!(
        ws.rehearse(&source, &backup, &output, Fault::AfterCommit),
        Err(Error::AfterCommit)
    );
    assert_eq!(version(&output), 1);
    assert_eq!(version(&backup), 0);
    let restored = ws.path().join("restored.sqlite3");
    ws.restore(&backup, &restored).unwrap();
    assert_eq!(version(&restored), 0);
    assert_eq!(
        ws.retry(&backup, &output, Fault::None),
        Ok(Outcome::AlreadyV1)
    );
}
#[test]
fn writer_contention_and_schema_drift_fail_without_losing_backup() {
    let (ws, source) = fixture(true);
    let backup = ws.path().join("backup.sqlite3");
    let output = ws.path().join("output.sqlite3");
    assert_eq!(
        ws.rehearse(&source, &backup, &output, Fault::BeforeCommit),
        Err(Error::BeforeCommit)
    );
    let before = fs::read(&backup).unwrap();
    let writer = Connection::open(&output).unwrap();
    writer.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert_eq!(ws.retry(&backup, &output, Fault::None), Err(Error::Locked));
    writer.execute_batch("ROLLBACK").unwrap();
    assert_eq!(
        ws.retry(&backup, &output, Fault::None),
        Ok(Outcome::Migrated)
    );
    writer
        .execute_batch("ALTER TABLE projects ADD COLUMN drift TEXT")
        .unwrap();
    assert_eq!(ws.retry(&backup, &output, Fault::None), Err(Error::Schema));
    drop(writer);
    assert_eq!(fs::read(&backup).unwrap(), before);
    ws.restore(&backup, &ws.path().join("recovery.sqlite3"))
        .unwrap();
}
#[test]
fn unsafe_paths_collisions_and_unowned_outputs_are_rejected() {
    let (ws, source) = fixture(false);
    let backup = ws.path().join("backup.sqlite3");
    assert_eq!(
        ws.backup(&source, &source, Fault::None),
        Err(Error::Collision)
    );
    assert_eq!(
        ws.backup(&source, &ws.path().join("../outside.sqlite3"), Fault::None),
        Err(Error::UnsafePath)
    );
    assert_eq!(
        ws.backup(Path::new("context.sqlite3"), &backup, Fault::None),
        Err(Error::UnsafePath)
    );
    assert_eq!(
        ws.retry(&source, &source, Fault::None),
        Err(Error::UnsafePath)
    );
    assert_eq!(ws.restore(&source, &backup), Err(Error::UnsafePath));
    fs::write(ws.path().join("backup.sqlite3-wal"), "collision").unwrap();
    assert_eq!(
        ws.backup(&source, &backup, Fault::None),
        Err(Error::Collision)
    );
    fs::remove_file(ws.path().join("backup.sqlite3-wal")).unwrap();
    ws.backup(&source, &backup, Fault::None).unwrap();
    let bytes = fs::read(&backup).unwrap();
    assert_eq!(
        ws.backup(&source, &backup, Fault::None),
        Err(Error::Collision)
    );
    assert_eq!(fs::read(backup).unwrap(), bytes);
}
#[test]
fn future_corrupt_and_partial_v1_schemas_never_publish_backup() {
    for ddl in [
        "PRAGMA user_version=2",
        "PRAGMA user_version=1",
        "ALTER TABLE projects ADD COLUMN drift TEXT",
        "DROP INDEX prefs_lookup",
    ] {
        let (ws, source) = fixture(true);
        let writer = Connection::open(&source).unwrap();
        writer.execute_batch(ddl).unwrap();
        drop(writer);
        let backup = ws.path().join("backup.sqlite3");
        assert!(ws.backup(&source, &backup, Fault::None).is_err());
        assert!(!backup.exists());
    }
    let ws = SyntheticWorkspace::new().unwrap();
    let source = ws.path().join("corrupt.sqlite3");
    fs::write(&source, b"not sqlite").unwrap();
    assert!(
        ws.backup(&source, &ws.path().join("backup.sqlite3"), Fault::None)
            .is_err()
    );
}
#[cfg(unix)]
#[test]
fn file_parent_sidecar_symlinks_and_hardlinks_are_denied() {
    use std::os::unix::fs::symlink;
    let (ws, source) = fixture(false);
    let backup = ws.path().join("backup.sqlite3");
    let link = ws.path().join("source-link");
    symlink(&source, &link).unwrap();
    assert_eq!(
        ws.backup(&link, &backup, Fault::None),
        Err(Error::UnsafePath)
    );
    let parent = ws.path().join("parent-link");
    symlink(source.parent().unwrap(), &parent).unwrap();
    assert_eq!(
        ws.backup(&parent.join("context.sqlite3"), &backup, Fault::None),
        Err(Error::UnsafePath)
    );
    let hard = ws.path().join("hard");
    fs::hard_link(&source, &hard).unwrap();
    assert_eq!(
        ws.backup(&hard, &backup, Fault::None),
        Err(Error::UnsafePath)
    );
    fs::remove_file(hard).unwrap();
    let sidecar = source.with_file_name("context.sqlite3-wal");
    symlink(&source, &sidecar).unwrap();
    assert_eq!(
        ws.backup(&source, &backup, Fault::None),
        Err(Error::UnsafePath)
    );
}

#[test]
fn changed_backup_is_not_trusted_as_a_recovery_source() {
    let (ws, source) = fixture(false);
    let backup = ws.path().join("backup.sqlite3");
    let output = ws.path().join("output.sqlite3");
    ws.rehearse(&source, &backup, &output, Fault::None).unwrap();
    let writer = Connection::open(&backup).unwrap();
    writer
        .execute("UPDATE preferences SET value='changed'", [])
        .unwrap();
    drop(writer);
    let restored = ws.path().join("restored.sqlite3");
    assert_eq!(ws.restore(&backup, &restored), Err(Error::Verification));
    assert!(!restored.exists());
    assert_eq!(
        ws.retry(&backup, &output, Fault::None),
        Err(Error::Verification)
    );
}

#[test]
fn concurrent_readers_keep_read_only_access_to_v1_outputs() {
    let (ws, source) = fixture(false);
    let backup = ws.path().join("backup.sqlite3");
    let output = ws.path().join("output.sqlite3");
    ws.rehearse(&source, &backup, &output, Fault::None).unwrap();
    let before = fs::read(&output).unwrap();
    std::thread::scope(|scope| {
        let threads: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    let reader = SqliteStore::open(&output).unwrap();
                    assert_eq!(reader.schema().unwrap(), Schema::Version1);
                    assert_eq!(
                        reader
                            .read_handoff("alpha", "shared")
                            .unwrap()
                            .validation
                            .len(),
                        4
                    );
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
    });
    assert_eq!(fs::read(output).unwrap(), before);
}

#[cfg(windows)]
#[test]
fn windows_file_and_parent_symlinks_are_denied() {
    let (ws, source) = fixture(false);
    let backup = ws.path().join("backup.sqlite3");
    let link = ws.path().join("source-link");
    std::os::windows::fs::symlink_file(&source, &link)
        .expect("Windows symlink capability required by CI");
    assert_eq!(
        ws.backup(&link, &backup, Fault::None),
        Err(Error::UnsafePath)
    );
    let parent = ws.path().join("parent-link");
    std::os::windows::fs::symlink_dir(source.parent().unwrap(), &parent).unwrap();
    assert_eq!(
        ws.backup(&parent.join("context.sqlite3"), &backup, Fault::None),
        Err(Error::UnsafePath)
    );
}
