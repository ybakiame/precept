//! Agent activity: real daemon, real `acp-mcp`, real operator read. The log
//! names the MCP client, counts what it read and never carries values.
#[allow(dead_code)]
#[path = "support/mcp.rs"]
mod mcp;
#[allow(dead_code)]
mod support;
use acp_core::operator::{Channel, Origin};
use acp_ipc::{Credential, Error};
use acp_write::{Command, Expected, Scope};
use mcp::Mcp;
use serde_json::{Value, json};
use support::*;

const SECRET: &str = "ACTIVITY_PRIVATE_VALUE";

fn events(f: &Fixture, params: Value) -> Vec<Value> {
    f.daemon.control("activity.list", params).unwrap()["events"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn mcp_starts_reads_and_refusals_are_logged_by_count_for_the_operator_only() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    f.register(&b, "beta");
    let receipt = f
        .daemon
        .mutate(
            &b,
            "activity-suggest",
            Command::SuggestPreference {
                scope: Scope::Global,
                key: "style".into(),
                value: SECRET.into(),
                tags: vec!["style".into()],
                origin: Origin::Operator,
                expected: Expected::Absent,
            },
        )
        .unwrap();
    let id: i64 = receipt["affected"][0]["record_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let preview = f
        .daemon
        .control(
            "profile.preferences.get",
            json!({"scope":Scope::Global,"id":id}),
        )
        .unwrap();
    f.daemon
        .mutate(
            &b,
            "activity-confirm",
            Command::ConfirmPreference {
                id,
                scope: Scope::Global,
                proposal_digest: preview["record"]["proposal_digest"]
                    .as_str()
                    .unwrap()
                    .into(),
                channel: Channel::Desktop,
                expected: Expected::Revision(1),
            },
        )
        .unwrap();
    // Nothing yet: an empty log, with the daemon's start time.
    let empty = f.daemon.control("activity.list", json!({})).unwrap();
    assert_eq!(empty["events"], json!([]));
    assert!(empty["daemon_started_ms"].as_u64().unwrap() > 0);

    // A client that initializes the classic way names itself.
    let mut mcp = Mcp::start(&f.daemon, "alpha");
    let init = mcp
        .send(
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-11-25","capabilities":{},
            "clientInfo":{"name":"synthetic-client\n","version":"9.1"}}}),
        )
        .unwrap();
    assert!(init["result"].is_object());
    mcp.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let reply = mcp
        .send(
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
            "name":"get_effective_context","arguments":{}}}),
        )
        .unwrap();
    assert_eq!(reply["result"]["isError"], false);
    let reply = mcp
        .send(
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
            "name":"get_handoff","arguments":{"task_id":"missing-task"}}}),
        )
        .unwrap();
    assert_eq!(reply["result"]["isError"], true);
    drop(mcp);

    // A stale credential is refused at the handshake and logged without a project.
    let stale: Credential = {
        let mut c = serde_json::to_value(f.daemon.credential("agent-beta.cap")).unwrap();
        c["token"] = match &c["token"] {
            Value::String(s) => json!("0".repeat(s.len())),
            Value::Array(a) => json!(vec![0; a.len()]),
            other => panic!("unexpected token shape {other}"),
        };
        serde_json::from_value(c).unwrap()
    };
    assert!(f.daemon.client(&stale).is_err());

    let alpha = events(&f, json!({"project":"alpha"}));
    let kinds: Vec<&str> = alpha.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["refused", "handoff", "context", "started"]);
    assert_eq!(alpha[0]["project"], Value::Null);
    assert_eq!(alpha[0]["outcome"], "Unauthorized");
    assert_eq!(alpha[1]["outcome"], "NotFound");
    assert_eq!(alpha[1]["client"], "synthetic-client 9.1");
    let context = &alpha[2];
    assert_eq!(context["via"], "mcp");
    assert_eq!(context["outcome"], "ok");
    assert_eq!(context["counts"]["preferences"], 1);
    assert_eq!(context["counts"]["contract_status"], "not_configured");
    assert_eq!(alpha[3]["client"], Value::Null);
    assert!(events(&f, json!({"project":"beta"})).len() == 1);
    let all = f.daemon.control("activity.list", json!({})).unwrap();
    let text = all.to_string();
    assert!(!text.contains(SECRET) && !text.contains("missing-task"));
    assert!(!text.contains(f.temp.path().to_str().unwrap()));
    let before = alpha[1]["id"].as_u64().unwrap();
    let older = events(&f, json!({"project":"alpha","before":before}));
    assert_eq!(older.len(), 2);

    // Agents cannot read the log; malformed reads are refused.
    let agent = f.daemon.credential("agent-alpha.cap");
    assert_eq!(
        f.daemon.call(&agent, "activity.list", json!({})),
        Err(Error::Unauthorized)
    );
    for bad in [json!({"project":"../x"}), json!({"limit":5})] {
        assert_eq!(
            f.daemon.control("activity.list", bad),
            Err(Error::InvalidRequest)
        );
    }
}
