use super::support::{Fixture, suggest};
use precept_ipc::Error;
use serde_json::json;

#[test]
fn s3_intervening_commit_changes_page_watermark_without_snapshot_claim() {
    let f = Fixture::created();
    let binding = f.daemon.enable();
    for index in 0..13 {
        f.daemon
            .mutate(&binding, &format!("watermark-{index}"), suggest())
            .unwrap();
    }
    let first = f
        .daemon
        .control(
            "profile.review.queue",
            json!({"scope":{"kind":"global"},"after_id":0}),
        )
        .unwrap();
    assert_eq!(first["records"].as_array().unwrap().len(), 12);
    assert_eq!(first["next_after_id"], 12);
    // The count is the whole scope (the queue pages it 12 at a time), at the
    // same watermark; an empty project scope counts zero, not absent.
    let count = f
        .daemon
        .control("profile.review.count", json!({"scope":{"kind":"global"}}))
        .unwrap();
    assert_eq!(count["suggested"], 13);
    assert_eq!(count["commit_sequence"], first["commit_sequence"]);
    assert_eq!(
        f.daemon.control(
            "profile.review.count",
            json!({"scope":{"kind":"global"},"x":1})
        ),
        Err(Error::InvalidRequest)
    );
    f.daemon.mutate(&binding, "intervening", suggest()).unwrap();
    assert_eq!(
        f.daemon
            .control("profile.review.count", json!({"scope":{"kind":"global"}}))
            .unwrap()["suggested"],
        14
    );
    let second = f
        .daemon
        .control(
            "profile.review.queue",
            json!({"scope":{"kind":"global"},"after_id":first["next_after_id"]}),
        )
        .unwrap();
    let ids: Vec<_> = second["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| record["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, [13, 14]);
    assert_eq!(
        second["commit_sequence"].as_i64().unwrap(),
        first["commit_sequence"].as_i64().unwrap() + 1
    );
    assert!(second["next_after_id"].is_null());
}
