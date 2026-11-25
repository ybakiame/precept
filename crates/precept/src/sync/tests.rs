use super::*;
use precept_ipc::operator::{PersonalSyncMerge, PersonalSyncPreviewItem, RuleOptions};
use std::collections::BTreeMap;

fn preview() -> PersonalSyncPreview {
    let local = |key: &str| PersonalSyncLocalVersion {
        key: key.into(),
        clock: BTreeMap::new(),
        digest: "0".repeat(64),
    };
    let selection = |key: &str, resolution| PersonalSyncSelection {
        key: key.into(),
        incoming_digest: "1".repeat(64),
        expected_local_digest: "0".repeat(64),
        resolution,
    };
    PersonalSyncPreview {
        local_contents: Vec::new(),
        bundle: precept_ipc::operator::PersonalSyncBundle {
            format: precept_ipc::operator::PERSONAL_SYNC_FORMAT.into(),
            version: precept_ipc::operator::PERSONAL_SYNC_VERSION,
            bundle_id: "b".repeat(32),
            owner_id: "c".repeat(32),
            created_at_ms: 1,
            entries: Vec::new(),
            tombstones: Vec::new(),
        },
        local_versions: vec![local("one"), local("two")],
        sender_device_id: "d".repeat(32),
        device_id: "e".repeat(32),
        expected_commit: 7,
        selections: vec![
            selection("one", PersonalSyncResolution::Accept),
            selection("two", PersonalSyncResolution::KeepLocal),
        ],
        items: vec![
            PersonalSyncPreviewItem {
                key: "one".into(),
                merge: PersonalSyncMerge::Accept,
                selected: true,
                applied: false,
            },
            PersonalSyncPreviewItem {
                key: "two".into(),
                merge: PersonalSyncMerge::Conflict,
                selected: false,
                applied: false,
            },
        ],
    }
}

#[test]
fn selection_excludes_unresolved_conflicts_and_can_accept_them_explicitly() {
    let mut value = preview();
    select(&mut value, &[], false, None).unwrap();
    assert_eq!(value.selections.len(), 1);
    assert_eq!(value.selections[0].key, "one");
    assert!(value.items[0].selected);
    assert!(!value.items[1].selected);

    let mut accepted = preview();
    select(&mut accepted, &["two".into()], true, None).unwrap();
    assert_eq!(accepted.selections.len(), 1);
    assert_eq!(accepted.selections[0].key, "two");
    assert_eq!(
        accepted.selections[0].resolution,
        PersonalSyncResolution::Accept
    );
}

#[test]
fn already_applied_entries_stay_excluded_even_when_selected_explicitly() {
    let mut value = preview();
    value.items[0].applied = true;
    select(&mut value, &["one".into()], true, None).unwrap();
    assert!(value.selections.is_empty());
    assert!(!value.items[0].selected);
}

#[test]
fn resolution_files_validate_manual_content_and_reject_duplicates() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("resolutions.json");
    write_new(
        &path,
        &json!({
            "version":1,
            "items":[{
                "key":"two",
                "resolution":{
                    "resolution":"manual",
                    "key":"two",
                    "value":"merged",
                    "tags":["test"],
                    "rule":RuleOptions::default(),
                    "enabled":true
                }
            }]
        }),
    )
    .unwrap();
    let resolutions = read_resolutions(&path).unwrap();
    let mut value = preview();
    select(&mut value, &[], false, Some(&resolutions)).unwrap();
    assert_eq!(value.selections.len(), 2);
    assert_eq!(
        value.selections[1].resolution,
        PersonalSyncResolution::Manual(precept_ipc::operator::PersonalSyncContent {
            key: "two".into(),
            value: "merged".into(),
            tags: vec!["test".into()],
            rule: RuleOptions::default(),
            enabled: true,
        })
    );

    let duplicate = temp.path().join("duplicate.json");
    write_new(
        &duplicate,
        &json!({
            "version":1,
            "items":[
                {"key":"two","resolution":"accept"},
                {"key":"two","resolution":"keep_local"}
            ]
        }),
    )
    .unwrap();
    assert_eq!(read_resolutions(&duplicate), Err(Error::InvalidRequest));
}

#[test]
fn unknown_and_duplicate_key_filters_are_refused() {
    let mut value = preview();
    assert_eq!(
        select(&mut value, &["missing".into()], false, None),
        Err(Error::InvalidRequest)
    );
    assert_eq!(
        select(&mut value, &["one".into(), "one".into()], false, None),
        Err(Error::InvalidRequest)
    );
}

#[test]
fn settlement_replies_must_name_the_requested_operation() {
    let value = json!({"operation_id":"operation-1","applied":1});
    assert_eq!(correlated(value.clone(), "operation-1").unwrap(), value);
    assert_eq!(
        correlated(value.clone(), "operation-2"),
        Err(Error::OutcomeUnknown)
    );
    assert_eq!(
        correlated(json!({}), "operation-1"),
        Err(Error::OutcomeUnknown)
    );
}

#[test]
fn prepared_files_round_trip_the_exact_retry_input_without_overwriting() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("prepared.json");
    let value = preview();
    let prepared = Prepared {
        format: PREPARED_FORMAT.into(),
        version: PREPARED_VERSION,
        scope: precept_ipc::operator::PersonalSyncProfileScope {
            profile_id: "a".repeat(32),
            generation: "b".repeat(32),
        },
        envelope: PersonalSyncEnvelope {
            format: precept_ipc::operator::PERSONAL_SYNC_FORMAT.into(),
            version: precept_ipc::operator::PERSONAL_SYNC_VERSION,
            ciphertext: "ciphertext".into(),
        },
        local_versions: value.local_versions,
        sender_device_id: value.sender_device_id,
        device_id: value.device_id,
        expected_commit: value.expected_commit,
        selections: value.selections,
    };
    write_new(&path, &serde_json::to_value(&prepared).unwrap()).unwrap();
    let loaded: Prepared = read(&path).unwrap();
    assert_eq!(loaded, prepared);
    assert_eq!(
        write_new(&path, &serde_json::to_value(&prepared).unwrap()),
        Err(Error::Conflict)
    );
    let malformed = temp.path().join("malformed.json");
    let mut value = serde_json::to_value(&prepared).unwrap();
    value["passphrase"] = json!("must-not-be-accepted");
    write_new(&malformed, &value).unwrap();
    assert_eq!(read::<Prepared>(&malformed), Err(Error::InvalidRequest));
}

#[test]
fn passphrase_files_remove_only_the_line_ending() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("passphrase.txt");
    let mut file = precept_ipc::transport::new_private(&path).unwrap();
    file.write_all(b"  correct horse battery staple  \r\n")
        .unwrap();
    drop(file);
    assert_eq!(
        read_passphrase(&path).unwrap(),
        "  correct horse battery staple  "
    );
}
