use super::state::{PendingApply, SyncState};
use super::*;
use crate::state::StateDir;
use precept_core::operator::{
    Binding, Command, PersonalSyncAbortRequest, PersonalSyncAbortResult, PersonalSyncApplyRequest,
    PersonalSyncApplyResult, PersonalSyncApplySubmission, PersonalSyncBundle, PersonalSyncContent,
    PersonalSyncDeviceStatus, PersonalSyncEntry, PersonalSyncEnvelope, PersonalSyncExportRequest,
    PersonalSyncInvitation, PersonalSyncMerge, PersonalSyncOpenRequest, PersonalSyncPreview,
    PersonalSyncPreviewItem, PersonalSyncResetRequest, PersonalSyncResolution,
    PersonalSyncRevokeRequest, PersonalSyncSelection, PersonalSyncStatus, PersonalSyncTombstone,
    PersonalSyncTrustRequest, PersonalSyncVersion, Scope, classify_merge, plan_personal_sync_apply,
    valid_identity,
};
use precept_ipc::{Error, Result};
use precept_store::{SqliteStore, profile::Receipt};
use precept_sync::{OpenedBundle, invitation, open_signed_bundle, seal_bundle};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) fn uninitialized_status() -> PersonalSyncStatus {
    PersonalSyncStatus {
        initialized: false,
        owner_fingerprint: None,
        device_id: None,
        trusted_devices: Vec::new(),
        tracked_keys: 0,
        tombstones: 0,
        applied_entries: 0,
        pending_operation: None,
    }
}

pub(crate) fn current_globals(
    store: &SqliteStore,
) -> Result<BTreeMap<String, PersonalSyncContent>> {
    let mut current = BTreeMap::new();
    let mut after = 0;
    loop {
        let page = store
            .operator_preferences(&Scope::Global, after)
            .map_err(|_| Error::Unavailable)?;
        for record in &page.records {
            if record.status == "confirmed" {
                current.insert(
                    record.key.clone(),
                    PersonalSyncContent {
                        key: record.key.clone(),
                        value: record.value.clone(),
                        tags: record.tags.clone(),
                        rule: record.rule.clone(),
                        enabled: record.enabled,
                    },
                );
            }
        }
        if current.len() > MAX_TRACKED_KEYS {
            return Err(Error::Capacity);
        }
        match page.next_after_id {
            Some(next) => after = next,
            None => return Ok(current),
        }
    }
}

pub(crate) fn status(state_dir: &StateDir, profile_id: &str) -> Result<PersonalSyncStatus> {
    Ok(state::load(state_dir, profile_id)?
        .as_ref()
        .map(SyncState::status)
        .unwrap_or_else(uninitialized_status))
}

pub(crate) fn init(
    state_dir: &StateDir,
    profile_id: &str,
    current: &BTreeMap<String, PersonalSyncContent>,
) -> Result<PersonalSyncInvitation> {
    if let Some(state) = state::load(state_dir, profile_id)? {
        return invitation(&state.identity).map_err(|_| Error::Unavailable);
    }
    let identity = precept_sync::generate_identity().map_err(|_| Error::Unavailable)?;
    let mut state = SyncState::new(profile_id, identity);
    state.reconcile(current)?;
    let result = invitation(&state.identity).map_err(|_| Error::Unavailable)?;
    state::save(state_dir, &state)?;
    Ok(result)
}

pub(crate) fn pair(
    state_dir: &StateDir,
    profile_id: &str,
    request: PersonalSyncTrustRequest,
    current: &BTreeMap<String, PersonalSyncContent>,
) -> Result<PersonalSyncInvitation> {
    if state::load(state_dir, profile_id)?.is_some()
        || !valid_identity(&request.expected_owner_fingerprint)
        || !valid_identity(&request.expected_invitation_fingerprint)
        || request.invitation.owner_fingerprint != request.expected_owner_fingerprint
        || precept_sync::invitation_fingerprint(&request.invitation).as_deref()
            != Ok(&request.expected_invitation_fingerprint)
        || !precept_sync::verify_invitation(&request.invitation)
    {
        return Err(Error::Conflict);
    }
    let identity = precept_sync::generate_identity_for_owner(&request.invitation.owner_id)
        .map_err(|_| Error::Unavailable)?;
    let mut state = SyncState::new(profile_id, identity);
    let now = now_ms();
    state.trust(
        PersonalSyncDeviceStatus {
            invitation: request.invitation,
            trusted_at_ms: now,
            revoked: false,
        },
        &request.expected_owner_fingerprint,
        now,
    )?;
    state.reconcile(current)?;
    let result = invitation(&state.identity).map_err(|_| Error::Unavailable)?;
    state::save(state_dir, &state)?;
    Ok(result)
}

