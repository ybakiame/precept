use super::super::support::{Fixture, logical, suggest};
use super::super::{denied, preview_digest};
use precept_core::operator::Channel;
use precept_ipc::Error;
use precept_write::{Command, Expected, Scope};
use serde_json::json;

#[test]
fn review_queue_inspect_and_digest_binding_are_exact_and_operator_only() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    f.register(&b, "beta");
    for i in 0..14 {
        f.daemon
            .mutate(&b, &format!("review-suggest-{i}"), suggest())
            .unwrap();
    }
    f.daemon
        .mutate(
            &b,
            "review-project",
            Command::SuggestPreference {
                rule: Default::default(),
                scope: Scope::Project {
                    name: "beta".into(),
                },
                key: "k".into(),
                value: "synthetic-other-scope".into(),
                tags: vec!["test".into()],
                origin: precept_core::operator::Origin::Agent,
                expected: Expected::Absent,
            },
        )
        .unwrap();
    let global = json!({"kind":"global"});
    let queue = f
        .daemon
        .control("profile.review.queue", json!({"scope":global,"after_id":0}))
        .unwrap();
    assert_eq!(queue["records"].as_array().unwrap().len(), 12);
    assert_eq!(queue["next_after_id"], 12);
    assert_eq!(queue["commit_sequence"], 17);
    assert!(queue["records"].as_array().unwrap().iter().all(|r| {
        r["status"] == "suggested"
            && r["scope"]["kind"] == "global"
            && r["origin"] == "agent"
            && r["proposal_digest"].as_str().unwrap().len() == 64
    }));
    let next = f
        .daemon
        .control(
            "profile.review.queue",
            json!({"scope":global,"after_id":12}),
        )
        .unwrap();
    assert_eq!(next["records"].as_array().unwrap().len(), 2);
    assert!(next["next_after_id"].is_null());
    // Scopes are never unioned: the project row stays in its own queue.
    let project = f
        .daemon
        .control(
            "profile.review.queue",
            json!({"scope":{"kind":"project","name":"beta"},"after_id":0}),
        )
        .unwrap();
    assert_eq!(project["records"].as_array().unwrap().len(), 1);
    assert_eq!(project["records"][0]["id"], 15);
    let inspect = f
        .daemon
        .control(
            "profile.review.inspect",
            json!({"scope":global,"id":1,"revision":1}),
        )
        .unwrap();
    assert_eq!(inspect["record"]["id"], 1);
    assert_eq!(inspect["commit_sequence"], 17);
    assert!(inspect["decisions"].as_array().unwrap().is_empty());
    assert!(inspect["peer_demotions"].as_array().unwrap().is_empty());
    assert_eq!(inspect["peer_demotion_total"], 0);
    // A moved revision is a conflict, never a silently different preview.
    assert_eq!(
        f.daemon.control(
            "profile.review.inspect",
            json!({"scope":global,"id":1,"revision":2})
        ),
        Err(Error::Conflict)
    );
    assert_eq!(
        f.daemon.control(
            "profile.review.inspect",
            json!({"scope":global,"id":999,"revision":1})
        ),
        Err(Error::NotFound)
    );
    assert_eq!(
        f.daemon.control(
            "profile.review.inspect",
            json!({"scope":{"kind":"project","name":"alpha"},"id":1,"revision":1})
        ),
        Err(Error::NotFound)
    );
    let digest = preview_digest(&f, &Scope::Global, 1);
    f.daemon
        .mutate(
            &b,
            "review-confirm-1",
            Command::ConfirmPreference {
                id: 1,
                scope: Scope::Global,
                proposal_digest: digest.clone(),
                channel: Channel::Cli,
                expected: Expected::Revision(1),
                note: None,
            },
        )
        .unwrap();
    let inspect = f
        .daemon
        .control(
            "profile.review.inspect",
            json!({"scope":global,"id":2,"revision":1}),
        )
        .unwrap();
    assert_eq!(inspect["peer_demotions"].as_array().unwrap().len(), 1);
    assert_eq!(inspect["peer_demotions"][0]["id"], 1);
    assert_eq!(inspect["peer_demotions"][0]["revision"], 2);
    assert_eq!(inspect["peer_demotion_total"], 1);
    assert!(inspect["decisions"].as_array().unwrap().is_empty());
    // A substituted proposal at the same revision is ReviewMismatch, not a race.
    let before = logical(&f.db);
    assert_eq!(
        f.daemon.mutate(
            &b,
            "review-substituted",
            Command::ConfirmPreference {
                id: 2,
                scope: Scope::Global,
                proposal_digest: "0".repeat(64),
                channel: Channel::Desktop,
                expected: Expected::Revision(1),
                note: None,
            },
        ),
        Err(Error::ReviewMismatch)
    );
    assert_eq!(logical(&f.db), before);
    f.daemon
        .mutate(
            &b,
            "review-confirm-2",
            Command::ConfirmPreference {
                id: 2,
                scope: Scope::Global,
                proposal_digest: digest.clone(),
                channel: Channel::Desktop,
                expected: Expected::Revision(1),
                note: None,
            },
        )
        .unwrap();
    let history = f
        .daemon
        .control(
            "profile.review.inspect",
            json!({"scope":global,"id":1,"revision":3}),
        )
        .unwrap();
    let decisions = history["decisions"].as_array().unwrap();
    assert_eq!(decisions.len(), 2);
    assert_eq!(decisions[0]["action"], "demote");
    assert_eq!(decisions[0]["from_revision"], 2);
    assert_eq!(decisions[0]["to_revision"], 3);
    assert_eq!(decisions[0]["channel"], "desktop");
    assert_eq!(decisions[0]["proposal_digest"], digest);
    assert_eq!(
        decisions[0]["receipt_commit_sequence"],
        history["commit_sequence"]
    );
    assert_eq!(decisions[1]["action"], "confirm");
    assert_eq!(decisions[1]["from_revision"], 1);
    assert_eq!(decisions[1]["to_revision"], 2);
    assert_eq!(decisions[1]["channel"], "cli");
    let queue = f
        .daemon
        .control("profile.review.queue", json!({"scope":global,"after_id":0}))
        .unwrap();
    assert_eq!(queue["records"].as_array().unwrap().len(), 12);
    assert!(queue["next_after_id"].is_null());
    assert!(
        queue["records"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["id"] != 1 && r["id"] != 2)
    );
    for name in ["agent-alpha.cap", "operator-write.cap"] {
        let cap = f.daemon.credential(name);
        for (method, params) in [
            ("profile.review.queue", json!({"scope":global,"after_id":0})),
            (
                "profile.review.inspect",
                json!({"scope":global,"id":1,"revision":3}),
            ),
        ] {
            let before = logical(&f.db);
            assert_eq!(
                f.daemon.call(&cap, method, params),
                Err(Error::Unauthorized)
            );
            assert_eq!(logical(&f.db), before);
        }
    }
    for params in [
        json!({"scope":global,"after_id":-1}),
        json!({"scope":{"kind":"global","name":"beta"},"after_id":0}),
        json!({"scope":global,"after_id":0,"sql":"never"}),
    ] {
        denied(&f, "profile.review.queue", params, Error::InvalidRequest);
    }
    for params in [
        json!({"scope":global,"id":0,"revision":1}),
        json!({"scope":global,"id":1,"revision":0}),
        json!({"scope":global,"id":1,"revision":3,"extra":true}),
    ] {
        denied(&f, "profile.review.inspect", params, Error::InvalidRequest);
    }
    denied(
        &f,
        "profile.review.queue",
        json!({"scope":{"kind":"project","name":"outside"},"after_id":0}),
        Error::Unauthorized,
    );
    // Review history is operator-only and never reaches agent context.
    let context = f
        .daemon
        .control(
            "context.get",
            json!({"project":"alpha","include_documents":false}),
        )
        .unwrap();
    // Only the confirmed global preference is effective; suggested rows,
    // decision history and digests never reach agent-facing context.
    let preferences = context["preferences"].as_array().unwrap();
    assert_eq!(preferences.len(), 1);
    assert_eq!(preferences[0]["id"], 2);
    assert!(context.get("decisions").is_none());
    assert!(!context.to_string().contains(&digest));
}
