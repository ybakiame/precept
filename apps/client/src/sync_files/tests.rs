use super::*;
use serde_json::json;

fn invitation() -> PersonalSyncInvitation {
    PersonalSyncInvitation {
        format: precept_core::operator::PERSONAL_SYNC_FORMAT.into(),
        version: precept_core::operator::PERSONAL_SYNC_VERSION,
        owner_id: "a".repeat(32),
        device_id: "b".repeat(32),
        age_recipient: "age1synthetic".into(),
        signing_public: "c".repeat(64),
        owner_fingerprint: "d".repeat(32),
        signature: "0".repeat(128),
    }
}

#[test]
fn invitation_files_bind_the_complete_public_invitation() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("invitation.json");
    let file = InvitationFile::new(invitation()).unwrap();
    write_new(&path, &file.value().unwrap()).unwrap();
    assert_eq!(read_invitation(&path).unwrap(), file);
    assert_eq!(write_new(&path, &file.value().unwrap()), Err("Conflict"));

    let changed = temp.path().join("changed.json");
    let mut value = file.value().unwrap();
    value["invitation"]["device_id"] = json!("e".repeat(32));
    write_new(&changed, &value).unwrap();
    assert_eq!(read_invitation(&changed), Err("InvalidRequest"));
}

#[test]
fn json_reads_are_bounded_strict_and_private() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("strict.json");
    write_new(&path, &json!({"known":true,"unknown":1})).unwrap();
    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Known {
        known: bool,
    }
    assert_eq!(read_json::<Known>(&path), Err("InvalidRequest"));
    assert_eq!(
        read_json::<Known>(&temp.path().join("missing.json")),
        Err("NotFound")
    );
}

#[test]
fn passphrase_files_preserve_whitespace_except_one_line_ending() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("passphrase.txt");
    let mut file = new_private(&path).unwrap();
    file.write_all(b"  correct horse battery staple  \r\n")
        .unwrap();
    drop(file);
    assert_eq!(
        read_passphrase(&path).unwrap(),
        "  correct horse battery staple  "
    );
}
