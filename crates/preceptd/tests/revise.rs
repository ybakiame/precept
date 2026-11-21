//! Decision notes and edited suggestions through a real daemon: a note stays
//! with its decision, an edit replaces a suggestion with a new one in one
//! write (still to be confirmed), and the Profile verifies after both.
#[allow(dead_code)] // Shared harness; this suite uses part of it.
mod support;
use precept_core::operator::{Channel, Origin, RuleOptions};
use precept_ipc::Error;
use precept_write::{Binding, Command, Expected, Scope};
use serde_json::{Value, json};
use support::*;

fn suggest(f: &Fixture, b: &Binding, op: &str, key: &str, tags: &[&str]) -> (i64, String) {
    let receipt = f
        .daemon
        .mutate(
            b,
            op,
            Command::SuggestPreference {
                scope: Scope::Global,
                key: key.into(),
                value: "Keep commits small.".into(),
                tags: tags.iter().map(|t| t.to_string()).collect(),
                origin: Origin::Agent,
                rule: RuleOptions::default(),
                expected: Expected::Absent,
            },
        )
        .unwrap();
    let id: i64 = receipt["affected"][0]["record_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let record = get(f, id);
    (id, record["proposal_digest"].as_str().unwrap().to_owned())
}
fn get(f: &Fixture, id: i64) -> Value {
    f.daemon
        .control(
            "profile.preferences.get",
            json!({"scope":Scope::Global,"id":id}),
        )
        .unwrap()["record"]
        .clone()
}
fn history(f: &Fixture, id: i64, revision: i64) -> Vec<Value> {
    f.daemon
        .control(
            "profile.review.inspect",
            json!({"scope":Scope::Global,"id":id,"revision":revision}),
        )
        .unwrap()["decisions"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn notes_stay_with_decisions_and_an_edit_replaces_a_suggestion_in_one_write() {
    let mut f = Fixture::created();
    let b = f.daemon.enable();
    // A rejection with a note.
    let (id, digest) = suggest(&f, &b, "s1", "commits", &["git"]);
    let reject = |note: &str| Command::RejectPreference {
        id,
        scope: Scope::Global,
        proposal_digest: digest.clone(),
        channel: Channel::Desktop,
        expected: Expected::Revision(1),
        note: Some(note.into()),
    };
    assert_eq!(
        f.daemon
            .mutate(&b, "r-long", reject(&"x".repeat(501)))
            .err(),
        Some(Error::InvalidRequest)
    );
    f.daemon
        .mutate(&b, "r1", reject("Wrong category; filed again as #2."))
        .unwrap();
    let h = history(&f, id, 2);
    assert_eq!(h[0]["action"], "reject");
    assert_eq!(h[0]["note"], "Wrong category; filed again as #2.");

    // An edit: the new record is suggested and the operator's; the old one is
    // rejected by `replace`, naming it.
    let (old, digest) = suggest(&f, &b, "s2", "commits-2", &["style"]);
    let revise = |digest: String, key: &str| Command::ReviseSuggestion {
        id: old,
        scope: Scope::Global,
        proposal_digest: digest,
        key: key.into(),
        value: "Keep commits small and focused.".into(),
        tags: vec!["git".into()],
        rule: RuleOptions::default(),
        channel: Channel::Desktop,
        expected: Expected::Revision(1),
        note: Some("Tag corrected.".into()),
    };
    assert_eq!(
        f.daemon
            .mutate(&b, "v-stale", revise("0".repeat(64), "commits-2"))
            .err(),
        Some(Error::ReviewMismatch)
    );
    let receipt = f
        .daemon
        .mutate(&b, "v1", revise(digest.clone(), "commits-2"))
        .unwrap();
    let ids: Vec<&str> = receipt["affected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["record_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    let new: i64 = (old + 1).max(3);
    let fresh = get(&f, new);
    assert_eq!(fresh["status"], "suggested");
    assert_eq!(fresh["origin"], "operator");
    assert_eq!(fresh["tags"], json!(["git"]));
    assert_eq!(fresh["value"], "Keep commits small and focused.");
    assert_eq!(get(&f, old)["status"], "rejected");
    let h = history(&f, old, 2);
    assert_eq!(h[0]["action"], "replace");
    assert_eq!(h[0]["replaced_by"], new);
    assert_eq!(h[0]["note"], "Tag corrected.");
    // Only a suggestion can be edited.
    assert_eq!(
        f.daemon.mutate(&b, "v2", revise(digest, "again")).err(),
        Some(Error::Conflict)
    );

    // A new daemon re-verifies the whole Profile.
    f.daemon.kill();
    f.daemon = Daemon::relaunch(f.temp.path(), "run-b");
    f.daemon.ready();
    f.daemon
        .control("profile.select", json!({"path":f.db}))
        .expect("notes and replacements verify");
}
