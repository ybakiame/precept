mod support;
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
        version: 1,
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
    f.daemon
        .mutate(
            &b,
            "confirm-1",
            Command::ConfirmPreference {
                id,
                scope: Scope::Global,
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
                expected: Expected::Revision(3),
            },
            Error::InvalidTransition,
        ),
        (
            "bad-cas",
            Command::DeletePreference {
                id,
                scope: Scope::Global,
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
    let alias =
        f.db.parent()
            .unwrap()
            .join("")
            .join(f.db.file_name().unwrap());
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
        private_bytes(&copied, &fs::read(&original).unwrap());
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
        assert_eq!(fs::read(path).ok(), before);
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
