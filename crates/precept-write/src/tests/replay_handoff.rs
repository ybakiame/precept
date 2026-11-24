use super::support::*;
use super::*;

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
