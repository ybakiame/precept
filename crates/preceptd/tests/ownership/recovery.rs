use super::*;

#[test]
fn missing_legacy_and_unsafe_startup_never_create_or_repair() {
    let f = Fixture::created();
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
        // A remembered Profile that fails its checks never stops the daemon and
        // is never created or repaired: it starts unselected and says why.
        let mut d = Daemon::launch(f.temp.path(), &format!("bad-{n}"), Some(path));
        d.ready();
        let info = d.control("profile.info", json!({})).unwrap();
        assert_eq!(info["selected"], false, "{n}");
        assert!(info["startup_failure"].is_string(), "{n}: {info}");
        assert_eq!(info["write_enabled"], false);
        d.kill();
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
            precept_write::owner::Selected::select(&alias),
            Err(precept_write::Error::UnsafeWorkspace)
        ));
        let nested = f.temp.path().join("parent-link");
        symlink(parent, &nested).unwrap();
        assert!(matches!(
            precept_write::owner::Selected::select(&nested.join("profile.sqlite3")),
            Err(precept_write::Error::UnsafeWorkspace)
        ));
        let before = logical(&f.db);
        fs::set_permissions(&f.db, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            precept_write::owner::Selected::select(&f.db),
            Err(precept_write::Error::UnsafeWorkspace)
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
            precept_write::owner::Selected::select(&junction.join("profile.sqlite3")),
            Err(precept_write::Error::UnsafeWorkspace)
        ));
        fs::remove_dir(junction).unwrap();
        let inherited = parent.join("inherited.sqlite3");
        fs::write(&inherited, fs::read(&f.db).unwrap()).unwrap();
        assert!(matches!(
            precept_write::owner::Selected::select(&inherited),
            Err(precept_write::Error::UnsafeWorkspace)
        ));
    }
}
#[test]
fn real_ipc_disconnected_response_preserves_same_epoch_receipt_replay_and_v1_refusal() {
    let f = Fixture::created();
    let b = f.daemon.enable();
    let write = f.daemon.credential("operator-write.cap");
    let token = serde_json::to_value(&write).unwrap()["token"].clone();
    let payload = request(&b, "lost-response", suggest());
    f.daemon.runtime.block_on(async {
        let mut stream=precept_ipc::transport::connect(&f.daemon.location).await.unwrap();
        precept_ipc::write_frame(&mut stream,&json!({"api_version":precept_ipc::API_VERSION,"credential":token})).await.unwrap();
        assert_eq!(precept_ipc::read_frame(&mut stream).await.unwrap()["ready"],true);
        precept_ipc::write_frame(&mut stream,&json!({"api_version":precept_ipc::API_VERSION,"id":"lost","method":"profile.mutate","params":payload})).await.unwrap();
        // Deliberately disconnect without consuming the mutation response.
        drop(stream);
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if precept_store::SqliteStore::open(&f.db)
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
        let mut stream = precept_ipc::transport::connect(&f.daemon.location)
            .await
            .unwrap();
        precept_ipc::write_frame(&mut stream, &json!({"api_version":1,"credential":token}))
            .await
            .unwrap();
        let refused = precept_ipc::read_frame(&mut stream).await.unwrap();
        assert_eq!(refused["error"], "Version");
        assert_eq!(refused["api_version"], 2);
    });
    assert_eq!(logical(&f.db), before);
}
