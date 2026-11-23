//! Work sessions through a real daemon: an agent records its goal and work
//! state (only where the operator allows agent writes), every agent of the
//! project reads it back in context and in the session-start briefing (the
//! continuity between agents and after compaction), and the operator lists
//! and deletes sessions. Kept outside the Profile.
#[path = "support/mcp.rs"]
mod mcp;
#[allow(dead_code)] // Shared harness; this suite uses part of it.
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
        "todo":[{"id":"parser","text":"parser"},{"text":"daemon scan"}]});
    // Off until the operator allows this project's agents to write.
    assert_eq!(
        call("session.start", start.clone()),
        Err(Error::Unauthorized)
    );
    f.daemon
        .control("project.propose", json!({"name":"alpha","enabled":true}))
        .unwrap();
    let s = call("session.start", start.clone()).unwrap();
    let id = s["id"].as_str().unwrap().to_owned();
    assert_eq!(s["status"], "open");
    assert_eq!(s["agent"], "claude-code 2.1");
    // The same conversation finds its session again.
    assert_eq!(call("session.start", start).unwrap()["id"], id.as_str());
    let updated = call(
        "session.update",
        json!({"id":id,"expected_rev":2,"done":[{"id":"parser","text":"parser"}],
            "doing":[{"text":"daemon scan"}],"todo":[{"text":"app dialog"}]}),
    )
    .unwrap();
    assert_eq!(updated["rev"], 3);
    assert_eq!(
        updated["todo"][0]["id"].as_str().unwrap().len(),
        8,
        "assigned"
    );
    // A stale revision never overwrites another agent's write.
    assert_eq!(
        call(
            "session.update",
            json!({"id":id,"expected_rev":1,"goal":"stale"})
        ),
        Err(Error::Conflict)
    );
    // Every agent of the project reads it back: in context, and in the
    // briefing another harness gets at its session start.
    let context = call("context.get", json!({})).unwrap();
    assert_eq!(
        context["work_sessions"][0]["doing"][0]["text"],
        "daemon scan"
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
    assert!(text.contains("**Add import from files** (open, claude-code 2.1"));
    assert!(text.contains("Doing: daemon scan"));
    assert!(text.contains("Next: app dialog"));
    // Closing keeps the summary; a closed session takes no more updates.
    let closed = call(
        "session.close",
        json!({"id":id,"expected_rev":3,"summary":"Parser and scan done; dialog left.","todo":[{"text":"app dialog"}]}),
    )
    .unwrap();
    assert_eq!(closed["status"], "closed");
    assert_eq!(
        call("session.update", json!({"id":id,"doing":[{"text":"x"}]})),
        Err(Error::InvalidTransition)
    );
    assert_eq!(
        call(
            "session.update",
            json!({"id":"nope","doing":[{"text":"x"}]})
        ),
        Err(Error::NotFound)
    );
    // Agents cannot list or delete; the operator can.
    assert_eq!(
        call("sessions.list", json!({"project":"alpha"})),
        Err(Error::Unauthorized)
    );
    let listed = f
        .daemon
        .control("sessions.list", json!({"project":"alpha"}))
        .unwrap();
    assert_eq!(
        listed["sessions"][0]["summary"],
        "Parser and scan done; dialog left."
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
    // Sessions never touch the Profile file.
    assert!(
        f.temp
            .path()
            .join("daemon-state/sessions-alpha.json")
            .exists()
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
    // A new daemon still has it, chain intact.
    f.daemon.kill();
    f.daemon = Daemon::relaunch(f.temp.path(), "run-b");
    f.daemon.ready();
    f.daemon
        .control("profile.select", json!({"path":f.db}))
        .unwrap();
    let a = activity(&f);
    assert_eq!(a["chain"]["ok"], true);
    assert!(a["events"].as_array().unwrap().len() >= 3);
    // Editing an entry shows as a break.
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
fn a_closed_session_becomes_a_handoff_the_next_agent_resumes() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    f.daemon
        .control("project.propose", json!({"name":"alpha","enabled":true}))
        .unwrap();
    let agent = f.daemon.credential("agent-alpha.cap");
    let call = |method: &str, params: Value| f.daemon.call(&agent, method, params);
    let id = call("session.start", json!({"goal":"Add import from AGENTS.md"})).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    call(
        "session.close",
        json!({"id":id,"summary":"Scan and dialog done.","todo":[{"text":"Visual check"}]}),
    )
    .unwrap();
    let handoff = |cap: &str, task: &str| {
        f.daemon.call(
            &f.daemon.credential(cap),
            "sessions.handoff",
            json!({"project":"alpha","id":id,"task_id":task}),
        )
    };
    // The operator's write only.
    assert_eq!(handoff("operator.cap", "import"), Err(Error::Unauthorized));
    assert_eq!(
        handoff("agent-alpha.cap", "import"),
        Err(Error::Unauthorized)
    );
    handoff("operator-write.cap", "import").unwrap();
    // A task id is taken once.
    assert_eq!(
        handoff("operator-write.cap", "import"),
        Err(Error::Conflict)
    );
    // The next agent resumes it with get_handoff's daemon call.
    let resumed = call("handoff.get", json!({"task_id":"import"})).unwrap();
    let text = resumed.to_string();
    assert!(text.contains("Add import from AGENTS.md"), "{text}");
    assert!(text.contains("Scan and dialog done."));
    assert!(text.contains("- Visual check"));
    // The operator lists the project's saved handoffs; agents cannot.
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