pub(crate) fn get_invitation(
    state_dir: &StateDir,
    profile_id: &str,
) -> Result<PersonalSyncInvitation> {
    let state = state::load(state_dir, profile_id)?.ok_or(Error::NotFound)?;
    invitation(&state.identity).map_err(|_| Error::Unavailable)
}

pub(crate) fn trust(
    state_dir: &StateDir,
    profile_id: &str,
    request: PersonalSyncTrustRequest,
) -> Result<PersonalSyncStatus> {
    if !valid_identity(&request.expected_owner_fingerprint)
        || !valid_identity(&request.expected_invitation_fingerprint)
        || request.invitation.owner_fingerprint != request.expected_owner_fingerprint
        || precept_sync::invitation_fingerprint(&request.invitation).as_deref()
            != Ok(&request.expected_invitation_fingerprint)
    {
        return Err(Error::InvalidRequest);
    }
    let mut state = state::load(state_dir, profile_id)?.ok_or(Error::NotFound)?;
    state.require_idle()?;
    state.trust(
        PersonalSyncDeviceStatus {
            invitation: request.invitation,
            trusted_at_ms: now_ms(),
            revoked: false,
        },
        &request.expected_owner_fingerprint,
        now_ms(),
    )?;
    state::save(state_dir, &state)?;
    Ok(state.status())
}

pub(crate) fn revoke(
    state_dir: &StateDir,
    profile_id: &str,
    request: PersonalSyncRevokeRequest,
) -> Result<PersonalSyncStatus> {
    if !valid_identity(&request.device_id) {
        return Err(Error::InvalidRequest);
    }
    let mut state = state::load(state_dir, profile_id)?.ok_or(Error::NotFound)?;
    state.revoke(&request.device_id)?;
    state::save(state_dir, &state)?;
    Ok(state.status())
}

pub(crate) fn rotate(state_dir: &StateDir, profile_id: &str) -> Result<PersonalSyncInvitation> {
    let mut state = state::load(state_dir, profile_id)?.ok_or(Error::NotFound)?;
    state.require_idle()?;
    let identity =
        precept_sync::rotate_identity(&state.identity.owner_id, &state.identity.device_id)
            .map_err(|_| Error::Unavailable)?;
    let result = invitation(&identity).map_err(|_| Error::Unavailable)?;
    state.rotate(identity)?;
    state::save(state_dir, &state)?;
    Ok(result)
}

pub(crate) fn reset(
    state_dir: &StateDir,
    profile_id: &str,
    request: PersonalSyncResetRequest,
    current: &BTreeMap<String, PersonalSyncContent>,
) -> Result<PersonalSyncInvitation> {
    if !request.confirm {
        return Err(Error::InvalidRequest);
    }
    if state::load(state_dir, profile_id)?
        .as_ref()
        .is_some_and(|s| s.pending().is_some())
    {
        return Err(Error::Conflict);
    }
    let identity = precept_sync::generate_identity().map_err(|_| Error::Unavailable)?;
    let mut state = SyncState::new(profile_id, identity);
    state.reconcile(current)?;
    let result = invitation(&state.identity).map_err(|_| Error::Unavailable)?;
    state::save(state_dir, &state)?;
    Ok(result)
}

pub(crate) fn export(
    state_dir: &StateDir,
    profile_id: &str,
    request: PersonalSyncExportRequest,
    current: &BTreeMap<String, PersonalSyncContent>,
) -> Result<PersonalSyncEnvelope> {
    if request.recipient.is_some() == request.passphrase.is_some() {
        return Err(Error::InvalidRequest);
    }
    init(state_dir, profile_id, current)?;
    let mut state = state::load(state_dir, profile_id)?.ok_or(Error::NotFound)?;
    state.require_idle()?;
    state.reconcile(current)?;
    let keys = selected_keys(&state, request.keys)?;
    let mut entries = Vec::new();
    let mut tombstones = Vec::new();
    for key in keys {
        let version = state.version(&key, current).ok_or(Error::Conflict)?;
        match version.content {
            Some(content) => entries.push(PersonalSyncEntry {
                content,
                clock: version.clock,
                digest: version.digest,
            }),
            None => tombstones.push(PersonalSyncTombstone {
                key: version.key,
                clock: version.clock,
                digest: version.digest,
            }),
        }
    }
    let bundle = PersonalSyncBundle {
        format: precept_core::operator::PERSONAL_SYNC_FORMAT.into(),
        version: precept_core::operator::PERSONAL_SYNC_VERSION,
        bundle_id: random_hex()?,
        owner_id: state.identity.owner_id.clone(),
        created_at_ms: now_ms(),
        entries,
        tombstones,
    };
    if !bundle.valid() {
        return Err(Error::InvalidRequest);
    }
    let envelope = seal_bundle(
        &bundle,
        &state.identity.device_id,
        &state.identity.signing_secret,
        request.recipient.as_deref(),
        request.passphrase.as_deref(),
    )
    .map_err(|_| Error::InvalidRequest)?;
    state::save(state_dir, &state)?;
    Ok(envelope)
}

