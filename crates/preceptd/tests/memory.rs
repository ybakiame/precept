//! Project memory through a real daemon: agents keep notes only where the
//! operator allows agent writes, every agent reads them back in context and
//! in the session-start briefing (environment first), recall ranks and
//! counts, and the operator edits, pins, restores, deletes and clears them.
//! Kept outside the Profile.
#[allow(dead_code)] // Shared harness; this suite uses part of it.
mod support;
use precept_ipc::Error;
use serde_json::{Value, json};
use support::*;

#[test]
fn agents_keep_project_memory_and_the_operator_governs_it() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    let agent = f.daemon.credential("agent-alpha.cap");
    let call = |method: &str, params: Value| f.daemon.call(&agent, method, params);
    let node =
        json!({"kind":"environment","title":"Node","body":"Node 22 via mise","client":"codex 1.0"});
    // Off until the operator allows this project's agents to write.
    assert_eq!(
        call("memory.remember", node.clone()),
        Err(Error::Unauthorized)
    );
    f.daemon
        .control("project.propose", json!({"name":"alpha","enabled":true}))
        .unwrap();
    let added = call("memory.remember", node.clone()).unwrap();
    assert_eq!(added["status"], "added");
    assert_eq!(added["entry"]["by"], "codex 1.0");
    let id = added["entry"]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        call("memory.remember", node).unwrap()["status"],
        "reaffirmed"
    );
    call(
        "memory.remember",
        json!({"kind":"caution","title":"WSL 路径","body":"/mnt/c 下构建很慢"}),
    )
    .unwrap();
    // Malformed: an unknown kind, an unknown field, a missing body.
    for bad in [
        json!({"kind":"rule","title":"x","body":"y"}),
        json!({"kind":"lesson","title":"x","body":"y","extra":1}),
        json!({"kind":"lesson","title":"x"}),
    ] {
        assert_eq!(call("memory.remember", bad), Err(Error::InvalidRequest));
    }
    // Context and briefing carry it, environment first.
    let context = call("context.get", json!({})).unwrap();
    assert_eq!(context["memory"]["counts"]["environment"], 1);
    assert_eq!(context["memory"]["environment"][0]["title"], "Node");
    assert_eq!(context["memory"]["relevant"][0]["kind"], "caution");
    let hits = call("memory.recall", json!({"query":"很慢"})).unwrap();
    assert_eq!(hits["entries"].as_array().unwrap().len(), 1);
    assert_eq!(hits["entries"][0]["recalls"], 1);
    // Index mode: one line per note without the body; a fetch by id returns
    // the full note and counts no recall.
    let index = call("memory.recall", json!({"query":"很慢","index":true})).unwrap();
    assert_eq!(index["index"], true);
    let line = &index["entries"][0];
    assert!(line.get("body").is_none());
    assert!(line["weight"].as_f64().is_some());
    let note_id = line["id"].as_str().unwrap().to_owned();
    let full = call("memory.get", json!({"id":note_id})).unwrap();
    assert_eq!(full["entry"]["body"], "/mnt/c 下构建很慢");
    assert_eq!(full["entry"]["unverified_days"], 0);
    assert!(full["entry"].get("notice").is_none());
    assert_eq!(
        call("memory.get", json!({"id":"zz"})),
        Err(Error::InvalidRequest)
    );
    // A reaffirmation may state what verified the note.
    let affirmed = call(
        "memory.reaffirm",
        json!({"id":note_id,"basis":"checked on this machine"}),
    )
    .unwrap();
    assert_eq!(affirmed["entry"]["basis"], "checked on this machine");
    // Agents cannot use the operator's methods, nor the operator an agent's.
    assert_eq!(
        call("memories.list", json!({"project":"alpha"})),
        Err(Error::Unauthorized)
    );
    assert_eq!(
        f.daemon.control("memory.recall", json!({})).err(),
        Some(Error::Unauthorized)
    );
    // The operator edits (a correction kept in history), pins and restores.
    let edited = f
        .daemon
        .control(
            "memories.edit",
            json!({"project":"alpha","id":id,"body":"Node 24 via mise","pinned":true}),
        )
        .unwrap();
    assert_eq!(edited["entry"]["edited_by_operator"], true);
    assert_eq!(edited["entry"]["pinned"], true);
    assert_eq!(edited["entry"]["history"][0]["body"], "Node 22 via mise");
    let restored = f
        .daemon
        .control(
            "memories.restore",
            json!({"project":"alpha","id":id,"index":0}),
        )
        .unwrap();
    assert_eq!(restored["entry"]["body"], "Node 22 via mise");
    let listed = f
        .daemon
        .control("memories.list", json!({"project":"alpha"}))
        .unwrap();
    assert_eq!(listed["entries"][0]["id"], id.as_str(), "pinned first");
    assert!(listed["entries"][0]["weight"].as_f64().unwrap() > 1000.0);
    assert_eq!(
        f.daemon
            .control("memories.list", json!({"project":"nope"}))
            .err(),
        Some(Error::NotFound)
    );
    // An agent forgets; the operator clears the rest.
    call("memory.forget", json!({"id":id})).unwrap();
    assert_eq!(
        call("memory.forget", json!({"id":id})),
        Err(Error::NotFound)
    );
    assert_eq!(
        f.daemon
            .control("memories.clear", json!({"project":"alpha"}))
            .unwrap()["cleared"],
        1
    );
    // Every agent call is in the project's activity.
    let events = f
        .daemon
        .control("activity.list", json!({"project":"alpha"}))
        .unwrap();
    assert!(
        events["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "memory" && e["outcome"] == "ok")
    );
    // Memory never touches the Profile.
    assert!(
        f.temp
            .path()
            .join("daemon-state/memory-alpha.json")
            .exists()
    );
}

#[test]
fn a_note_that_keeps_holding_is_filed_once_as_a_suggested_rule() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    f.daemon
        .control("project.propose", json!({"name":"alpha","enabled":true}))
        .unwrap();
    let agent = f.daemon.credential("agent-alpha.cap");
    let call = |method: &str, params: Value| f.daemon.call(&agent, method, params);
    let lesson = json!({"kind":"lesson","title":"Run nextest","body":"Use cargo nextest run; cargo test misses the doctests split."});
    let id = call("memory.remember", lesson.clone()).unwrap()["entry"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let suggested = || {
        f.daemon
            .control(
                "profile.preferences.list",
                json!({"scope":{"kind":"project","name":"alpha"},"after_id":0}),
            )
            .unwrap()["records"]
            .as_array()
            .unwrap()
            .clone()
    };
    call("memory.reaffirm", json!({"id":id})).unwrap();
    call("memory.remember", lesson.clone()).unwrap();
    assert!(suggested().is_empty(), "two confirmations are not enough");
    let third = call("memory.reaffirm", json!({"id":id})).unwrap();
    assert!(third["entry"]["proposed"].is_string());
    let records = suggested();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["key"], "run-nextest");
    assert_eq!(records[0]["status"], "suggested");
    assert_eq!(records[0]["origin"], "agent");
    assert_eq!(records[0]["tags"], json!(["lesson", "memory"]));
    // Once: more confirmations file nothing more.
    call("memory.reaffirm", json!({"id":id})).unwrap();
    assert_eq!(suggested().len(), 1);
    // Environment notes stay memory.
    let env = json!({"kind":"environment","title":"Node","body":"Node 22"});
    let env_id = call("memory.remember", env).unwrap()["entry"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    for _ in 0..4 {
        call("memory.reaffirm", json!({"id":env_id})).unwrap();
    }
    assert_eq!(suggested().len(), 1);
}
