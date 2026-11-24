use super::harness::Cli;
use super::support::{Fixture, logical};
use precept_core::operator::{Receipt, RecordVersion, Snapshot};
use precept_ipc::Error;
use precept_write::{Binding, Command, Expected, Request};
use rusqlite::{Connection, params};
use serde_json::json;
use sha2::{Digest, Sha256};

fn saved_handoff(expected: Expected) -> Command {
    Command::PutHandoff {
        project: "alpha".into(),
        task_id: "capacity".into(),
        objective: "Synthetic capacity fixture".into(),
        completed: String::new(),
        next_action: "Inspect current evidence".into(),
        validation_json: r#"[{"check":"synthetic historical","status":"PASS"}]"#.into(),
        snapshot: Snapshot {
            base_sha: String::new(),
            branch: String::new(),
            state: "unknown".into(),
            fingerprint: None,
        },
        expected,
    }
}

/// Contained fixture injection, not 4094 production mutations or native evidence.
/// Build a complete structurally valid revision/receipt chain in one transaction.
fn seed_receipt_ceiling(f: &Fixture, binding: &Binding) -> (Request, Receipt) {
    let mut db = Connection::open(&f.db).unwrap();
    let tx = db.transaction().unwrap();
    let mut last = None;
    for sequence in 3..=4096 {
        let request = Request {
            version: 2,
            binding: binding.clone(),
            operation_id: format!("s3-capacity-{sequence}"),
            command: saved_handoff(Expected::Revision(sequence - 2)),
        };
        let receipt = Receipt {
            generation: binding.generation.clone(),
            operation_id: request.operation_id.clone(),
            commit_sequence: sequence,
            affected: vec![RecordVersion {
                kind: "handoff".into(),
                project: "alpha".into(),
                record_id: "capacity".into(),
                revision: sequence - 1,
                deleted: false,
            }],
        };
        tx.execute(
            "INSERT INTO acp_receipts VALUES(?1,?2,?3,?4,?5)",
            params![
                request.operation_id,
                binding.generation,
                Sha256::digest(serde_json::to_vec(&request).unwrap()).to_vec(),
                sequence,
                serde_json::to_string(&receipt).unwrap()
            ],
        )
        .unwrap();
        last = Some((request, receipt));
    }
    tx.execute_batch(
        "UPDATE handoffs SET revision=4095 WHERE project='alpha' AND task_id='capacity';
         UPDATE acp_profile SET commit_sequence=4096",
    )
    .unwrap();
    tx.commit().unwrap();
    // No validation seam is bypassed when admitting the injected fixture.
    let metadata = precept_store::SqliteStore::open(&f.db)
        .unwrap()
        .metadata()
        .unwrap();
    assert_eq!(metadata.commit_sequence, 4096);
    last.unwrap()
}

#[test]
fn s3_contained_4096_fixture_process_capacity_conflict_and_exact_replay() {
    let f = Fixture::created();
    let binding = f.daemon.enable();
    f.register(&binding, "alpha");
    f.daemon
        .mutate(
            &binding,
            "historical-handoff",
            saved_handoff(Expected::Absent),
        )
        .unwrap();
    let (request, receipt) = seed_receipt_ceiling(&f, &binding);
    let write = f.daemon.credential("operator-write.cap");
    let before = logical(&f.db);
    let replay = f
        .daemon
        .call(
            &write,
            "profile.mutate",
            serde_json::to_value(&request).unwrap(),
        )
        .unwrap();
    assert_eq!(replay, serde_json::to_value(receipt).unwrap());
    let mut changed = request.clone();
    if let Command::PutHandoff {
        validation_json, ..
    } = &mut changed.command
    {
        validation_json.push(' ');
    }
    assert_eq!(
        f.daemon.call(
            &write,
            "profile.mutate",
            serde_json::to_value(changed).unwrap()
        ),
        Err(Error::IdempotencyConflict)
    );
    let denied = Cli::start(
        &f,
        &binding,
        "capacity-refused",
        &[
            "preference-suggest",
            "--scope",
            "global",
            "--key",
            "synthetic",
            "--value",
            "synthetic",
            "--tag",
            "synthetic",
            "--origin",
            "agent",
        ],
    )
    .finish();
    assert!(!denied.0);
    assert_eq!(denied.1["error"], "Capacity");
    assert_eq!(denied.1["operation_id"], "capacity-refused");
    assert_eq!(denied.1["outcome_unknown"], false);
    let saved = f
        .daemon
        .control(
            "handoff.get",
            json!({"project":"alpha","task_id":"capacity"}),
        )
        .unwrap();
    assert_eq!(saved["validation"][0]["status"], "PASS");
    assert!(
        saved["resume_warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| {
                warning
                    .as_str()
                    .unwrap()
                    .contains("recorded revision and environment")
            })
    );
    assert_eq!(logical(&f.db), before);
    f.daemon
        .control("profile.write.disable", json!({}))
        .unwrap();
    let next = f.daemon.enable();
    assert_eq!(next.owner_epoch, binding.owner_epoch + 1);
    let before = logical(&f.db);
    assert_eq!(
        f.daemon.call(
            &f.daemon.credential("operator-write.cap"),
            "profile.mutate",
            serde_json::to_value(request).unwrap(),
        ),
        Err(Error::Unauthorized)
    );
    assert_eq!(logical(&f.db), before);
}