pub(crate) fn preview(
    state_dir: &StateDir,
    profile_id: &str,
    request: PersonalSyncOpenRequest,
    current: &BTreeMap<String, PersonalSyncContent>,
    current_commit: i64,
) -> Result<PersonalSyncPreview> {
    let mut state = state::load(state_dir, profile_id)?.ok_or(Error::NotFound)?;
    state.require_idle()?;
    let opened = open_envelope(&state, &request, false)?;
    let bundle = opened.bundle;
    state.reconcile(current)?;
    let mut keys: Vec<String> = bundle
        .entries
        .iter()
        .map(|entry| entry.content.key.clone())
        .chain(
            bundle
                .tombstones
                .iter()
                .map(|tombstone| tombstone.key.clone()),
        )
        .collect();
    keys.sort();
    let mut local_versions = Vec::new();
    let mut selections = Vec::new();
    let mut items = Vec::new();
    for key in keys {
        let incoming = bundle_version(&bundle, &key).ok_or(Error::InvalidRequest)?;
        let local = state
            .version(&key, current)
            .unwrap_or_else(|| PersonalSyncVersion::absent(&key));
        let merge = classify_merge(&incoming, &local);
        let applied = state.applied.iter().any(|entry| {
            entry.bundle_id == bundle.bundle_id
                && entry.key == key
                && entry.digest == incoming.digest
        });
        let resolution = match merge {
            PersonalSyncMerge::Conflict => PersonalSyncResolution::KeepLocal,
            _ => PersonalSyncResolution::Accept,
        };
        let selected = !applied && merge == PersonalSyncMerge::Accept;
        selections.push(PersonalSyncSelection {
            key: key.clone(),
            incoming_digest: incoming.digest,
            expected_local_digest: local.digest.clone(),
            resolution,
        });
        items.push(PersonalSyncPreviewItem {
            key,
            merge,
            selected,
            applied,
        });
        local_versions.push(local.reference());
    }
    Ok(PersonalSyncPreview {
        local_contents: local_versions
            .iter()
            .filter_map(|local| current.get(&local.key).cloned())
            .collect(),
        bundle,
        local_versions,
        sender_device_id: opened.header.sender_device_id,
        device_id: state.identity.device_id,
        expected_commit: current_commit,
        selections,
        items,
    })
}

