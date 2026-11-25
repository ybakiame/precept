use super::*;
use serde_json::json;

fn model() -> Model {
    let mut model = Model::fixture();
    *model.profile_info = Some(ProfileInfo {
        selected: true,
        binding: Some(Binding {
            profile_id: "a".repeat(32),
            generation: "b".repeat(32),
            owner_epoch: 1,
        }),
        write_enabled: true,
        ..Default::default()
    });
    model
}

fn preview() -> PersonalSyncPreview {
    let content = |key: &str| PersonalSyncContent {
        key: key.into(),
        value: format!("value for {key}"),
        tags: vec!["sync".into()],
        rule: RuleOptions::default(),
        enabled: true,
    };
    let entry = |key: &str| PersonalSyncEntry {
        content: content(key),
        clock: BTreeMap::from([("sender".into(), 1)]),
        digest: "1".repeat(64),
    };
    let selection = |key: &str| PersonalSyncSelection {
        key: key.into(),
        incoming_digest: "1".repeat(64),
        expected_local_digest: "0".repeat(64),
        resolution: PersonalSyncResolution::Accept,
    };
    PersonalSyncPreview {
        local_contents: Vec::new(),
        bundle: PersonalSyncBundle {
            format: PERSONAL_SYNC_FORMAT.into(),
            version: PERSONAL_SYNC_VERSION,
            bundle_id: "b".repeat(32),
            owner_id: "c".repeat(32),
            created_at_ms: 1,
            entries: vec![entry("one"), entry("two")],
            tombstones: Vec::new(),
        },
        local_versions: vec![],
        sender_device_id: "d".repeat(32),
        device_id: "e".repeat(32),
        expected_commit: 7,
        selections: vec![selection("one"), selection("two"), selection("three")],
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
            PersonalSyncPreviewItem {
                key: "three".into(),
                merge: PersonalSyncMerge::Noop,
                selected: false,
                applied: true,
            },
        ],
    }
}

fn preview_state() -> PreviewState {
    PreviewState::new(
        preview(),
        PersonalSyncEnvelope {
            format: PERSONAL_SYNC_FORMAT.into(),
            version: PERSONAL_SYNC_VERSION,
            ciphertext: "ciphertext".into(),
        },
        Some("passphrase".into()),
        Guard::capture(&model()),
        "operation-1".into(),
    )
}

#[test]
fn default_selection_skips_conflicts_and_already_applied_entries() {
    let state = preview_state();
    let selections = state.selections().unwrap();
    assert_eq!(selections.len(), 1);
    assert_eq!(selections[0].key, "one");
    assert_eq!(state.choice("two"), ConflictChoice::Skip);
}

#[test]
fn form_passwords_are_redacted_and_cleared_when_the_profile_changes() {
    let mut value = model();
    let mut state = State::default();
    state.invalidate_for(&value);
    state.export.passphrase = "synthetic-export-password".to_owned().into();
    state.import.passphrase = "synthetic-import-password".to_owned().into();
    assert!(!format!("{:?}", state.export).contains("synthetic-export-password"));
    assert!(!format!("{:?}", state.import).contains("synthetic-import-password"));
    value.profile_epoch = value.profile_epoch.wrapping_add(1);
    state.invalidate_for(&value);
    assert!(state.export.passphrase.is_empty());
    assert!(state.import.passphrase.is_empty());
}

#[test]
fn already_applied_entries_cannot_be_reselected() {
    let mut state = preview_state();
    state.set_selected("three", true);
    assert!(!state.selected("three"));
    assert_eq!(state.selected_count(), 1);
}

#[test]
fn conflicts_require_an_explicit_choice_and_manual_content_is_validated() {
    let mut state = preview_state();
    state.set_selected("one", false);
    assert!(state.selections().is_none());
    state.set_choice("two", ConflictChoice::Accept);
    assert_eq!(
        state.selections().unwrap()[0].resolution,
        PersonalSyncResolution::Accept
    );
    state.set_choice("two", ConflictChoice::KeepLocal);
    assert_eq!(
        state.selections().unwrap()[0].resolution,
        PersonalSyncResolution::KeepLocal
    );
    state.set_choice("two", ConflictChoice::Manual);
    let mut draft = state.manual_draft("two");
    draft.value.clear();
    state.set_manual("two", draft.clone());
    assert!(
        state.selections().is_none(),
        "empty manual value is refused"
    );
    draft.value = "merged".into();
    draft.value = "merged".into();
    draft.tags = "Sync, testing".into();
    state.set_manual("two", draft);
    let selections = state.selections().unwrap();
    assert_eq!(
        selections[0].resolution,
        PersonalSyncResolution::Manual(PersonalSyncContent {
            key: "two".into(),
            value: "merged".into(),
            tags: vec!["sync".into(), "testing".into()],
            rule: RuleOptions::default(),
            enabled: true,
        })
    );
}

