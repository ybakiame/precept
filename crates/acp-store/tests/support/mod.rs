//! Test-only Rust fixture construction; never compiled into a product binary.
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub fn fixture(base: &Path) -> PathBuf {
    assert!(!base.exists());
    fs::create_dir(base).unwrap();
    fs::create_dir(base.join("state")).unwrap();
    let path = base.join("state/context.sqlite3");
    assert!(!path.exists());
    let db = Connection::open(&path).unwrap();
    for ddl in acp_store::profile::DDL {
        db.execute_batch(ddl).unwrap();
    }
    db.pragma_update(None, "user_version", 1).unwrap();
    db.pragma_update(None, "foreign_keys", true).unwrap();
    let generation = "11111111111111111111111111111111";
    db.execute(
        "INSERT INTO acp_profile VALUES(1,'00000000000000000000000000000000',?1,0,1,0)",
        [generation],
    )
    .unwrap();
    let data: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/p1-preference-contract.v1.json"
    ))
    .unwrap();
    let time = "2026-01-01T00:00:00Z";
    let mut sequence = 0;
    let mut receipt = |kind: &str, project: &str, id: String| {
        sequence += 1;
        let op = format!("fixture-{sequence}");
        let value = json!({"generation":generation,"operation_id":op,"commit_sequence":sequence,"affected":[{"kind":kind,"project":project,"record_id":id,"revision":1,"deleted":false}]});
        db.execute(
            "INSERT INTO acp_receipts VALUES(?1,?2,zeroblob(32),?3,?4)",
            params![op, generation, sequence, value.to_string()],
        )
        .unwrap();
    };
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
        receipt("project", "", name.clone());
    }
    let mut id = 0;
    for p in data["preferences"].as_array().unwrap() {
        id += 1;
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
        db.execute(
            "INSERT INTO preferences VALUES(?1,?2,?3,?4,?5,?6,'fixture',1,?7,?7)",
            params![
                id,
                scope,
                project,
                p["key"].as_str().unwrap(),
                p["value"].as_str().unwrap(),
                p["status"].as_str().unwrap(),
                time
            ],
        )
        .unwrap();
        receipt("preference", "", id.to_string());
    }
    id += 1;
    db.execute("INSERT INTO preferences VALUES(?1,'project','alpha','commit.style','newest','confirmed','fixture-new',1,?2,?2)",(id,time)).unwrap();
    receipt("preference", "", id.to_string());
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
        receipt("handoff", project, task.into());
    }
    db.execute(
        "UPDATE acp_profile SET commit_sequence=?1,next_preference_id=?2",
        (sequence, id + 1),
    )
    .unwrap();
    drop(db);
    path
}