pub(crate) fn apply<F, G>(
    state_dir: &StateDir,
    binding: &Binding,
    submission: PersonalSyncApplySubmission,
    current: &BTreeMap<String, PersonalSyncContent>,
    current_commit: i64,
    execute: F,
    lookup: G,
) -> Result<PersonalSyncApplyResult>
where
    F: FnOnce(&precept_write::Request) -> Result<Receipt>,
    G: Fn(&precept_write::Request) -> Result<Option<(Vec<u8>, Receipt)>>,
{
    if !precept_store::profile::operation_valid(&submission.operation_id)
        || submission.expected_commit < 0
    {
        return Err(Error::InvalidRequest);
    }
    let request_bytes = serde_json::to_vec(&submission).map_err(|_| Error::InvalidRequest)?;
    if request_bytes.len() > precept_ipc::MAX_FRAME {
        return Err(Error::TooLarge);
    }
    let request_digest = hex(&Sha256::digest(&request_bytes));
    let mut state = state::load(state_dir, &binding.profile_id)?.ok_or(Error::NotFound)?;
    if let Some(operation) = state.operation(&submission.operation_id) {
        if operation.request_digest != request_digest {
            return Err(Error::IdempotencyConflict);
        }
        if operation.completed {
            return Ok(PersonalSyncApplyResult {
                operation_id: operation.operation_id.clone(),
                commit_sequence: operation.commit_sequence,
                applied: operation.applied,
                changed: operation.changed,
            });
        }
    }
    let recovering = state.pending().is_some();
    let authenticated = open_submission(&state, &submission, recovering)?;
    validate_bundle(
        &state,
        &authenticated,
        &submission.sender_device_id,
        &submission.device_id,
        submission.passphrase.is_some(),
    )?;
    if !recovering {
        if submission.expected_commit != current_commit {
            return Err(Error::Conflict);
        }
        let pending = prepare_pending(
            &mut state,
            &submission,
            authenticated,
            binding,
            &request_digest,
            current,
            current_commit,
        )?;
        state.begin(pending)?;
        state::save(state_dir, &state)?;
    }
    let mut pending = state.pending().cloned().ok_or(Error::Conflict)?;
    if pending.request_digest != request_digest {
        return Err(Error::IdempotencyConflict);
    }
    let commit_sequence = if pending.command.is_some() {
        let original = pending_request(&pending, &submission.operation_id)?;
        let original_bytes = serde_json::to_vec(&original).map_err(|_| Error::InvalidRequest)?;
        match lookup(&original) {
            Ok(Some((digest, receipt))) => {
                if digest != Sha256::digest(&original_bytes).to_vec() {
                    state.abort(&submission.operation_id)?;
                    state::save(state_dir, &state).map_err(|_| Error::OutcomeUnknown)?;
                    return Err(Error::IdempotencyConflict);
                }
                Some(receipt.commit_sequence)
            }
            Ok(None) => {
                if pending.expected_commit != current_commit {
                    state.abort(&submission.operation_id)?;
                    state::save(state_dir, &state).map_err(|_| Error::OutcomeUnknown)?;
                    return Err(Error::Conflict);
                }
                if pending.binding != *binding {
                    pending.binding = binding.clone();
                    state.pending = Some(pending.clone());
                    state::save(state_dir, &state)?;
                }
                let request = pending_request(&pending, &submission.operation_id)?;
                let receipt = match execute(&request) {
                    Ok(receipt) => receipt,
                    Err(Error::OutcomeUnknown | Error::OwnerMismatch | Error::StaleGeneration) => {
                        return Err(Error::OutcomeUnknown);
                    }
                    Err(error) => {
                        state.abort(&submission.operation_id)?;
                        state::save(state_dir, &state).map_err(|_| Error::OutcomeUnknown)?;
                        return Err(error);
                    }
                };
                Some(receipt.commit_sequence)
            }
            Err(_) => return Err(Error::OutcomeUnknown),
        }
    } else {
        if pending.expected_commit != current_commit {
            state.abort(&submission.operation_id)?;
            state::save(state_dir, &state).map_err(|_| Error::OutcomeUnknown)?;
            return Err(Error::Conflict);
        }
        None
    };
    state
        .finish(&submission.operation_id, commit_sequence)
        .map_err(|_| Error::OutcomeUnknown)?;
    state::save(state_dir, &state).map_err(|_| Error::OutcomeUnknown)?;
    Ok(PersonalSyncApplyResult {
        operation_id: submission.operation_id,
        commit_sequence,
        applied: pending.applied.len(),
        changed: pending
            .command
            .as_ref()
            .and_then(|command| match command {
                Command::ApplyPersonalSync { changes, .. } => Some(changes.len()),
                _ => None,
            })
            .unwrap_or(0),
    })
}

