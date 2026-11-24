use super::*;

#[test]
fn independent_daemons_lease_contention_process_kill_and_restart() {
    let mut f = Fixture::created();
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
    let mut other = Daemon::launch(f.temp.path(), "run-b", Some(&alias));
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
    f.daemon = Daemon::launch(f.temp.path(), "run-a", Some(&f.db));
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
fn sqlite_contention_does_not_advance_epoch_or_mutate_data() {
    let f = Fixture::created();
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
