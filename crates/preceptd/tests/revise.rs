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
    let revise = |digest: String, key: &str| Command::RevisePreference {
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
    // The replaced suggestion is closed: editing it again is stale.
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

#[test]
fn an_edit_of_a_confirmed_record_takes_its_place_and_keeps_the_earlier_text() {
    let mut f = Fixture::created();
    let b = f.daemon.enable();
    let confirm = |f: &Fixture, op: &str, id: i64, digest: &str| {
        f.daemon
            .mutate(
                &b,
                op,
                Command::ConfirmPreference {
                    id,
                    scope: Scope::Global,
                    proposal_digest: digest.into(),
                    channel: Channel::Desktop,
                    expected: Expected::Revision(1),
                    note: None,
                },
            )
            .unwrap();
    };
    let (old, digest) = suggest(&f, &b, "s1", "editor", &["tools"]);
    confirm(&f, "c1", old, &digest);
    // Another confirmed record under the key the edit moves to.
    let (peer, peer_digest) = suggest(&f, &b, "s2", "editor-choice", &["tools"]);
    confirm(&f, "c2", peer, &peer_digest);
    let edit = |op: &str, expected: i64| {
        f.daemon.mutate(
            &b,
            op,
            Command::RevisePreference {
                id: old,
                scope: Scope::Global,
                proposal_digest: digest.clone(),
                key: "editor-choice".into(),
                value: "Use Helix; Vim keys elsewhere.".into(),
                tags: vec!["tools".into()],
                rule: RuleOptions::default(),
                channel: Channel::Desktop,
                expected: Expected::Revision(expected),
                note: Some("Switched editors.".into()),
            },
        )
    };
    assert_eq!(edit("e-stale", 1).err(), Some(Error::Conflict));
    let receipt = edit("e1", 2).unwrap();
    assert_eq!(receipt["affected"].as_array().unwrap().len(), 3);
    let new = peer + 1;
    let fresh = get(&f, new);
    assert_eq!(fresh["status"], "confirmed");
    assert_eq!(fresh["key"], "editor-choice");
    assert_eq!(fresh["value"], "Use Helix; Vim keys elsewhere.");
    // The earlier text stays, deprecated, naming its replacement.
    let earlier = get(&f, old);
    assert_eq!(earlier["status"], "deprecated");
    assert_eq!(earlier["value"], "Keep commits small.");
    let h = history(&f, old, 3);
    assert_eq!(h[0]["action"], "replace");
    assert_eq!(h[0]["replaced_by"], new);
    assert_eq!(h[0]["note"], "Switched editors.");
    assert_eq!(history(&f, new, 2)[0]["action"], "confirm");
    // The other record of that key stepped aside.
    assert_eq!(get(&f, peer)["status"], "deprecated");
    assert_eq!(history(&f, peer, 3)[0]["action"], "demote");
    // A switched-off rule is switched on before it is edited.
    let fresh_digest = fresh["proposal_digest"].as_str().unwrap().to_owned();
    f.daemon
        .mutate(
            &b,
            "off",
            Command::SetPreferenceEnabled {
                id: new,
                scope: Scope::Global,
                enabled: false,
                channel: Channel::Desktop,
                expected: Expected::Revision(2),
            },
        )
        .unwrap();
    assert_eq!(
        f.daemon
            .mutate(
                &b,
                "e2",
                Command::RevisePreference {
                    id: new,
                    scope: Scope::Global,
                    proposal_digest: fresh_digest,
                    key: "editor-choice".into(),
                    value: "x".into(),
                    tags: vec!["tools".into()],
                    rule: RuleOptions::default(),
                    channel: Channel::Desktop,
                    expected: Expected::Revision(3),
                    note: None,
                },
            )
            .err(),
        Some(Error::InvalidTransition)
    );
    // A new daemon re-verifies the whole Profile.
    f.daemon.kill();
    f.daemon = Daemon::relaunch(f.temp.path(), "run-b");
    f.daemon.ready();
    f.daemon
        .control("profile.select", json!({"path":f.db}))
        .expect("an edited confirmed record verifies");
}

#[test]
fn a_batch_decides_exact_suggestions_all_or_nothing() {
    use precept_core::operator::{BatchDecision, BatchItem};
    let mut f = Fixture::created();
    let b = f.daemon.enable();
    let (a, da) = suggest(&f, &b, "a", "alpha-key", &["git"]);
    let (bb, db) = suggest(&f, &b, "b", "beta-key", &["git"]);
    let (c, dc) = suggest(&f, &b, "c", "alpha-key", &["style"]);
    let item = |id: i64, digest: &str, revision: i64| BatchItem {
        id,
        scope: Scope::Global,
        proposal_digest: digest.into(),
        expected: Expected::Revision(revision),
    };
    let batch = |decision, items: Vec<BatchItem>| Command::DecideBatch {
        decision,
        items,
        channel: Channel::Desktop,
        note: Some("Imported again after the schema change.".into()),
    };
    // Two of one key in one confirm batch: refused, nothing written.
    assert_eq!(
        f.daemon
            .mutate(
                &b,
                "same-key",
                batch(
                    BatchDecision::Confirm,
                    vec![item(a, &da, 1), item(c, &dc, 1)]
                )
            )
            .err(),
        Some(Error::Conflict)
    );
    // A stale item refuses the whole batch.
    assert_eq!(
        f.daemon
            .mutate(
                &b,
                "stale",
                batch(
                    BatchDecision::Confirm,
                    vec![item(a, &da, 1), item(bb, &db, 2)]
                )
            )
            .err(),
        Some(Error::Conflict)
    );
    assert_eq!(get(&f, a)["status"], "suggested", "nothing written");
    let receipt = f
        .daemon
        .mutate(
            &b,
            "confirm-two",
            batch(
                BatchDecision::Confirm,
                vec![item(a, &da, 1), item(bb, &db, 1)],
            ),
        )
        .unwrap();
    assert_eq!(receipt["affected"].as_array().unwrap().len(), 2);
    assert_eq!(get(&f, a)["status"], "confirmed");
    assert_eq!(get(&f, bb)["status"], "confirmed");
    assert_eq!(
        history(&f, bb, 2)[0]["note"],
        "Imported again after the schema change."
    );
    f.daemon
        .mutate(
            &b,
            "reject-one",
            batch(BatchDecision::Reject, vec![item(c, &dc, 1)]),
        )
        .unwrap();
    assert_eq!(get(&f, c)["status"], "rejected");
    // Empty and oversized batches are malformed.
    assert_eq!(
        f.daemon
            .mutate(&b, "empty", batch(BatchDecision::Confirm, vec![]))
            .err(),
        Some(Error::InvalidRequest)
    );
    f.daemon.kill();
    f.daemon = Daemon::relaunch(f.temp.path(), "run-b");
    f.daemon.ready();
    f.daemon
        .control("profile.select", json!({"path":f.db}))
        .expect("batch decisions verify");
}