pub(crate) fn abort<G>(
    state_dir: &StateDir,
    binding: &Binding,
    request: PersonalSyncAbortRequest,
    lookup: G,
) -> Result<PersonalSyncAbortResult>
where
    G: Fn(&precept_write::Request) -> Result<Option<(Vec<u8>, Receipt)>>,
{
    if !precept_store::profile::operation_valid(&request.operation_id) {
        return Err(Error::InvalidRequest);
    }
    let mut state = state::load(state_dir, &binding.profile_id)?.ok_or(Error::NotFound)?;
    let pending = state.pending().cloned().ok_or(Error::NotFound)?;
    if pending.operation_id != request.operation_id {
        return Err(Error::NotFound);
    }
    if let Some(command) = pending.command {
        let original = pending_request(
            &PendingApply {
                command: Some(command),
                ..pending
            },
            &request.operation_id,
        )?;
        let original_bytes = serde_json::to_vec(&original).map_err(|_| Error::InvalidRequest)?;
        match lookup(&original) {
            Ok(Some((digest, receipt))) => {
                if digest == Sha256::digest(&original_bytes).to_vec() {
                    state
                        .finish(&request.operation_id, Some(receipt.commit_sequence))
                        .map_err(|_| Error::OutcomeUnknown)?;
                    state::save(state_dir, &state).map_err(|_| Error::OutcomeUnknown)?;
                    return Ok(PersonalSyncAbortResult {
                        operation_id: request.operation_id,
                        settled_commit: Some(receipt.commit_sequence),
                        aborted: false,
                    });
                }
            }
            Ok(None) => {}
            Err(_) => return Err(Error::OutcomeUnknown),
        }
    }
    state.abort(&request.operation_id)?;
    state::save(state_dir, &state).map_err(|_| Error::OutcomeUnknown)?;
    Ok(PersonalSyncAbortResult {
        operation_id: request.operation_id,
        settled_commit: None,
        aborted: true,
    })
}

fn pending_request(pending: &PendingApply, operation_id: &str) -> Result<precept_write::Request> {
    let command = pending.command.clone().ok_or(Error::Conflict)?;
    let raw = serde_json::to_vec(&precept_write::Request {
        version: 2,
        binding: pending.binding.clone(),
        operation_id: operation_id.to_owned(),
        command,
    })
    .map_err(|_| Error::InvalidRequest)?;
    precept_write::Request::from_sync_json(&raw).map_err(|_| Error::InvalidRequest)
}

fn open_envelope(
    state: &SyncState,
    request: &PersonalSyncOpenRequest,
    allow_revoked: bool,
) -> Result<OpenedBundle> {
    let identity_secret = request
        .passphrase
        .is_none()
        .then_some(state.identity.age_identity.as_str());
    let opened = open_signed_bundle(
        &request.envelope,
        identity_secret,
        request.passphrase.as_deref(),
    )
    .map_err(|_| Error::InvalidRequest)?;
    let sender_id = &opened.header.sender_device_id;
    let known = state.known_sender(sender_id);
    if known.is_some() && !allow_revoked && state.trusted_sender(sender_id).is_none() {
        return Err(Error::Unauthorized);
    }
    if let Some(sender) = known
        && sender.invitation.signing_public != opened.header.signing_public
    {
        return Err(Error::Unauthorized);
    }
    if request.passphrase.is_none()
        && (known.is_none() || opened.bundle.owner_id != state.identity.owner_id)
    {
        return Err(Error::Unauthorized);
    }
    // Password imports authorize only the exact previewed contents. They never add trust.
    Ok(opened)
}

fn open_submission(
    state: &SyncState,
    submission: &PersonalSyncApplySubmission,
    allow_revoked: bool,
) -> Result<PersonalSyncBundle> {
    let opened = open_envelope(
        state,
        &PersonalSyncOpenRequest {
            envelope: submission.envelope.clone(),
            passphrase: submission.passphrase.clone(),
        },
        allow_revoked,
    )?;
    if submission.sender_device_id != opened.header.sender_device_id {
        return Err(Error::Unauthorized);
    }
    Ok(opened.bundle)
}