#[test]
fn prepared_apply_keeps_exact_retry_inputs_and_profile_identity_only() {
    let mut model = model();
    let state = preview_state();
    let apply = state.prepare().unwrap();
    let first = apply.params();
    assert_eq!(first["passphrase"], "passphrase");
    assert_eq!(first["operation_id"], "operation-1");
    assert_eq!(apply.params(), first);
    assert!(apply.submission.passphrase.is_none());

    model
        .profile_info
        .as_mut()
        .unwrap()
        .binding
        .as_mut()
        .unwrap()
        .owner_epoch = 2;
    model.epoch = model.epoch.wrapping_add(1);
    model.selected_project = Some("another-project".into());
    model.profile_read_epoch = model.profile_read_epoch.wrapping_add(1);
    assert!(
        apply.retryable(&model),
        "restart and Profile reads do not change the selected Profile"
    );

    model.profile_epoch = model.profile_epoch.wrapping_add(1);
    assert!(!apply.retryable(&model));
    model.profile_epoch = model.profile_epoch.wrapping_sub(1);

    model
        .profile_info
        .as_mut()
        .unwrap()
        .binding
        .as_mut()
        .unwrap()
        .generation = "c".repeat(32);
    assert!(!apply.retryable(&model));
}

#[test]
fn state_drops_secret_and_prepared_work_when_the_target_or_profile_changes() {
    let mut model = model();
    let mut state = State::default();
    state.invalidate_for(&model);
    state.preview = Some(preview_state());
    state.apply = state.preview.as_ref().and_then(PreviewState::prepare);
    state.invalidate_for(&model);
    assert!(
        state.preview.is_some(),
        "ordinary model updates preserve work"
    );
    model.connection.endpoint = "another".into();
    state.invalidate_for(&model);
    assert!(state.preview.is_none());
    assert!(state.apply.is_none());
}

#[test]
fn terminal_apply_drops_secrets_and_cannot_be_reused() {
    let mut state = preview_state();
    state.set_choice("two", ConflictChoice::Manual);
    let mut draft = state.manual_draft("two");
    draft.value = "private merged value".into();
    state.set_manual("two", draft);
    let mut apply = state.prepare().unwrap();
    assert!(apply.passphrase.is_some());
    apply.succeed(PersonalSyncApplyResult {
        operation_id: "operation-1".into(),
        commit_sequence: Some(8),
        applied: 2,
        changed: 2,
    });
    assert!(apply.passphrase.is_none());
    assert!(apply.submission.passphrase.is_none());
    assert!(apply.submission.selections.is_empty());
    assert!(!apply.retryable(&model()));
    assert!(!apply.params().to_string().contains("private merged value"));

    let mut apply = state.prepare().unwrap();
    apply.settle_abort(PersonalSyncAbortResult {
        operation_id: "operation-1".into(),
        settled_commit: None,
        aborted: true,
    });
    assert!(apply.passphrase.is_none());
    assert!(!apply.retryable(&model()));
}

#[test]
fn connection_transitions_fence_sync_even_when_coordinates_return() {
    let mut model = model();
    let guard = Guard::capture(&model);
    model.connection_epoch = model.connection_epoch.wrapping_add(1);
    assert!(!guard.matches(&model));

    model.connection_draft = model.connection.clone();
    model.sync.preview = Some(preview_state());
    assert!(model.apply_connection());
    assert!(model.profile_info.is_none());
    assert!(model.sync.preview.is_none());
}

#[test]
fn file_picker_completions_require_the_originating_dialog_generation() {
    let model = model();
    let mut state = State::default();
    state.invalidate_for(&model);
    state.dialog = Dialog::Apply;
    let guard = Guard::capture(&model);
    let generation = state.dialog_generation;
    assert!(state.dialog_is_current(&guard, generation, &Dialog::Apply));
    state.close_dialog();
    state.dialog = Dialog::Apply;
    assert!(!state.dialog_is_current(&guard, generation, &Dialog::Apply));
}

#[test]
fn stale_completions_cannot_reuse_a_serial_after_profile_or_connection_changes() {
    let mut model = model();
    let mut state = State::default();
    state.invalidate_for(&model);
    let (serial, origin) = (state.begin(), Guard::capture(&model));

    model.profile_epoch = model.profile_epoch.wrapping_add(1);
    let replacement = Guard::capture(&model);
    state.invalidate(replacement.clone());
    let next = state.begin();
    assert_ne!(serial, next);
    assert!(!state.finish(serial, &origin, &replacement, Ok(json!({}))));

    state.clear();
    let after_clear = state.begin();
    assert!(after_clear > next);
    assert!(!state.finish(serial, &origin, &replacement, Ok(json!({}))));
}

#[test]
fn late_or_cross_profile_completions_cannot_finish_newer_work() {
    let mut model = model();
    let mut state = State::default();
    state.invalidate_for(&model);
    let serial = state.begin();
    let origin = Guard::capture(&model);
    assert!(!state.finish(serial.wrapping_add(1), &origin, &origin, Ok(json!({}))));
    assert!(state.busy);
    model
        .profile_info
        .as_mut()
        .unwrap()
        .binding
        .as_mut()
        .unwrap()
        .profile_id = "c".repeat(32);
    let current = Guard::capture(&model);
    assert!(!state.finish(serial, &origin, &current, Ok(json!({}))));
    assert!(state.busy);
}
