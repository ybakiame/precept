use super::*;
use precept_core::operator::{
    PersonalSyncEntry, PersonalSyncLocalVersion, PersonalSyncSelection, RuleOptions,
    tombstone_digest,
};
use precept_store::profile::Receipt;
use std::sync::{Arc, Mutex};

type SavedReceipt = (Vec<u8>, Receipt);

fn fixture() -> (
    tempfile::TempDir,
    StateDir,
    PersonalSyncApplySubmission,
    Binding,
) {
    let temp = tempfile::tempdir().unwrap();
    let state_dir = StateDir::open(&temp.path().join("state")).unwrap();
    let local = precept_sync::generate_identity().unwrap();
    let remote = precept_sync::generate_identity_for_owner(&local.owner_id).unwrap();
    let profile_id = "a".repeat(32);
    let mut state = SyncState::new(&profile_id, local.clone());
    state
        .trust(
            precept_core::operator::PersonalSyncDeviceStatus {
                invitation: precept_sync::invitation(&remote).unwrap(),
                trusted_at_ms: 1,
                revoked: false,
            },
            &local.owner_fingerprint,
            1,
        )
        .unwrap();
    let content = PersonalSyncContent {
        key: "tone".into(),
        value: "remote".into(),
        tags: vec!["sync-test".into()],
        rule: RuleOptions::default(),
        enabled: true,
    };
    let digest = content.digest();
    let bundle = PersonalSyncBundle {
        format: precept_core::operator::PERSONAL_SYNC_FORMAT.into(),
        version: precept_core::operator::PERSONAL_SYNC_VERSION,
        bundle_id: "b".repeat(32),
        owner_id: local.owner_id.clone(),
        created_at_ms: 1,
        entries: vec![PersonalSyncEntry {
            content: content.clone(),
            clock: std::collections::BTreeMap::from([(remote.device_id.clone(), 1)]),
            digest: digest.clone(),
        }],
        tombstones: Vec::new(),
    };
    let envelope = seal_bundle(
        &bundle,
        &remote.device_id,
        &remote.signing_secret,
        Some(&local.age_recipient),
        None,
    )
    .unwrap();
    let binding = Binding {
        profile_id,
        generation: "c".repeat(32),
        owner_epoch: 1,
    };
    let submission = PersonalSyncApplySubmission {
        operation_id: "recovery".into(),
        expected_commit: 0,
        channel: precept_core::operator::Channel::Desktop,
        envelope,
        passphrase: None,
        local_versions: vec![PersonalSyncLocalVersion {
            key: content.key,
            clock: std::collections::BTreeMap::new(),
            digest: tombstone_digest("tone"),
        }],
        sender_device_id: remote.device_id,
        device_id: local.device_id,
        selections: vec![PersonalSyncSelection {
            key: "tone".into(),
            incoming_digest: digest,
            expected_local_digest: tombstone_digest("tone"),
            resolution: PersonalSyncResolution::Accept,
        }],
    };
    state::save(&state_dir, &state).unwrap();
    (temp, state_dir, submission, binding)
}

fn receipt(binding: &Binding, operation_id: &str) -> Receipt {
    Receipt {
        generation: binding.generation.clone(),
        operation_id: operation_id.into(),
        commit_sequence: 1,
        affected: Vec::new(),
    }
}

