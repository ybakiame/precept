use super::super::denied;
use super::super::support::Fixture;
use precept_ipc::Error;
use serde_json::json;

#[test]
fn unselected_bootstrap_strict_selection_and_no_implicit_grant() {
    let f = Fixture::unselected();
    assert_eq!(
        f.daemon.control("daemon.health", json!({})).unwrap()["profile_selected"],
        false
    );
    assert_eq!(
        f.daemon.control("profile.info", json!({})).unwrap()["selected"],
        false
    );
    // The daemon's own project set is listed without a Profile (nothing is
    // registered yet); every Profile-backed read stays unavailable.
    let listed = f.daemon.control("project.list", json!({})).unwrap();
    assert!(
        listed
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["state"] == "unregistered")
    );
    let agent = f.daemon.credential("agent-alpha.cap");
    assert_eq!(
        f.daemon.call(&agent, "context.get", json!({})),
        Err(Error::Unavailable)
    );
    assert_eq!(
        f.daemon
            .call(&agent, "profile.create", json!({"path":f.db})),
        Err(Error::Unauthorized)
    );
    assert_eq!(
        f.daemon.control("profile.select", json!({"path":f.db})),
        Err(Error::UnsafeRuntime)
    );
    assert!(!f.db.exists());
    assert_eq!(
        f.daemon
            .control("profile.create", json!({"path":f.db,"extra":true})),
        Err(Error::InvalidRequest)
    );
    assert_eq!(
        f.daemon
            .control("profile.create", json!({"path":"relative"})),
        Err(Error::InvalidRequest)
    );
    let info = f
        .daemon
        .control("profile.create", json!({"path":f.db}))
        .unwrap();
    assert_eq!(info["binding"]["owner_epoch"], 0);
    assert_eq!(info["write_enabled"], false);
    assert!(
        !f.daemon
            .location
            .runtime_dir
            .join("operator-write.cap")
            .exists()
    );
    denied(
        &f,
        "profile.select",
        json!({"path":f.db}),
        Error::RestartRequired,
    );
    denied(
        &f,
        "profile.create",
        json!({"path":f.db}),
        Error::RestartRequired,
    );
    denied(
        &f,
        "profile.write.enable",
        json!({"profile_id":info["binding"]["profile_id"],"generation":info["binding"]["generation"],"confirm":false}),
        Error::InvalidRequest,
    );
    denied(
        &f,
        "profile.write.enable",
        json!({"profile_id":"00000000000000000000000000000000","generation":info["binding"]["generation"],"confirm":true}),
        Error::Unauthorized,
    );
}
