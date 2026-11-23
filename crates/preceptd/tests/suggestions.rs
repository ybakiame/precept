//! Agent rule suggestions through a real daemon: only where the operator
//! allowed them, only while writes are on, deduplicated, always landing as
//! `suggested` with origin `agent` for the operator to review.
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

    // The same rule again is refused; the personal scope works; there is no
    // environment scope (machine facts are project memory); other projects
    // cannot be named.
    assert_eq!(suggest(&f, rule("fmt")), Err(Error::Conflict));
    let personal = json!({"scope":"personal","key":"tone","value":"Short answers."});
    assert_eq!(suggest(&f, personal).map(|_| ()), Ok(()));
    let env = json!({"scope":"environment","key":"shell","value":"WSL"});
    assert_eq!(suggest(&f, env), Err(Error::InvalidRequest));
    let other = json!({"scope":"project","key":"x","value":"y","project":"beta"});
    assert_eq!(suggest(&f, other), Err(Error::InvalidRequest));
    assert_eq!(
        suggest(&f, json!({"scope":"project","key":"empty","value":"  "})),
        Err(Error::InvalidRequest)
    );

    // Thirty new project suggestions plus the two initial records are all accepted.
    let mut accepted = 2;
    for i in 0..30 {
        suggest(&f, rule(&format!("rule-{i}"))).unwrap();
        accepted += 1;
    }
    assert_eq!(accepted, 32);

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
        "writes explicitly enabled for this run"
    );
    propose(&f, false);
    assert_eq!(suggest(&f, rule("off-again")), Err(Error::Unauthorized));
}

#[test]
fn an_agent_proposes_an_update_of_a_confirmed_rule_for_review() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    propose(&f, true);
    let env = json!({"kind":"project","name":"alpha"});
    let filed = suggest(
        &f,
        json!({"scope":"project","key":"toolchain","value":"Rust 1.98 via rustup.","tags":["rust"]}),
    )
    .unwrap();
    let old: i64 = filed["id"].as_str().unwrap().parse().unwrap();
    let inspect = |id: i64, revision: i64| {
        f.daemon
            .control(
                "profile.review.inspect",
                json!({"scope":env,"id":id,"revision":revision}),
            )
            .unwrap()
    };
    let digest = inspect(old, 1)["record"]["proposal_digest"].clone();
    f.daemon
        .mutate(
            &b,
            "confirm-old",
            precept_write::Command::ConfirmPreference {
                id: old,
                scope: precept_write::Scope::Project {
                    name: "alpha".into(),
                },
                proposal_digest: digest.as_str().unwrap().into(),
                channel: precept_core::operator::Channel::Desktop,
                expected: precept_write::Expected::Revision(1),
                note: None,
            },
        )
        .unwrap();
    // The agent sees the rule's id in its context and proposes the new fact.
    let agent = f.daemon.credential("agent-alpha.cap");
    let context = f.daemon.call(&agent, "context.get", json!({})).unwrap();
    assert!(
        context["preferences"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == old)
    );
    // A wrong id, or one of another scope, is refused.
    assert_eq!(
        suggest(&f, json!({"scope":"project","value":"x","replaces":9999})),
        Err(Error::NotFound)
    );
    assert_eq!(
        suggest(&f, json!({"scope":"personal","value":"x","replaces":old})),
        Err(Error::NotFound)
    );
    let update = suggest(
        &f,
        json!({"scope":"project","value":"Rust 1.99 via rustup.","replaces":old}),
    )
    .unwrap();
    let new: i64 = update["id"].as_str().unwrap().parse().unwrap();
    let proposed = inspect(new, 1);
    assert_eq!(proposed["record"]["key"], "toolchain");
    assert_eq!(proposed["record"]["tags"], json!(["rust"]));
    assert_eq!(proposed["record"]["status"], "suggested");
    assert_eq!(proposed["record"]["origin"], "agent");
    // Review names the rule it would supersede.
    assert_eq!(proposed["peer_demotions"][0]["id"], old);
    // Until confirmed, agents keep reading the old fact.
    let context = f.daemon.call(&agent, "context.get", json!({})).unwrap();
    assert!(context.to_string().contains("Rust 1.98"));
    assert!(!context.to_string().contains("Rust 1.99"));
}
