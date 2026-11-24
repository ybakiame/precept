use super::support::*;
use super::*;

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
