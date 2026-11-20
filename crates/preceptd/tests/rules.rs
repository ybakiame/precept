//! The rule model through a real daemon: applies_to narrows delivery only on a
//! definite mismatch, priority decides inside a key, environment notes stand
//! apart, the operator's switch takes a rule out of context and back, and the
//! Profile still verifies after every write.
#[allow(dead_code)] // Shared harness; this suite uses part of it.
mod support;
use precept_core::operator::{AppliesTo, Channel, Enforcement, Origin, RuleOptions};
use precept_ipc::Error;
use precept_write::{Binding, Command, Expected, Scope};
use serde_json::{Value, json};
use support::*;

fn suggest(
    f: &Fixture,
    b: &Binding,
    op: &str,
    scope: Scope,
    key: &str,
    value: &str,
    rule: RuleOptions,
) -> Result<(i64, String), Error> {
    let receipt = f.daemon.mutate(
        b,
        op,
        Command::SuggestPreference {
            scope: scope.clone(),
            key: key.into(),
            value: value.into(),
            tags: vec!["rules".into()],
            origin: Origin::Operator,
            rule,
            expected: Expected::Absent,
        },
    )?;
    let id: i64 = receipt["affected"][0]["record_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let preview = f
        .daemon
        .control("profile.preferences.get", json!({"scope":scope,"id":id}))
        .unwrap();
    Ok((
        id,
        preview["record"]["proposal_digest"]
            .as_str()
            .unwrap()
            .to_owned(),
    ))
}

fn confirm(f: &Fixture, b: &Binding, op: &str, scope: Scope, id: i64, digest: String) {
    f.daemon
        .mutate(
            b,
            op,
            Command::ConfirmPreference {
                id,
                scope,
                proposal_digest: digest,
                channel: Channel::Desktop,
                expected: Expected::Revision(1),
            },
        )
        .unwrap();
}

fn context(f: &Fixture, selector: Value) -> Vec<Value> {
    let agent = f.daemon.credential("agent-alpha.cap");
    f.daemon
        .call(&agent, "context.get", json!({"selector":selector}))
        .unwrap()["preferences"]
        .as_array()
        .unwrap()
        .clone()
}

fn values(prefs: &[Value]) -> Vec<String> {
    prefs
        .iter()
        .map(|p| p["value"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn applies_to_priority_environment_and_the_switch_shape_what_agents_receive() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    let alpha = Scope::Project {
        name: "alpha".into(),
    };
    let env = Scope::Environment {
        name: "alpha".into(),
    };
    let rust_only = RuleOptions {
        applies_to: AppliesTo {
            languages: vec!["rust".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    // Two project rules for one key: the higher priority wins.
    let (low, d) = suggest(
        &f,
        &b,
        "s1",
        alpha.clone(),
        "style",
        "low",
        RuleOptions::default(),
    )
    .unwrap();
    confirm(&f, &b, "c1", alpha.clone(), low, d);
    let (high, d) = suggest(
        &f,
        &b,
        "s2",
        alpha.clone(),
        "style",
        "high",
        RuleOptions {
            priority: 10,
            enforcement: Enforcement::Verify,
            ..Default::default()
        },
    )
    .unwrap();
    // Confirming a second rule of one key demotes the first (one confirmed per key).
    confirm(&f, &b, "c2", alpha.clone(), high, d);
    let (rust, d) = suggest(&f, &b, "s3", Scope::Global, "fmt", "rustfmt", rust_only).unwrap();
    confirm(&f, &b, "c3", Scope::Global, rust, d);
    let (note, d) = suggest(
        &f,
        &b,
        "s4",
        env.clone(),
        "shell",
        "agents run inside WSL",
        RuleOptions::default(),
    )
    .unwrap();
    confirm(&f, &b, "c4", env.clone(), note, d);
    // Environment notes are advisory only.
    assert_eq!(
        suggest(
            &f,
            &b,
            "s5",
            env.clone(),
            "x",
            "y",
            RuleOptions {
                enforcement: Enforcement::Hard,
                ..Default::default()
            }
        )
        .err(),
        Some(Error::InvalidRequest)
    );

    let all = context(&f, json!({}));
    assert_eq!(values(&all), ["rustfmt", "high", "agents run inside WSL"]);
    let fmt = &all[0];
    assert_eq!(fmt["applies_to"]["languages"], json!(["rust"]));
    assert_eq!(all[1]["priority"], 10);
    assert_eq!(all[1]["enforcement"], "verify");
    assert_eq!(all[2]["scope"], "environment");
    // A definite mismatch drops the rule; a match keeps it.
    assert_eq!(
        values(&context(&f, json!({"language":"python"}))),
        ["high", "agents run inside WSL"]
    );
    assert_eq!(
        values(&context(&f, json!({"language":"rust"})))[0],
        "rustfmt"
    );

    // The switch: only confirmed rules, revision-checked, out of context and back.
    let rev = |id: i64, scope: &Scope| {
        f.daemon
            .control("profile.preferences.get", json!({"scope":scope,"id":id}))
            .unwrap()["record"]["revision"]
            .as_i64()
            .unwrap()
    };
    let switch = |op: &str, id: i64, scope: Scope, enabled: bool, expected: i64| {
        f.daemon.mutate(
            &b,
            op,
            Command::SetPreferenceEnabled {
                id,
                scope,
                enabled,
                channel: Channel::Desktop,
                expected: Expected::Revision(expected),
            },
        )
    };
    let r = rev(rust, &Scope::Global);
    assert_eq!(
        switch("off-stale", rust, Scope::Global, false, r + 1).err(),
        Some(Error::Conflict)
    );
    switch("off", rust, Scope::Global, false, r).unwrap();
    let record = f
        .daemon
        .control(
            "profile.preferences.get",
            json!({"scope":Scope::Global,"id":rust}),
        )
        .unwrap();
    assert_eq!(record["record"]["enabled"], false);
    assert_eq!(
        values(&context(&f, json!({"language":"rust"}))),
        ["high", "agents run inside WSL"]
    );
    assert_eq!(
        switch("off-again", rust, Scope::Global, false, r + 1).err(),
        Some(Error::InvalidTransition)
    );
    assert_eq!(
        switch("demoted", low, alpha.clone(), false, rev(low, &alpha)).err(),
        Some(Error::InvalidTransition)
    );
    switch("on", rust, Scope::Global, true, r + 1).unwrap();
    assert_eq!(
        values(&context(&f, json!({"language":"rust"})))[0],
        "rustfmt"
    );

    let info = f.daemon.control("profile.info", json!({})).unwrap();
    assert_eq!(info["selected"], true);
    let review = f
        .daemon
        .control(
            "profile.review.inspect",
            json!({"scope":Scope::Global,"id":rust,"revision":r + 2}),
        )
        .unwrap();
    let actions: Vec<&str> = review["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["action"].as_str().unwrap())
        .collect();
    assert_eq!(actions, ["enable", "disable", "confirm"], "newest first");

    // A new daemon re-verifies the whole Profile when it is selected again.
    let mut f = f;
    f.daemon.kill();
    f.daemon = Daemon::relaunch(f.temp.path(), "run-a");
    f.daemon.ready();
    f.daemon
        .control("profile.select", json!({"path":f.db}))
        .expect("the switched Profile verifies");
}
