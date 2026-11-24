use super::support::*;
use super::*;

#[test]
fn malformed_requests_and_text_bounds_are_sanitized_noops() {
    let (_t, p) = fresh();
    let mut w = p.writer().unwrap();
    let before = dump(&w.db);
    for bytes in [b"{}".as_slice(), b"[]", b"not json"] {
        assert!(Request::from_json(bytes).is_err());
    }
    let mut value = serde_json::to_value(request(&p, "invalid", suggest(Scope::Global))).unwrap();
    value["extra"] = true.into();
    assert!(Request::from_json(&serde_json::to_vec(&value).unwrap()).is_err());
    for value in ["", "  ", "nul\0value"] {
        let mut command = suggest(Scope::Global);
        if let Command::SuggestPreference { value: v, .. } = &mut command {
            *v = value.into()
        }
        assert_eq!(
            w.execute(&request(&p, "invalid", command)),
            Err(Error::InvalidCommand)
        );
    }
    let mut command = suggest(Scope::Global);
    if let Command::SuggestPreference { value, .. } = &mut command {
        *value = "a".repeat(4097)
    }
    assert_eq!(
        w.execute(&request(&p, "oversized", command)),
        Err(Error::InvalidCommand)
    );
    // The retired v1 envelope and non-canonical digests are refused, not read.
    let mut legacy = request(&p, "legacy", suggest(Scope::Global));
    legacy.version = 1;
    assert_eq!(w.execute(&legacy), Err(Error::InvalidCommand));
    for bad in ["0".repeat(63), "A".repeat(64), "g".repeat(64)] {
        let mut command = transition(1, Scope::Global, 1, 0);
        if let Command::ConfirmPreference {
            proposal_digest, ..
        } = &mut command
        {
            *proposal_digest = bad;
        }
        assert_eq!(
            w.execute(&request(&p, "bad-digest", command)),
            Err(Error::InvalidCommand)
        );
    }
    assert_eq!(dump(&w.db), before);
}
#[test]
fn invalid_handoff_status_snapshot_and_nul_are_refused() {
    let (_t, p) = ready();
    let mut w = p.writer().unwrap();
    let before = dump(&w.db);
    for raw in [
        "{}",
        "not-json",
        "[{\"check\":\"x\",\"status\":\"SUCCESS\"}]",
        "[{\"check\":\"x\",\"status\":\"PASS\",\"extra\":\"\\u0000\"}]",
    ] {
        let mut command = handoff("alpha", Expected::Absent);
        if let Command::PutHandoff {
            validation_json, ..
        } = &mut command
        {
            *validation_json = raw.into()
        }
        assert_eq!(
            w.execute(&request(&p, "bad-handoff", command)),
            Err(Error::InvalidCommand)
        );
    }
    let mut command = handoff("alpha", Expected::Absent);
    if let Command::PutHandoff { snapshot, .. } = &mut command {
        snapshot.state = "verified".into()
    }
    assert_eq!(
        w.execute(&request(&p, "bad-snapshot", command)),
        Err(Error::InvalidCommand)
    );
    assert_eq!(dump(&w.db), before);
}
#[test]
fn replaced_identity_hardlinks_and_bad_permissions_are_refused() {
    let (t, p) = fresh();
    let alias = t.path().join("alias");
    fs::hard_link(&p.path, &alias).unwrap();
    assert!(p.writer().is_err());
    fs::remove_file(alias).unwrap();
    let replacement = t.path().join("replacement");
    let file = files::create_private(&replacement).unwrap();
    drop(file);
    fs::write(&replacement, fs::read(&p.path).unwrap()).unwrap();
    files::check_file(&replacement).unwrap();
    assert_eq!(
        SqliteStore::open(&replacement)
            .unwrap()
            .metadata()
            .unwrap()
            .binding,
        p.binding()
    );
    fs::remove_file(&p.path).unwrap();
    fs::rename(replacement, &p.path).unwrap();
    assert!(p.writer().is_err());
}
#[cfg(unix)]
#[test]
fn symlink_parents_and_nonprivate_modes_are_refused() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let t = tempfile::tempdir().unwrap();
    let alias = t.path().join("alias");
    let parent = t.path().join("parent");
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&parent, &alias).unwrap();
    assert!(Profile::create(&alias.join("db")).is_err());
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(Profile::create(&parent.join("db")).is_err());
    let (_t, p) = fresh();
    fs::set_permissions(&p.path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(p.writer().is_err());
}
#[cfg(windows)]
#[test]
fn inherited_temp_acl_is_not_a_private_profile_parent() {
    let t = tempfile::tempdir().unwrap();
    assert!(Profile::create(&t.path().join("db")).is_err());
    files::private_test_parent(t.path());
    Profile::create(&t.path().join("db")).unwrap();
}
#[test]
fn debug_and_errors_do_not_disclose_private_inputs() {
    let (_t, p) = fresh();
    let req = request(&p, "secret-operation", suggest(Scope::Global));
    assert_eq!(format!("{p:?}"), "Profile(redacted)");
    assert_eq!(format!("{req:?}"), "Request(redacted)");
    let r = p.writer().unwrap().execute(&req).unwrap();
    assert_eq!(format!("{r:?}"), "Receipt(redacted)");
    assert_eq!(format!("{:?}", p.writer().unwrap()), "Writer(redacted)");
    let error = crate::sql_error(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_IOERR),
        Some("private SQL path and payload".into()),
    ));
    assert_eq!(error.to_string(), "IoFailure");
    let raw = serde_json::to_string(&r).unwrap();
    assert!(!raw.contains("synthetic"));
    assert!(!raw.contains("formatter"));
}
