//! Test-only Rust fixture construction; never compiled into a product binary.
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Insert one identity-only receipt at the next commit_sequence and return it.
fn push_receipt(
    db: &Connection,
    sequence: &mut i64,
    generation: &str,
    kind: &str,
    project: &str,
    id: &str,
    revision: i64,
) -> i64 {
    *sequence += 1;
    let op = format!("fixture-{sequence}");
    let value = json!({"generation":generation,"operation_id":op,"commit_sequence":*sequence,"affected":[{"kind":kind,"project":project,"record_id":id,"revision":revision,"deleted":false}]});
    db.execute(
        "INSERT INTO acp_receipts VALUES(?1,?2,zeroblob(32),?3,?4)",
        params![op, generation, *sequence, value.to_string()],
    )
    .unwrap();
    *sequence
}

/// Append one review decision row, allocating the next review id.
#[allow(clippy::too_many_arguments)]
fn push_decision(
    db: &Connection,
    review_id: &mut i64,
    preference_id: i64,
    scope: &str,
    project: Option<&str>,
    action: &str,
    from: i64,
    to: i64,
    digest: &[u8],
    receipt_sequence: i64,
    time: &str,
) {
    *review_id += 1;
    db.execute(
        "INSERT INTO acp_review_decisions VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'cli',?10)",
        params![
            *review_id,
            scope,
            project,
            preference_id,
            action,
            from,
            to,
            digest,
            receipt_sequence,
            time
        ],
    )
    .unwrap();
}

pub fn fixture(base: &Path) -> PathBuf {
    assert!(!base.exists());
    fs::create_dir(base).unwrap();
    precept_ipc::transport::private_dir(&base.join("state"), true).unwrap();
    let path = base.join("state/context.sqlite3");
    assert!(!path.exists());
    drop(precept_ipc::transport::new_private(&path).unwrap());
    let db = Connection::open(&path).unwrap();
    for ddl in precept_store::profile::DDL {
        db.execute_batch(ddl).unwrap();
    }
    db.pragma_update(None, "user_version", 1).unwrap();
    db.pragma_update(None, "foreign_keys", true).unwrap();
    let generation = "11111111111111111111111111111111";
    db.execute(
        "INSERT INTO acp_profile VALUES(1,'00000000000000000000000000000000',?1,0,1,0,1)",
        [generation],
    )
    .unwrap();
    let data: Value = serde_json::from_str(include_str!(
        "../../../precept-core/tests/preference-resolution.v1.json"
    ))
    .unwrap();
    let time = "2026-01-01T00:00:00Z";
    let mut sequence = 0i64;
    let mut review_id = 0i64;
    for (name, marker) in data["projects"].as_object().unwrap() {
        let root = base.join(name);
        fs::create_dir(&root).unwrap();
        fs::write(root.join("AGENTS.md"), marker.as_str().unwrap()).unwrap();
        let root = root.canonicalize().unwrap();
        db.execute(
            "INSERT INTO projects VALUES(?1,?2,1,?3)",
            (name, root.to_str().unwrap(), time),
        )
        .unwrap();
        push_receipt(&db, &mut sequence, generation, "project", "", name, 1);
    }
    let mut id = 0i64;
    // Build each preference through the same state/receipt/decision structure the
    // real writer produces: a suggestion at revision 1, then one receipt and one
    // decision per status transition. A `suggested` row keeps revision 1 and no
    // decision; confirmed/rejected reach revision 2; deprecated reaches revision 3
    // via confirm then deprecate. No row is marked final at revision 1.
    let mut build =
        |scope: &str, project: Option<&str>, key: &str, value: &str, status: &str, tag: &str| {
            id += 1;
            let pref_id = id;
            let origin = precept_ipc::operator::Origin::Agent;
            let tags = vec![tag.to_owned()];
            let typed = match project {
                Some(name) => precept_ipc::operator::Scope::Project { name: name.into() },
                None => precept_ipc::operator::Scope::Global,
            };
            let digest =
                precept_ipc::operator::proposal_digest(&typed, project, key, value, origin, &tags);
            let digest_bytes = digest.to_vec();
            // Suggestion: revision 1, status suggested, identity-only receipt.
            push_receipt(
                &db,
                &mut sequence,
                generation,
                "preference",
                "",
                &pref_id.to_string(),
                1,
            );
            db.execute(
                "INSERT INTO preferences VALUES(?1,?2,?3,?4,?5,'suggested',?6,1,?7,?7,'agent',?8)",
                params![
                    pref_id,
                    scope,
                    project,
                    key,
                    value,
                    precept_ipc::operator::tags_json(&tags),
                    time,
                    digest_bytes
                ],
            )
            .unwrap();
            let mut revision = 1i64;
            let mut current = "suggested";
            // Replay the writer's transition chain to reach the requested status.
            let transitions: &[(&str, &str)] = match status {
                "suggested" => &[],
                "confirmed" => &[("confirm", "confirmed")],
                "rejected" => &[("reject", "rejected")],
                "deprecated" => &[("confirm", "confirmed"), ("deprecate", "deprecated")],
                other => panic!("unsupported fixture status {other}"),
            };
            for (action, next) in transitions {
                let from = revision;
                revision += 1;
                let seq = push_receipt(
                    &db,
                    &mut sequence,
                    generation,
                    "preference",
                    "",
                    &pref_id.to_string(),
                    revision,
                );
                push_decision(
                    &db,
                    &mut review_id,
                    pref_id,
                    scope,
                    project,
                    action,
                    from,
                    revision,
                    &digest_bytes,
                    seq,
                    time,
                );
                current = next;
            }
            if revision != 1 || current != "suggested" {
                db.execute(
                    "UPDATE preferences SET status=?1,revision=?2 WHERE id=?3",
                    params![current, revision, pref_id],
                )
                .unwrap();
            }
        };
    for p in data["preferences"].as_array().unwrap() {
        let scope = if p["scope"] == "global" {
            "global"
        } else {
            "project"
        };
        let project = if scope == "global" {
            None
        } else {
            p["scope"].as_str()
        };
        build(
            scope,
            project,
            p["key"].as_str().unwrap(),
            p["value"].as_str().unwrap(),
            p["status"].as_str().unwrap(),
            "fixture",
        );
    }
    build(
        "project",
        Some("alpha"),
        "commit.style",
        "newest",
        "confirmed",
        "fixture-new",
    );
    let validation:Vec<_>=["PASS","FAIL","SKIPPED","NOT_RUN"].into_iter().map(|s|json!({"check":s,"status":s,"environment":"synthetic","revision":"base","evidence":{"id":"record-id","updated_at":"recorded","nested":[null,true,2,{"log":"ref"}]}})).collect();
    for (project, task) in [
        ("alpha", "shared"),
        ("beta", "shared"),
        ("beta", "beta-only"),
        ("alpha", "null-fingerprint"),
    ] {
        let raw = if task == "null-fingerprint" {
            "[]".to_owned()
        } else {
            serde_json::to_string(&validation).unwrap()
        };
        db.execute(
            "INSERT INTO handoffs VALUES(?1,?2,?3,'','','unknown',NULL,'','inspect',?4,1,?5)",
            params![project, task, format!("synthetic-{project}"), raw, time],
        )
        .unwrap();
        push_receipt(&db, &mut sequence, generation, "handoff", project, task, 1);
    }
    db.execute(
        "UPDATE acp_profile SET commit_sequence=?1,next_preference_id=?2,next_review_id=?3",
        (sequence, id + 1, review_id + 1),
    )
    .unwrap();
    drop(db);
    path
}
