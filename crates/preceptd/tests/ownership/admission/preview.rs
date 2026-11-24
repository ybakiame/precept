use super::super::denied;
use super::super::support::{Fixture, logical, suggest};
use precept_ipc::Error;
use precept_write::{Command, Expected, Scope};
use serde_json::json;

#[test]
fn operator_preview_exact_scope_bounds_roles_and_malformed_state_preservation() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    f.register(&b, "beta");
    for i in 0..14 {
        f.daemon
            .mutate(&b, &format!("preview-{i}"), suggest())
            .unwrap();
    }
    f.daemon
        .mutate(
            &b,
            "project-secret",
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
    let params = json!({"scope":{"kind":"global"},"after_id":0});
    let page = f
        .daemon
        .control("profile.preferences.list", params.clone())
        .unwrap();
    assert_eq!(page["records"].as_array().unwrap().len(), 12);
    assert_eq!(page["next_after_id"], 12);
    let next = f
        .daemon
        .control(
            "profile.preferences.list",
            json!({"scope":{"kind":"global"},"after_id":12}),
        )
        .unwrap();
    assert_eq!(next["records"].as_array().unwrap().len(), 2);
    assert!(next["next_after_id"].is_null());
    assert!(
        f.daemon
            .control(
                "profile.preferences.list",
                json!({"scope":{"kind":"project","name":"alpha"},"after_id":0})
            )
            .unwrap()["records"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for name in ["agent-alpha.cap", "operator-write.cap"] {
        let cap = f.daemon.credential(name);
        let mut peer = f.daemon.client(&cap).unwrap();
        for (method, params) in [
            ("profile.preferences.list", params.clone()),
            (
                "profile.preferences.get",
                json!({"scope":{"kind":"global"},"id":1}),
            ),
            (
                "profile.handoff.get",
                json!({"project":"alpha","task_id":"task"}),
            ),
        ] {
            let before = logical(&f.db);
            assert_eq!(
                f.daemon.runtime.block_on(peer.request(method, params)),
                Err(Error::Unauthorized)
            );
            assert_eq!(logical(&f.db), before);
        }
    }
    for params in [
        json!({"scope":{"kind":"global"},"after_id":-1}),
        json!({"scope":{"kind":"global","name":"beta"},"after_id":0}),
        json!({"scope":{"kind":"global"},"after_id":0,"sql":"never"}),
    ] {
        denied(
            &f,
            "profile.preferences.list",
            params,
            Error::InvalidRequest,
        );
    }
    let context = f
        .daemon
        .control(
            "context.get",
            json!({"project":"alpha","include_documents":false}),
        )
        .unwrap();
    assert!(context["preferences"].as_array().unwrap().is_empty());
}
