use super::support::*;
use super::*;
use sha2::Digest;

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
