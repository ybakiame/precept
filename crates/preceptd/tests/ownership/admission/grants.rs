use super::super::support::{Fixture, logical, private_bytes};
use precept_ipc::Error;
use serde_json::json;
use std::fs;

#[test]
fn failed_grant_publication_and_epoch_overflow_never_activate_a_writer() {
    let f = Fixture::created();
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
        precept_store::SqliteStore::open(&f.db)
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
fn unsafe_existing_lease_files_fail_before_epoch_admission() {
    let f = Fixture::created();
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
