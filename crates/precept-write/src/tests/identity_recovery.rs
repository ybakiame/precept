use super::support::*;
use super::*;

#[cfg(windows)]
#[test]
fn windows_extended_prefix_keeps_the_same_private_parent_identity() {
    let t = tempfile::tempdir().unwrap();
    files::private_test_parent(t.path());
    let raw = files::check_parent(t.path()).unwrap();
    let canonical = t.path().canonicalize().unwrap();
    assert!(files::check_parent(&canonical).unwrap() == raw);
    let p = Profile::create(&canonical.join("extended.sqlite3")).unwrap();
    validated(&p);
}
#[test]
fn erased_receipts_deleted_ids_and_metadata_drift_never_enable_reuse() {
    for mode in 0..4 {
        let (_t, p) = fresh();
        let id = inserted(&p, "first", Scope::Global);
        p.writer()
            .unwrap()
            .execute(&request(
                &p,
                "delete",
                Command::DeletePreference {
                    id,
                    scope: Scope::Global,
                    proposal_digest: proposal(&Scope::Global),
                    channel: precept_core::operator::Channel::Cli,
                    expected: Expected::Revision(1),
                },
            ))
            .unwrap();
        let mut w = p.writer().unwrap();
        match mode {
            0 => {
                w.db.execute_batch("DELETE FROM acp_receipts WHERE commit_sequence=1")
                    .unwrap();
            }
            1 => {
                w.db.execute_batch(
                    "DELETE FROM acp_receipts;UPDATE acp_profile SET commit_sequence=0",
                )
                .unwrap();
            }
            2 => {
                w.db.execute("UPDATE acp_profile SET next_preference_id=?1", [id])
                    .unwrap();
            }
            _ => {
                let mut r: Receipt = serde_json::from_str(
                    &w.db
                        .query_row::<String, _, _>(
                            "SELECT result_json FROM acp_receipts WHERE commit_sequence=2",
                            [],
                            |r| r.get(0),
                        )
                        .unwrap(),
                )
                .unwrap();
                r.affected[0].deleted = false;
                w.db.execute(
                    "UPDATE acp_receipts SET result_json=?1 WHERE commit_sequence=2",
                    [serde_json::to_string(&r).unwrap()],
                )
                .unwrap();
            }
        }
        let before = dump(&w.db);
        assert_eq!(
            w.execute(&request(&p, "reuse", suggest(Scope::Global))),
            Err(Error::InvalidRecord)
        );
        assert_eq!(dump(&w.db), before);
        assert!(SqliteStore::open(&p.path).is_err());
    }
}
#[test]
fn registration_replay_survives_directory_unavailability() {
    let (_t, p) = fresh();
    let directory = p.path.parent().unwrap().join("project");
    fs::create_dir(&directory).unwrap();
    let req = request(
        &p,
        "register",
        Command::RegisterProject {
            name: "alpha".into(),
            root: directory.canonicalize().unwrap().to_str().unwrap().into(),
            expected: Expected::Absent,
        },
    );
    let mut w = p.writer().unwrap();
    let original = w.execute(&req).unwrap();
    let before = dump(&w.db);
    fs::remove_dir(directory).unwrap();
    assert_eq!(w.execute(&req).unwrap(), original);
    assert_eq!(dump(&w.db), before);
    let mut changed = req.clone();
    changed.operation_id = "register-new".into();
    assert_eq!(w.execute(&changed), Err(Error::InvalidCommand));
    assert_eq!(dump(&w.db), before);
}
#[test]
fn prepublication_failure_removes_only_its_owned_staging_artifacts() {
    let t = tempfile::tempdir().unwrap();
    files::private_test_parent(t.path());
    let neighbor = t.path().join("unrelated");
    fs::write(&neighbor, b"preserve").unwrap();
    let target = t.path().join("fresh.sqlite3");
    assert!(matches!(
        Profile::create_inner(&target, Fault::BeforeCommit),
        Err(Error::BeforeCommit)
    ));
    assert!(!target.exists());
    assert_eq!(fs::read(&neighbor).unwrap(), b"preserve");
    assert_eq!(fs::read_dir(t.path()).unwrap().count(), 1);
    let p = Profile::create(&target).unwrap();
    validated(&p);
}
