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
fn confirmed_environment_notes_move_to_memory_in_one_reviewed_write() {
    use precept_core::operator::{Channel, Enforcement, Origin, RuleOptions};
    use precept_write::{Command, Expected, Scope};
    let mut f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    let env = Scope::Environment {
        name: "alpha".into(),
    };
    let mut ids = vec![];
    for (op, key, value) in [
        ("s1", "shell", "Agents run inside WSL."),
        ("s2", "node", "Node 22 via mise."),
    ] {
        let r = f
            .daemon
            .mutate(
                &b,
                op,
                Command::SuggestPreference {
                    scope: env.clone(),
                    key: key.into(),
                    value: value.into(),
                    tags: vec!["setup".into()],
                    origin: Origin::Agent,
                    rule: RuleOptions {
                        enforcement: Enforcement::Advisory,
                        ..RuleOptions::default()
                    },
                    expected: Expected::Absent,
                },
            )
            .unwrap();
        let id: i64 = r["affected"][0]["record_id"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        ids.push(id);
    }
    // One stays a suggestion; the other is confirmed.
    let preview = |f: &Fixture| {
        f.daemon
            .control("memories.environment", json!({"project":"alpha"}))
            .unwrap()["records"]
            .as_array()
            .unwrap()
            .clone()
    };
    assert!(preview(&f).is_empty(), "only confirmed notes move");
    let digest = f
        .daemon
        .control("profile.preferences.get", json!({"scope":env,"id":ids[0]}))
        .unwrap()["record"]["proposal_digest"]
        .as_str()
        .unwrap()
        .to_owned();
    f.daemon
        .mutate(
            &b,
            "c1",
            Command::ConfirmPreference {
                id: ids[0],
                scope: env.clone(),
                proposal_digest: digest.clone(),
                channel: Channel::Desktop,
                expected: Expected::Revision(1),
                note: None,
            },
        )
        .unwrap();
    let records = preview(&f);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["key"], "shell");
    let item = |revision: i64| json!({"project":"alpha","items":[{"id":ids[0],"revision":revision,"proposal_digest":digest}]});
    let mv = |cap: &str, params: Value| {
        f.daemon.call(
            &f.daemon.credential(cap),
            "memories.move_environment",
            params,
        )
    };
    // The operator's write only, of exactly the previewed revision.
    assert_eq!(mv("operator.cap", item(2)), Err(Error::Unauthorized));
    assert_eq!(mv("agent-alpha.cap", item(2)), Err(Error::Unauthorized));
    assert_eq!(mv("operator-write.cap", item(1)), Err(Error::Conflict));
    assert_eq!(
        mv(
            "operator-write.cap",
            json!({"project":"alpha","items":[{"id":ids[1],"revision":1,"proposal_digest":digest}]})
        ),
        Err(Error::Conflict),
        "a suggestion is not moved"
    );
    let moved = mv("operator-write.cap", item(2)).unwrap();
    assert_eq!(moved["moved"], 1);
    assert!(preview(&f).is_empty());
    let record = f
        .daemon
        .control("profile.preferences.get", json!({"scope":env,"id":ids[0]}))
        .unwrap()["record"]
        .clone();
    assert_eq!(record["status"], "deprecated");
    let listed = f
        .daemon
        .control("memories.list", json!({"project":"alpha"}))
        .unwrap();
    assert_eq!(listed["entries"][0]["kind"], "environment");
    assert_eq!(listed["entries"][0]["title"], "shell");
    assert_eq!(listed["entries"][0]["body"], "Agents run inside WSL.");
    // A new daemon re-verifies the Profile after the batch deprecation.
    f.daemon.kill();
    f.daemon = Daemon::relaunch(f.temp.path(), "run-b");
    f.daemon.ready();
    f.daemon
        .control("profile.select", json!({"path":f.db}))
        .expect("a batch deprecation verifies");
}