fn validate_bundle(
    state: &SyncState,
    bundle: &PersonalSyncBundle,
    sender_device_id: &str,
    device_id: &str,
    password_import: bool,
) -> Result<()> {
    if (!password_import && bundle.owner_id != state.identity.owner_id)
        || device_id != state.identity.device_id
        || (!password_import && state.known_sender(sender_device_id).is_none())
        || bundle
            .entries
            .iter()
            .any(|entry| !entry.clock.contains_key(sender_device_id))
        || bundle
            .tombstones
            .iter()
            .any(|tombstone| !tombstone.clock.contains_key(sender_device_id))
    {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

fn prepare_pending(
    state: &mut SyncState,
    submission: &PersonalSyncApplySubmission,
    bundle: PersonalSyncBundle,
    binding: &precept_store::profile::Binding,
    request_digest: &str,
    current: &BTreeMap<String, PersonalSyncContent>,
    current_commit: i64,
) -> Result<PendingApply> {
    validate_bundle(
        state,
        &bundle,
        &submission.sender_device_id,
        &submission.device_id,
        submission.passphrase.is_some(),
    )?;
    if !submission
        .local_versions
        .iter()
        .all(precept_core::operator::PersonalSyncLocalVersion::valid)
    {
        return Err(Error::InvalidRequest);
    }
    state.reconcile(current)?;
    let expected: BTreeMap<String, PersonalSyncVersion> = bundle
        .entries
        .iter()
        .map(|entry| {
            (
                entry.content.key.clone(),
                PersonalSyncVersion::absent(&entry.content.key),
            )
        })
        .chain(bundle.tombstones.iter().map(|tombstone| {
            (
                tombstone.key.clone(),
                PersonalSyncVersion::absent(&tombstone.key),
            )
        }))
        .map(|(key, absent)| {
            let version = state.version(&key, current).unwrap_or(absent);
            (key, version)
        })
        .collect();
    let supplied: BTreeMap<String, precept_core::operator::PersonalSyncLocalVersion> = submission
        .local_versions
        .iter()
        .cloned()
        .map(|version| (version.key.clone(), version))
        .collect();
    let expected_refs: BTreeMap<String, precept_core::operator::PersonalSyncLocalVersion> =
        expected
            .iter()
            .map(|(key, version)| (key.clone(), version.reference()))
            .collect();
    if supplied.len() != submission.local_versions.len() || supplied != expected_refs {
        return Err(Error::Conflict);
    }
    let request = PersonalSyncApplyRequest {
        bundle,
        local_versions: expected.into_values().collect(),
        sender_device_id: submission.sender_device_id.clone(),
        device_id: submission.device_id.clone(),
        selections: submission.selections.clone(),
    };
    let plan = plan_personal_sync_apply(&request).ok_or(Error::InvalidRequest)?;
    let (versions, applied) = state.plan_versions(&plan)?;
    let merged_keys = state
        .versions
        .iter()
        .map(|version| version.key.as_str())
        .chain(versions.iter().map(|version| version.key.as_str()))
        .collect::<BTreeSet<_>>();
    if merged_keys.len() > MAX_TRACKED_KEYS {
        return Err(Error::Capacity);
    }
    let command = (!plan.changes.is_empty()).then(|| Command::ApplyPersonalSync {
        bundle_id: plan.bundle_id.clone(),
        bundle_digest: plan.bundle_digest.clone(),
        device_id: request.device_id.clone(),
        expected_commit: current_commit,
        channel: submission.channel,
        changes: plan.changes.clone(),
    });
    let pending = PendingApply {
        operation_id: submission.operation_id.clone(),
        request_digest: request_digest.to_owned(),
        binding: binding.clone(),
        expected_commit: current_commit,
        versions,
        applied,
        command,
    };
    if pending.command.is_some() {
        pending_request(&pending, &submission.operation_id)?;
    }
    Ok(pending)
}

fn bundle_version(bundle: &PersonalSyncBundle, key: &str) -> Option<PersonalSyncVersion> {
    if let Some(entry) = bundle.entries.iter().find(|entry| entry.content.key == key) {
        Some(PersonalSyncVersion {
            key: key.to_owned(),
            clock: entry.clock.clone(),
            digest: entry.digest.clone(),
            content: Some(entry.content.clone()),
        })
    } else {
        bundle
            .tombstones
            .iter()
            .find(|tombstone| tombstone.key == key)
            .map(|tombstone| PersonalSyncVersion {
                key: key.to_owned(),
                clock: tombstone.clock.clone(),
                digest: tombstone.digest.clone(),
                content: None,
            })
    }
}

fn selected_keys(state: &SyncState, requested: Option<Vec<String>>) -> Result<Vec<String>> {
    let Some(keys) = requested else {
        if state.versions.len() > precept_core::operator::MAX_SYNC_ITEMS {
            return Err(Error::InvalidRequest);
        }
        return Ok(state
            .versions
            .iter()
            .map(|version| version.key.clone())
            .collect());
    };
    let mut seen = BTreeSet::new();
    if keys.is_empty()
        || keys.len() > precept_core::operator::MAX_SYNC_ITEMS
        || keys.iter().any(|key| {
            !PersonalSyncVersion::absent(key).valid()
                || !seen.insert(key.as_str())
                || !state.versions.iter().any(|version| version.key == *key)
        })
    {
        return Err(Error::InvalidRequest);
    }
    Ok(keys)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(1)
}

fn random_hex() -> Result<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::Unavailable)?;
    Ok(hex(&bytes))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(DIGITS[usize::from(byte >> 4)] as char);
        out.push(DIGITS[usize::from(byte & 15)] as char);
    }
    out
}

#[cfg(test)]
mod tests;
