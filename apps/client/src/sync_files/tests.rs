use super::*;
use precept_core::operator::*;
use serde_json::json;

fn envelope() -> PersonalSyncEnvelope {
    let identity = precept_sync::generate_identity().unwrap();
    let bundle = PersonalSyncBundle {
        format: PERSONAL_SYNC_FORMAT.into(),
        version: PERSONAL_SYNC_VERSION,
        bundle_id: "b".repeat(32),
        owner_id: identity.owner_id,
        created_at_ms: 1,
        entries: Vec::new(),
        tombstones: Vec::new(),
    };
    precept_sync::seal_bundle(
        &bundle,
        &identity.device_id,
        &identity.signing_secret,
        None,
        Some("synthetic-file-password"),
    )
    .unwrap()
}

#[test]
fn age_files_round_trip_without_overwriting_or_exposing_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("preferences.age");
    let envelope = envelope();
    let value = serde_json::to_value(&envelope).unwrap();
    write_envelope(&path, &value).unwrap();
    assert_eq!(read_envelope(&path).unwrap(), envelope);
    assert_eq!(write_envelope(&path, &value), Err("Conflict"));
    let file = std::fs::read_to_string(&path).unwrap();
    assert!(file.starts_with("-----BEGIN AGE ENCRYPTED FILE-----"));
    assert!(!file.contains("bundle_id"));
    assert!(!file.contains("synthetic-file-password"));
}

#[test]
fn ciphertext_accepts_download_permissions_and_refuses_json_special_files_and_oversize() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("download.age");
    let envelope = envelope();
    std::fs::write(&path, &envelope.ciphertext).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
    assert_eq!(read_envelope(&path).unwrap(), envelope);
    let json_path = temp.path().join("file.json");
    std::fs::write(&json_path, json!({"ciphertext":"invalid"}).to_string()).unwrap();
    assert_eq!(read_envelope(&json_path), Err("InvalidRequest"));
    assert_eq!(read_envelope(temp.path()), Err("InvalidRequest"));
    assert_eq!(
        read_envelope(&temp.path().join("missing.age")),
        Err("NotFound")
    );
    let oversized = temp.path().join("oversized.age");
    std::fs::write(&oversized, vec![b'x'; MAX_JSON as usize + 1]).unwrap();
    assert_eq!(read_envelope(&oversized), Err("TooLarge"));
    #[cfg(unix)]
    {
        let link = temp.path().join("link.age");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert_eq!(read_envelope(&link), Err("InvalidRequest"));
    }
}
