use super::*;

#[test]
fn distinct_write_role_revokes_established_peers_and_stale_binding() {
    let f = Fixture::created();
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
    let mut bridge = std::process::Command::new(exe("precept-mcp"))
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
fn all_read_roles_keep_the_independent_registered_root_binding() {
    let f = Fixture::created();
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
    precept_store::SqliteStore::open(&f.db).unwrap(); // Structurally valid, but root differs.
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
        let f = Fixture::created();
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
                        note: None,
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
                note: None,
            },
            "reject" => Command::RejectPreference {
                id: 1,
                scope,
                proposal_digest: digest,
                channel: Channel::Desktop,
                expected: Expected::Revision(revision),
                note: None,
            },
            "deprecate" => Command::DeprecatePreference {
                id: 1,
                scope,
                proposal_digest: digest,
                channel: Channel::Desktop,
                expected: Expected::Revision(revision),
                note: None,
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
fn epoch_drift_refuses_existing_grant_and_never_reports_it_as_current() {
    let f = Fixture::created();
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
