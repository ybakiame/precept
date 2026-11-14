mod support;
use acp_core::operator::Channel;
use acp_ipc::{Access, Error};
use acp_write::{Command, Expected, Request, Scope, Snapshot};
use serde_json::{Value, json};
use std::{
    fs, thread,
    time::{Duration, Instant},
};
use support::*;
fn handoff(project: &str, expected: Expected) -> Command {
    Command::PutHandoff {
        project: project.into(),
        task_id: "shared".into(),
        objective: "synthetic objective".into(),
        completed: String::new(),
        next_action: "inspect".into(),
        validation_json: "[]".into(),
        snapshot: Snapshot {
            base_sha: String::new(),
            branch: String::new(),
            state: "unknown".into(),
            fingerprint: None,
        },
        expected,
    }
}
fn request(b: &acp_write::Binding, op: &str, command: Command) -> Value {
    serde_json::to_value(Request {
        version: 2,
        binding: b.clone(),
        operation_id: op.into(),
        command,
    })
    .unwrap()
}
fn denied(f: &Fixture, method: &str, params: Value, error: Error) {
    let before = logical(&f.db);
    assert_eq!(f.daemon.control(method, params), Err(error));
    assert_eq!(logical(&f.db), before);
}
/// The digest a real operator preview returns; never a guessed constant.
fn preview_digest(f: &Fixture, scope: &Scope, id: i64) -> String {
    f.daemon
        .control("profile.preferences.get", json!({"scope":scope,"id":id}))
        .unwrap()["record"]["proposal_digest"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn unselected_bootstrap_strict_selection_and_no_implicit_grant() {
    let f = Fixture::unselected(false);
    assert_eq!(
        f.daemon.control("daemon.health", json!({})).unwrap()["profile_selected"],
        false
    );
    assert_eq!(
        f.daemon.control("profile.info", json!({})).unwrap()["selected"],
        false
    );
    assert_eq!(
        f.daemon.control("project.list", json!({})),
        Err(Error::Unavailable)
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
        f.daemon.control(
            "session.list",
            json!({"session_api_version":acp_session::SESSION_API,"project":"alpha"})
        ),
        Err(Error::Unavailable)
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

#[test]
fn seven_real_ipc_commands_scope_cas_transitions_and_denials() {
    let f = Fixture::created(false);
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

#[test]
fn distinct_write_role_revokes_established_peers_and_stale_binding() {
    let f = Fixture::created(false);
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    let write = f.daemon.credential("operator-write.cap");
    let agent = f.daemon.credential("agent-alpha.cap");
    let mut established = f.daemon.client(&write).unwrap();
    let mut read = f
        .daemon
        .client(&f.daemon.credential("operator.cap"))
        .unwrap();
    let before = logical(&f.db);
    for c in [&agent, &f.daemon.credential("operator.cap")] {
        assert_eq!(
            f.daemon
                .call(c, "profile.mutate", request(&b, "denied", suggest())),
            Err(Error::Unauthorized)
        );
        if c.access != Access::Operator {
            assert_eq!(
                f.daemon.call(c, "profile.write.enable", json!({})),
                Err(Error::Unauthorized)
            );
        }
    }
    for method in [
        "profile.info",
        "profile.write.enable",
        "profile.write.disable",
        "context.get",
        "session.list",
    ] {
        assert_eq!(
            f.daemon.call(&write, method, json!({})),
            Err(Error::Unauthorized)
        );
    }
    assert_eq!(logical(&f.db), before);
    let mut forged = agent.clone();
    forged.access = write.access.clone();
    assert!(matches!(f.daemon.client(&forged), Err(Error::Unauthorized)));
    let mut bridge = std::process::Command::new(exe("acp-mcp"))
        .arg("--runtime-dir")
        .arg(&f.daemon.location.runtime_dir)
        .arg("--endpoint")
        .arg(&f.daemon.location.endpoint)
        .args(["--project", "alpha", "--capability-file"])
        .arg(f.daemon.location.runtime_dir.join("operator-write.cap"))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = bridge.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    };
    assert!(!status.success());
    assert_eq!(logical(&f.db), before);
    f.daemon
        .control("profile.write.disable", json!({}))
        .unwrap();
    assert!(
        !f.daemon
            .location
            .runtime_dir
            .join("operator-write.cap")
            .exists()
    );
    assert_eq!(
        f.daemon.runtime.block_on(
            established.request("profile.mutate", request(&b, "after-revoke", suggest()))
        ),
        Err(Error::Unauthorized)
    );
    assert!(matches!(f.daemon.client(&write), Err(Error::Unauthorized)));
    assert!(
        f.daemon
            .runtime
            .block_on(read.request("context.get", json!({"project":"alpha"})))
            .is_ok()
    );
    let new = f.daemon.enable();
    assert_eq!(new.owner_epoch, b.owner_epoch + 1);
    let before = logical(&f.db);
    assert_eq!(
        f.daemon.call(
            &f.daemon.credential("operator-write.cap"),
            "profile.mutate",
            request(&b, "old-binding", suggest())
        ),
        Err(Error::Unauthorized)
    );
    assert_eq!(logical(&f.db), before);
}

#[test]
fn independent_daemons_lease_contention_process_kill_and_restart() {
    let mut f = Fixture::created(false);
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    let old = f.daemon.credential("operator-write.cap");
    #[cfg(unix)]
    let alias = std::path::PathBuf::from(format!(
        "{}/./profile.sqlite3",
        f.db.parent().unwrap().to_str().unwrap()
    ));
    #[cfg(windows)]
    let alias = f.db.canonicalize().unwrap();
    assert_ne!(alias.as_os_str(), f.db.as_os_str());
    let mut other = Daemon::launch(f.temp.path(), "run-b", Some(&alias), false);
    other.ready();
    let before = logical(&f.db);
    let start = Instant::now();
    assert_eq!(
        other.control(
            "profile.write.enable",
            json!({"profile_id":b.profile_id,"generation":b.generation,"confirm":true})
        ),
        Err(Error::Busy)
    );
    assert!(start.elapsed() < Duration::from_secs(4));
    assert_eq!(logical(&f.db), before);
    f.daemon.kill();
    let next = other.enable();
    assert_eq!(next.owner_epoch, b.owner_epoch + 1);
    assert!(matches!(other.client(&old), Err(Error::Unauthorized)));
    f.daemon = Daemon::launch(f.temp.path(), "run-a", Some(&f.db), false);
    f.daemon.ready();
    assert!(
        !f.daemon
            .location
            .runtime_dir
            .join("operator-write.cap")
            .exists()
    );
    let info = f.daemon.control("profile.info", json!({})).unwrap();
    assert_eq!(info["write_enabled"], false);
    assert_eq!(
        f.daemon.control(
            "profile.write.enable",
            json!({"profile_id":b.profile_id,"generation":b.generation,"confirm":true})
        ),
        Err(Error::Busy)
    );
    other.control("profile.write.disable", json!({})).unwrap();
    let third = f.daemon.enable();
    assert_eq!(third.owner_epoch, next.owner_epoch + 1);
    assert!(matches!(f.daemon.client(&old), Err(Error::Unauthorized)));
}

#[test]
fn selected_file_and_lease_substitution_refuse_valid_private_replacements() {
    for replace_db in [true, false] {
        let f = Fixture::created(false);
        let b = f.daemon.enable();
        let original = if replace_db {
            f.db.clone()
        } else {
            fs::read_dir(f.db.parent().unwrap())
                .unwrap()
                .map(|e| e.unwrap().path())
                .find(|p| {
                    p.file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with(".acp-writer-")
                })
                .unwrap()
        };
        let copied = f.db.parent().unwrap().join("replacement");
        // Windows byte-range locking refuses reading the held empty lease.
        // Its replacement is deliberately empty; only DB copies need contents.
        let bytes = if replace_db {
            fs::read(&original).unwrap()
        } else {
            Vec::new()
        };
        private_bytes(&copied, &bytes);
        if replace_db {
            let copy = acp_store::SqliteStore::open(&copied)
                .unwrap()
                .metadata()
                .unwrap()
                .binding;
            assert_eq!(copy, b);
        }
        fs::rename(&original, original.with_extension("saved")).unwrap();
        fs::rename(&copied, &original).unwrap();
        let before = logical(&f.db);
        assert_eq!(
            f.daemon.mutate(&b, "replacement-denied", suggest()),
            Err(Error::UnsafeRuntime)
        );
        if replace_db {
            assert_eq!(
                f.daemon.control("profile.info", json!({})),
                Err(Error::UnsafeRuntime)
            );
        }
        assert_eq!(logical(&f.db), before);
        f.daemon
            .control("profile.write.disable", json!({}))
            .unwrap();
    }
}

#[test]
fn missing_legacy_and_unsafe_startup_never_create_or_repair() {
    let f = Fixture::created(false);
    let parent = f.db.parent().unwrap();
    let mut cases = vec![parent.join("missing")];
    for version in [1, 2] {
        let path = parent.join(format!("legacy-{version}"));
        private_bytes(&path, b"");
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE legacy(value TEXT)").unwrap();
        db.pragma_update(None, "user_version", version).unwrap();
        drop(db);
        cases.push(path);
    }
    let linked = parent.join("hardlink");
    fs::hard_link(&f.db, &linked).unwrap();
    cases.push(linked);
    for (n, path) in cases.iter().enumerate() {
        let before = fs::read(path).ok();
        let mut d = Daemon::launch(f.temp.path(), &format!("bad-{n}"), Some(path), false);
        let deadline = Instant::now() + Duration::from_secs(15);
        while d.child.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(10));
        }
        assert!(!d.location.runtime_dir.join("operator.cap").exists());
        assert!(
            fs::read(path).ok() == before,
            "refused profile must remain unchanged"
        );
    }
    // Removing only our hardlink restores the original file's safe link count.
    fs::remove_file(parent.join("hardlink")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let alias = parent.join("symlink");
        symlink(&f.db, &alias).unwrap();
        assert!(matches!(
            acp_write::owner::Selected::select(&alias),
            Err(acp_write::Error::UnsafeWorkspace)
        ));
        let nested = f.temp.path().join("parent-link");
        symlink(parent, &nested).unwrap();
        assert!(matches!(
            acp_write::owner::Selected::select(&nested.join("profile.sqlite3")),
            Err(acp_write::Error::UnsafeWorkspace)
        ));
        let before = logical(&f.db);
        fs::set_permissions(&f.db, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            acp_write::owner::Selected::select(&f.db),
            Err(acp_write::Error::UnsafeWorkspace)
        ));
        assert_eq!(logical(&f.db), before);
    }
    #[cfg(windows)]
    {
        let junction = f.temp.path().join("parent-link");
        let status = std::process::Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(&junction)
            .arg(parent)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        assert!(matches!(
            acp_write::owner::Selected::select(&junction.join("profile.sqlite3")),
            Err(acp_write::Error::UnsafeWorkspace)
        ));
        fs::remove_dir(junction).unwrap();
        let inherited = parent.join("inherited.sqlite3");
        fs::write(&inherited, fs::read(&f.db).unwrap()).unwrap();
        assert!(matches!(
            acp_write::owner::Selected::select(&inherited),
            Err(acp_write::Error::UnsafeWorkspace)
        ));
    }
}

#[test]
fn sqlite_contention_does_not_advance_epoch_or_mutate_data() {
    let f = Fixture::created(false);
    let before = logical(&f.db);
    let db = rusqlite::Connection::open(&f.db).unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    let info = f.daemon.control("profile.info", json!({})).unwrap();
    let start = Instant::now();
    assert_eq!(f.daemon.control("profile.write.enable",json!({"profile_id":info["binding"]["profile_id"],"generation":info["binding"]["generation"],"confirm":true})),Err(Error::Busy));
    assert!(start.elapsed() < Duration::from_secs(4));
    assert_eq!(logical(&f.db), before);
    assert!(
        !f.daemon
            .location
            .runtime_dir
            .join("operator-write.cap")
            .exists()
    );
    db.execute_batch("ROLLBACK").unwrap();
    let b = f.daemon.enable();
    let before = logical(&f.db);
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert_eq!(f.daemon.mutate(&b, "busy", suggest()), Err(Error::Busy));
    assert_eq!(logical(&f.db), before);
    db.execute_batch("ROLLBACK").unwrap();
    f.daemon.mutate(&b, "busy", suggest()).unwrap();
}

#[test]
fn real_ipc_disconnected_response_preserves_same_epoch_receipt_replay_and_v1_refusal() {
    let f = Fixture::created(false);
    let b = f.daemon.enable();
    let write = f.daemon.credential("operator-write.cap");
    let token = serde_json::to_value(&write).unwrap()["token"].clone();
    let payload = request(&b, "lost-response", suggest());
    f.daemon.runtime.block_on(async {
        let mut stream=acp_ipc::transport::connect(&f.daemon.location).await.unwrap();
        acp_ipc::write_frame(&mut stream,&json!({"api_version":acp_ipc::API_VERSION,"credential":token})).await.unwrap();
        assert_eq!(acp_ipc::read_frame(&mut stream).await.unwrap()["ready"],true);
        acp_ipc::write_frame(&mut stream,&json!({"api_version":acp_ipc::API_VERSION,"id":"lost","method":"profile.mutate","params":payload})).await.unwrap();
        // Deliberately disconnect without consuming the mutation response.
        drop(stream);
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if acp_store::SqliteStore::open(&f.db)
            .ok()
            .and_then(|s| s.metadata().ok())
            .is_some_and(|m| m.commit_sequence == 1)
        {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    let before = logical(&f.db);
    let receipt = f.daemon.call(&write, "profile.mutate", payload).unwrap();
    assert_eq!(receipt["operation_id"], "lost-response");
    assert_eq!(receipt["commit_sequence"], 1);
    assert_eq!(logical(&f.db), before);
    f.daemon.runtime.block_on(async {
        let mut stream = acp_ipc::transport::connect(&f.daemon.location)
            .await
            .unwrap();
        acp_ipc::write_frame(&mut stream, &json!({"api_version":1,"credential":token}))
            .await
            .unwrap();
        let refused = acp_ipc::read_frame(&mut stream).await.unwrap();
        assert_eq!(refused["error"], "Version");
        assert_eq!(refused["api_version"], 2);
    });
    assert_eq!(logical(&f.db), before);
}

#[test]
fn real_fake_session_mcp_v2_read_only_and_established_peer_revocation() {
    let f = Fixture::created(true);
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    f.daemon
        .mutate(&b, "handoff", handoff("alpha", Expected::Absent))
        .unwrap();
    let p = json!({"session_api_version":acp_session::SESSION_API,"project":"alpha"});
    let mut start = p.clone();
    start["executable"] = json!("fake");
    start["confirmed"] = json!(true);
    start["share_context"] = json!(true);
    f.daemon.control("session.start", start).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let view = loop {
        let view = f.daemon.control("session.get", p.clone()).unwrap()["session"].clone();
        if view["state"] == "ready" {
            break view;
        }
        assert!(![json!("failed"), json!("auth_required")].contains(&view["state"]));
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(15));
    };
    let cap = fs::read_dir(&f.daemon.location.runtime_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("session-")
        })
        .unwrap();
    let session = acp_ipc::load_credential(&f.daemon.location, &cap).unwrap();
    let mut established = f.daemon.client(&session).unwrap();
    assert_eq!(
        f.daemon.call(
            &session,
            "profile.mutate",
            request(&b, "session-denied", suggest())
        ),
        Err(Error::Unauthorized)
    );
    assert_eq!(
        f.daemon.call(&session, "profile.write.enable", json!({})),
        Err(Error::Unauthorized)
    );
    assert_eq!(
        f.daemon
            .runtime
            .block_on(established.request("context.get", json!({})))
            .unwrap()["project"],
        "alpha"
    );
    for (method, params) in [
        (
            "profile.preferences.list",
            json!({"scope":{"kind":"global"},"after_id":0}),
        ),
        (
            "profile.preferences.get",
            json!({"scope":{"kind":"global"},"id":1}),
        ),
        (
            "profile.handoff.get",
            json!({"project":"alpha","task_id":"shared"}),
        ),
    ] {
        let before = logical(&f.db);
        assert_eq!(
            f.daemon
                .runtime
                .block_on(established.request(method, params)),
            Err(Error::Unauthorized)
        );
        assert_eq!(logical(&f.db), before);
    }
    let mut prompt = p.clone();
    prompt["session_id"] = view["session_id"].clone();
    prompt["prompt"] = json!("synthetic context smoke");
    f.daemon.control("session.prompt", prompt).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let observation = f.daemon.control("session.context", p.clone()).unwrap();
        if observation["context_calls"].as_u64().unwrap_or(0) >= 2
            && observation["handoff_calls"].as_u64().unwrap_or(0) >= 1
        {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(15));
    }
    let mut stop = p;
    stop["session_id"] = view["session_id"].clone();
    f.daemon.control("session.stop", stop).unwrap();
    assert_eq!(
        f.daemon
            .runtime
            .block_on(established.request("context.get", json!({}))),
        Err(Error::Unauthorized)
    );
    assert!(matches!(
        f.daemon.client(&session),
        Err(Error::Unauthorized)
    ));
    assert!(!cap.exists());
    // Operator write authority is independent of the revoked session context.
    f.daemon.mutate(&b, "still-authorized", suggest()).unwrap();
}

#[test]
fn failed_grant_publication_and_epoch_overflow_never_activate_a_writer() {
    let f = Fixture::created(false);
    let path = f.daemon.location.runtime_dir.join("operator-write.cap");
    private_bytes(&path, b"synthetic collision");
    let info = f.daemon.control("profile.info", json!({})).unwrap();
    let params = json!({"profile_id":info["binding"]["profile_id"],"generation":info["binding"]["generation"],"confirm":true});
    assert_eq!(
        f.daemon.control("profile.write.enable", params),
        Err(Error::UnsafeRuntime)
    );
    assert_eq!(fs::read(&path).unwrap(), b"synthetic collision");
    let info = f.daemon.control("profile.info", json!({})).unwrap();
    assert_eq!(info["write_enabled"], false);
    assert_eq!(info["binding"]["owner_epoch"], 1);
    assert_eq!(
        acp_store::SqliteStore::open(&f.db)
            .unwrap()
            .metadata()
            .unwrap()
            .commit_sequence,
        0
    );
    fs::remove_file(path).unwrap();
    assert_eq!(f.daemon.enable().owner_epoch, 2);
    f.daemon
        .control("profile.write.disable", json!({}))
        .unwrap();
    let db = rusqlite::Connection::open(&f.db).unwrap();
    db.execute("UPDATE acp_profile SET owner_epoch=?1", [i64::MAX])
        .unwrap();
    drop(db);
    let before = logical(&f.db);
    let info = f.daemon.control("profile.info", json!({})).unwrap();
    assert_eq!(f.daemon.control("profile.write.enable",json!({"profile_id":info["binding"]["profile_id"],"generation":info["binding"]["generation"],"confirm":true})),Err(Error::Capacity));
    assert_eq!(logical(&f.db), before);
    assert!(
        !f.daemon
            .location
            .runtime_dir
            .join("operator-write.cap")
            .exists()
    );
}

#[test]
fn all_read_roles_keep_the_independent_registered_root_binding() {
    let f = Fixture::created(false);
    let b = f.daemon.enable();
    f.register(&b, "alpha");
    let agent = f.daemon.credential("agent-alpha.cap");
    let db = rusqlite::Connection::open(&f.db).unwrap();
    db.execute(
        "UPDATE projects SET root=?1 WHERE name='alpha'",
        [f.temp
            .path()
            .join("beta")
            .canonicalize()
            .unwrap()
            .to_str()
            .unwrap()],
    )
    .unwrap();
    drop(db);
    acp_store::SqliteStore::open(&f.db).unwrap(); // Structurally valid, but root differs.
    let before = logical(&f.db);
    assert_eq!(
        f.daemon.control("context.get", json!({"project":"alpha"})),
        Err(Error::UnsafeRuntime)
    );
    assert_eq!(
        f.daemon.call(&agent, "context.get", json!({})),
        Err(Error::UnsafeRuntime)
    );
    assert_eq!(logical(&f.db), before);
    let mut command = suggest();
    if let Command::SuggestPreference { scope, .. } = &mut command {
        *scope = Scope::Project {
            name: "alpha".into(),
        };
    }
    assert_eq!(
        f.daemon.mutate(&b, "changed-registered-root", command),
        Err(Error::UnsafeRuntime)
    );
    assert_eq!(logical(&f.db), before);
}

#[test]
fn project_review_decisions_refuse_a_root_lost_after_inspection_without_writing() {
    for action in ["confirm", "reject", "deprecate", "delete"] {
        let f = Fixture::created(false);
        let binding = f.daemon.enable();
        f.register(&binding, "alpha");
        let scope = Scope::Project {
            name: "alpha".into(),
        };
        let mut proposal = suggest();
        if let Command::SuggestPreference { scope: target, .. } = &mut proposal {
            *target = scope.clone();
        }
        f.daemon.mutate(&binding, "stage", proposal).unwrap();
        let digest = preview_digest(&f, &scope, 1);
        let revision = if action == "deprecate" {
            f.daemon
                .mutate(
                    &binding,
                    "confirm",
                    Command::ConfirmPreference {
                        id: 1,
                        scope: scope.clone(),
                        proposal_digest: digest.clone(),
                        channel: Channel::Desktop,
                        expected: Expected::Revision(1),
                    },
                )
                .unwrap();
            2
        } else {
            1
        };
        f.daemon
            .control(
                "profile.review.inspect",
                json!({"scope":scope,"id":1,"revision":revision}),
            )
            .unwrap();
        let command = match action {
            "confirm" => Command::ConfirmPreference {
                id: 1,
                scope,
                proposal_digest: digest,
                channel: Channel::Desktop,
                expected: Expected::Revision(revision),
            },
            "reject" => Command::RejectPreference {
                id: 1,
                scope,
                proposal_digest: digest,
                channel: Channel::Desktop,
                expected: Expected::Revision(revision),
            },
            "deprecate" => Command::DeprecatePreference {
                id: 1,
                scope,
                proposal_digest: digest,
                channel: Channel::Desktop,
                expected: Expected::Revision(revision),
            },
            "delete" => Command::DeletePreference {
                id: 1,
                scope,
                proposal_digest: digest,
                channel: Channel::Desktop,
                expected: Expected::Revision(revision),
            },
            _ => unreachable!(),
        };
        let root = f.temp.path().join("alpha");
        let moved = f.temp.path().join("moved-alpha");
        fs::rename(&root, &moved).unwrap();
        let before = logical(&f.db);
        assert_eq!(
            f.daemon.mutate(&binding, "decision", command.clone()),
            Err(Error::UnsafeRuntime),
            "{action}"
        );
        assert_eq!(
            logical(&f.db),
            before,
            "{action} must not change rows or receipts"
        );
        fs::rename(&moved, &root).unwrap();
        // Refusal did not consume the operation ID or make the valid action stale.
        f.daemon.mutate(&binding, "decision", command).unwrap();
    }
}

#[test]
fn unsafe_existing_lease_files_fail_before_epoch_admission() {
    let f = Fixture::created(false);
    f.daemon.enable();
    f.daemon
        .control("profile.write.disable", json!({}))
        .unwrap();
    let lock = fs::read_dir(f.db.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(".acp-writer-")
        })
        .unwrap();
    let before = logical(&f.db);
    let info = f.daemon.control("profile.info", json!({})).unwrap();
    let params = json!({"profile_id":info["binding"]["profile_id"],"generation":info["binding"]["generation"],"confirm":true});
    let linked = lock.with_extension("linked");
    fs::hard_link(&lock, &linked).unwrap();
    assert_eq!(
        f.daemon.control("profile.write.enable", params.clone()),
        Err(Error::UnsafeRuntime)
    );
    assert_eq!(logical(&f.db), before);
    fs::remove_file(linked).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};
        fs::set_permissions(&lock, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            f.daemon.control("profile.write.enable", params.clone()),
            Err(Error::UnsafeRuntime)
        );
        assert_eq!(logical(&f.db), before);
        fs::set_permissions(&lock, fs::Permissions::from_mode(0o600)).unwrap();
        let saved = lock.with_extension("saved");
        fs::rename(&lock, &saved).unwrap();
        symlink(&saved, &lock).unwrap();
        assert_eq!(
            f.daemon.control("profile.write.enable", params.clone()),
            Err(Error::UnsafeRuntime)
        );
        assert_eq!(logical(&f.db), before);
        fs::remove_file(&lock).unwrap();
        fs::rename(saved, &lock).unwrap();
    }
    #[cfg(windows)]
    {
        fs::remove_file(&lock).unwrap();
        fs::write(&lock, b"").unwrap();
        assert_eq!(
            f.daemon.control("profile.write.enable", params.clone()),
            Err(Error::UnsafeRuntime)
        );
        assert_eq!(logical(&f.db), before);
    }
    assert!(
        !f.daemon
            .location
            .runtime_dir
            .join("operator-write.cap")
            .exists()
    );
}

