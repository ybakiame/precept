//! Agent rule suggestions through a real daemon: only where the operator
//! allowed them, only while writes are on, bounded and deduplicated, always
//! landing as `suggested` with origin `agent` for the operator to review.
#[allow(dead_code)] // Shared harness; this suite uses part of it.
mod support;
use precept_ipc::Error;
use serde_json::{Value, json};
use support::*;

fn suggest(f: &Fixture, body: Value) -> precept_ipc::Result<Value> {
    let agent = f.daemon.credential("agent-alpha.cap");
    f.daemon.call(&agent, "preference.suggest", body)
}
fn rule(key: &str) -> Value {
    json!({"scope":"project","key":key,"value":"Run cargo fmt before committing.","tags":["Style"],"client":"test agent"})
}
fn propose(f: &Fixture, enabled: bool) -> Value {
    f.daemon
        .control("project.propose", json!({"name":"alpha","enabled":enabled}))
        .unwrap()
}

#[test]
fn agents_suggest_only_where_allowed_while_writes_are_on_and_never_confirm() {
    let mut f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    // Off by default.
    let listed = f.daemon.control("project.list", json!({})).unwrap();
    assert_eq!(listed[0]["propose"], false);
    assert_eq!(suggest(&f, rule("fmt")), Err(Error::Unauthorized));
    // The operator method is not an agent's.
    let agent = f.daemon.credential("agent-alpha.cap");
    assert_eq!(
        f.daemon.call(
            &agent,
            "project.propose",
            json!({"name":"alpha","enabled":true})
        ),
        Err(Error::Unauthorized)
    );
    assert_eq!(propose(&f, true)["propose"], true);

    let done = suggest(&f, rule("fmt")).unwrap();
    assert_eq!(done["status"], "suggested");
    let id: i64 = done["id"].as_str().unwrap().parse().unwrap();
    let record = f
        .daemon
        .control(
            "profile.preferences.get",
            json!({"scope":{"kind":"project","name":"alpha"},"id":id}),
        )
        .unwrap()["record"]
        .clone();
    assert_eq!(record["status"], "suggested");
    assert_eq!(record["origin"], "agent");
    assert_eq!(record["tags"], json!(["style"]));
    // Not delivered: suggestions never reach agents' context.
    let context = f.daemon.call(&agent, "context.get", json!({})).unwrap();
    assert!(!context.to_string().contains("cargo fmt"));

    // The same rule again is refused; personal and environment scopes work;
    // environment notes stay advisory; other projects cannot be named.
    assert_eq!(suggest(&f, rule("fmt")), Err(Error::Conflict));
    let personal = json!({"scope":"personal","key":"tone","value":"Short answers."});
    assert_eq!(suggest(&f, personal).map(|_| ()), Ok(()));
    let hard_env = json!({"scope":"environment","key":"shell","value":"WSL","enforcement":"hard"});
    assert_eq!(suggest(&f, hard_env), Err(Error::InvalidRequest));
    let other = json!({"scope":"project","key":"x","value":"y","project":"beta"});
    assert_eq!(suggest(&f, other), Err(Error::InvalidRequest));
    assert_eq!(
        suggest(&f, json!({"scope":"project","key":"empty","value":"  "})),
        Err(Error::InvalidRequest)
    );

    // A bounded number per daemon run.
    let mut accepted = 2;
    let mut last = Ok(Value::Null);
    for i in 0..30 {
        last = suggest(&f, rule(&format!("rule-{i}")));
        if last.is_err() {
            break;
        }
        accepted += 1;
    }
    assert_eq!(last, Err(Error::Capacity));
    assert_eq!(accepted, 20);

    let events = f
        .daemon
        .control("activity.list", json!({"project":"alpha"}))
        .unwrap();
    assert!(
        events["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "suggested" && e["outcome"] == "ok")
    );

    // The switch persists; with writes off nothing is accepted or queued.
    f.daemon.kill();
    f.daemon = Daemon::relaunch(f.temp.path(), "run-b");
    f.daemon.ready();
    f.daemon
        .control("profile.select", json!({"path":f.db}))
        .unwrap();
    let listed = f.daemon.control("project.list", json!({})).unwrap();
    assert_eq!(listed[0]["propose"], true);
    assert_eq!(suggest(&f, rule("after-restart")), Err(Error::Unavailable));
    f.daemon.enable();
    assert!(
        suggest(&f, rule("after-restart")).is_ok(),
        "a new run, a new budget"
    );
    propose(&f, false);
    assert_eq!(suggest(&f, rule("off-again")), Err(Error::Unauthorized));
}
