mod support;
use precept_store::{Error, Schema, SqliteStore, profile};
use rusqlite::Connection;
use std::{fs, path::Path};
fn fresh() -> (tempfile::TempDir, std::path::PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let p = support::fixture(&t.path().join("fixture"));
    (t, p)
}
#[test]
fn sqlite_opened_file_matches_native_guard_not_an_identical_copy() {
    let (_t, p) = fresh();
    let copy = p.with_extension("copy");
    fs::copy(&p, &copy).unwrap();
    let original = SqliteStore::open(&p).unwrap();
    let same = fs::File::open(&p).unwrap();
    let different = fs::File::open(&copy).unwrap();
    assert_eq!(
        original.metadata().unwrap().binding,
        SqliteStore::open(&copy)
            .unwrap()
            .metadata()
            .unwrap()
            .binding
    );
    original.check_open_file(&same).unwrap();
    assert_eq!(original.check_open_file(&different), Err(Error::OpenFailed));
    assert_eq!(
        SqliteStore::open(&copy).unwrap().check_open_file(&same),
        Err(Error::OpenFailed)
    );
}
#[test]
fn exact_v1_reads_are_scoped_readonly_and_expose_separate_revisions() {
    let (_t, p) = fresh();
    let before = fs::read(&p).unwrap();
    let s = SqliteStore::open(&p).unwrap();
    assert_eq!(s.schema().unwrap(), Schema::Version1);
    assert_eq!(s.metadata().unwrap().binding.owner_epoch, 0);
    assert_eq!(s.project_revision("alpha").unwrap(), 1);
    assert_eq!(s.preference_revision(1).unwrap(), 2);
    assert_eq!(s.handoff_revision("alpha", "shared").unwrap(), 1);
    assert!(
        s.read_preferences("alpha")
            .unwrap()
            .iter()
            .all(|p| p.project.as_deref() != Some("beta"))
    );
    assert_eq!(
        s.read_handoff("alpha", "beta-only").unwrap_err(),
        Error::HandoffNotFound
    );
    assert_eq!(
        s.read_handoff("alpha", "shared").unwrap().validation.len(),
        4
    );
    assert_eq!(fs::read(p).unwrap(), before);
}
#[test]
fn missing_empty_legacy_and_m1_profiles_are_never_initialized() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("missing");
    assert!(SqliteStore::open(&p).is_err());
    assert!(!p.exists());
    for bytes in [b"".as_slice(), b"invalid"] {
        fs::write(&p, bytes).unwrap();
        assert!(SqliteStore::open(&p).is_err());
        assert_eq!(fs::read(&p).unwrap(), bytes);
    }
    fs::remove_file(&p).unwrap();
    let db = Connection::open(&p).unwrap();
    db.execute_batch("CREATE TABLE projects(name TEXT PRIMARY KEY,root TEXT NOT NULL,created_at TEXT NOT NULL); PRAGMA user_version=1").unwrap();
    drop(db);
    let before = fs::read(&p).unwrap();
    assert!(matches!(
        SqliteStore::open(&p),
        Err(Error::IncompatibleSchema)
    ));
    assert_eq!(fs::read(&p).unwrap(), before);
    let db = Connection::open(&p).unwrap();
    db.pragma_update(None, "user_version", 2).unwrap();
    drop(db);
    let before = fs::read(&p).unwrap();
    assert!(matches!(
        SqliteStore::open(&p),
        Err(Error::UnsupportedVersion)
    ));
    assert_eq!(fs::read(&p).unwrap(), before);
}
#[test]
fn schema_drift_is_refused_before_reads_and_without_repair() {
    for sql in [
        "PRAGMA user_version=3",
        "CREATE TABLE extra(x)",
        "CREATE INDEX extra ON projects(root)",
        "CREATE VIEW extra AS SELECT * FROM projects",
        "CREATE TRIGGER extra AFTER INSERT ON projects BEGIN SELECT 1; END",
        "ALTER TABLE preferences ADD COLUMN extra TEXT",
        "DROP INDEX prefs_lookup",
        "ALTER TABLE projects RENAME COLUMN root TO wrong",
    ] {
        let (_t, p) = fresh();
        let s = SqliteStore::open(&p).unwrap();
        let db = Connection::open(&p).unwrap();
        db.execute_batch(sql).unwrap();
        drop(db);
        let before = fs::read(&p).unwrap();
        assert!(s.schema().is_err());
        assert!(s.read_project("alpha").is_err());
        assert!(SqliteStore::open(&p).is_err());
        assert_eq!(fs::read(&p).unwrap(), before);
    }
}
#[test]
fn corrupt_types_domains_metadata_receipts_and_fks_are_rejected() {
    for sql in [
        "DELETE FROM acp_profile",
        "UPDATE acp_profile SET next_preference_id=1.5",
        "UPDATE acp_profile SET next_preference_id=1",
        "UPDATE acp_profile SET commit_sequence=0",
        "UPDATE acp_profile SET profile_id='BAD'",
        "UPDATE preferences SET revision=3 WHERE id=1",
        "UPDATE preferences SET status='wrong'",
        "UPDATE preferences SET status='confirmed' WHERE id=5",
        "UPDATE preferences SET status='rejected' WHERE id=1",
        "UPDATE acp_review_decisions SET project='beta' WHERE preference_id=1",
        "UPDATE acp_review_decisions SET scope='project' WHERE preference_id=1",
        "UPDATE preferences SET project='missing' WHERE scope='project'",
        "UPDATE handoffs SET validation_json='{}'",
        "UPDATE handoffs SET worktree_state='verified'",
        "UPDATE handoffs SET base_sha='bad'",
        "UPDATE projects SET created_at='2026-02-30T00:00:00Z'",
        "UPDATE acp_receipts SET request_digest=CAST(request_digest AS TEXT)",
        "DELETE FROM acp_receipts WHERE commit_sequence=1",
        "UPDATE acp_receipts SET result_json='{}'",
        "UPDATE acp_receipts SET generation='22222222222222222222222222222222'",
    ] {
        let (_t, p) = fresh();
        let db = Connection::open(&p).unwrap();
        db.execute_batch("PRAGMA ignore_check_constraints=ON; PRAGMA foreign_keys=OFF;")
            .unwrap();
        db.execute_batch(sql).unwrap();
        drop(db);
        let before = fs::read(&p).unwrap();
        assert!(
            SqliteStore::open(&p).is_err(),
            "corrupt fixture was accepted"
        );
        assert_eq!(fs::read(&p).unwrap(), before);
    }
}
#[test]
fn empty_current_schema_is_valid_but_orphaned_domain_data_is_not() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("fresh");
    let db = Connection::open(&p).unwrap();
    for ddl in profile::DDL {
        db.execute_batch(ddl).unwrap()
    }
    db.execute_batch("PRAGMA user_version=1; INSERT INTO acp_profile VALUES(1,'00000000000000000000000000000000','11111111111111111111111111111111',0,1,0,1)").unwrap();
    drop(db);
    assert!(SqliteStore::open(&p).is_ok());
    let db = Connection::open(&p).unwrap();
    db.execute(
        "INSERT INTO projects VALUES('alpha',?1,1,'2026-01-01T00:00:00Z')",
        [t.path().canonicalize().unwrap().to_str().unwrap()],
    )
    .unwrap();
    drop(db);
    assert!(SqliteStore::open(&p).is_err());
}
#[test]
fn review_decision_chain_digest_and_domain_drift_are_refused() {
    // The empty drift is the accepted control: the base fixture's writer-producible
    // decision history (suggestion receipts plus confirm/reject/deprecate chains).
    // Negative cases corrupt that real history or forge a decision whose referenced
    // receipt never produced the claimed transition. D7 delete semantics are
    // exercised separately by the writer test.
    for drift in [
        "",
        "UPDATE acp_review_decisions SET from_revision=3,to_revision=4 WHERE id=1",
        "UPDATE acp_review_decisions SET to_revision=5 WHERE id=1",
        "UPDATE acp_review_decisions SET proposal_digest=zeroblob(32) WHERE id=1",
        "UPDATE acp_review_decisions SET channel='web' WHERE id=1",
        "UPDATE acp_review_decisions SET receipt_commit_sequence=4097 WHERE id=1",
        "UPDATE acp_review_decisions SET decided_at='2026-02-30T00:00:00Z' WHERE id=1",
        "UPDATE acp_review_decisions SET preference_id=999 WHERE id=1",
        "UPDATE acp_review_decisions SET project='beta' WHERE id=1",
        "UPDATE acp_review_decisions SET scope='project' WHERE id=1",
        "UPDATE acp_profile SET next_review_id=1",
        "DELETE FROM acp_review_decisions WHERE preference_id=7 AND action='deprecate'",
        "UPDATE preferences SET status='confirmed' WHERE id=5",
        "UPDATE preferences SET status='rejected' WHERE id=1",
    ] {
        let (_t, p) = fresh();
        let db = Connection::open(&p).unwrap();
        db.pragma_update(None, "foreign_keys", true).unwrap();
        db.execute_batch("PRAGMA ignore_check_constraints=ON")
            .unwrap();
        if !drift.is_empty() {
            db.execute_batch(drift).unwrap();
        }
        drop(db);
        let before = fs::read(&p).unwrap();
        assert_eq!(
            SqliteStore::open(&p).is_ok(),
            drift.is_empty(),
            "unexpected decision-history outcome for: {drift}"
        );
        assert_eq!(fs::read(&p).unwrap(), before);
    }
}

