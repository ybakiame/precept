#[allow(dead_code)]
mod support;

use precept_core::operator::{
    Channel, Command, Expected, Origin, PersonalSyncApplySubmission, PersonalSyncChange,
    PersonalSyncContent, PersonalSyncEntry, PersonalSyncInvitation, PersonalSyncPreview,
    RuleOptions, Scope, tombstone_digest,
};
use precept_ipc::Error;
use precept_write::Binding;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};

static OPERATIONS: AtomicUsize = AtomicUsize::new(0);

fn operation(prefix: &str) -> String {
    format!("{prefix}-{}", OPERATIONS.fetch_add(1, Ordering::SeqCst))
}

fn write_call(fixture: &support::Fixture, method: &str, params: Value) -> Value {
    fixture
        .daemon
        .call(
            &fixture.daemon.credential("operator-write.cap"),
            method,
            params,
        )
        .unwrap()
}

fn trust_params(invitation: &PersonalSyncInvitation) -> Value {
    json!({
        "invitation": invitation,
        "expected_owner_fingerprint": invitation.owner_fingerprint,
        "expected_invitation_fingerprint": precept_sync::invitation_fingerprint(invitation).unwrap(),
    })
}

fn pair() -> (
    support::Fixture,
    support::Fixture,
    PersonalSyncInvitation,
    PersonalSyncInvitation,
) {
    let a = support::Fixture::created();
    let b = support::Fixture::created();
    let _ = a.daemon.enable();
    let _ = b.daemon.enable();
    let a_invitation: PersonalSyncInvitation =
        serde_json::from_value(write_call(&a, "profile.sync.init", json!({}))).unwrap();
    let b_invitation: PersonalSyncInvitation = serde_json::from_value(write_call(
        &b,
        "profile.sync.pair",
        trust_params(&a_invitation),
    ))
    .unwrap();
    write_call(&a, "profile.sync.trust", trust_params(&b_invitation));
    a.daemon
        .control("profile.write.disable", json!({}))
        .unwrap();
    b.daemon
        .control("profile.write.disable", json!({}))
        .unwrap();
    (a, b, a_invitation, b_invitation)
}

