use super::super::support::{Fixture, logical, suggest};
use super::super::{handoff, preview_digest, request};
use precept_core::operator::Channel;
use precept_ipc::Error;
use precept_write::{Command, Expected, Scope};
use serde_json::json;

#[test]
fn seven_real_ipc_commands_scope_cas_transitions_and_denials() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    assert_eq!(b.owner_epoch, 1);
    f.register(&b, "alpha");
    f.register(&b, "beta");
    let first = f.daemon.mutate(&b, "suggest-1", suggest()).unwrap();
    let id = first["affected"][0]["record_id"]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
    // Every synthetic suggestion here is the same global proposal, so one
    // previewed digest binds all of these decisions.
    let digest = preview_digest(&f, &Scope::Global, id);
    f.daemon
        .mutate(
            &b,
            "confirm-1",
            Command::ConfirmPreference {
                id,
                scope: Scope::Global,
                proposal_digest: digest.clone(),
                channel: Channel::Cli,
                expected: Expected::Revision(1),
                note: None,
            },
        )
        .unwrap();
    let second = f.daemon.mutate(&b, "suggest-2", suggest()).unwrap();
    let id2 = second["affected"][0]["record_id"]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
    let confirmed = f
        .daemon
        .mutate(
            &b,
            "confirm-2",
            Command::ConfirmPreference {
                id: id2,
                scope: Scope::Global,
                proposal_digest: digest.clone(),
                channel: Channel::Cli,
                expected: Expected::Revision(1),
                note: None,
            },
        )
        .unwrap();
    assert_eq!(confirmed["affected"].as_array().unwrap().len(), 2);
    f.daemon
        .mutate(
            &b,
            "deprecate-2",
            Command::DeprecatePreference {
                id: id2,
                scope: Scope::Global,
                proposal_digest: digest.clone(),
                channel: Channel::Cli,
                expected: Expected::Revision(2),
                note: None,
            },
        )
        .unwrap();
    let third = f.daemon.mutate(&b, "suggest-3", suggest()).unwrap();
    let id3 = third["affected"][0]["record_id"]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
    f.daemon
        .mutate(
            &b,
            "reject-3",
            Command::RejectPreference {
                id: id3,
                scope: Scope::Global,
                proposal_digest: digest.clone(),
                channel: Channel::Cli,
                expected: Expected::Revision(1),
                note: None,
            },
        )
        .unwrap();
    f.daemon
        .mutate(
            &b,
            "delete-3",
            Command::DeletePreference {
                id: id3,
                scope: Scope::Global,
                proposal_digest: digest.clone(),
                channel: Channel::Cli,
                expected: Expected::Revision(2),
            },
        )
        .unwrap();
    f.daemon
        .mutate(&b, "handoff-1", handoff("alpha", Expected::Absent))
        .unwrap();
    f.daemon
        .mutate(&b, "handoff-2", handoff("alpha", Expected::Revision(1)))
        .unwrap();
    f.daemon
        .mutate(&b, "handoff-beta", handoff("beta", Expected::Absent))
        .unwrap();
    let c = f.daemon.credential("operator-write.cap");
    for (op, command, error) in [
        (
            "bad-transition",
            Command::ConfirmPreference {
                id,
                scope: Scope::Global,
                proposal_digest: digest.clone(),
                channel: Channel::Cli,
                expected: Expected::Revision(3),
                note: None,
            },
            Error::InvalidTransition,
        ),
        (
            "bad-cas",
            Command::DeletePreference {
                id,
                scope: Scope::Global,
                proposal_digest: digest.clone(),
                channel: Channel::Cli,
                expected: Expected::Revision(999),
            },
            Error::Conflict,
        ),
        (
            "bad-scope",
            Command::DeletePreference {
                id,
                scope: Scope::Project {
                    name: "beta".into(),
                },
                proposal_digest: digest.clone(),
                channel: Channel::Cli,
                expected: Expected::Revision(3),
            },
            Error::ScopeMismatch,
        ),
        (
            "bad-root",
            Command::RegisterProject {
                name: "alpha".into(),
                root: f
                    .temp
                    .path()
                    .join("beta")
                    .canonicalize()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .into(),
                expected: Expected::Absent,
            },
            Error::Unauthorized,
        ),
        (
            "bad-project",
            handoff("gamma", Expected::Absent),
            Error::Unauthorized,
        ),
    ] {
        let before = logical(&f.db);
        assert_eq!(
            f.daemon
                .call(&c, "profile.mutate", request(&b, op, command)),
            Err(error)
        );
        assert_eq!(logical(&f.db), before);
    }
    let before = logical(&f.db);
    assert_eq!(f.daemon.mutate(&b, "suggest-1", suggest()).unwrap(), first);
    assert_eq!(logical(&f.db), before);
    let mut malformed = request(&b, "malformed", suggest());
    malformed["extra"] = json!(true);
    assert_eq!(
        f.daemon.call(&c, "profile.mutate", malformed),
        Err(Error::InvalidRequest)
    );
    // The retired v1 envelope is refused; origin is required and closed-set.
    let mut legacy = request(&b, "legacy-envelope", suggest());
    legacy["version"] = json!(1);
    assert_eq!(
        f.daemon.call(&c, "profile.mutate", legacy),
        Err(Error::InvalidRequest)
    );
    let mut missing = request(&b, "missing-origin", suggest());
    missing["command"].as_object_mut().unwrap().remove("origin");
    assert_eq!(
        f.daemon.call(&c, "profile.mutate", missing),
        Err(Error::InvalidRequest)
    );
    let mut unknown = request(&b, "unknown-origin", suggest());
    unknown["command"]["origin"] = json!("verified");
    assert_eq!(
        f.daemon.call(&c, "profile.mutate", unknown),
        Err(Error::InvalidRequest)
    );
    let mut stale = request(&b, "stale", suggest());
    stale["binding"]["owner_epoch"] = json!(0);
    assert_eq!(
        f.daemon.call(&c, "profile.mutate", stale),
        Err(Error::Unauthorized)
    );
    assert_eq!(logical(&f.db), before);
    let mut changed = suggest();
    if let Command::SuggestPreference { value, .. } = &mut changed {
        *value = "different synthetic value".into();
    }
    assert_eq!(
        f.daemon.mutate(&b, "suggest-1", changed),
        Err(Error::IdempotencyConflict)
    );
    assert_eq!(logical(&f.db), before);
    assert_eq!(
        f.daemon
            .control("handoff.get", json!({"project":"alpha","task_id":"shared"}))
            .unwrap()["project"],
        "alpha"
    );
}
