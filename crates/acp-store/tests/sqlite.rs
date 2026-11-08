use acp_core::{ContextResolver, Project, Worktree, WorktreeInspector, documents};
use acp_store::{Error, Schema, SqliteStore};
use rusqlite::Connection;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn fixture(legacy: bool, empty: bool) -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cmd =
        Command::new(std::env::var("ACP_TEST_PYTHON").unwrap_or_else(|_| "python".into()));
    cmd.arg(repo.join("tools/sqlite_fixture.py"))
        .arg(temp.path())
        .env("PYTHONPATH", repo.join("src"));
    if legacy {
        cmd.arg("--legacy");
    }
    if empty {
        cmd.arg("--empty");
    }
    let output = cmd.output().expect("Python fixture builder required");
    assert!(
        output.status.success(),
        "synthetic Python fixture generation failed"
    );
    let path = temp.path().join("state/context.sqlite3");
    (temp, path)
}
struct UnknownWorktree;
impl WorktreeInspector for UnknownWorktree {
    fn inspect(&self, _: &Project, _: bool) -> acp_core::Result<Worktree> {
        Ok(Worktree::default())
    }
}
fn context(store: &SqliteStore, project: &str) -> acp_core::Result<serde_json::Value> {
    ContextResolver::new(
        project,
        store,
        store,
        store,
        &UnknownWorktree,
        &documents::FileDocuments,
    )?
    .context(true)
}
#[test]
fn python_legacy_and_current_are_read_without_changing_database() {
    for legacy in [false, true] {
        let (_temp, path) = fixture(legacy, false);
        let before = fs::read(&path).unwrap();
        {
            let store = SqliteStore::open(&path).unwrap();
            assert_eq!(
                store.schema().unwrap(),
                if legacy {
                    Schema::Legacy
                } else {
                    Schema::Fingerprinted
                }
            );
            let result = context(&store, "alpha").unwrap();
            let prefs = result["preferences"].as_array().unwrap();
            assert_eq!(prefs.len(), 2);
            assert_eq!(prefs[0]["value"], "newest");
            assert_eq!(prefs[0]["source"], "fixture-new");
            for marker in [
                "BETA_PRIVATE_PREFERENCE",
                "PENDING_NOT_FOR_AGENT",
                "REJECTED_NOT_FOR_AGENT",
                "DEPRECATED_NOT_FOR_AGENT",
            ] {
                assert!(!result.to_string().contains(marker));
            }
            let h = store.read_handoff("alpha", "shared").unwrap();
            assert_eq!(
                h.worktree_fingerprint.as_deref(),
                if legacy { None } else { Some("saved") }
            );
            assert_eq!(h.validation.len(), 4);
            assert_eq!(h.validation[0].extra["evidence"]["nested"][3]["log"], "ref");
            assert!(
                store
                    .read_handoff("alpha", "null-fingerprint")
                    .unwrap()
                    .worktree_fingerprint
                    .is_none()
            );
            assert_eq!(
                store.read_handoff("alpha", "beta-only").unwrap_err(),
                Error::HandoffNotFound
            );
            assert_eq!(
                store.read_project("missing").unwrap_err(),
                Error::UnknownProject
            );
            assert_eq!(
                store.read_preferences("missing").unwrap_err(),
                Error::UnknownProject
            );
            assert_eq!(
                store.read_handoff("missing", "shared").unwrap_err(),
                Error::UnknownProject
            );
            assert_eq!(
                store.read_project("alpha' OR 1=1 --").unwrap_err(),
                Error::UnknownProject
            );
            assert_eq!(
                store.read_handoff("alpha", "' OR 1=1 --").unwrap_err(),
                Error::HandoffNotFound
            );
        }
        assert_eq!(fs::read(path).unwrap(), before);
    }
}
#[test]
fn missing_empty_and_invalid_databases_never_get_initialized() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing.sqlite3");
    assert!(matches!(
        SqliteStore::open(&missing),
        Err(Error::OpenFailed)
    ));
    assert!(!missing.exists());
    for bytes in [b"".as_slice(), b"not a SQLite database".as_slice()] {
        let path = temp.path().join("empty.sqlite3");
        fs::write(&path, bytes).unwrap();
        assert!(matches!(
            SqliteStore::open(&path),
            Err(Error::IncompatibleSchema)
        ));
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    let (_temp, path) = fixture(false, true);
    let store = SqliteStore::open(&path).unwrap();
    assert_eq!(
        store.read_project("alpha").unwrap_err(),
        Error::UnknownProject
    );
}
#[test]
fn future_versions_and_incompatible_shapes_fail_closed() {
    for ddl in [
        "PRAGMA user_version=1",
        "ALTER TABLE projects ADD COLUMN future TEXT",
        "DROP TABLE preferences",
        "ALTER TABLE projects RENAME COLUMN root TO other",
        "CREATE VIEW future AS SELECT name FROM projects",
        "CREATE TABLE sqliteXfuture(value TEXT)",
        "ALTER TABLE projects ADD COLUMN generated TEXT GENERATED ALWAYS AS (root) VIRTUAL",
    ] {
        let (_temp, path) = fixture(false, false);
        let store = SqliteStore::open(&path).unwrap();
        let writer = Connection::open(&path).unwrap();
        writer.execute_batch(ddl).unwrap();
        drop(writer);
        let before = fs::read(&path).unwrap();
        assert!(matches!(
            store.schema(),
            Err(Error::UnsupportedVersion | Error::IncompatibleSchema)
        ));
        assert!(store.read_project("alpha").is_err());
        assert!(SqliteStore::open(&path).is_err());
        drop(store);
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}
#[test]
fn malformed_records_are_rejected_and_malformed_scopes_are_not_exposed() {
    for sql in [
        "UPDATE preferences SET status='invalid' WHERE scope='global'",
        "UPDATE handoffs SET validation_json='[{\"check\":\"x\",\"status\":\"SUCCESS\"}]' WHERE project='alpha'",
        "UPDATE handoffs SET validation_json='not-json' WHERE project='alpha'",
        "UPDATE handoffs SET validation_json='{}' WHERE project='alpha'",
    ] {
        let (_temp, path) = fixture(false, false);
        let writer = Connection::open(&path).unwrap();
        writer
            .execute_batch("PRAGMA ignore_check_constraints=ON")
            .unwrap();
        writer.execute_batch(sql).unwrap();
        drop(writer);
        let store = SqliteStore::open(&path).unwrap();
        if sql.contains("preferences") {
            assert_eq!(
                store.read_preferences("alpha").unwrap_err(),
                Error::InvalidRecord
            );
        } else {
            assert_eq!(
                store.read_handoff("alpha", "shared").unwrap_err(),
                Error::InvalidRecord
            );
        }
    }
    let (_temp, path) = fixture(false, false);
    let writer = Connection::open(&path).unwrap();
    writer.execute_batch("PRAGMA ignore_check_constraints=ON; UPDATE preferences SET project='beta' WHERE scope='global'; UPDATE preferences SET scope='invalid' WHERE project='alpha';").unwrap();
    drop(writer);
    let result = context(&SqliteStore::open(&path).unwrap(), "alpha").unwrap();
    assert_eq!(result["preferences"].as_array().unwrap().len(), 0);
}
#[test]
fn independent_concurrent_readers_observe_committed_wal_updates() {
    let (_temp, path) = fixture(false, false);
    let mut writer = Connection::open(&path).unwrap();
    writer
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
        .unwrap();
    let first = SqliteStore::open(&path).unwrap();
    let second = SqliteStore::open(&path).unwrap();
    let tx = writer.transaction().unwrap();
    tx.execute(
        "UPDATE preferences SET value='wal-committed' WHERE source='fixture-new'",
        [],
    )
    .unwrap();
    assert_eq!(
        context(&first, "alpha").unwrap()["preferences"][0]["value"],
        "newest"
    );
    tx.commit().unwrap();
    assert!(path.with_file_name("context.sqlite3-wal").is_file());
    for reader in [&first, &second] {
        assert_eq!(
            context(reader, "alpha").unwrap()["preferences"][0]["value"],
            "wal-committed"
        );
    }
    std::thread::scope(|scope| {
        let threads: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    let reader = SqliteStore::open(&path).unwrap();
                    assert_eq!(
                        context(&reader, "alpha").unwrap()["preferences"][0]["value"],
                        "wal-committed"
                    );
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
    });
    drop(first);
    drop(second);
    drop(writer);
}