fn confirm_global(
    fixture: &support::Fixture,
    binding: &Binding,
    key: &str,
    value: &str,
    enabled: bool,
) {
    let receipt = fixture
        .daemon
        .mutate(
            binding,
            &operation("suggest"),
            Command::SuggestPreference {
                scope: Scope::Global,
                key: key.into(),
                value: value.into(),
                tags: vec!["sync-test".into()],
                rule: RuleOptions::default(),
                origin: Origin::Operator,
                expected: Expected::Absent,
            },
        )
        .unwrap();
    let id = receipt["affected"][0]["record_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let page = fixture
        .daemon
        .control(
            "profile.preferences.list",
            json!({"scope":Scope::Global,"after_id":0}),
        )
        .unwrap();
    let record = page["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["id"].as_i64() == id.parse().ok())
        .unwrap()
        .clone();
    fixture
        .daemon
        .mutate(
            binding,
            &operation("confirm"),
            Command::ConfirmPreference {
                id: id.parse().unwrap(),
                scope: Scope::Global,
                proposal_digest: record["proposal_digest"].as_str().unwrap().into(),
                channel: Channel::Desktop,
                expected: Expected::Revision(1),
                note: None,
            },
        )
        .unwrap();
    if !enabled {
        fixture
            .daemon
            .mutate(
                binding,
                &operation("disable"),
                Command::SetPreferenceEnabled {
                    id: id.parse().unwrap(),
                    scope: Scope::Global,
                    enabled: false,
                    channel: Channel::Desktop,
                    expected: Expected::Revision(2),
                },
            )
            .unwrap();
    }
}

fn deprecate_global(fixture: &support::Fixture, binding: &Binding, key: &str) {
    let page = fixture
        .daemon
        .control(
            "profile.preferences.list",
            json!({"scope":Scope::Global,"after_id":0}),
        )
        .unwrap();
    let record = page["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|record| record["key"] == key && record["status"] == "confirmed")
        .max_by_key(|record| record["id"].as_i64().unwrap())
        .unwrap()
        .clone();
    fixture
        .daemon
        .mutate(
            binding,
            &operation("deprecate"),
            Command::DeprecatePreference {
                id: record["id"].as_i64().unwrap(),
                scope: Scope::Global,
                proposal_digest: record["proposal_digest"].as_str().unwrap().into(),
                channel: Channel::Desktop,
                expected: Expected::Revision(record["revision"].as_i64().unwrap()),
                note: None,
            },
        )
        .unwrap();
}

fn revise_global(fixture: &support::Fixture, binding: &Binding, key: &str, value: &str) {
    let page = fixture
        .daemon
        .control(
            "profile.preferences.list",
            json!({"scope":Scope::Global,"after_id":0}),
        )
        .unwrap();
    let record = page["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|record| record["key"] == key && record["status"] == "confirmed")
        .max_by_key(|record| record["id"].as_i64().unwrap())
        .unwrap()
        .clone();
    fixture
        .daemon
        .mutate(
            binding,
            &operation("revise"),
            Command::RevisePreference {
                id: record["id"].as_i64().unwrap(),
                scope: Scope::Global,
                proposal_digest: record["proposal_digest"].as_str().unwrap().into(),
                key: key.into(),
                value: value.into(),
                tags: serde_json::from_value(record["tags"].clone()).unwrap(),
                rule: serde_json::from_value(record["rule"].clone()).unwrap(),
                channel: Channel::Desktop,
                expected: Expected::Revision(record["revision"].as_i64().unwrap()),
                note: None,
            },
        )
        .unwrap();
}

fn export(fixture: &support::Fixture, recipient: &str) -> Value {
    write_call(
        fixture,
        "profile.sync.export",
        json!({"recipient":recipient,"passphrase":null,"keys":null}),
    )
}

fn open_preview(fixture: &support::Fixture, envelope: &Value) -> PersonalSyncPreview {
    serde_json::from_value(
        fixture
            .daemon
            .control(
                "profile.sync.preview",
                json!({"envelope":envelope,"passphrase":null}),
            )
            .unwrap(),
    )
    .unwrap()
}

fn submission(
    preview: &PersonalSyncPreview,
    envelope: &Value,
    passphrase: Option<&str>,
    operation_id: String,
) -> PersonalSyncApplySubmission {
    PersonalSyncApplySubmission {
        operation_id,
        expected_commit: preview.expected_commit,
        channel: Channel::Desktop,
        envelope: serde_json::from_value(envelope.clone()).unwrap(),
        passphrase: passphrase.map(str::to_owned),
        local_versions: preview.local_versions.clone(),
        sender_device_id: preview.sender_device_id.clone(),
        device_id: preview.device_id.clone(),
        selections: preview.selections.clone(),
    }
}

fn apply(fixture: &support::Fixture, submission: PersonalSyncApplySubmission) -> Value {
    fixture
        .daemon
        .call(
            &fixture.daemon.credential("operator-write.cap"),
            "profile.sync.apply",
            serde_json::to_value(submission).unwrap(),
        )
        .unwrap()
}

fn confirmed(fixture: &support::Fixture, key: &str) -> Option<Value> {
    let page = fixture
        .daemon
        .control(
            "profile.preferences.list",
            json!({"scope":Scope::Global,"after_id":0}),
        )
        .unwrap();
    page["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|record| record["key"] == key && record["status"] == "confirmed")
        .max_by_key(|record| record["id"].as_i64().unwrap())
        .cloned()
}

#[test]
fn paired_devices_round_trip_disabled_entries_and_replay_without_duplicates() {
    let (a, b, a_invitation, b_invitation) = pair();
    let a_binding = a.daemon.enable();
    confirm_global(&a, &a_binding, "tone", "Concise answers.", true);
    confirm_global(&a, &a_binding, "quiet", "No filler.", false);
    let envelope = export(&a, &b_invitation.age_recipient);
    let preview = open_preview(&b, &envelope);
    assert_eq!(preview.items.len(), 2);
    assert!(preview.items.iter().all(|item| item.selected));

    let _ = b.daemon.enable();
    let submit = submission(&preview, &envelope, None, operation("import"));
    let result = apply(&b, submit.clone());
    assert_eq!(result["changed"], 2);
    assert_eq!(confirmed(&b, "tone").unwrap()["value"], "Concise answers.");
    assert_eq!(confirmed(&b, "quiet").unwrap()["enabled"], false);
    assert_eq!(apply(&b, submit), result);

    let a_envelope = export(&b, &a_invitation.age_recipient);
    let a_preview = open_preview(&a, &a_envelope);
    assert!(
        a_preview
            .items
            .iter()
            .all(|item| item.merge == precept_core::operator::PersonalSyncMerge::Accept)
    );
    let result = apply(
        &a,
        submission(&a_preview, &a_envelope, None, operation("return")),
    );
    assert_eq!(result["changed"], 0);
    let _ = a_binding;
}

#[test]
fn selective_import_can_resume_and_tombstones_delete_only_the_named_key() {
    let (a, b, _, b_invitation) = pair();
    let a_binding = a.daemon.enable();
    confirm_global(&a, &a_binding, "one", "First.", true);
    confirm_global(&a, &a_binding, "two", "Second.", true);
    let envelope = export(&a, &b_invitation.age_recipient);
    let preview = open_preview(&b, &envelope);
    let mut first = submission(&preview, &envelope, None, operation("partial"));
    first.selections.retain(|selection| selection.key == "two");
    let _ = b.daemon.enable();
    apply(&b, first);
    assert!(confirmed(&b, "one").is_none());
    assert!(confirmed(&b, "two").is_some());

    let resumed = open_preview(&b, &envelope);
    assert!(
        resumed
            .items
            .iter()
            .find(|item| item.key == "one")
            .unwrap()
            .selected
    );
    assert!(
        !resumed
            .items
            .iter()
            .find(|item| item.key == "two")
            .unwrap()
            .selected
    );
    let mut resumed = submission(&resumed, &envelope, None, operation("resume"));
    resumed
        .selections
        .retain(|selection| selection.key == "one");
    apply(&b, resumed);
    assert!(confirmed(&b, "one").is_some());

    deprecate_global(&a, &a_binding, "one");
    let deleted_envelope = export(&a, &b_invitation.age_recipient);
    let mut deleted = open_preview(&b, &deleted_envelope);
    assert_eq!(deleted.bundle.tombstones.len(), 1);
    for selection in &mut deleted.selections {
        selection.resolution = precept_core::operator::PersonalSyncResolution::Accept;
    }
    apply(
        &b,
        submission(&deleted, &deleted_envelope, None, operation("delete")),
    );
    assert!(confirmed(&b, "one").is_none());
    assert!(confirmed(&b, "two").is_some());

    confirm_global(&a, &a_binding, "one", "Recreated.", true);
    let recreated_envelope = export(&a, &b_invitation.age_recipient);
    let mut recreated = open_preview(&b, &recreated_envelope);
    assert_eq!(
        recreated.items[0].merge,
        precept_core::operator::PersonalSyncMerge::Conflict
    );
    for selection in &mut recreated.selections {
        selection.resolution = precept_core::operator::PersonalSyncResolution::Accept;
    }
    apply(
        &b,
        submission(&recreated, &recreated_envelope, None, operation("recreate")),
    );
    assert_eq!(confirmed(&b, "one").unwrap()["value"], "Recreated.");
}

#[test]
fn authority_tampering_and_revocation_are_refused() {
    let (a, b, _, b_invitation) = pair();
    let a_binding = a.daemon.enable();
    confirm_global(&a, &a_binding, "tone", "Concise.", true);
    let envelope = export(&a, &b_invitation.age_recipient);
    let preview = open_preview(&b, &envelope);
    assert!(
        std::fs::symlink_metadata(b.daemon.location.runtime_dir.join("operator-write.cap"))
            .is_err()
    );
    assert_eq!(
        b.daemon.call(
            &b.daemon.credential("operator.cap"),
            "profile.sync.revoke",
            json!({"device_id":a.daemon.control("profile.sync.status", json!({})).unwrap()["device_id"]}),
        ),
        Err(Error::Unauthorized)
    );
    assert_eq!(
        b.daemon.call(
            &b.daemon.credential("operator.cap"),
            "profile.sync.apply",
            json!({}),
        ),
        Err(Error::Unauthorized)
    );
    let _ = b.daemon.enable();
    let attacker = precept_sync::generate_identity_for_owner(&b_invitation.owner_id).unwrap();
    let attacker = precept_sync::invitation(&attacker).unwrap();
    let mut forged_pair = trust_params(&attacker);
    forged_pair["expected_invitation_fingerprint"] =
        json!(precept_sync::invitation_fingerprint(&b_invitation).unwrap());
    assert_eq!(
        b.daemon.call(
            &b.daemon.credential("operator-write.cap"),
            "profile.sync.trust",
            forged_pair,
        ),
        Err(Error::InvalidRequest)
    );
    let mut forged = submission(&preview, &envelope, None, operation("forged"));
    forged.envelope.ciphertext.push('x');
    assert_eq!(
        b.daemon.call(
            &b.daemon.credential("operator-write.cap"),
            "profile.sync.apply",
            serde_json::to_value(forged).unwrap(),
        ),
        Err(Error::InvalidRequest)
    );
    assert_eq!(
        b.daemon.call(
            &b.daemon.credential("operator-write.cap"),
            "profile.sync.status",
            json!({}),
        ),
        Err(Error::Unauthorized)
    );

    let mut tampered = envelope.clone();
    tampered["ciphertext"] = json!(format!("{}x", tampered["ciphertext"].as_str().unwrap()));
    assert_eq!(
        b.daemon.control(
            "profile.sync.preview",
            json!({"envelope":tampered,"passphrase":null}),
        ),
        Err(Error::InvalidRequest)
    );
    write_call(
        &b,
        "profile.sync.revoke",
        json!({"device_id":a.daemon.control("profile.sync.status", json!({})).unwrap()["device_id"]}),
    );
    assert_eq!(
        b.daemon.control(
            "profile.sync.preview",
            json!({"envelope":envelope,"passphrase":null}),
        ),
        Err(Error::Unauthorized)
    );
}

#[test]
fn conflict_keep_local_advances_the_clock_and_does_not_prompt_again() {
    let (a, b, a_invitation, b_invitation) = pair();
    let a_binding = a.daemon.enable();
    let b_binding = b.daemon.enable();
    confirm_global(&a, &a_binding, "tone", "initial", true);
    let envelope = export(&a, &b_invitation.age_recipient);
    apply(
        &b,
        submission(
            &open_preview(&b, &envelope),
            &envelope,
            None,
            operation("seed"),
        ),
    );
    revise_global(&a, &a_binding, "tone", "from-a");
    revise_global(&b, &b_binding, "tone", "from-b");
    let envelope = export(&a, &b_invitation.age_recipient);
    let mut preview = open_preview(&b, &envelope);
    assert_eq!(
        preview.items[0].merge,
        precept_core::operator::PersonalSyncMerge::Conflict
    );
    assert!(!preview.items[0].selected);
    preview.selections[0].resolution = precept_core::operator::PersonalSyncResolution::KeepLocal;
    let result = apply(&b, submission(&preview, &envelope, None, operation("keep")));
    assert_eq!(result["changed"], 0);
    assert_eq!(confirmed(&b, "tone").unwrap()["value"], "from-b");

    let envelope = export(&b, &a_invitation.age_recipient);
    let preview = open_preview(&a, &envelope);
    assert_eq!(
        preview.items[0].merge,
        precept_core::operator::PersonalSyncMerge::Accept
    );
    apply(
        &a,
        submission(&preview, &envelope, None, operation("return")),
    );
    assert_eq!(confirmed(&a, "tone").unwrap()["value"], "from-b");
}

#[test]
fn a_local_edit_after_preview_refuses_the_stale_apply() {
    let (a, b, _, b_invitation) = pair();
    let a_binding = a.daemon.enable();
    let b_binding = b.daemon.enable();
    confirm_global(&a, &a_binding, "tone", "remote", true);
    let envelope = export(&a, &b_invitation.age_recipient);
    let preview = open_preview(&b, &envelope);
    confirm_global(&b, &b_binding, "local-key", "local edit", true);
    let submit = submission(&preview, &envelope, None, operation("stale"));
    assert_eq!(
        b.daemon.call(
            &b.daemon.credential("operator-write.cap"),
            "profile.sync.apply",
            serde_json::to_value(submit).unwrap(),
        ),
        Err(Error::Conflict)
    );
}

#[test]
fn identity_and_trust_survive_a_daemon_restart() {
    let mut a = support::Fixture::unselected();
    a.daemon
        .control("profile.create", json!({"path":a.db,"remember":true}))
        .unwrap();
    let _ = a.daemon.enable();
    write_call(&a, "profile.sync.init", json!({}));
    let before: Value = a.daemon.control("profile.sync.status", json!({})).unwrap();
    let base = a.temp.path().to_path_buf();
    a.daemon.kill();
    a.daemon = support::Daemon::relaunch(&base, "run-a");
    a.daemon.ready();
    let after: Value = a.daemon.control("profile.sync.status", json!({})).unwrap();
    assert_eq!(before["device_id"], after["device_id"]);
    assert_eq!(before["owner_fingerprint"], after["owner_fingerprint"]);
    assert_eq!(before["trusted_devices"], after["trusted_devices"]);
}

#[test]
fn rotated_keys_replace_trust_only_after_explicit_revocation() {
    let (a, b, _, b_invitation) = pair();
    let a_binding = a.daemon.enable();
    let _ = b.daemon.enable();
    confirm_global(&a, &a_binding, "tone", "Rotated.", true);
    let rotated: PersonalSyncInvitation =
        serde_json::from_value(write_call(&a, "profile.sync.rotate", json!({}))).unwrap();
    let old_device =
        a.daemon.control("profile.sync.status", json!({})).unwrap()["device_id"].clone();
    assert_eq!(
        b.daemon.call(
            &b.daemon.credential("operator-write.cap"),
            "profile.sync.trust",
            trust_params(&rotated),
        ),
        Err(Error::Conflict)
    );
    write_call(&b, "profile.sync.revoke", json!({"device_id":old_device}));
    write_call(&b, "profile.sync.trust", trust_params(&rotated));
    let envelope = export(&a, &b_invitation.age_recipient);
    let preview = open_preview(&b, &envelope);
    apply(
        &b,
        submission(&preview, &envelope, None, operation("rotated")),
    );
    assert_eq!(confirmed(&b, "tone").unwrap()["value"], "Rotated.");
}

#[test]
fn passphrase_bundles_use_the_same_sender_trust_and_apply_path() {
    let (a, b, _, _) = pair();
    let a_binding = a.daemon.enable();
    confirm_global(&a, &a_binding, "tone", "passphrase", true);
    let passphrase = "correct horse battery staple";
    let envelope = write_call(
        &a,
        "profile.sync.export",
        json!({"recipient":null,"passphrase":passphrase,"keys":null}),
    );
    let preview: PersonalSyncPreview = serde_json::from_value(
        b.daemon
            .control(
                "profile.sync.preview",
                json!({"envelope":envelope,"passphrase":passphrase}),
            )
            .unwrap(),
    )
    .unwrap();
    let _ = b.daemon.enable();
    apply(
        &b,
        submission(
            &preview,
            &envelope,
            Some(passphrase),
            operation("passphrase"),
        ),
    );
    assert_eq!(confirmed(&b, "tone").unwrap()["value"], "passphrase");
}

#[test]
fn direct_writer_sync_commands_cannot_bypass_sidecar_recovery() {
    let (a, _, _, _) = pair();
    let binding = a.daemon.enable();
    let content = PersonalSyncContent {
        key: "tone".into(),
        value: "Concise.".into(),
        tags: vec!["sync-test".into()],
        rule: RuleOptions::default(),
        enabled: true,
    };
    let digest = content.digest();
    let command = Command::ApplyPersonalSync {
        bundle_id: "b".repeat(32),
        bundle_digest: "0".repeat(64),
        device_id: "d".repeat(32),
        expected_commit: 0,
        channel: Channel::Desktop,
        changes: vec![PersonalSyncChange {
            key: content.key.clone(),
            expected_digest: tombstone_digest(&content.key),
            entry: Some(PersonalSyncEntry {
                content,
                clock: std::collections::BTreeMap::from([("d".repeat(32), 1)]),
                digest,
            }),
            tombstone: None,
        }],
    };
    assert_eq!(
        a.daemon.mutate(&binding, &operation("direct"), command,),
        Err(Error::InvalidRequest)
    );
}
