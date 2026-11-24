use super::support::*;
use super::*;

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
                channel: precept_core::operator::Channel::Cli,
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
            channel: precept_core::operator::Channel::Cli,
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
