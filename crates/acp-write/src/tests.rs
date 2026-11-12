use super::*;
use crate::{
    schema::{self, Shape},
    workspace::{self, Role},
};
use rusqlite::{Connection, types::Value};
use std::{
    sync::{Arc, Barrier},
    time::{Duration, Instant},
};
const SHAPES: [FixtureShape; 3] = [
    FixtureShape::LegacyV0,
    FixtureShape::FingerprintedV0,
    FixtureShape::Version1,
];
fn ready(shape: FixtureShape) -> SyntheticWorkspace {
    let mut w = SyntheticWorkspace::new(shape).unwrap();
    w.prepare().unwrap();
    w
}
fn request(w: &SyntheticWorkspace, id: &str, command: Command) -> Request {
    Request {
        version: 1,
        binding: w.binding(),
        operation_id: id.into(),
        command,
    }
}
fn transition(id: i64, scope: Scope, expected: i64, action: usize) -> Command {
    match action {
        0 => Command::ConfirmPreference {
            id,
            scope,
            expected: Expected::Revision(expected),
        },
        1 => Command::RejectPreference {
            id,
            scope,
            expected: Expected::Revision(expected),
        },
        _ => Command::DeprecatePreference {
            id,
            scope,
            expected: Expected::Revision(expected),
        },
    }
}
fn suggest(scope: Scope) -> Command {
    Command::SuggestPreference {
        scope,
        key: "new-key".into(),
        value: "synthetic-payload".into(),
        source: "synthetic-source".into(),
        expected: Expected::Absent,
    }
}
fn handoff(project: &str, task: &str, expected: Expected) -> Command {
    Command::PutHandoff{project:project.into(),task_id:task.into(),objective:"synthetic objective".into(),completed:"complete".into(),next_action:"inspect".into(),validation_json:"[ {\"check\":\"unit\",\"status\":\"NOT_RUN\",\"evidence\":{\"id\":\"opaque\",\"unknown\":[1,null]}} ]".into(),snapshot:Snapshot::UnknownFingerprint,expected}
}
fn dump(db: &Connection) -> Vec<Vec<Vec<Value>>> {
    let mut rows = schema::records(db).unwrap();
    for q in [
        "SELECT * FROM acp_profile ORDER BY singleton",
        "SELECT * FROM acp_revisions ORDER BY kind,project,record_id",
        "SELECT * FROM acp_receipts ORDER BY operation_id",
    ] {
        let mut stmt = db.prepare(q).unwrap();
        let n = stmt.column_count();
        rows.push(
            stmt.query_map([], |r| {
                (0..n)
                    .map(|i| r.get::<_, Value>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap(),
        );
    }
    rows
}
fn status(db: &Connection, id: i64) -> String {
    db.query_row("SELECT status FROM preferences WHERE id=?1", [id], |r| {
        r.get(0)
    })
    .unwrap()
}
fn rev(db: &Connection, id: i64) -> (i64, bool) {
    db.query_row(
        "SELECT revision,deleted FROM acp_revisions WHERE kind='preference' AND record_id=?1",
        [id.to_string()],
        |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0)),
    )
    .unwrap()
}
#[test]
fn frozen_document_ddl_matches_exact_implementation() {
    let text = include_str!("../../../docs/p1/profile-v2-schema-and-writer.md");
    let block = text
        .split("```sql\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let frozen: Vec<_> = block
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    assert_eq!(
        frozen,
        [
            schema::PROJECTS,
            schema::PREFERENCES,
            schema::PREFS_LOOKUP,
            schema::HANDOFFS,
            schema::PROFILE,
            schema::REVISIONS,
            schema::RECEIPTS
        ]
    );
}
#[test]
fn all_source_shapes_migrate_exact_history_and_allocation_without_writes_to_prior_copies() {
    for shape in SHAPES {
        let mut w = SyntheticWorkspace::new(shape).unwrap();
        let source = workspace::connect(&w.path(Role::Source), true).unwrap();
        let before = schema::records(&source).unwrap();
        drop(source);
        let source_bytes = std::fs::read(w.path(Role::Source)).unwrap();
        w.prepare().unwrap();
        let writer = w.writer().unwrap();
        assert_eq!(schema::inspect(&writer.db).unwrap(), Shape::V2);
        assert_eq!(schema::records(&writer.db).unwrap(), before);
        for id in [4, 9] {
            assert_eq!(status(&writer.db, id), "deprecated");
            assert_eq!(rev(&writer.db, id), (1, false));
        }
        assert_eq!(std::fs::read(w.path(Role::Source)).unwrap(), source_bytes);
        let count: i64 = writer
            .db
            .query_row("SELECT count(*) FROM acp_revisions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 14);
        let backup_bytes = std::fs::read(w.path(Role::Backup)).unwrap();
        drop(writer);
        let mut writer = w.writer().unwrap();
        let result = writer
            .execute(&request(&w, "allocate", suggest(Scope::Global)))
            .unwrap();
        assert_eq!(result.affected[0].record_id, "1001");
        w.verify_prior().unwrap();
        assert_eq!(std::fs::read(w.path(Role::Backup)).unwrap(), backup_bytes);
        assert_eq!(std::fs::read(w.path(Role::Source)).unwrap(), source_bytes);
        assert!(matches!(
            acp_store::SqliteStore::open(&w.path(Role::Candidate)),
            Err(acp_store::Error::UnsupportedVersion)
        ));
    }
}
#[test]
fn complete_strict_transition_matrix_and_denials_preserve_every_logical_table() {
    for shape in SHAPES {
        for (id, old) in [
            (1, "suggested"),
            (2, "confirmed"),
            (3, "rejected"),
            (4, "deprecated"),
        ] {
            for action in 0..3 {
                let w = ready(shape);
                let mut writer = w.writer().unwrap();
                let before = dump(&writer.db);
                let result = writer.execute(&request(
                    &w,
                    "matrix",
                    transition(id, Scope::Global, 1, action),
                ));
                let next = match (old, action) {
                    ("suggested", 0) => Some("confirmed"),
                    ("suggested", 1) => Some("rejected"),
                    ("confirmed", 2) => Some("deprecated"),
                    _ => None,
                };
                if let Some(next) = next {
                    let receipt = result.unwrap();
                    assert_eq!(status(&writer.db, id), next);
                    assert_eq!(receipt.commit_sequence, 1);
                    assert_eq!(rev(&writer.db, id), (2, false));
                } else {
                    assert_eq!(result, Err(Error::InvalidTransition));
                    assert_eq!(dump(&writer.db), before);
                }
                w.verify_prior().unwrap();
            }
        }
    }
}
#[test]
fn confirmation_is_atomic_and_never_deprecates_another_scope_or_project() {
    for shape in SHAPES {
        let w = ready(shape);
        let mut writer = w.writer().unwrap();
        let receipt = writer
            .execute(&request(
                &w,
                "confirm",
                transition(
                    5,
                    Scope::Project {
                        name: "alpha".into(),
                    },
                    1,
                    0,
                ),
            ))
            .unwrap();
        assert_eq!(receipt.affected.len(), 3);
        for id in [5, 6, 7] {
            assert_eq!(rev(&writer.db, id), (2, false));
        }
        assert_eq!(status(&writer.db, 5), "confirmed");
        for id in [6, 7] {
            assert_eq!(status(&writer.db, id), "deprecated");
        }
        for id in [2, 10] {
            assert_eq!(status(&writer.db, id), "confirmed");
            assert_eq!(rev(&writer.db, id), (1, false));
        }
        crate::writer::verify(&writer.db, &w.binding()).unwrap();
    }
}
#[test]
fn cas_scope_generation_epoch_and_occupied_absence_refuse_without_changes() {
    let w = ready(FixtureShape::Version1);
    let mut writer = w.writer().unwrap();
    let base = request(&w, "cas", transition(1, Scope::Global, 1, 0));
    let mut cases = Vec::new();
    let mut r = base.clone();
    r.command = transition(1, Scope::Global, 2, 0);
    cases.push((r, Error::Conflict));
    let mut r = base.clone();
    r.command = transition(
        1,
        Scope::Project {
            name: "alpha".into(),
        },
        1,
        0,
    );
    cases.push((r, Error::ScopeMismatch));
    let mut r = base.clone();
    r.command = transition(900, Scope::Global, 1, 0);
    cases.push((r, Error::Conflict));
    let mut r = base.clone();
    r.binding.generation = "0".repeat(32);
    cases.push((r, Error::StaleGeneration));
    let mut r = base.clone();
    r.binding.owner_epoch = 2;
    cases.push((r, Error::OwnerMismatch));
    let mut r = base.clone();
    r.binding.profile_id = "0".repeat(32);
    cases.push((r, Error::OwnerMismatch));
    cases.push((
        request(&w, "occupied", handoff("alpha", "shared", Expected::Absent)),
        Error::Conflict,
    ));
    cases.push((
        request(
            &w,
            "missing",
            handoff("alpha", "missing", Expected::Revision(1)),
        ),
        Error::Conflict,
    ));
    for (r, error) in cases {
        let before = dump(&writer.db);
        assert_eq!(writer.execute(&r), Err(error));
        assert_eq!(dump(&writer.db), before);
    }
    writer.execute(&base).unwrap();
    let before = dump(&writer.db);
    assert_eq!(
        writer.execute(&request(&w, "stale", transition(1, Scope::Global, 1, 1))),
        Err(Error::Conflict)
    );
    assert_eq!(dump(&writer.db), before);
    assert_eq!(
        writer.execute(&request(
            &w,
            "fresh-state",
            transition(1, Scope::Global, 2, 1)
        )),
        Err(Error::InvalidTransition)
    );
    assert_eq!(dump(&writer.db), before);
}
#[test]
fn independent_connection_race_has_one_cas_winner() {
    let w = ready(FixtureShape::Version1);
    let barrier = Arc::new(Barrier::new(2));
    let results = std::thread::scope(|s| {
        let handles: Vec<_> = (0..2)
            .map(|i| {
                let barrier = barrier.clone();
                let w = &w;
                s.spawn(move || {
                    let mut writer = w.writer().unwrap();
                    let r = request(w, &format!("race-{i}"), transition(1, Scope::Global, 1, i));
                    barrier.wait();
                    writer.execute(&r)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(
        results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .all(|e| *e == Error::Conflict)
    );
    let writer = w.writer().unwrap();
    assert_eq!(rev(&writer.db, 1), (2, false));
    let seq: i64 = writer
        .db
        .query_row("SELECT commit_sequence FROM acp_profile", [], |r| r.get(0))
        .unwrap();
    assert_eq!(seq, 1);
}
#[test]
fn receipt_replay_after_lost_response_and_reopen_precedes_cas_and_capacity() {
    for shape in SHAPES {
        let w = ready(shape);
        let r = request(&w, "lost-response", transition(1, Scope::Global, 1, 0));
        let mut writer = w.writer().unwrap();
        assert_eq!(
            writer.execute_inner(&r, Fault::AfterCommit),
            Err(Error::OutcomeUnknown)
        );
        let before = dump(&writer.db);
        drop(writer);
        let mut writer = w.writer().unwrap();
        let first = writer.execute(&r).unwrap();
        assert_eq!(first.commit_sequence, 1);
        assert_eq!(writer.execute(&r).unwrap(), first);
        assert_eq!(dump(&writer.db), before);
        let mut changed = r.clone();
        changed.command = transition(1, Scope::Global, 2, 2);
        assert_eq!(writer.execute(&changed), Err(Error::IdempotencyConflict));
        assert_eq!(dump(&writer.db), before);
        let mut stale = r.clone();
        stale.binding.generation = "0".repeat(32);
        assert_eq!(writer.execute(&stale), Err(Error::StaleGeneration));
        writer
            .db
            .execute("UPDATE acp_profile SET receipt_limit=1", [])
            .unwrap();
        let full = dump(&writer.db);
        assert_eq!(writer.execute(&r).unwrap(), first);
        assert_eq!(
            writer.execute(&request(&w, "full", suggest(Scope::Global))),
            Err(Error::Capacity)
        );
        assert_eq!(dump(&writer.db), full);
    }
}
#[test]
fn canonical_requests_ignore_outer_json_order_but_preserve_opaque_string_bytes() {
    let w = ready(FixtureShape::Version1);
    let r = request(
        &w,
        "canonical",
        handoff("alpha", "new-task", Expected::Absent),
    );
    let json = serde_json::to_value(&r).unwrap();
    let parsed = Request::from_json(&serde_json::to_vec_pretty(&json).unwrap()).unwrap();
    assert_eq!(parsed, r);
    let mut writer = w.writer().unwrap();
    let first = writer.execute(&r).unwrap();
    assert_eq!(writer.execute(&parsed).unwrap(), first);
    let mut changed = r.clone();
    if let Command::PutHandoff {
        validation_json, ..
    } = &mut changed.command
    {
        validation_json.push(' ');
    }
    let before = dump(&writer.db);
    assert_eq!(writer.execute(&changed), Err(Error::IdempotencyConflict));
    assert_eq!(dump(&writer.db), before);
}
#[test]
fn partial_command_faults_rollback_data_revisions_sequence_receipt_and_allocator() {
    for shape in SHAPES {
        for fault in [
            Fault::AfterData,
            Fault::AfterRevisions,
            Fault::BeforeCommit,
            Fault::StorageFull,
        ] {
            for command in [
                suggest(Scope::Global),
                transition(
                    5,
                    Scope::Project {
                        name: "alpha".into(),
                    },
                    1,
                    0,
                ),
            ] {
                let w = ready(shape);
                let mut writer = w.writer().unwrap();
                let before = dump(&writer.db);
                assert_eq!(
                    writer.execute_inner(&request(&w, "fault", command), fault),
                    Err(if fault == Fault::StorageFull {
                        Error::StorageFull
                    } else {
                        Error::BeforeCommit
                    })
                );
                assert_eq!(dump(&writer.db), before);
                w.verify_prior().unwrap();
            }
        }
    }
}
#[test]
fn migration_precommit_rollback_and_committed_unknown_require_explicit_verification() {
    for shape in SHAPES {
        let mut w = SyntheticWorkspace::new(shape).unwrap();
        assert_eq!(
            w.prepare_inner(Fault::BeforeCommit),
            Err(Error::BeforeCommit)
        );
        assert_eq!(w.writer().unwrap_err(), Error::Verification);
        let db = workspace::connect(&w.path(Role::Candidate), true).unwrap();
        assert_ne!(schema::inspect(&db).unwrap(), Shape::V2);
        let original = workspace::connect(&w.path(Role::Source), true).unwrap();
        assert_eq!(
            schema::records(&db).unwrap(),
            schema::records(&original).unwrap()
        );
        w.verify_prior().unwrap();
        drop(original);
        drop(db);
        assert_eq!(w.prepare(), Err(Error::Collision));
        let mut w = SyntheticWorkspace::new(shape).unwrap();
        assert_eq!(
            w.prepare_inner(Fault::AfterCommit),
            Err(Error::OutcomeUnknown)
        );
        assert_eq!(w.writer().unwrap_err(), Error::Verification);
        w.verify_candidate().unwrap();
        assert!(w.writer().is_ok());
        w.verify_prior().unwrap();
    }
}
#[test]
fn interrupted_backup_and_injected_full_never_enable_candidate_or_mutate_source() {
    for shape in SHAPES {
        for fault in [Fault::BackupPartial, Fault::StorageFull] {
            let mut w = SyntheticWorkspace::new(shape).unwrap();
            let before = std::fs::read(w.path(Role::Source)).unwrap();
            assert_eq!(
                w.prepare_inner(fault),
                Err(if fault == Fault::StorageFull {
                    Error::StorageFull
                } else {
                    Error::BeforeCommit
                })
            );
            assert_eq!(w.writer().unwrap_err(), Error::Verification);
            assert_eq!(std::fs::read(w.path(Role::Source)).unwrap(), before);
            assert!(!w.path(Role::Candidate).exists());
        }
    }
}
#[test]
fn delete_keeps_minimal_tombstone_and_never_reuses_allocated_identity() {
    for shape in SHAPES {
        for id in [1, 2, 3, 4] {
            let w = ready(shape);
            let mut writer = w.writer().unwrap();
            writer
                .execute(&request(
                    &w,
                    "delete",
                    Command::DeletePreference {
                        id,
                        scope: Scope::Global,
                        expected: Expected::Revision(1),
                    },
                ))
                .unwrap();
            assert_eq!(rev(&writer.db, id), (2, true));
            let count: i64 = writer
                .db
                .query_row("SELECT count(*) FROM preferences WHERE id=?1", [id], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(count, 0);
            let before = dump(&writer.db);
            assert_eq!(
                writer.execute(&request(&w, "deleted", transition(id, Scope::Global, 2, 0))),
                Err(Error::Conflict)
            );
            assert_eq!(dump(&writer.db), before);
            let receipt = writer
                .execute(&request(&w, "new", suggest(Scope::Global)))
                .unwrap();
            assert_eq!(receipt.affected[0].record_id, "1001");
            let serialized = serde_json::to_string(&receipt).unwrap();
            assert!(!serialized.contains("synthetic-payload"));
            assert!(!serialized.contains("synthetic-source"));
        }
    }
}
#[test]
fn handoff_and_registration_preserve_scope_raw_json_and_revision_contract() {
    for shape in SHAPES {
        let w = ready(shape);
        let mut writer = w.writer().unwrap();
        let command = Command::RegisterProject {
            name: "gamma".into(),
            root: FixtureRoot::Gamma,
            expected: Expected::Absent,
        };
        let receipt = writer
            .execute(&request(&w, "register", command.clone()))
            .unwrap();
        assert_eq!(receipt.affected[0].revision, 1);
        let before = dump(&writer.db);
        assert_eq!(
            writer.execute(&request(&w, "again", command)),
            Err(Error::Conflict)
        );
        assert_eq!(dump(&writer.db), before);
        let beta: String = writer
            .db
            .query_row(
                "SELECT validation_json FROM handoffs WHERE project='beta' AND task_id='shared'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let r = request(
            &w,
            "handoff",
            handoff("alpha", "shared", Expected::Revision(1)),
        );
        let receipt = writer.execute(&r).unwrap();
        assert_eq!(receipt.affected[0].revision, 2);
        let (raw,fp):(String,Option<String>)=writer.db.query_row("SELECT validation_json,worktree_fingerprint FROM handoffs WHERE project='alpha' AND task_id='shared'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        if let Command::PutHandoff {
            validation_json, ..
        } = &r.command
        {
            assert_eq!(&raw, validation_json);
        }
        assert!(fp.is_none());
        assert_eq!(writer.db.query_row("SELECT validation_json FROM handoffs WHERE project='beta' AND task_id='shared'",[],|r|r.get::<_,String>(0)).unwrap(),beta);
        writer
            .execute(&request(
                &w,
                "new-handoff",
                handoff("gamma", "shared", Expected::Absent),
            ))
            .unwrap();
        let before = dump(&writer.db);
        assert_eq!(
            writer.execute(&request(
                &w,
                "unknown",
                handoff("unknown", "shared", Expected::Absent)
            )),
            Err(Error::Conflict)
        );
        assert_eq!(dump(&writer.db), before);
    }
}
#[test]
fn independent_writer_contention_is_bounded_busy_then_retryable() {
    let w = ready(FixtureShape::Version1);
    let mut writer = w.writer().unwrap();
    let lock = workspace::connect(&w.path(Role::Candidate), false).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    let r = request(&w, "busy", suggest(Scope::Global));
    let start = Instant::now();
    assert_eq!(writer.execute(&r), Err(Error::Busy));
    assert!(start.elapsed() < Duration::from_secs(3));
    lock.execute_batch("ROLLBACK").unwrap();
    drop(lock);
    assert!(writer.execute(&r).is_ok());
}
#[test]
fn live_wal_backup_includes_committed_and_excludes_uncommitted_source_changes() {
    for shape in SHAPES {
        let mut w = SyntheticWorkspace::new(shape).unwrap();
        let source = workspace::connect(&w.path(Role::Source), false).unwrap();
        source.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; UPDATE preferences SET value='committed-wal' WHERE id=1; BEGIN IMMEDIATE; UPDATE preferences SET value='uncommitted-wal' WHERE id=1;").unwrap();
        assert!(
            w.path(Role::Source)
                .with_file_name("profile.sqlite3-wal")
                .exists()
        );
        w.prepare().unwrap();
        let writer = w.writer().unwrap();
        assert_eq!(
            writer
                .db
                .query_row("SELECT value FROM preferences WHERE id=1", [], |r| r
                    .get::<_, String>(0))
                .unwrap(),
            "committed-wal"
        );
        source.execute_batch("ROLLBACK").unwrap();
        drop(source);
        w.verify_prior().unwrap();
    }
}
#[test]
fn malformed_requests_and_bounds_are_sanitized_noops() {
    let w = ready(FixtureShape::Version1);
    let mut writer = w.writer().unwrap();
    let base = request(&w, "good", suggest(Scope::Global));
    let mut invalid = Vec::new();
    let mut r = base.clone();
    r.version = 99;
    invalid.push(r);
    let mut r = base.clone();
    r.operation_id = "/private/synthetic-path".into();
    invalid.push(r);
    let mut r = base.clone();
    r.binding.generation = "invalid".into();
    invalid.push(r);
    let mut r = base.clone();
    r.command = transition(1, Scope::Global, 0, 0);
    invalid.push(r);
    for value in [
        "".to_owned(),
        " ".to_owned(),
        "x".repeat(4097),
        "nul\0value".into(),
    ] {
        let mut r = base.clone();
        if let Command::SuggestPreference { value: v, .. } = &mut r.command {
            *v = value;
        }
        invalid.push(r);
    }
    for raw in [
        "{}",
        "[1]",
        "[{\"check\":1,\"status\":\"PASS\"}]",
        "[{\"check\":\"x\",\"status\":\"BLOCKED\"}]",
    ] {
        let mut r = request(&w, "bad-json", handoff("alpha", "task", Expected::Absent));
        if let Command::PutHandoff {
            validation_json, ..
        } = &mut r.command
        {
            *validation_json = raw.into();
        }
        invalid.push(r);
    }
    for r in invalid {
        let before = dump(&writer.db);
        assert_eq!(writer.execute(&r), Err(Error::InvalidCommand));
        assert_eq!(dump(&writer.db), before);
    }
    let mut value = serde_json::to_value(&base).unwrap();
    value["extra"] = serde_json::json!("synthetic-private-value");
    assert_eq!(
        Request::from_json(&serde_json::to_vec(&value).unwrap()),
        Err(Error::InvalidCommand)
    );
    assert_eq!(
        Request::from_json(&vec![b' '; 128 * 1024 + 1]),
        Err(Error::InvalidCommand)
    );
    assert_eq!(format!("{base:?}"), "Request(redacted)");
    for e in [
        Error::InvalidCommand,
        Error::InvalidTransition,
        Error::IdempotencyConflict,
        Error::IoFailure,
        Error::OutcomeUnknown,
    ] {
        assert!(!format!("{e} {e:?}").contains("synthetic"));
    }
}
#[test]
fn malformed_legacy_and_v2_schema_objects_are_rejected_without_mutation() {
    let cases = [
        ("PRAGMA user_version=99", Error::UnsupportedVersion),
        (
            "ALTER TABLE projects ADD COLUMN extra TEXT",
            Error::IncompatibleSchema,
        ),
        ("DROP INDEX prefs_lookup", Error::IncompatibleSchema),
        (
            "DROP INDEX prefs_lookup; CREATE INDEX prefs_lookup ON preferences(status)",
            Error::IncompatibleSchema,
        ),
        (
            "CREATE INDEX unexpected ON preferences(value)",
            Error::IncompatibleSchema,
        ),
        (
            "CREATE VIEW unexpected AS SELECT * FROM preferences",
            Error::IncompatibleSchema,
        ),
        (
            "CREATE TRIGGER unexpected AFTER UPDATE ON preferences BEGIN SELECT 1; END",
            Error::IncompatibleSchema,
        ),
        ("CREATE TABLE extra(x TEXT)", Error::IncompatibleSchema),
        (
            "PRAGMA writable_schema=ON; UPDATE sqlite_schema SET sql=replace(sql,'value TEXT','value BLOB') WHERE name='preferences'; PRAGMA writable_schema=OFF",
            Error::IncompatibleSchema,
        ),
        (
            "PRAGMA writable_schema=ON; UPDATE sqlite_schema SET sql=replace(sql,'AUTOINCREMENT','') WHERE name='preferences'; PRAGMA writable_schema=OFF",
            Error::IncompatibleSchema,
        ),
        (
            "PRAGMA writable_schema=ON; UPDATE sqlite_schema SET sql=replace(sql,\"CHECK (scope IN ('global','project'))\",'') WHERE name='preferences'; PRAGMA writable_schema=OFF",
            Error::IncompatibleSchema,
        ),
    ];
    for shape in SHAPES {
        for (sql, error) in cases {
            let mut w = SyntheticWorkspace::new(shape).unwrap();
            let db = workspace::connect(&w.path(Role::Source), false).unwrap();
            db.execute_batch(sql).unwrap();
            drop(db);
            let before = std::fs::read(w.path(Role::Source)).unwrap();
            assert_eq!(w.prepare(), Err(error));
            assert_eq!(std::fs::read(w.path(Role::Source)).unwrap(), before);
            assert!(!w.path(Role::Backup).exists());
            let w = ready(shape);
            let db = workspace::connect(&w.path(Role::Candidate), false).unwrap();
            db.execute_batch(sql).unwrap();
            drop(db);
            let before = std::fs::read(w.path(Role::Candidate)).unwrap();
            assert_eq!(w.writer().unwrap_err(), error);
            assert_eq!(std::fs::read(w.path(Role::Candidate)).unwrap(), before);
        }
    }
    let mut w = SyntheticWorkspace::new(FixtureShape::LegacyV0).unwrap();
    let db = workspace::connect(&w.path(Role::Source), false).unwrap();
    db.pragma_update(None, "user_version", 1).unwrap();
    drop(db);
    assert_eq!(w.prepare(), Err(Error::IncompatibleSchema));
}
#[test]
fn invalid_legacy_records_and_technical_metadata_cannot_be_repaired_by_writer() {
    for sql in [
        "PRAGMA ignore_check_constraints=ON; UPDATE preferences SET status='unknown' WHERE id=1",
        "UPDATE handoffs SET validation_json='{}'",
        "UPDATE sqlite_sequence SET seq=1 WHERE name='preferences'",
    ] {
        let mut w = SyntheticWorkspace::new(FixtureShape::Version1).unwrap();
        let db = workspace::connect(&w.path(Role::Source), false).unwrap();
        db.execute_batch(sql).unwrap();
        drop(db);
        let before = std::fs::read(w.path(Role::Source)).unwrap();
        assert_eq!(w.prepare(), Err(Error::InvalidRecord));
        assert_eq!(std::fs::read(w.path(Role::Source)).unwrap(), before);
    }
    for (sql, error) in [
        ("UPDATE acp_profile SET owner_epoch=2", Error::OwnerMismatch),
        (
            "UPDATE acp_profile SET generation='00000000000000000000000000000000'",
            Error::StaleGeneration,
        ),
        (
            "DELETE FROM acp_revisions WHERE kind='preference' AND record_id='1'",
            Error::InvalidRecord,
        ),
        (
            "UPDATE acp_profile SET commit_sequence=1",
            Error::InvalidRecord,
        ),
        (
            "UPDATE acp_revisions SET deleted=1 WHERE kind='preference' AND record_id='1'",
            Error::InvalidRecord,
        ),
    ] {
        let w = ready(FixtureShape::Version1);
        let mut writer = w.writer().unwrap();
        writer.db.execute_batch(sql).unwrap();
        let before = dump(&writer.db);
        assert_eq!(
            writer.execute(&request(&w, "corrupt", suggest(Scope::Global))),
            Err(error)
        );
        assert_eq!(dump(&writer.db), before);
    }
}
#[test]
fn allocation_revision_and_receipt_payload_capacity_fail_atomically() {
    let w = ready(FixtureShape::Version1);
    let mut writer = w.writer().unwrap();
    writer
        .db
        .execute(
            "UPDATE acp_revisions SET revision=?1 WHERE kind='preference' AND record_id='1'",
            [i64::MAX],
        )
        .unwrap();
    let before = dump(&writer.db);
    assert_eq!(
        writer.execute(&request(
            &w,
            "overflow",
            transition(1, Scope::Global, i64::MAX, 0)
        )),
        Err(Error::Capacity)
    );
    assert_eq!(dump(&writer.db), before);
    writer
        .db
        .execute(
            "UPDATE sqlite_sequence SET seq=?1 WHERE name='preferences'",
            [i64::MAX],
        )
        .unwrap();
    let before = dump(&writer.db);
    assert_eq!(
        writer.execute(&request(&w, "allocator", suggest(Scope::Global))),
        Err(Error::StorageFull)
    );
    assert_eq!(dump(&writer.db), before);
    for count in [900, 1024] {
        let mut w = SyntheticWorkspace::new(FixtureShape::Version1).unwrap();
        let db = workspace::connect(&w.path(Role::Source), false).unwrap();
        db.execute_batch("BEGIN").unwrap();
        for _ in 0..count {
            db.execute_batch("INSERT INTO preferences(scope,project,pref_key,value,status,source,created_at,updated_at) VALUES('project','alpha','formatter','synthetic','confirmed','fixture','created','updated')").unwrap();
        }
        db.execute_batch("COMMIT").unwrap();
        drop(db);
        w.prepare().unwrap();
        let mut writer = w.writer().unwrap();
        let before = dump(&writer.db);
        assert_eq!(
            writer.execute(&request(
                &w,
                "oversized-confirm",
                transition(
                    5,
                    Scope::Project {
                        name: "alpha".into()
                    },
                    1,
                    0
                )
            )),
            Err(Error::Capacity)
        );
        assert_eq!(dump(&writer.db), before);
        w.verify_prior().unwrap();
    }
}
#[test]
fn live_v2_wal_reader_snapshot_never_observes_partial_data_or_receipt() {
    let w = ready(FixtureShape::Version1);
    let mut writer = w.writer().unwrap();
    writer
        .db
        .pragma_update(None, "journal_mode", "WAL")
        .unwrap();
    writer
        .db
        .pragma_update(None, "wal_autocheckpoint", 0)
        .unwrap();
    let reader = workspace::connect(&w.path(Role::Candidate), true).unwrap();
    reader.execute_batch("BEGIN").unwrap();
    assert_eq!(status(&reader, 1), "suggested");
    writer
        .execute(&request(
            &w,
            "wal-command",
            transition(1, Scope::Global, 1, 0),
        ))
        .unwrap();
    assert_eq!(status(&reader, 1), "suggested");
    assert_eq!(rev(&reader, 1), (1, false));
    reader.execute_batch("COMMIT").unwrap();
    assert_eq!(status(&reader, 1), "confirmed");
    assert_eq!(rev(&reader, 1), (2, false));
    let seq: i64 = reader
        .query_row("SELECT commit_sequence FROM acp_profile", [], |r| r.get(0))
        .unwrap();
    assert_eq!(seq, 1);
    let other = workspace::connect(&w.path(Role::Candidate), false).unwrap();
    other
        .execute_batch("BEGIN IMMEDIATE; UPDATE preferences SET value='uncommitted' WHERE id=1")
        .unwrap();
    assert_ne!(
        reader
            .query_row("SELECT value FROM preferences WHERE id=1", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "uncommitted"
    );
    other.execute_batch("ROLLBACK").unwrap();
    crate::writer::verify(&reader, &w.binding()).unwrap();
}
#[test]
fn workspace_rejects_target_sidecars_replaced_identities_and_hardlink_aliases() {
    let mut w = SyntheticWorkspace::new(FixtureShape::Version1).unwrap();
    let sidecar = w
        .path(Role::Candidate)
        .with_file_name("profile.sqlite3-wal");
    let sidecar_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&sidecar)
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        sidecar_file
            .set_permissions(std::fs::Permissions::from_mode(0o600))
            .unwrap();
    }
    drop(sidecar_file);
    assert_eq!(w.prepare(), Err(Error::Collision));
    assert_eq!(w.writer().unwrap_err(), Error::Verification);
    let w = ready(FixtureShape::Version1);
    let external = tempfile::tempdir().unwrap();
    std::fs::hard_link(
        w.path(Role::Candidate),
        external.path().join("alias.sqlite3"),
    )
    .unwrap();
    assert_eq!(w.writer().unwrap_err(), Error::UnsafeWorkspace);
    let w = ready(FixtureShape::Version1);
    let external = tempfile::tempdir().unwrap();
    let moved = external.path().join("original.sqlite3");
    std::fs::rename(w.path(Role::Candidate), &moved).unwrap();
    std::fs::copy(&moved, w.path(Role::Candidate)).unwrap();
    assert_eq!(w.writer().unwrap_err(), Error::UnsafeWorkspace);
}
#[cfg(unix)]
#[test]
fn symlink_escape_and_permission_changes_refuse_before_sqlite_access() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let w = ready(FixtureShape::Version1);
    let external = tempfile::tempdir().unwrap();
    let moved = external.path().join("original.sqlite3");
    std::fs::rename(w.path(Role::Candidate), &moved).unwrap();
    symlink(&moved, w.path(Role::Candidate)).unwrap();
    let before = std::fs::read(&moved).unwrap();
    assert_eq!(w.writer().unwrap_err(), Error::UnsafeWorkspace);
    assert_eq!(std::fs::read(&moved).unwrap(), before);
    let w = ready(FixtureShape::Version1);
    std::fs::set_permissions(
        w.path(Role::Candidate),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert_eq!(w.writer().unwrap_err(), Error::UnsafeWorkspace);
}
#[test]
fn latest_legacy_state_has_lossless_synthetic_v1_projection_for_future_owner_rollback() {
    for shape in SHAPES {
        let w = ready(shape);
        let mut writer = w.writer().unwrap();
        writer
            .execute(&request(
                &w,
                "confirm",
                transition(
                    5,
                    Scope::Project {
                        name: "alpha".into(),
                    },
                    1,
                    0,
                ),
            ))
            .unwrap();
        writer
            .execute(&request(
                &w,
                "delete",
                Command::DeletePreference {
                    id: 4,
                    scope: Scope::Global,
                    expected: Expected::Revision(1),
                },
            ))
            .unwrap();
        writer
            .execute(&request(&w, "suggest", suggest(Scope::Global)))
            .unwrap();
        let expected = schema::records(&writer.db).unwrap();
        let destination = tempfile::NamedTempFile::new().unwrap();
        let mut db = workspace::connect(destination.path(), false).unwrap();
        {
            let copy = rusqlite::backup::Backup::new(&writer.db, &mut db).unwrap();
            copy.run_to_completion(16, Duration::from_millis(1), None)
                .unwrap();
        }
        db.execute_batch("BEGIN IMMEDIATE; DROP TABLE acp_receipts; DROP TABLE acp_revisions; DROP TABLE acp_profile; PRAGMA user_version=1; COMMIT").unwrap();
        assert_eq!(schema::inspect(&db).unwrap(), Shape::V1);
        assert_eq!(schema::records(&db).unwrap(), expected);
        drop(db);
        let reader = acp_store::SqliteStore::open(destination.path()).unwrap();
        assert_eq!(reader.schema().unwrap(), acp_store::Schema::Version1);
        assert!(reader.read_preferences("alpha").is_ok());
        crate::writer::verify(&writer.db, &w.binding()).unwrap();
        w.verify_prior().unwrap();
    }
}
#[test]
fn minimal_receipts_tombstones_and_errors_do_not_retain_command_contents() {
    let w = ready(FixtureShape::Version1);
    let mut writer = w.writer().unwrap();
    let r = request(&w, "privacy", suggest(Scope::Global));
    let receipt = writer.execute(&r).unwrap();
    let id = receipt.affected[0].record_id.parse::<i64>().unwrap();
    writer
        .execute(&request(
            &w,
            "privacy-delete",
            Command::DeletePreference {
                id,
                scope: Scope::Global,
                expected: Expected::Revision(1),
            },
        ))
        .unwrap();
    for table in ["acp_receipts", "acp_revisions", "acp_profile"] {
        let mut stmt = writer
            .db
            .prepare(&format!("SELECT * FROM {table}"))
            .unwrap();
        let cols = stmt.column_count();
        let rows = stmt
            .query_map([], |r| {
                (0..cols)
                    .map(|i| r.get::<_, Value>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        for row in rows {
            for value in row {
                if let Value::Text(s) = value {
                    for forbidden in [
                        "synthetic-payload",
                        "synthetic-source",
                        "new-key",
                        "/home/",
                        "C:\\Users\\",
                        "SELECT ",
                        "INSERT ",
                    ] {
                        assert!(!s.contains(forbidden));
                    }
                }
            }
        }
    }
    let columns: Vec<String> = writer
        .db
        .prepare("SELECT name FROM pragma_table_info('acp_revisions') ORDER BY cid")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(
        columns,
        ["kind", "project", "record_id", "revision", "deleted"]
    );
    assert_eq!(format!("{writer:?}"), "SyntheticWriter(redacted)");
    assert_eq!(format!("{w:?}"), "SyntheticWorkspace(redacted)");
    let error = crate::sql_error(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
        Some("private-command-value /private/path SELECT key".into()),
    ));
    assert_eq!(error.to_string(), "IoFailure");
}
#[test]
fn python_independently_compares_all_legacy_fields_on_three_v2_candidates_readonly() {
    use std::{
        io::Write,
        process::{Command as Process, Stdio},
    };
    for shape in SHAPES {
        let w = ready(shape);
        let root = w
            .path(Role::Candidate)
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .canonicalize()
            .unwrap();
        let before: [Vec<u8>; 3] = [Role::Source, Role::Backup, Role::Candidate]
            .map(|r| std::fs::read(w.path(r)).unwrap());
        let mut child =
            Process::new(std::env::var_os("ACP_TEST_PYTHON").unwrap_or_else(|| "python".into()))
                .args(["-c", include_str!("../tests/readonly_parity.py")])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
        let payload =
            serde_json::to_vec(&serde_json::json!({"root":root,"binding":w.binding()})).unwrap();
        child.stdin.take().unwrap().write_all(&payload).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "synthetic Python verifier failed");
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "PASS");
        assert!(output.stderr.is_empty());
        for (i, role) in [Role::Source, Role::Backup, Role::Candidate]
            .iter()
            .enumerate()
        {
            assert_eq!(std::fs::read(w.path(*role)).unwrap(), before[i]);
        }
    }
}
#[cfg(windows)]
#[test]
fn windows_readonly_attribute_is_refused_without_weakening_original_permissions() {
    let w = ready(FixtureShape::Version1);
    let path = w.path(Role::Candidate);
    let original = std::fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&path, readonly).unwrap();
    assert_eq!(w.writer().unwrap_err(), Error::UnsafeWorkspace);
    std::fs::set_permissions(path, original).unwrap();
    assert!(w.writer().is_ok());
}
// An ignored helper is selected explicitly only by the subprocess test. The
// bounded private stdin carries a fresh parent temp directory, never a DB path.
#[test]
#[ignore = "private child worker; invoked explicitly by the genuine kill test"]
fn subprocess_worker() {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::io::stdin().take(4097).read_to_end(&mut bytes).unwrap();
    assert!(bytes.len() <= 4096);
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let parent = std::path::Path::new(value["parent"].as_str().unwrap());
    let shape = match value["shape"].as_u64().unwrap() {
        0 => FixtureShape::LegacyV0,
        1 => FixtureShape::FingerprintedV0,
        _ => FixtureShape::Version1,
    };
    let mut w = SyntheticWorkspace::for_test_parent(parent, shape).unwrap();
    w.prepare().unwrap();
    let fault = if value["after_commit"].as_bool().unwrap() {
        Fault::PauseAfterCommit
    } else {
        Fault::PauseBeforeCommit
    };
    let r = request(
        &w,
        "child-confirm",
        transition(
            5,
            Scope::Project {
                name: "alpha".into(),
            },
            1,
            0,
        ),
    );
    let mut writer = w.writer().unwrap();
    let _ = writer.execute_inner(&r, fault);
    panic!("worker must pause at its selected boundary");
}
#[test]
fn genuine_child_kill_before_and_after_commit_keeps_v2_transaction_atomic() {
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Child, Command as Process, Stdio},
    };
    struct Guard(Child);
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    for (shape_index, _) in SHAPES.iter().enumerate() {
        for after in [false, true] {
            let parent = tempfile::Builder::new()
                .prefix("acp-m1-parent-")
                .tempdir()
                .unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(parent.path(), std::fs::Permissions::from_mode(0o700))
                    .unwrap();
            }
            let mut child = Guard(
                Process::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "tests::subprocess_worker",
                        "--ignored",
                        "--nocapture",
                    ])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap(),
            );
            let payload=serde_json::to_vec(&serde_json::json!({"parent":parent.path(),"shape":shape_index,"after_commit":after})).unwrap();
            child.0.stdin.take().unwrap().write_all(&payload).unwrap();
            let stdout = child.0.stdout.take().unwrap();
            let (send, receive) = std::sync::mpsc::channel();
            let reader = std::thread::spawn(move || {
                let ready = BufReader::new(stdout)
                    .lines()
                    .map_while(std::result::Result::ok)
                    .any(|line| line == "READY");
                let _ = send.send(ready);
            });
            let ready = receive.recv_timeout(Duration::from_secs(15));
            child.0.kill().unwrap();
            let exit = child.0.wait().unwrap();
            reader.join().unwrap();
            assert!(ready.is_ok_and(|r| r), "child readiness timeout/failure");
            assert!(!exit.success());
            let roots: Vec<_> = std::fs::read_dir(parent.path())
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            assert_eq!(roots.len(), 1);
            let path = roots[0].join("candidate/profile.sqlite3");
            // SQLite is allowed to recover the dead child's owned hot journal here.
            let recovered = workspace::connect(&path, false).unwrap();
            let binding: Binding = recovered
                .query_row(
                    "SELECT profile_id,generation,owner_epoch FROM acp_profile",
                    [],
                    |r| {
                        Ok(Binding {
                            profile_id: r.get(0)?,
                            generation: r.get(1)?,
                            owner_epoch: r.get(2)?,
                        })
                    },
                )
                .unwrap();
            crate::writer::verify(&recovered, &binding).unwrap();
            assert_eq!(
                status(&recovered, 5),
                if after { "confirmed" } else { "suggested" }
            );
            for id in [5, 6, 7] {
                assert_eq!(rev(&recovered, id), (if after { 2 } else { 1 }, false));
            }
            for id in [6, 7] {
                assert_eq!(
                    status(&recovered, id),
                    if after { "deprecated" } else { "confirmed" }
                );
            }
            let (sequence, receipts): (i64, i64) = recovered
                .query_row(
                    "SELECT commit_sequence,(SELECT count(*) FROM acp_receipts) FROM acp_profile",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!((sequence, receipts), if after { (1, 1) } else { (0, 0) });
            let source =
                workspace::connect(&roots[0].join("source/profile.sqlite3"), true).unwrap();
            let backup =
                workspace::connect(&roots[0].join("backup/profile.sqlite3"), true).unwrap();
            assert_eq!(
                schema::records(&source).unwrap(),
                schema::records(&backup).unwrap()
            );
        }
    }
}
#[test]
fn initial_candidate_verification_rejects_changed_technical_import_metadata() {
    for shape in SHAPES {
        for sql in [
            "UPDATE acp_revisions SET revision=2 WHERE kind='preference' AND record_id='1'",
            "UPDATE acp_profile SET receipt_limit=2",
        ] {
            let mut w = SyntheticWorkspace::new(shape).unwrap();
            assert_eq!(
                w.prepare_inner(Fault::AfterCommit),
                Err(Error::OutcomeUnknown)
            );
            let db = workspace::connect(&w.path(Role::Candidate), false).unwrap();
            db.execute_batch(sql).unwrap();
            let before = dump(&db);
            drop(db);
            assert_eq!(w.verify_candidate(), Err(Error::Verification));
            assert_eq!(w.writer().unwrap_err(), Error::Verification);
            let db = workspace::connect(&w.path(Role::Candidate), true).unwrap();
            assert_eq!(dump(&db), before);
            w.verify_prior().unwrap();
        }
    }
}