#[test]
fn forged_decision_without_matching_receipt_transition_is_refused() {
    // Preference 5 is a real suggestion at revision 1 with no decision. A forged
    // confirm decision claiming revision 2 must be refused because no receipt
    // ever changed preference 5 to revision 2 (structural causality, not domain).
    let (_t, p) = fresh();
    let db = Connection::open(&p).unwrap();
    let digest: Vec<u8> = db
        .query_row(
            "SELECT proposal_digest FROM preferences WHERE id=5",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let next_id: i64 = db
        .query_row("SELECT next_review_id FROM acp_profile", [], |r| r.get(0))
        .unwrap();
    db.execute(
        "INSERT INTO acp_review_decisions VALUES(?1,'global',NULL,5,'confirm',1,2,?2,1,'cli','2026-01-01T00:00:00Z')",
        rusqlite::params![next_id, digest],
    )
    .unwrap();
    db.execute("UPDATE acp_profile SET next_review_id=?1", [next_id + 1])
        .unwrap();
    drop(db);
    assert!(SqliteStore::open(&p).is_err());
}
#[test]
fn ddl_contains_all_profile_v1_statements() {
    assert_eq!(
        profile::DDL,
        [
            profile::PROFILE,
            profile::PROJECTS,
            profile::PREFERENCES,
            profile::PREFS_LOOKUP,
            profile::HANDOFFS,
            profile::RECEIPTS,
            profile::REVIEW_DECISIONS,
            profile::REVIEW_LOOKUP,
        ]
    );
}
#[test]
fn input_paths_are_explicit() {
    assert!(SqliteStore::open(Path::new("relative.sqlite3")).is_err());
}
