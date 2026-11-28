use super::support::*;
use super::*;

#[test]
fn fresh_identity_schema_counter_and_no_legacy_objects() {
    let (_t, p) = fresh();
    let r = SqliteStore::open(&p.path).unwrap();
    let m = r.metadata().unwrap();
    assert_eq!(m.binding, p.binding());
    assert_ne!(m.binding.profile_id, m.binding.generation);
    assert_eq!(
        (
            m.next_preference_id,
            m.commit_sequence,
            m.binding.owner_epoch
        ),
        (1, 0, 0)
    );
    assert!(r.read_project("missing").is_err());
    let db = connect(&p.path, true).unwrap();
    let n: i64 = db
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name IN ('sqlite_sequence','precept_revisions')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 0);
}
#[test]
fn explicit_creation_never_overwrites_existing_objects_or_sidecars() {
    let t = tempfile::tempdir().unwrap();
    files::private_test_parent(t.path());
    let p = t.path().join("target");
    fs::write(&p, b"sentinel").unwrap();
    assert!(matches!(Profile::create(&p), Err(Error::Collision)));
    assert_eq!(fs::read(&p).unwrap(), b"sentinel");
    let dir = t.path().join("existing-dir");
    fs::create_dir(&dir).unwrap();
    assert!(matches!(Profile::create(&dir), Err(Error::Collision)));
    for suffix in ["-wal", "-shm", "-journal"] {
        let target = t.path().join(format!("db{suffix}"));
        fs::write(&target, b"sidecar sentinel").unwrap();
        assert!(matches!(
            Profile::create(&t.path().join("db")),
            Err(Error::Collision)
        ));
        assert_eq!(fs::read(&target).unwrap(), b"sidecar sentinel");
        fs::remove_file(target).unwrap();
    }
    assert!(Profile::create(Path::new("relative")).is_err());
    for name in [
        "daemon.lock",
        "operator.cap",
        "operator-write.cap",
        "daemon.sock",
    ] {
        assert!(Profile::create(&t.path().join(name)).is_err());
    }
}
#[test]
#[cfg(windows)]
fn windows_mixed_case_reservations_and_collisions_preserve_neighbors() {
    let t = tempfile::tempdir().unwrap();
    files::private_test_parent(t.path());
    // Absent targets must fail lexical checks, independently of filesystem
    // existence; checking only existing aliases would miss these cases.
    for name in [
        "Daemon.Lock",
        "DAEMON.SOCK",
        "Operator.Cap",
        "OPERATOR-Write.CAP",
        ".Precept-Create-synthetic",
        "profile-WaL",
        "profile-ShM",
        "profile-JoUrNaL",
    ] {
        let target = t.path().join(name);
        assert!(matches!(Profile::create(&target), Err(Error::Collision)));
        assert!(!target.exists());
    }
    for name in [
        "Daemon.Lock",
        "DAEMON.SOCK",
        "Operator.Cap",
        "OPERATOR-Write.CAP",
        "target-WaL",
        "target-ShM",
        "target-JoUrNaL",
        "TaRgEt",
    ] {
        let neighbor = t.path().join(name);
        fs::write(&neighbor, b"synthetic sentinel").unwrap();
        assert!(matches!(
            Profile::create(&t.path().join("target")),
            Err(Error::Collision)
        ));
        assert_eq!(fs::read(&neighbor).unwrap(), b"synthetic sentinel");
        fs::remove_file(neighbor).unwrap();
    }
    // Folding reservations must neither rename a valid target nor overmatch
    // names containing a reserved word away from the reserved boundary.
    let target = t.path().join("Profile-WaL.sqlite3");
    let p = Profile::create(&target).unwrap();
    validated(&p);
    assert_eq!(p.path.file_name(), target.file_name());
    assert_eq!(fs::read_dir(t.path()).unwrap().count(), 1);
}
#[test]
fn concurrent_creators_have_one_no_replace_winner() {
    let t = tempfile::tempdir().unwrap();
    files::private_test_parent(t.path());
    let target = t.path().join("race");
    let barrier = Barrier::new(2);
    let result = std::thread::scope(|s| {
        let a = s.spawn(|| {
            barrier.wait();
            Profile::create(&target)
        });
        let b = s.spawn(|| {
            barrier.wait();
            Profile::create(&target)
        });
        [a.join().unwrap(), b.join().unwrap()]
    });
    assert_eq!(result.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        result
            .iter()
            .filter(|r| matches!(r, Err(Error::Collision)))
            .count(),
        1
    );
    let p = result.into_iter().find_map(Result::ok).unwrap();
    validated(&p);
}
#[test]
fn publication_unknown_preserves_valid_published_file() {
    let t = tempfile::tempdir().unwrap();
    files::private_test_parent(t.path());
    let target = t.path().join("published");
    assert!(matches!(
        Profile::create_inner(&target, Fault::AfterCommit),
        Err(Error::OutcomeUnknown)
    ));
    let before = fs::read(&target).unwrap();
    SqliteStore::open(&target).unwrap();
    assert!(matches!(Profile::create(&target), Err(Error::Collision)));
    assert_eq!(fs::read(&target).unwrap(), before);
    assert_eq!(fs::read_dir(t.path()).unwrap().count(), 1);
}
#[test]
fn registration_uniqueness_and_expected_absence_are_atomic() {
    let (_t, p) = ready();
    let mut w = p.writer().unwrap();
    let before = dump(&w.db);
    let root = p
        .path
        .parent()
        .unwrap()
        .join("alpha")
        .canonicalize()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    for name in ["alpha", "other"] {
        assert_eq!(
            w.execute(&request(
                &p,
                &format!("duplicate-{name}"),
                Command::RegisterProject {
                    name: name.into(),
                    root: root.clone(),
                    expected: Expected::Absent
                }
            )),
            Err(Error::Conflict)
        );
        assert_eq!(dump(&w.db), before);
    }
    assert_eq!(
        w.execute(&request(
            &p,
            "unknown-project",
            suggest(project_scope("unknown"))
        )),
        Err(Error::Conflict)
    );
    assert_eq!(dump(&w.db), before);
}
#[test]
fn relocation_moves_the_root_by_revision_and_keeps_scoped_records() {
    let (_t, p) = ready();
    let mut w = p.writer().unwrap();
    w.execute(&request(&p, "scoped", suggest(project_scope("alpha"))))
        .unwrap();
    let dir = |name: &str| {
        let root = p.path.parent().unwrap().join(name);
        fs::create_dir_all(&root).unwrap();
        root.canonicalize().unwrap().to_str().unwrap().to_owned()
    };
    let moved = dir("alpha-moved");
    let beta = dir("beta");
    let relocate = |id: &str, root: &str, expected: Expected| {
        request(
            &p,
            id,
            Command::RelocateProject {
                name: "alpha".into(),
                root: root.into(),
                expected,
            },
        )
    };
    let before = dump(&w.db);
    // Another project's root, a stale revision, an unknown project, an
    // absent expectation or a non-canonical root change nothing.
    for (id, request, error) in [
        (
            "onto-beta",
            relocate("onto-beta", &beta, Expected::Revision(1)),
            Error::Conflict,
        ),
        (
            "stale",
            relocate("stale", &moved, Expected::Revision(2)),
            Error::Conflict,
        ),
        (
            "absent",
            relocate("absent", &moved, Expected::Absent),
            Error::InvalidCommand,
        ),
        (
            "dotted",
            relocate("dotted", &format!("{moved}/."), Expected::Revision(1)),
            Error::InvalidCommand,
        ),
        (
            "unknown",
            request(
                &p,
                "unknown",
                Command::RelocateProject {
                    name: "gamma".into(),
                    root: moved.clone(),
                    expected: Expected::Revision(1),
                },
            ),
            Error::Conflict,
        ),
    ] {
        assert_eq!(w.execute(&request), Err(error), "{id}");
        assert_eq!(dump(&w.db), before, "{id}");
    }
    let receipt = w
        .execute(&relocate("move", &moved, Expected::Revision(1)))
        .unwrap();
    assert_eq!(receipt.affected.len(), 1);
    assert_eq!(receipt.affected[0].record_id, "alpha");
    assert_eq!(receipt.affected[0].revision, 2);
    // The identical retry replays; a new move needs the new revision.
    assert_eq!(
        w.execute(&relocate("move", &moved, Expected::Revision(1)))
            .unwrap(),
        receipt
    );
    let store = SqliteStore::open(&p.path).unwrap();
    let project = store.read_project("alpha").unwrap();
    assert_eq!(project.root.to_str().unwrap(), moved);
    assert_eq!(store.project_revision("alpha").unwrap(), 2);
    assert_eq!(
        store.project_at_root(Path::new(&moved)).unwrap().as_deref(),
        Some("alpha")
    );
    assert!(schema::verify(&w.db).is_ok());
    let back = dir("alpha");
    w.execute(&relocate("back", &back, Expected::Revision(2)))
        .unwrap();
    assert!(schema::verify(&w.db).is_ok());
}