#[test]
fn a_committed_pending_apply_settles_from_its_receipt_after_an_epoch_change() {
    let (_temp, state_dir, submission, binding) = fixture();
    let original = Arc::new(Mutex::new(None));
    let first_original = Arc::clone(&original);
    assert_eq!(
        apply(
            &state_dir,
            &binding,
            submission.clone(),
            &BTreeMap::new(),
            0,
            |request| {
                *first_original.lock().unwrap() = Some(request.clone());
                Err(Error::OutcomeUnknown)
            },
            |_| Ok(None),
        ),
        Err(Error::OutcomeUnknown)
    );

    let saved: Arc<Mutex<Option<SavedReceipt>>> = Arc::new(Mutex::new(None));
    let stored = Arc::clone(&saved);
    let saved_original = Arc::clone(&original);
    *saved.lock().unwrap() = Some((
        Sha256::digest(
            serde_json::to_vec(saved_original.lock().unwrap().as_ref().unwrap()).unwrap(),
        )
        .to_vec(),
        receipt(&binding, &submission.operation_id),
    ));
    let mut next_binding = binding.clone();
    next_binding.owner_epoch += 1;
    let result = apply(
        &state_dir,
        &next_binding,
        submission.clone(),
        &BTreeMap::new(),
        0,
        |_| Err(Error::Busy),
        |_| Ok(stored.lock().unwrap().clone()),
    )
    .unwrap();
    assert_eq!(result.commit_sequence, Some(1));
    assert!(
        state::load(&state_dir, &binding.profile_id)
            .unwrap()
            .unwrap()
            .pending()
            .is_none()
    );
}

#[test]
fn explicit_abort_discards_only_a_proven_uncommitted_pending_apply() {
    let (_temp, state_dir, submission, binding) = fixture();
    assert_eq!(
        apply(
            &state_dir,
            &binding,
            submission.clone(),
            &BTreeMap::new(),
            0,
            |_| Err(Error::OutcomeUnknown),
            |_| Ok(None),
        ),
        Err(Error::OutcomeUnknown)
    );
    let result = abort(
        &state_dir,
        &binding,
        precept_core::operator::PersonalSyncAbortRequest {
            operation_id: submission.operation_id,
        },
        |_| Ok(None),
    )
    .unwrap();
    assert!(result.aborted);
    assert_eq!(result.settled_commit, None);
    assert!(
        state::load(&state_dir, &binding.profile_id)
            .unwrap()
            .unwrap()
            .pending()
            .is_none()
    );
}

#[test]
fn completed_replay_returns_even_while_another_operation_is_pending() {
    let (_temp, state_dir, submission, binding) = fixture();
    let expected = apply(
        &state_dir,
        &binding,
        submission.clone(),
        &BTreeMap::new(),
        0,
        |_| Ok(receipt(&binding, &submission.operation_id)),
        |_| Ok(None),
    )
    .unwrap();
    let mut state = state::load(&state_dir, &binding.profile_id)
        .unwrap()
        .unwrap();
    state
        .begin(PendingApply {
            operation_id: "other".into(),
            request_digest: "1".repeat(64),
            binding: binding.clone(),
            expected_commit: 1,
            versions: Vec::new(),
            applied: Vec::new(),
            command: None,
        })
        .unwrap();
    state::save(&state_dir, &state).unwrap();
    let replayed = apply(
        &state_dir,
        &binding,
        submission,
        &BTreeMap::new(),
        1,
        |_| Err(Error::Busy),
        |_| Err(Error::Unavailable),
    )
    .unwrap();
    assert_eq!(replayed, expected);
}

#[test]
fn an_uncommitted_pending_apply_rebinds_only_after_proving_no_receipt() {
    let (_temp, state_dir, submission, binding) = fixture();
    assert_eq!(
        apply(
            &state_dir,
            &binding,
            submission.clone(),
            &BTreeMap::new(),
            0,
            |_| Err(Error::OutcomeUnknown),
            |_| Ok(None),
        ),
        Err(Error::OutcomeUnknown)
    );
    let mut next_binding = binding.clone();
    next_binding.owner_epoch += 1;
    let executed = Arc::new(Mutex::new(None));
    let observed = Arc::clone(&executed);
    let result = apply(
        &state_dir,
        &next_binding,
        submission.clone(),
        &BTreeMap::new(),
        0,
        |request| {
            *observed.lock().unwrap() = Some(request.binding.clone());
            Ok(receipt(&next_binding, &submission.operation_id))
        },
        |_| Ok(None),
    )
    .unwrap();
    assert_eq!(result.commit_sequence, Some(1));
    assert_eq!(executed.lock().unwrap().as_ref(), Some(&next_binding));
}