#[test]
fn epoch_drift_refuses_existing_grant_and_never_reports_it_as_current() {
    let f = Fixture::created(false);
    let b = f.daemon.enable();
    let db = rusqlite::Connection::open(&f.db).unwrap();
    db.execute("UPDATE acp_profile SET owner_epoch=?1", [b.owner_epoch + 1])
        .unwrap();
    drop(db);
    let before = logical(&f.db);
    assert_eq!(
        f.daemon.control("profile.info", json!({})).unwrap()["write_enabled"],
        false
    );
    assert_eq!(
        f.daemon.mutate(&b, "epoch-drift", suggest()),
        Err(Error::OwnerMismatch)
    );
    assert_eq!(logical(&f.db), before);
    f.daemon
        .control("profile.write.disable", json!({}))
        .unwrap();
    assert_eq!(f.daemon.enable().owner_epoch, b.owner_epoch + 2);
}

fn cli(f: &Fixture, args: &[&str]) -> (bool, Value) {
    let output = std::process::Command::new(exe("acp-query"))
        .arg("--runtime-dir")
        .arg(&f.daemon.location.runtime_dir)
        .arg("--endpoint")
        .arg(&f.daemon.location.endpoint)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.stderr.is_empty(),
        "CLI must sanitize errors on stdout"
    );
    let value = serde_json::from_slice(&output.stdout).expect("bounded CLI JSON");
    (output.status.success(), value)
}
fn cli_mutate(f: &Fixture, b: &acp_write::Binding, op: &str, args: &[&str]) -> (bool, Value) {
    let epoch = b.owner_epoch.to_string();
    let mut argv = args.to_vec();
    argv.extend([
        "--profile-id",
        &b.profile_id,
        "--generation",
        &b.generation,
        "--owner-epoch",
        &epoch,
        "--operation-id",
        op,
    ]);
    cli(f, &argv)
}
#[test]
fn native_cli_lifecycle_seven_commands_preview_cas_and_exact_retry() {
    let f = Fixture::unselected(false);
    assert_eq!(cli(&f, &["profile", "info"]).1["selected"], false);
    assert!(cli(&f, &["profile", "create", f.db.to_str().unwrap()]).0);
    let info = cli(&f, &["profile", "info"]).1;
    assert_eq!(info["write_enabled"], false);
    assert_eq!(
        cli(&f, &["profile", "select", f.db.to_str().unwrap()]).1["error"],
        "RestartRequired"
    );
    let b0: acp_write::Binding = serde_json::from_value(info["binding"].clone()).unwrap();
    let state = logical(&f.db);
    assert_eq!(
        cli(
            &f,
            &[
                "profile",
                "write",
                "enable",
                "--profile-id",
                &b0.profile_id,
                "--generation",
                &b0.generation
            ]
        )
        .1["error"],
        "InvalidRequest"
    );
    assert_eq!(logical(&f.db), state);
    let info = cli(
        &f,
        &[
            "profile",
            "write",
            "enable",
            "--profile-id",
            &b0.profile_id,
            "--generation",
            &b0.generation,
            "--confirm",
        ],
    );
    assert!(info.0);
    let b: acp_write::Binding = serde_json::from_value(info.1["binding"].clone()).unwrap();
    let root = f.temp.path().join("alpha").canonicalize().unwrap();
    assert!(
        cli_mutate(
            &f,
            &b,
            "cli-register",
            &["project-register", "alpha", root.to_str().unwrap()]
        )
        .0
    );
    let result = cli_mutate(
        &f,
        &b,
        "cli-suggest",
        &[
            "preference-suggest",
            "--scope",
            "project",
            "--project",
            "alpha",
            "--key",
            "editor",
            "--value",
            "synthetic",
            "--source",
            "operator",
            "--origin",
            "agent_reported",
        ],
    );
    assert!(result.0);
    let id = result.1["affected"][0]["record_id"].as_str().unwrap();
    let state = logical(&f.db);
    let replay = cli_mutate(
        &f,
        &b,
        "cli-suggest",
        &[
            "preference-suggest",
            "--scope",
            "project",
            "--project",
            "alpha",
            "--key",
            "editor",
            "--value",
            "synthetic",
            "--source",
            "operator",
            "--origin",
            "agent_reported",
        ],
    );
    assert_eq!(result, replay);
    assert_eq!(logical(&f.db), state);
    assert_eq!(
        cli_mutate(
            &f,
            &b,
            "cli-suggest",
            &[
                "preference-suggest",
                "--scope",
                "project",
                "--project",
                "alpha",
                "--key",
                "editor",
                "--value",
                "changed",
                "--source",
                "operator",
                "--origin",
                "agent_reported"
            ]
        )
        .1["error"],
        "IdempotencyConflict"
    );
    assert_eq!(logical(&f.db), state);
    let preview = cli(
        &f,
        &[
            "preferences",
            "get",
            id,
            "--scope",
            "project",
            "--project",
            "alpha",
        ],
    );
    assert!(preview.0);
    assert_eq!(preview.1["record"]["revision"], 1);
    assert_eq!(preview.1["record"]["status"], "suggested");
    assert_eq!(preview.1["record"]["origin"], "agent_reported");
    let digest = preview.1["record"]["proposal_digest"].as_str().unwrap();
    assert_eq!(digest.len(), 64);
    // The operator review reads are reachable from the same bounded CLI.
    let queue = cli(
        &f,
        &["review-queue", "--scope", "project", "--project", "alpha"],
    );
    assert!(queue.0);
    assert_eq!(queue.1["records"].as_array().unwrap().len(), 1);
    assert_eq!(queue.1["records"][0]["proposal_digest"], digest);
    let inspected = cli(
        &f,
        &[
            "review-inspect",
            id,
            "--scope",
            "project",
            "--project",
            "alpha",
            "--revision",
            "1",
        ],
    );
    assert!(inspected.0);
    assert_eq!(inspected.1["record"]["proposal_digest"], digest);
    assert!(inspected.1["decisions"].as_array().unwrap().is_empty());
    assert_eq!(
        cli(
            &f,
            &[
                "review-inspect",
                id,
                "--scope",
                "project",
                "--project",
                "alpha",
                "--revision",
                "2"
            ]
        )
        .1["error"],
        "Conflict"
    );
    assert!(
        cli(&f, &["preferences", "list", "--scope", "global"]).1["records"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        cli(&f, &["preferences", "get", id, "--scope", "global"]).1["error"],
        "NotFound"
    );
    assert_eq!(
        cli(
            &f,
            &[
                "preferences",
                "list",
                "--scope",
                "project",
                "--project",
                "outside"
            ]
        )
        .1["error"],
        "Unauthorized"
    );
    assert_eq!(
        cli_mutate(
            &f,
            &b,
            "cli-no-confirm",
            &[
                "preference-confirm",
                id,
                "--scope",
                "project",
                "--project",
                "alpha",
                "--expected-revision",
                "1",
                "--proposal-digest",
                digest
            ]
        )
        .1["error"],
        "InvalidRequest"
    );
    assert_eq!(logical(&f.db), state);
    assert!(
        cli_mutate(
            &f,
            &b,
            "cli-confirm",
            &[
                "preference-confirm",
                id,
                "--scope",
                "project",
                "--project",
                "alpha",
                "--expected-revision",
                "1",
                "--proposal-digest",
                digest,
                "--confirm"
            ]
        )
        .0
    );
    let state = logical(&f.db);
    assert_eq!(
        cli_mutate(
            &f,
            &b,
            "cli-cas",
            &[
                "preference-deprecate",
                id,
                "--scope",
                "project",
                "--project",
                "alpha",
                "--expected-revision",
                "1",
                "--proposal-digest",
                digest,
                "--confirm"
            ]
        )
        .1["error"],
        "Conflict"
    );
    assert_eq!(logical(&f.db), state);
    assert!(
        cli_mutate(
            &f,
            &b,
            "cli-deprecate",
            &[
                "preference-deprecate",
                id,
                "--scope",
                "project",
                "--project",
                "alpha",
                "--expected-revision",
                "2",
                "--proposal-digest",
                digest,
                "--confirm"
            ]
        )
        .0
    );
    assert!(
        cli_mutate(
            &f,
            &b,
            "cli-delete",
            &[
                "preference-delete",
                id,
                "--scope",
                "project",
                "--project",
                "alpha",
                "--expected-revision",
                "3",
                "--proposal-digest",
                digest,
                "--confirm"
            ]
        )
        .0
    );
    let result = cli_mutate(
        &f,
        &b,
        "cli-suggest-reject",
        &[
            "preference-suggest",
            "--scope",
            "global",
            "--key",
            "editor",
            "--value",
            "synthetic",
            "--source",
            "operator",
            "--origin",
            "daemon_observed",
        ],
    );
    assert!(result.0);
    let id = result.1["affected"][0]["record_id"].as_str().unwrap();
    let global_preview = cli(&f, &["preferences", "get", id, "--scope", "global"]).1;
    assert_eq!(global_preview["record"]["origin"], "daemon_observed");
    let global_digest = global_preview["record"]["proposal_digest"]
        .as_str()
        .unwrap();
    assert_ne!(global_digest, digest);
    assert!(
        cli_mutate(
            &f,
            &b,
            "cli-reject",
            &[
                "preference-reject",
                id,
                "--scope",
                "global",
                "--expected-revision",
                "1",
                "--proposal-digest",
                global_digest,
                "--confirm"
            ]
        )
        .0
    );
    assert!(cli(&f, &["handoff-record", "alpha", "task"]).1["record"].is_null());
    let input = f.temp.path().join("handoff.json");
    fs::write(&input,json!({"objective":"synthetic objective","completed":"","next_action":"inspect","validation_json":"[]","snapshot":{"base_sha":"","branch":"","state":"unknown","fingerprint":null}}).to_string()).unwrap();
    assert!(
        cli_mutate(
            &f,
            &b,
            "cli-handoff",
            &[
                "handoff-put",
                "alpha",
                "task",
                "--expected-absent",
                "--input",
                input.to_str().unwrap()
            ]
        )
        .0
    );
    assert_eq!(
        cli(&f, &["handoff-record", "alpha", "task"]).1["record"]["revision"],
        1
    );
    assert_eq!(
        cli(&f, &["handoff", "alpha", "task"]).1["objective"],
        "synthetic objective"
    );
    assert!(cli(&f, &["profile", "write", "disable"]).0);
    assert_eq!(cli(&f, &["profile", "info"]).1["write_enabled"], false);
    let state = logical(&f.db);
    assert!(
        !cli_mutate(
            &f,
            &b,
            "cli-disabled",
            &[
                "preference-suggest",
                "--scope",
                "global",
                "--key",
                "a",
                "--value",
                "b",
                "--source",
                "c",
                "--origin",
                "legacy_unknown"
            ]
        )
        .0
    );
    assert_eq!(logical(&f.db), state);
}
#[test]
fn operator_preview_exact_scope_bounds_roles_and_malformed_state_preservation() {
    let f = Fixture::created(false);
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
                scope: Scope::Project {
                    name: "beta".into(),
                },
                key: "k".into(),
                value: "synthetic-other-scope".into(),
                source: "test".into(),
                origin: acp_core::operator::Origin::AgentReported,
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

#[test]
fn review_queue_inspect_and_digest_binding_are_exact_and_operator_only() {
    let f = Fixture::created(false);
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
                scope: Scope::Project {
                    name: "beta".into(),
                },
                key: "k".into(),
                value: "synthetic-other-scope".into(),
                source: "test".into(),
                origin: acp_core::operator::Origin::AgentReported,
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
            && r["origin"] == "agent_reported"
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
                channel: Channel::Tui,
                expected: Expected::Revision(1),
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
    assert_eq!(decisions[0]["channel"], "tui");
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
