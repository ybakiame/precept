use super::*;
use crate::{files, profile::connect};
use acp_store::{SqliteStore, profile as schema};
use rusqlite::{Connection, types::Value};
use std::{
    fs,
    path::Path,
    sync::Barrier,
    time::{Duration, Instant},
};
fn fresh() -> (tempfile::TempDir, Profile) {
    let t = tempfile::tempdir().unwrap();
    files::private_test_parent(t.path());
    let p = Profile::create(&t.path().join("profile.sqlite3")).unwrap();
    (t, p)
}
fn request(p: &Profile, id: &str, command: Command) -> Request {
    Request {
        version: 2,
        binding: p.binding(),
        operation_id: id.into(),
        command,
    }
}
fn suggest(scope: Scope) -> Command {
    Command::SuggestPreference {
        scope,
        key: "formatter".into(),
        value: "synthetic".into(),
        source: "fixture".into(),
        origin: acp_core::operator::Origin::AgentReported,
        expected: Expected::Absent,
    }
}
/// The exact digest of the synthetic proposal `suggest()` inserts.
fn proposal(scope: &Scope) -> String {
    acp_core::operator::digest_hex(&acp_core::operator::proposal_digest(
        scope,
        match scope {
            Scope::Project { name } => Some(name.as_str()),
            Scope::Global => None,
        },
        "formatter",
        "synthetic",
        acp_core::operator::Origin::AgentReported,
        "fixture",
    ))
}
fn project_scope(name: &str) -> Scope {
    Scope::Project { name: name.into() }
}
fn register(p: &Profile, name: &str) {
    let root = p.path.parent().unwrap().join(name);
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap().to_str().unwrap().to_owned();
    p.writer()
        .unwrap()
        .execute(&request(
            p,
            &format!("register-{name}"),
            Command::RegisterProject {
                name: name.into(),
                root,
                expected: Expected::Absent,
            },
        ))
        .unwrap();
}
fn ready() -> (tempfile::TempDir, Profile) {
    let (t, p) = fresh();
    register(&p, "alpha");
    register(&p, "beta");
    (t, p)
}
fn inserted(p: &Profile, op: &str, scope: Scope) -> i64 {
    p.writer()
        .unwrap()
        .execute(&request(p, op, suggest(scope)))
        .unwrap()
        .affected[0]
        .record_id
        .parse()
        .unwrap()
}
fn transition(id: i64, scope: Scope, revision: i64, action: usize) -> Command {
    let proposal_digest = proposal(&scope);
    let channel = acp_core::operator::Channel::Cli;
    match action {
        0 => Command::ConfirmPreference {
            id,
            scope,
            proposal_digest,
            channel,
            expected: Expected::Revision(revision),
        },
        1 => Command::RejectPreference {
            id,
            scope,
            proposal_digest,
            channel,
            expected: Expected::Revision(revision),
        },
        _ => Command::DeprecatePreference {
            id,
            scope,
            proposal_digest,
            channel,
            expected: Expected::Revision(revision),
        },
    }
}
fn handoff(project: &str, revision: Expected) -> Command {
    Command::PutHandoff{project:project.into(),task_id:"shared".into(),objective:"synthetic objective".into(),completed:"".into(),next_action:"inspect".into(),validation_json:"[ {\"check\":\"unit\",\"status\":\"NOT_RUN\",\"extra\":{\"opaque\":[1,null,\"合成\"]}} ]".into(),snapshot:Snapshot{base_sha:String::new(),branch:String::new(),state:"unknown".into(),fingerprint:None},expected:revision}
}
fn dump(db: &Connection) -> Vec<Vec<Vec<Value>>> {
    [
        "SELECT * FROM acp_profile",
        "SELECT * FROM projects ORDER BY name",
        "SELECT * FROM preferences ORDER BY id",
        "SELECT * FROM handoffs ORDER BY project,task_id",
        "SELECT * FROM acp_receipts ORDER BY commit_sequence",
        "SELECT * FROM acp_review_decisions ORDER BY id",
    ]
    .iter()
    .map(|q| {
        let mut s = db.prepare(q).unwrap();
        let n = s.column_count();
        s.query_map([], |r| {
            (0..n)
                .map(|i| r.get(i))
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
    })
    .collect()
}
fn validated(p: &Profile) {
    SqliteStore::open(&p.path).unwrap();
}
#[test]
fn fresh_identity_schema_counter_and_no_legacy_objects() {
    let (_t, p) = fresh();
    let r = SqliteStore::open(&p.path).unwrap();
    let m = r.metadata().unwrap();
    assert_eq!(m.binding, p.binding());
    assert_ne!(m.binding.profile_id, m.binding.generation);
    assert_eq!(
        (
            m.next_preference_id,
            m.commit_sequence,
            m.binding.owner_epoch
        ),
        (1, 0, 0)
    );
    assert!(r.read_project("missing").is_err());
    let db = connect(&p.path, true).unwrap();
    let n: i64 = db
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name IN ('sqlite_sequence','acp_revisions')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 0);
}
#[test]
fn explicit_creation_never_overwrites_existing_objects_or_sidecars() {
    let t = tempfile::tempdir().unwrap();
    files::private_test_parent(t.path());
    let p = t.path().join("target");
    fs::write(&p, b"sentinel").unwrap();
    assert!(matches!(Profile::create(&p), Err(Error::Collision)));
    assert_eq!(fs::read(&p).unwrap(), b"sentinel");
    let dir = t.path().join("existing-dir");
    fs::create_dir(&dir).unwrap();
    assert!(matches!(Profile::create(&dir), Err(Error::Collision)));
    for suffix in ["-wal", "-shm", "-journal"] {
        let target = t.path().join(format!("db{suffix}"));
        fs::write(&target, b"sidecar sentinel").unwrap();
        assert!(matches!(
            Profile::create(&t.path().join("db")),
            Err(Error::Collision)
        ));
        assert_eq!(fs::read(&target).unwrap(), b"sidecar sentinel");
        fs::remove_file(target).unwrap();
    }
    assert!(Profile::create(Path::new("relative")).is_err());
    for name in [
        "daemon.lock",
        "operator.cap",
        "operator-write.cap",
        "daemon.sock",
    ] {
        assert!(Profile::create(&t.path().join(name)).is_err());
    }
}
#[test]
#[cfg(windows)]
fn windows_mixed_case_reservations_and_collisions_preserve_neighbors() {
    let t = tempfile::tempdir().unwrap();
    files::private_test_parent(t.path());
    // Absent targets must fail lexical checks, independently of filesystem
    // existence; checking only existing aliases would miss these cases.
    for name in [
        "Daemon.Lock",
        "DAEMON.SOCK",
        "Operator.Cap",
        "OPERATOR-Write.CAP",
        ".ACP-Create-synthetic",
        "profile-WaL",
        "profile-ShM",
        "profile-JoUrNaL",
    ] {
        let target = t.path().join(name);
        assert!(matches!(Profile::create(&target), Err(Error::Collision)));
        assert!(!target.exists());
    }
    for name in [
        "Daemon.Lock",
        "DAEMON.SOCK",
        "Operator.Cap",
        "OPERATOR-Write.CAP",
        "target-WaL",
        "target-ShM",
        "target-JoUrNaL",
        "TaRgEt",
    ] {
        let neighbor = t.path().join(name);
        fs::write(&neighbor, b"synthetic sentinel").unwrap();
        assert!(matches!(
            Profile::create(&t.path().join("target")),
            Err(Error::Collision)
        ));
        assert_eq!(fs::read(&neighbor).unwrap(), b"synthetic sentinel");
        fs::remove_file(neighbor).unwrap();
    }
    // Folding reservations must neither rename a valid target nor overmatch
    // names containing a reserved word away from the reserved boundary.
    let target = t.path().join("Profile-WaL.sqlite3");
    let p = Profile::create(&target).unwrap();
    validated(&p);
    assert_eq!(p.path.file_name(), target.file_name());
    assert_eq!(fs::read_dir(t.path()).unwrap().count(), 1);
}
#[test]
fn concurrent_creators_have_one_no_replace_winner() {
    let t = tempfile::tempdir().unwrap();
    files::private_test_parent(t.path());
    let target = t.path().join("race");
    let barrier = Barrier::new(2);
    let result = std::thread::scope(|s| {
        let a = s.spawn(|| {
            barrier.wait();
            Profile::create(&target)
        });
        let b = s.spawn(|| {
            barrier.wait();
            Profile::create(&target)
        });
        [a.join().unwrap(), b.join().unwrap()]
    });
    assert_eq!(result.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        result
            .iter()
            .filter(|r| matches!(r, Err(Error::Collision)))
            .count(),
        1
    );
    let p = result.into_iter().find_map(Result::ok).unwrap();
    validated(&p);
}
#[test]
fn publication_unknown_preserves_valid_published_file() {
    let t = tempfile::tempdir().unwrap();
    files::private_test_parent(t.path());
    let target = t.path().join("published");
    assert!(matches!(
        Profile::create_inner(&target, Fault::AfterCommit),
        Err(Error::OutcomeUnknown)
    ));
    let before = fs::read(&target).unwrap();
    SqliteStore::open(&target).unwrap();
    assert!(matches!(Profile::create(&target), Err(Error::Collision)));
    assert_eq!(fs::read(&target).unwrap(), before);
    assert_eq!(fs::read_dir(t.path()).unwrap().count(), 1);
}
#[test]
fn registration_uniqueness_and_expected_absence_are_atomic() {
    let (_t, p) = ready();
    let mut w = p.writer().unwrap();
    let before = dump(&w.db);
    let root = p
        .path
        .parent()
        .unwrap()
        .join("alpha")
        .canonicalize()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    for name in ["alpha", "other"] {
        assert_eq!(
            w.execute(&request(
                &p,
                &format!("duplicate-{name}"),
                Command::RegisterProject {
                    name: name.into(),
                    root: root.clone(),
                    expected: Expected::Absent
                }
            )),
            Err(Error::Conflict)
        );
        assert_eq!(dump(&w.db), before);
    }
    assert_eq!(
        w.execute(&request(
            &p,
            "unknown-project",
            suggest(project_scope("unknown"))
        )),
        Err(Error::Conflict)
    );
    assert_eq!(dump(&w.db), before);
}
#[test]
fn full_strict_transition_matrix_and_denial_invariance() {
    for state in 0..4 {
        for action in 0..3 {
            let (_t, p) = fresh();
            let id = inserted(&p, "suggest", Scope::Global);
            let revision = match state {
                0 => 1,
                1 | 2 => {
                    p.writer()
                        .unwrap()
                        .execute(&request(
                            &p,
                            "prepare",
                            transition(id, Scope::Global, 1, state - 1),
                        ))
                        .unwrap();
                    2
                }
                _ => {
                    p.writer()
                        .unwrap()
                        .execute(&request(
                            &p,
                            "prepare-confirm",
                            transition(id, Scope::Global, 1, 0),
                        ))
                        .unwrap();
                    p.writer()
                        .unwrap()
                        .execute(&request(
                            &p,
                            "prepare-deprecate",
                            transition(id, Scope::Global, 2, 2),
                        ))
                        .unwrap();
                    3
                }
            };
            let mut w = p.writer().unwrap();
            let before = dump(&w.db);
            let result = w.execute(&request(
                &p,
                "edge",
                transition(id, Scope::Global, revision, action),
            ));
            if matches!((state, action), (0, 0) | (0, 1) | (1, 2)) {
                result.unwrap();
                schema::verify(&w.db).unwrap();
            } else {
                assert_eq!(result, Err(Error::InvalidTransition));
                assert_eq!(dump(&w.db), before);
            }
        }
    }
}
#[test]
fn confirming_peers_is_atomic_and_scope_exact() {
    let (_t, p) = ready();
    let mut keep = Vec::new();
    for (name, scope) in [
        ("global", Scope::Global),
        ("beta", project_scope("beta")),
        ("alpha", project_scope("alpha")),
    ] {
        let id = inserted(&p, &format!("suggest-{name}"), scope.clone());
        p.writer()
            .unwrap()
            .execute(&request(
                &p,
                &format!("confirm-{name}"),
                transition(id, scope, 1, 0),
            ))
            .unwrap();
        keep.push(id);
    }
    let id = inserted(&p, "replacement", project_scope("alpha"));
    let mut w = p.writer().unwrap();
    let before = dump(&w.db);
    let req = request(&p, "replace", transition(id, project_scope("alpha"), 1, 0));
    assert_eq!(
        w.execute_inner(&req, Fault::BeforeCommit),
        Err(Error::BeforeCommit)
    );
    assert_eq!(dump(&w.db), before);
    let receipt = w.execute(&req).unwrap();
    assert_eq!(receipt.affected.len(), 2);
    for (i, id) in keep.iter().enumerate() {
        let value: (String, i64) =
            w.db.query_row(
                "SELECT status,revision FROM preferences WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            value,
            if i == 2 {
                ("deprecated".into(), 3)
            } else {
                ("confirmed".into(), 2)
            }
        );
    }
    validated(&p);
}
#[test]
fn decision_history_records_exact_transitions_demotions_and_erasure() {
    let (_t, p) = ready();
    let scope = project_scope("alpha");
    let first = inserted(&p, "first", scope.clone());
    let second = inserted(&p, "second", scope.clone());
    let mut w = p.writer().unwrap();
    let one = w
        .execute(&request(
            &p,
            "confirm-first",
            transition(first, scope.clone(), 1, 0),
        ))
        .unwrap();
    let two = w
        .execute(&request(
            &p,
            "confirm-second",
            transition(second, scope.clone(), 1, 0),
        ))
        .unwrap();
    let history = |db: &Connection| -> Vec<(i64, i64, String, i64, i64, i64, String)> {
        let mut stmt = db
            .prepare("SELECT id,preference_id,action,from_revision,to_revision,receipt_commit_sequence,channel FROM acp_review_decisions ORDER BY id")
            .unwrap();
        stmt.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
    };
    assert_eq!(
        history(&w.db),
        vec![
            (
                1,
                first,
                "confirm".into(),
                1,
                2,
                one.commit_sequence,
                "cli".into()
            ),
            (
                2,
                first,
                "demote".into(),
                2,
                3,
                two.commit_sequence,
                "cli".into()
            ),
            (
                3,
                second,
                "confirm".into(),
                1,
                2,
                two.commit_sequence,
                "cli".into()
            ),
        ]
    );
    let next: i64 =
        w.db.query_row("SELECT next_review_id FROM acp_profile", [], |r| r.get(0))
            .unwrap();
    assert_eq!(next, 4);
    schema::verify(&w.db).unwrap();
    // An explicit deprecation chains from the confirm decision's to_revision.
    let three = w
        .execute(&request(
            &p,
            "deprecate-second",
            transition(second, scope.clone(), 2, 2),
        ))
        .unwrap();
    assert_eq!(
        history(&w.db).last().unwrap(),
        &(
            4,
            second,
            "deprecate".into(),
            2,
            3,
            three.commit_sequence,
            "cli".into()
        )
    );
    schema::verify(&w.db).unwrap();
    // A substituted proposal is refused before any row, counter or receipt moves.
    let third = inserted(&p, "third", scope.clone());
    let mut substituted = transition(third, scope.clone(), 1, 0);
    if let Command::ConfirmPreference {
        proposal_digest, ..
    } = &mut substituted
    {
        *proposal_digest = "0".repeat(64);
    }
    let before = dump(&w.db);
    assert_eq!(
        w.execute(&request(&p, "substituted", substituted)),
        Err(Error::ReviewMismatch)
    );
    assert_eq!(dump(&w.db), before);
    // Erasure removes the decision history with the record; receipts remain.
    let erased = w
        .execute(&request(
            &p,
            "delete-first",
            Command::DeletePreference {
                id: first,
                scope: scope.clone(),
                proposal_digest: proposal(&scope),
                channel: acp_core::operator::Channel::Cli,
                expected: Expected::Revision(3),
            },
        ))
        .unwrap();
    assert!(history(&w.db).iter().all(|row| row.1 != first));
    // The refused substitution committed nothing: one receipt per commit.
    assert_eq!(
        w.db.query_row("SELECT count(*) FROM acp_receipts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        erased.commit_sequence
    );
    schema::verify(&w.db).unwrap();
    validated(&p);
}
#[test]
fn scopes_cas_and_binding_denials_do_not_change_any_logical_row() {
    let (_t, p) = ready();
    let id = inserted(&p, "suggest", project_scope("alpha"));
    let mut w = p.writer().unwrap();
    for (command, error) in [
        (
            transition(id, project_scope("beta"), 1, 0),
            Error::ScopeMismatch,
        ),
        (transition(id, Scope::Global, 1, 0), Error::ScopeMismatch),
        (
            transition(id, project_scope("alpha"), 2, 0),
            Error::Conflict,
        ),
        (transition(999, Scope::Global, 1, 0), Error::Conflict),
    ] {
        let before = dump(&w.db);
        assert_eq!(w.execute(&request(&p, "denied", command)), Err(error));
        assert_eq!(dump(&w.db), before);
    }
    for field in 0..3 {
        let mut req = request(&p, "wrong-binding", suggest(Scope::Global));
        match field {
            0 => req.binding.profile_id = "0".repeat(32),
            1 => req.binding.generation = "0".repeat(32),
            _ => req.binding.owner_epoch = 1,
        }
        let before = dump(&w.db);
        assert_eq!(
            w.execute(&req),
            Err(if field == 1 {
                Error::StaleGeneration
            } else {
                Error::OwnerMismatch
            })
        );
        assert_eq!(dump(&w.db), before);
    }
}
#[test]
fn independently_connected_cas_race_has_one_winner() {
    let (_t, p) = fresh();
    let id = inserted(&p, "suggest", Scope::Global);
    let a = p.writer().unwrap();
    let b = p.writer().unwrap();
    let barrier = Barrier::new(2);
    let results = std::thread::scope(|s| {
        let ba = &barrier;
        let p = &p;
        let one = s.spawn(move || {
            let mut w = a;
            ba.wait();
            w.execute(&request(p, "race-a", transition(id, Scope::Global, 1, 0)))
        });
        let two = s.spawn(move || {
            let mut w = b;
            ba.wait();
            w.execute(&request(p, "race-b", transition(id, Scope::Global, 1, 1)))
        });
        [one.join().unwrap(), two.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(Error::Conflict)))
            .count(),
        1
    );
    validated(&p);
}
#[test]
fn deletion_preserves_receipts_and_never_reuses_ids() {
    let (_t, p) = fresh();
    let id = inserted(&p, "first", Scope::Global);
    let deletion = request(
        &p,
        "delete",
        Command::DeletePreference {
            id,
            scope: Scope::Global,
            proposal_digest: proposal(&Scope::Global),
            channel: acp_core::operator::Channel::Cli,
            expected: Expected::Revision(1),
        },
    );
    let mut w = p.writer().unwrap();
    let receipt = w.execute(&deletion).unwrap();
    assert!(receipt.affected[0].deleted);
    assert_eq!(receipt.affected[0].revision, 2);
    drop(w);
    validated(&p);
    assert_eq!(p.writer().unwrap().execute(&deletion).unwrap(), receipt);
    assert_eq!(inserted(&p, "second", Scope::Global), id + 1);
    let db = connect(&p.path, false).unwrap();
    db.execute("UPDATE acp_profile SET next_preference_id=?1", [id])
        .unwrap();
    drop(db);
    assert!(p.writer().is_err());
    assert!(SqliteStore::open(&p.path).is_err());
}
#[test]
fn raw_handoff_json_project_binding_and_revision_are_preserved() {
    let (_t, p) = ready();
    let a = request(&p, "handoff-a", handoff("alpha", Expected::Absent));
    let b = request(&p, "handoff-b", handoff("beta", Expected::Absent));
    let mut w = p.writer().unwrap();
    w.execute(&a).unwrap();
    w.execute(&b).unwrap();
    let before = dump(&w.db);
    assert_eq!(
        w.execute(&request(
            &p,
            "stale",
            handoff("alpha", Expected::Revision(2))
        )),
        Err(Error::Conflict)
    );
    assert_eq!(dump(&w.db), before);
    let r = w
        .execute(&request(
            &p,
            "update",
            handoff("alpha", Expected::Revision(1)),
        ))
        .unwrap();
    assert_eq!(r.affected[0].revision, 2);
    let raw: String =
        w.db.query_row(
            "SELECT validation_json FROM handoffs WHERE project='alpha'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    if let Command::PutHandoff {
        validation_json, ..
    } = &a.command
    {
        assert_eq!(&raw, validation_json)
    }
    assert_eq!(
        SqliteStore::open(&p.path)
            .unwrap()
            .handoff_revision("beta", "shared")
            .unwrap(),
        1
    );
}
#[test]
fn lost_response_and_canonical_replay_precede_cas() {
    let (_t, p) = fresh();
    let req = request(&p, "once", suggest(Scope::Global));
    let mut w = p.writer().unwrap();
    assert_eq!(
        w.execute_inner(&req, Fault::AfterCommit),
        Err(Error::OutcomeUnknown)
    );
    let before = dump(&w.db);
    drop(w);
    let mut w = p.writer().unwrap();
    let receipt = w.execute(&req).unwrap();
    assert_eq!(receipt.commit_sequence, 1);
    assert_eq!(dump(&w.db), before);
    let json: serde_json::Value =
        serde_json::from_slice(&serde_json::to_vec(&req).unwrap()).unwrap();
    let parsed =
        Request::from_json(serde_json::to_string_pretty(&json).unwrap().as_bytes()).unwrap();
    assert_eq!(w.execute(&parsed).unwrap(), receipt);
    assert_eq!(dump(&w.db), before);
    let mut changed = req.clone();
    if let Command::SuggestPreference { value, .. } = &mut changed.command {
        *value = "changed".into()
    }
    assert_eq!(w.execute(&changed), Err(Error::IdempotencyConflict));
    assert_eq!(dump(&w.db), before);
}
#[test]
fn raw_json_byte_change_is_an_idempotency_conflict() {
    let (_t, p) = ready();
    let req = request(&p, "raw", handoff("alpha", Expected::Absent));
    let mut w = p.writer().unwrap();
    w.execute(&req).unwrap();
    let before = dump(&w.db);
    let mut changed = req.clone();
    if let Command::PutHandoff {
        validation_json, ..
    } = &mut changed.command
    {
        *validation_json = serde_json::from_str::<serde_json::Value>(validation_json)
            .unwrap()
            .to_string();
    }
    assert_eq!(w.execute(&changed), Err(Error::IdempotencyConflict));
    assert_eq!(dump(&w.db), before);
}
#[test]
fn injected_precommit_failures_rollback_data_counter_revisions_and_receipts() {
    for fault in [
        Fault::BeforeCommit,
        Fault::AfterData,
        Fault::AfterRevisions,
        Fault::StorageFull,
    ] {
        let (_t, p) = fresh();
        let mut w = p.writer().unwrap();
        let before = dump(&w.db);
        let req = request(&p, "fault", suggest(Scope::Global));
        assert_eq!(
            w.execute_inner(&req, fault),
            Err(if fault == Fault::StorageFull {
                Error::StorageFull
            } else {
                Error::BeforeCommit
            })
        );
        assert_eq!(dump(&w.db), before);
        assert_eq!(w.execute(&req).unwrap().affected[0].record_id, "1");
        validated(&p);
    }
}
#[test]
fn real_sqlite_lock_contention_is_bounded_and_retry_is_explicit() {
    let (_t, p) = fresh();
    let mut w = p.writer().unwrap();
    let db = connect(&p.path, false).unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    let start = Instant::now();
    assert_eq!(
        w.execute(&request(&p, "busy", suggest(Scope::Global))),
        Err(Error::Busy)
    );
    assert!(start.elapsed() < Duration::from_secs(3));
    db.execute_batch("ROLLBACK").unwrap();
    assert_eq!(
        w.execute(&request(&p, "busy", suggest(Scope::Global)))
            .unwrap()
            .commit_sequence,
        1
    );
}
#[test]
fn sqlite_page_capacity_failure_rolls_back_current_command() {
    let (_t, p) = fresh();
    let mut w = p.writer().unwrap();
    let pages: i64 =
        w.db.query_row("PRAGMA page_count", [], |r| r.get(0))
            .unwrap();
    w.db.pragma_update(None, "max_page_count", pages).unwrap();
    let mut hit = false;
    for i in 0..64 {
        let before = dump(&w.db);
        let mut command = suggest(Scope::Global);
        if let Command::SuggestPreference { value, .. } = &mut command {
            *value = "合".repeat(4096)
        }
        let result = w.execute(&request(&p, &format!("full-{i}"), command));
        if result == Err(Error::StorageFull) {
            assert_eq!(dump(&w.db), before);
            hit = true;
            break;
        }
        result.unwrap();
    }
    assert!(hit);
    schema::verify(&w.db).unwrap();
}
#[test]
fn allocator_overflow_refuses_without_wraparound() {
    let (_t, p) = fresh();
    inserted(&p, "initial", Scope::Global);
    let mut w = p.writer().unwrap();
    w.db.execute("UPDATE acp_profile SET next_preference_id=?1", [i64::MAX])
        .unwrap();
    let before = dump(&w.db);
    assert_eq!(
        w.execute(&request(&p, "overflow", suggest(Scope::Global))),
        Err(Error::Capacity)
    );
    assert_eq!(dump(&w.db), before);
}
#[test]
fn malformed_requests_and_text_bounds_are_sanitized_noops() {
    let (_t, p) = fresh();
    let mut w = p.writer().unwrap();
    let before = dump(&w.db);
    for bytes in [b"{}".as_slice(), b"[]", b"not json"] {
        assert!(Request::from_json(bytes).is_err());
    }
    let mut value = serde_json::to_value(request(&p, "invalid", suggest(Scope::Global))).unwrap();
    value["extra"] = true.into();
    assert!(Request::from_json(&serde_json::to_vec(&value).unwrap()).is_err());
    for value in ["", "  ", "nul\0value"] {
        let mut command = suggest(Scope::Global);
        if let Command::SuggestPreference { value: v, .. } = &mut command {
            *v = value.into()
        }
        assert_eq!(
            w.execute(&request(&p, "invalid", command)),
            Err(Error::InvalidCommand)
        );
    }
    let mut command = suggest(Scope::Global);
    if let Command::SuggestPreference { value, .. } = &mut command {
        *value = "a".repeat(4097)
    }
    assert_eq!(
        w.execute(&request(&p, "oversized", command)),
        Err(Error::InvalidCommand)
    );
    // The retired v1 envelope and non-canonical digests are refused, not read.
    let mut legacy = request(&p, "legacy", suggest(Scope::Global));
    legacy.version = 1;
    assert_eq!(w.execute(&legacy), Err(Error::InvalidCommand));
    for bad in ["0".repeat(63), "A".repeat(64), "g".repeat(64)] {
        let mut command = transition(1, Scope::Global, 1, 0);
        if let Command::ConfirmPreference {
            proposal_digest, ..
        } = &mut command
        {
            *proposal_digest = bad;
        }
        assert_eq!(
            w.execute(&request(&p, "bad-digest", command)),
            Err(Error::InvalidCommand)
        );
    }
    assert_eq!(dump(&w.db), before);
}
#[test]
fn invalid_handoff_status_snapshot_and_nul_are_refused() {
    let (_t, p) = ready();
    let mut w = p.writer().unwrap();
    let before = dump(&w.db);
    for raw in [
        "{}",
        "not-json",
        "[{\"check\":\"x\",\"status\":\"SUCCESS\"}]",
        "[{\"check\":\"x\",\"status\":\"PASS\",\"extra\":\"\\u0000\"}]",
    ] {
        let mut command = handoff("alpha", Expected::Absent);
        if let Command::PutHandoff {
            validation_json, ..
        } = &mut command
        {
            *validation_json = raw.into()
        }
        assert_eq!(
            w.execute(&request(&p, "bad-handoff", command)),
            Err(Error::InvalidCommand)
        );
    }
    let mut command = handoff("alpha", Expected::Absent);
    if let Command::PutHandoff { snapshot, .. } = &mut command {
        snapshot.state = "verified".into()
    }
    assert_eq!(
        w.execute(&request(&p, "bad-snapshot", command)),
        Err(Error::InvalidCommand)
    );
    assert_eq!(dump(&w.db), before);
}
#[test]
fn replaced_identity_hardlinks_and_bad_permissions_are_refused() {
    let (t, p) = fresh();
    let alias = t.path().join("alias");
    fs::hard_link(&p.path, &alias).unwrap();
    assert!(p.writer().is_err());
    fs::remove_file(alias).unwrap();
    let replacement = t.path().join("replacement");
    let file = files::create_private(&replacement).unwrap();
    drop(file);
    fs::write(&replacement, fs::read(&p.path).unwrap()).unwrap();
    files::check_file(&replacement).unwrap();
    assert_eq!(
        SqliteStore::open(&replacement)
            .unwrap()
            .metadata()
            .unwrap()
            .binding,
        p.binding()
    );
    fs::remove_file(&p.path).unwrap();
    fs::rename(replacement, &p.path).unwrap();
    assert!(p.writer().is_err());
}
#[cfg(unix)]
#[test]
fn symlink_parents_and_nonprivate_modes_are_refused() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let t = tempfile::tempdir().unwrap();
    let alias = t.path().join("alias");
    let parent = t.path().join("parent");
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&parent, &alias).unwrap();
    assert!(Profile::create(&alias.join("db")).is_err());
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(Profile::create(&parent.join("db")).is_err());
    let (_t, p) = fresh();
    fs::set_permissions(&p.path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(p.writer().is_err());
}
#[cfg(windows)]
#[test]
fn inherited_temp_acl_is_not_a_private_profile_parent() {
    let t = tempfile::tempdir().unwrap();
    assert!(Profile::create(&t.path().join("db")).is_err());
    files::private_test_parent(t.path());
    Profile::create(&t.path().join("db")).unwrap();
}
#[test]
fn debug_and_errors_do_not_disclose_private_inputs() {
    let (_t, p) = fresh();
    let req = request(&p, "secret-operation", suggest(Scope::Global));
    assert_eq!(format!("{p:?}"), "Profile(redacted)");
    assert_eq!(format!("{req:?}"), "Request(redacted)");
    let r = p.writer().unwrap().execute(&req).unwrap();
    assert_eq!(format!("{r:?}"), "Receipt(redacted)");
    assert_eq!(format!("{:?}", p.writer().unwrap()), "Writer(redacted)");
    let error = crate::sql_error(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
        Some("private SQL path and payload".into()),
    ));
    assert_eq!(error.to_string(), "IoFailure");
    let raw = serde_json::to_string(&r).unwrap();
    assert!(!raw.contains("synthetic"));
    assert!(!raw.contains("formatter"));
}
#[test]
fn receipt_capacity_still_allows_exact_replay() {
    let (_t, p) = fresh();
    register(&p, "alpha");
    let mut w = p.writer().unwrap();
    let tx = w.db.transaction().unwrap();
    let mut last = None;
    for i in 2..=4096 {
        let mut req = request(
            &p,
            &format!("capacity-{i}"),
            handoff(
                "alpha",
                if i == 2 {
                    Expected::Absent
                } else {
                    Expected::Revision(i - 2)
                },
            ),
        );
        if let Command::PutHandoff {
            validation_json, ..
        } = &mut req.command
        {
            *validation_json = "[]".into()
        }
        let digest = sha2::Sha256::digest(serde_json::to_vec(&req).unwrap()).to_vec();
        let result = Receipt {
            generation: p.binding.generation.clone(),
            operation_id: req.operation_id.clone(),
            commit_sequence: i,
            affected: vec![RecordVersion {
                kind: "handoff".into(),
                project: "alpha".into(),
                record_id: "shared".into(),
                revision: i - 1,
                deleted: false,
            }],
        };
        tx.execute(
            "INSERT INTO acp_receipts VALUES(?1,?2,?3,?4,?5)",
            rusqlite::params![
                req.operation_id,
                p.binding.generation,
                digest,
                i,
                serde_json::to_string(&result).unwrap()
            ],
        )
        .unwrap();
        last = Some((req, result));
    }
    tx.execute_batch("INSERT INTO handoffs VALUES('alpha','shared','synthetic objective','','','unknown',NULL,'','inspect','[]',4095,'2026-01-01T00:00:00Z'); UPDATE acp_profile SET commit_sequence=4096").unwrap();
    tx.commit().unwrap();
    let before = dump(&w.db);
    let (req, result) = last.unwrap();
    assert_eq!(w.execute(&req).unwrap(), result);
    assert_eq!(dump(&w.db), before);
    assert_eq!(
        w.execute(&request(&p, "capacity-new", suggest(Scope::Global))),
        Err(Error::Capacity)
    );
    assert_eq!(dump(&w.db), before);
}
#[test]
fn read_snapshot_remains_consistent_during_a_concurrent_commit() {
    let (_t, p) = fresh();
    let id = inserted(&p, "initial", Scope::Global);
    let reader = connect(&p.path, true).unwrap();
    reader.execute_batch("BEGIN").unwrap();
    schema::verify(&reader).unwrap();
    let mut writer = p.writer().unwrap();
    let req = request(&p, "update", transition(id, Scope::Global, 1, 0));
    assert_eq!(writer.execute(&req), Err(Error::Busy));
    schema::verify(&reader).unwrap();
    reader.execute_batch("ROLLBACK").unwrap();
    writer.execute(&req).unwrap();
    validated(&p);
}
use sha2::Digest;
#[cfg(windows)]
#[test]
fn windows_extended_prefix_keeps_the_same_private_parent_identity() {
    let t = tempfile::tempdir().unwrap();
    files::private_test_parent(t.path());
    let raw = files::check_parent(t.path()).unwrap();
    let canonical = t.path().canonicalize().unwrap();
    assert!(files::check_parent(&canonical).unwrap() == raw);
    let p = Profile::create(&canonical.join("extended.sqlite3")).unwrap();
    validated(&p);
}
#[test]
fn erased_receipts_deleted_ids_and_metadata_drift_never_enable_reuse() {
    for mode in 0..4 {
        let (_t, p) = fresh();
        let id = inserted(&p, "first", Scope::Global);
        p.writer()
            .unwrap()
            .execute(&request(
                &p,
                "delete",
                Command::DeletePreference {
                    id,
                    scope: Scope::Global,
                    proposal_digest: proposal(&Scope::Global),
                    channel: acp_core::operator::Channel::Cli,
                    expected: Expected::Revision(1),
                },
            ))
            .unwrap();
        let mut w = p.writer().unwrap();
        match mode {
            0 => {
                w.db.execute_batch("DELETE FROM acp_receipts WHERE commit_sequence=1")
                    .unwrap();
            }
            1 => {
                w.db.execute_batch(
                    "DELETE FROM acp_receipts;UPDATE acp_profile SET commit_sequence=0",
                )
                .unwrap();
            }
            2 => {
                w.db.execute("UPDATE acp_profile SET next_preference_id=?1", [id])
                    .unwrap();
            }
            _ => {
                let mut r: Receipt = serde_json::from_str(
                    &w.db
                        .query_row::<String, _, _>(
                            "SELECT result_json FROM acp_receipts WHERE commit_sequence=2",
                            [],
                            |r| r.get(0),
                        )
                        .unwrap(),
                )
                .unwrap();
                r.affected[0].deleted = false;
                w.db.execute(
                    "UPDATE acp_receipts SET result_json=?1 WHERE commit_sequence=2",
                    [serde_json::to_string(&r).unwrap()],
                )
                .unwrap();
            }
        }
        let before = dump(&w.db);
        assert_eq!(
            w.execute(&request(&p, "reuse", suggest(Scope::Global))),
            Err(Error::InvalidRecord)
        );
        assert_eq!(dump(&w.db), before);
        assert!(SqliteStore::open(&p.path).is_err());
    }
}
#[test]
fn registration_replay_survives_directory_unavailability() {
    let (_t, p) = fresh();
    let directory = p.path.parent().unwrap().join("project");
    fs::create_dir(&directory).unwrap();
    let req = request(
        &p,
        "register",
        Command::RegisterProject {
            name: "alpha".into(),
            root: directory.canonicalize().unwrap().to_str().unwrap().into(),
            expected: Expected::Absent,
        },
    );
    let mut w = p.writer().unwrap();
    let original = w.execute(&req).unwrap();
    let before = dump(&w.db);
    fs::remove_dir(directory).unwrap();
    assert_eq!(w.execute(&req).unwrap(), original);
    assert_eq!(dump(&w.db), before);
    let mut changed = req.clone();
    changed.operation_id = "register-new".into();
    assert_eq!(w.execute(&changed), Err(Error::InvalidCommand));
    assert_eq!(dump(&w.db), before);
}
#[test]
fn prepublication_failure_removes_only_its_owned_staging_artifacts() {
    let t = tempfile::tempdir().unwrap();
    files::private_test_parent(t.path());
    let neighbor = t.path().join("unrelated");
    fs::write(&neighbor, b"preserve").unwrap();
    let target = t.path().join("fresh.sqlite3");
    assert!(matches!(
        Profile::create_inner(&target, Fault::BeforeCommit),
        Err(Error::BeforeCommit)
    ));
    assert!(!target.exists());
    assert_eq!(fs::read(&neighbor).unwrap(), b"preserve");
    assert_eq!(fs::read_dir(t.path()).unwrap().count(), 1);
    let p = Profile::create(&target).unwrap();
    validated(&p);
}
