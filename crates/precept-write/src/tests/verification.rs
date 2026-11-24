use super::support::*;
use super::*;

#[test]
fn writer_produced_decision_chain_passes_strict_verification() {
    let (t, p) = ready();
    let r = p
        .writer()
        .unwrap()
        .execute(&request(&p, "suggest-1", suggest(Scope::Global)))
        .unwrap();
    let id = r.affected[0].record_id.parse::<i64>().unwrap();
    let cmd = Command::ConfirmPreference {
        id,
        scope: Scope::Global,
        proposal_digest: proposal(&Scope::Global),
        channel: precept_core::operator::Channel::Cli,
        expected: Expected::Revision(1),
        note: None,
    };
    p.writer()
        .unwrap()
        .execute(&request(&p, "confirm-1", cmd))
        .unwrap();
    validated(&p);
    // Deleting the decision rows breaks the live-revision invariant.
    let db = rusqlite::Connection::open(&p.path).unwrap();
    db.execute(
        "DELETE FROM acp_review_decisions WHERE preference_id=?1",
        [id],
    )
    .unwrap();
    drop(db);
    assert!(SqliteStore::open(&p.path).is_err());
    let _ = t;
}

#[test]
fn writer_confirm_then_deprecate_chain_verifies_and_missing_last_decision_is_refused() {
    let (t, p) = ready();
    let r = p
        .writer()
        .unwrap()
        .execute(&request(&p, "suggest-1", suggest(Scope::Global)))
        .unwrap();
    let id = r.affected[0].record_id.parse::<i64>().unwrap();
    let digest = proposal(&Scope::Global);
    p.writer()
        .unwrap()
        .execute(&request(
            &p,
            "confirm-1",
            Command::ConfirmPreference {
                id,
                scope: Scope::Global,
                proposal_digest: digest.clone(),
                channel: precept_core::operator::Channel::Cli,
                expected: Expected::Revision(1),
                note: None,
            },
        ))
        .unwrap();
    p.writer()
        .unwrap()
        .execute(&request(
            &p,
            "deprecate-1",
            Command::DeprecatePreference {
                id,
                scope: Scope::Global,
                proposal_digest: digest,
                channel: precept_core::operator::Channel::Cli,
                expected: Expected::Revision(2),
                note: None,
            },
        ))
        .unwrap();
    // confirm(1->2) then deprecate(2->3): deprecated at revision 3, two decisions.
    validated(&p);
    let db = rusqlite::Connection::open(&p.path).unwrap();
    let (status, revision): (String, i64) = db
        .query_row(
            "SELECT status,revision FROM preferences WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((status.as_str(), revision), ("deprecated", 3));
    // Removing only the LAST decision leaves the chain ending at revision 2 while
    // the live row is deprecated at revision 3: strict verification must refuse.
    db.execute(
        "DELETE FROM acp_review_decisions WHERE preference_id=?1 AND action='deprecate'",
        [id],
    )
    .unwrap();
    drop(db);
    assert!(SqliteStore::open(&p.path).is_err());
    let _ = t;
}
