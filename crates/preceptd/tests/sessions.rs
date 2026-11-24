//! Work sessions through a real daemon: an agent records a goal and flat,
//! structured work items, every agent of the project reads them in context and
//! briefing, and single-item CAS updates preserve parallel work.
#[path = "support/mcp.rs"]
mod mcp;
#[allow(dead_code)]
mod support;
use mcp::Mcp;
use precept_ipc::Error;
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::process::{Command as Process, Stdio};
use support::*;

fn briefing(f: &Fixture, vendor: &str, stdin: &str) -> String {
    let mut child = Process::new(exe("precept-hook"))
        .arg("--runtime-dir")
        .arg(&f.daemon.location.runtime_dir)
        .arg("--endpoint")
        .arg(&f.daemon.location.endpoint)
        .args([
            "--project",
            "alpha",
            "--vendor",
            vendor,
            "--capability-file",
        ])
        .arg(f.daemon.location.runtime_dir.join("agent-alpha.cap"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap()
}

#[test]
fn work_moves_between_agents_through_their_sessions() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    let agent = f.daemon.credential("agent-alpha.cap");
    let call = |method: &str, params: Value| f.daemon.call(&agent, method, params);
    let start = json!({"goal":"Add import from files","conversation":"conv-1","client":"claude-code 2.1",
    "items":[
        {"id":"design","text":"Design","state":"done"},
        {"id":"implement","text":"Implement","depends_on":["design"]},
        {"id":"dialog","text":"Dialog","kind":"milestone"}
    ]});
    assert_eq!(
        call("session.start", start.clone()),
        Err(Error::Unauthorized)
    );
    f.daemon
        .control("project.propose", json!({"name":"alpha","enabled":true}))
        .unwrap();
    let s = call("session.start", start).unwrap();
    let id = s["id"].as_str().unwrap().to_owned();
    assert_eq!(s["status"], "open");
    assert_eq!(s["agent"], "claude-code 2.1");
    assert_eq!(s["items"].as_array().unwrap().len(), 3);
    let resumed = call(
        "session.start",
        json!({"id":id,"expected_rev":1,"goal":"Add bounded import from files","client":"codex 1.0"}),
    )
    .unwrap();
    assert_eq!(resumed["id"], id.as_str());
    assert_eq!(resumed["rev"], 2);
    for (method, params) in [
        ("session.start", json!({"goal":"Null","operations":null})),
        ("session.get", json!({"id":id,"client":null})),
        (
            "session.start",
            json!({"goal":"Null","items":[{"text":"Item","owner":null}]}),
        ),
        (
            "session.update",
            json!({"id":id,"expected_rev":2,"goal":"Null","items":null}),
        ),
        (
            "session.update",
            json!({"id":id,"expected_rev":2,"goal":"Null","summary":null}),
        ),
        (
            "session.close",
            json!({"id":id,"expected_rev":2,"goal":null}),
        ),
    ] {
        assert_eq!(call(method, params), Err(Error::InvalidRequest));
    }
    let updated = call(
        "session.update",
        json!({"id":id,"expected_rev":2,"client":"codex 1.0","operations":[
            {"op":"update","id":"implement","owner":"codex 1.0"},
            {"op":"start","id":"implement"}
        ]}),
    )
    .unwrap();
    assert_eq!(updated["rev"], 3);
    assert_eq!(updated["items"][1]["state"], "doing");
    assert_eq!(updated["items"][1]["owner"], "codex 1.0");
    assert_eq!(
        call(
            "session.update",
            json!({"id":id,"expected_rev":1,"goal":"stale"})
        ),
        Err(Error::Conflict)
    );
    let context = call("context.get", json!({})).unwrap();
    assert!(
        context["work_sessions"][0]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "implement" && item["state"] == "doing")
    );
    let out: Value = serde_json::from_str(&briefing(
        &f,
        "codex",
        &json!({"session_id":"other"}).to_string(),
    ))
    .unwrap();
    let text = out["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(text.contains("## Work in progress"));
    assert!(text.contains("**Add bounded import from files** (open, claude-code 2.1"));
    assert!(text.contains("Doing:"));
    assert!(text.contains("Implement"));
    assert!(text.contains("Next:"));
    let closed = call(
        "session.close",
        json!({"id":id,"expected_rev":3,"summary":"Design complete; implementation and dialog remain."}),
    )
    .unwrap();
    assert_eq!(closed["status"], "closed");
    assert_eq!(
        call(
            "session.update",
            json!({"id":id,"expected_rev":4,"operations":[{"op":"add","item":{"text":"x"}}]})
        ),
        Err(Error::InvalidTransition)
    );
    assert_eq!(
        call(
            "session.update",
            json!({"id":"nope","expected_rev":1,"operations":[{"op":"add","item":{"text":"x"}}]})
        ),
        Err(Error::NotFound)
    );
    assert_eq!(
        call("sessions.list", json!({"project":"alpha"})),
        Err(Error::Unauthorized)
    );
    assert_eq!(
        f.daemon
            .control("sessions.list", json!({"project":"alpha","id":null})),
        Err(Error::InvalidRequest)
    );
    let listed = f
        .daemon
        .control("sessions.list", json!({"project":"alpha"}))
        .unwrap();
    assert_eq!(
        listed["sessions"][0]["summary"],
        "Design complete; implementation and dialog remain."
    );
    let events = f
        .daemon
        .control("activity.list", json!({"project":"alpha"}))
        .unwrap();
    assert!(
        events["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "session" && e["outcome"] == "ok")
    );
    f.daemon
        .control("sessions.delete", json!({"project":"alpha","id":id}))
        .unwrap();
    assert!(
        f.daemon
            .control("sessions.list", json!({"project":"alpha"}))
            .unwrap()["sessions"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        f.temp
            .path()
            .join("daemon-state/sessions-alpha.json")
            .exists()
    );
}

#[test]
fn another_session_can_resume_and_parallel_adds_conflict_without_losing_work() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    f.daemon
        .control("project.propose", json!({"name":"alpha","enabled":true}))
        .unwrap();
    let agent = f.daemon.credential("agent-alpha.cap");
    let call = |method: &str, params: Value| f.daemon.call(&agent, method, params);
    let started = call(
        "session.start",
        json!({"goal":"Shared architecture","client":"agent-a 1.0","items":[{"id":"api","text":"API"}]}),
    )
    .unwrap();
    let id = started["id"].as_str().unwrap().to_owned();
    let resumed = call(
        "session.start",
        json!({"id":id,"expected_rev":1,"goal":"Shared architecture and UI","client":"agent-b 1.0"}),
    )
    .unwrap();
    assert_eq!(resumed["id"], id.as_str());
    assert_eq!(resumed["agent"], "agent-a 1.0");
    call(
        "session.update",
        json!({"id":id,"expected_rev":2,"client":"agent-b 1.0","operations":[
            {"op":"add","item":{"id":"ui","text":"UI","depends_on":["api"]}}
        ]}),
    )
    .unwrap();
    assert_eq!(
        call(
            "session.update",
            json!({"id":id,"expected_rev":2,"operations":[
                {"op":"add","item":{"id":"docs","text":"Docs"}}
            ]})
        ),
        Err(Error::Conflict)
    );
    let current = call("session.get", json!({"id":id})).unwrap();
    assert_eq!(current["rev"], 3);
    assert_eq!(
        call("session.get", json!({"id":id,"operations":[]})),
        Err(Error::InvalidRequest)
    );
    call(
        "session.update",
        json!({"id":id,"expected_rev":3,"operations":[
            {"op":"add","item":{"id":"docs","text":"Docs","depends_on":["ui"]}}
        ]}),
    )
    .unwrap();
    let current = call("session.get", json!({"id":id})).unwrap();
    assert_eq!(current["items"].as_array().unwrap().len(), 3);
    assert_eq!(current["items"][2]["depends_on"][0], "ui");
}

#[test]
fn mcp_structured_session_round_trip_returns_full_state_and_precise_errors() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    f.daemon
        .control("project.propose", json!({"name":"alpha","enabled":true}))
        .unwrap();
    let mut mcp = Mcp::start(&f.daemon, "alpha");
    let started = mcp.request(
        "tools/call",
        json!({"name":"start_session","arguments":{
            "goal":"Structured MCP",
            "items":[{"id":"api","text":"API","kind":"milestone","state":"doing"}]
        }}),
    );
    assert_eq!(started["result"]["isError"], false);
    let text = started["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("rev 1") && text.contains("\"items\""),
        "{text}"
    );
    let listed = f
        .daemon
        .control("sessions.list", json!({"project":"alpha"}))
        .unwrap();
    let id = listed["sessions"][0]["id"].as_str().unwrap().to_owned();
    let updated = mcp.request(
        "tools/call",
        json!({"name":"update_work_state","arguments":{
            "session_id":id,
            "expected_rev":1,
            "operations":[{"op":"add","item":{"id":"docs","text":"Docs","depends_on":["api"]}}]
        }}),
    );
    assert_eq!(updated["result"]["isError"], false);
    assert!(
        updated["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("rev 2")
    );
    let bad = mcp.request(
        "tools/call",
        json!({"name":"update_work_state","arguments":{
            "session_id":id,
            "expected_rev":2,
            "operations":[{"op":"start","id":"missing"}]
        }}),
    );
    assert_eq!(bad["result"]["isError"], true);
    assert!(
        bad["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("operations[0].id")
    );
    let current = mcp.request(
        "tools/call",
        json!({"name":"get_work_session","arguments":{"session_id":id}}),
    );
    assert_eq!(current["result"]["isError"], false);
    assert!(
        current["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("\"docs\"")
    );
    let closed = mcp.request(
        "tools/call",
        json!({"name":"close_session","arguments":{
            "session_id":id,"expected_rev":2,"summary":"API remains in progress."
        }}),
    );
    assert_eq!(closed["result"]["isError"], false);
    assert!(
        closed["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("\"closed\"")
    );
}

#[test]
fn a_sessions_calls_are_kept_as_a_chain_that_survives_restart_and_shows_edits() {
    let mut f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    f.daemon
        .control("project.propose", json!({"name":"alpha","enabled":true}))
        .unwrap();
    let id = {
        let mut mcp = Mcp::start(&f.daemon, "alpha");
        let started = mcp.request(
            "tools/call",
            json!({"name":"start_session","arguments":{"goal":"Ship evidence"}}),
        );
        assert_eq!(started["result"]["isError"], false);
        mcp.context(json!({}));
        let listed = f
            .daemon
            .control("sessions.list", json!({"project":"alpha"}))
            .unwrap();
        listed["sessions"][0]["id"].as_str().unwrap().to_owned()
    };
    let activity = |f: &Fixture| {
        f.daemon
            .control("activity.list", json!({"project":"alpha"}))
            .unwrap()
    };
    let a = activity(&f);
    assert_eq!(a["chain"]["ok"], true);
    let tagged: Vec<&Value> = a["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["session"] == id.as_str())
        .collect();
    let kinds: Vec<&str> = tagged.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        ["context", "session"],
        "newest first, both of this session"
    );
    f.daemon.kill();
    f.daemon = Daemon::relaunch(f.temp.path(), "run-b");
    f.daemon.ready();
    f.daemon
        .control("profile.select", json!({"path":f.db}))
        .unwrap();
    let a = activity(&f);
    assert_eq!(a["chain"]["ok"], true);
    assert!(a["events"].as_array().unwrap().len() >= 3);
    let path = f.temp.path().join("daemon-state/evidence-alpha.jsonl");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        text.replacen("\"outcome\":\"ok\"", "\"outcome\":\"Unauthorized\"", 1),
    )
    .unwrap();
    let a = activity(&f);
    assert_eq!(a["chain"]["ok"], false);
    assert_eq!(a["chain"]["broken_at"], 1);
}

#[test]
fn a_closed_session_becomes_a_structured_handoff_the_next_agent_resumes() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    f.daemon
        .control("project.propose", json!({"name":"alpha","enabled":true}))
        .unwrap();
    let agent = f.daemon.credential("agent-alpha.cap");
    let call = |method: &str, params: Value| f.daemon.call(&agent, method, params);
    let id = call(
        "session.start",
        json!({"goal":"Add import from AGENTS.md","items":[{"id":"visual","text":"Visual check"}]}),
    )
    .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    call(
        "session.close",
        json!({"id":id,"expected_rev":1,"summary":"Scan and dialog done."}),
    )
    .unwrap();
    let handoff = |cap: &str, task: &str| {
        f.daemon.call(
            &f.daemon.credential(cap),
            "sessions.handoff",
            json!({"project":"alpha","id":id,"task_id":task}),
        )
    };
    assert_eq!(handoff("operator.cap", "import"), Err(Error::Unauthorized));
    assert_eq!(
        handoff("agent-alpha.cap", "import"),
        Err(Error::Unauthorized)
    );
    handoff("operator-write.cap", "import").unwrap();
    assert_eq!(
        handoff("operator-write.cap", "import"),
        Err(Error::Conflict)
    );
    let resumed = call("handoff.get", json!({"task_id":"import"})).unwrap();
    let text = resumed.to_string();
    assert!(text.contains("Add import from AGENTS.md"), "{text}");
    assert!(text.contains("Scan and dialog done."));
    assert!(
        text.contains("- [task] visual Visual check (state: todo)"),
        "{text}"
    );
    let listed = f
        .daemon
        .control("profile.handoff.list", json!({"project":"alpha"}))
        .unwrap();
    assert_eq!(listed["handoffs"][0]["task_id"], "import");
    assert_eq!(
        listed["handoffs"][0]["objective"],
        "Add import from AGENTS.md"
    );
    assert_eq!(
        f.daemon.call(
            &f.daemon.credential("agent-alpha.cap"),
            "profile.handoff.list",
            json!({"project":"alpha"})
        ),
        Err(Error::Unauthorized)
    );
    assert_eq!(
        f.daemon
            .control("profile.handoff.list", json!({"project":"../x"})),
        Err(Error::InvalidRequest)
    );
}
