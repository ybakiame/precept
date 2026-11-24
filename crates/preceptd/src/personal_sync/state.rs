use super::{
    MAX_APPLIED_ENTRIES, MAX_OPERATIONS, MAX_STATE_BYTES, MAX_TRACKED_KEYS, MAX_TRUSTED_DEVICES,
};
use crate::state::StateDir;
use precept_core::operator::{
    Command, PersonalSyncAppliedEntry, PersonalSyncApplyPlan, PersonalSyncContent,
    PersonalSyncDeviceStatus, PersonalSyncStatus, PersonalSyncVersion, VectorClock, clock_tick,
    tombstone_digest, valid_identity, valid_sync_digest,
};
use precept_ipc::{Error, Result};
use precept_store::profile::{Binding, operation_valid};
use precept_sync::{GeneratedIdentity, invitation, verify_invitation};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub(super) const STATE_FORMAT: &str = "precept-personal-sync-state";
pub(super) const STATE_VERSION: u32 = 1;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredVersion {
    pub key: String,
    pub clock: VectorClock,
    pub digest: String,
}

impl StoredVersion {
    fn valid(&self) -> bool {
        PersonalSyncVersion::absent(&self.key).valid()
            && precept_core::operator::valid_clock(&self.clock)
            && valid_sync_digest(&self.digest)
    }

    fn from_version(version: &PersonalSyncVersion) -> Self {
        Self {
            key: version.key.clone(),
            clock: version.clock.clone(),
            digest: version.digest.clone(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OperationRecord {
    pub operation_id: String,
    pub request_digest: String,
    pub commit_sequence: Option<i64>,
    pub completed: bool,
    pub applied: usize,
    pub changed: usize,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PendingApply {
    pub operation_id: String,
    pub request_digest: String,
    pub binding: Binding,
    pub expected_commit: i64,
    pub versions: Vec<StoredVersion>,
    pub applied: Vec<PersonalSyncAppliedEntry>,
    pub command: Option<Command>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SyncState {
    pub(super) format: String,
    pub(super) version: u32,
    pub(super) profile_id: String,
    pub(super) identity: GeneratedIdentity,
    pub(super) trusted_devices: Vec<PersonalSyncDeviceStatus>,
    pub(super) versions: Vec<StoredVersion>,
    pub(super) applied: Vec<PersonalSyncAppliedEntry>,
    pub(super) operations: Vec<OperationRecord>,
    pub(super) pending: Option<PendingApply>,
}

impl SyncState {
    pub(super) fn new(profile_id: &str, identity: GeneratedIdentity) -> Self {
        Self {
            format: STATE_FORMAT.into(),
            version: STATE_VERSION,
            profile_id: profile_id.into(),
            identity,
            trusted_devices: Vec::new(),
            versions: Vec::new(),
            applied: Vec::new(),
            operations: Vec::new(),
            pending: None,
        }
    }

    pub(super) fn valid(&self) -> bool {
        self.format == STATE_FORMAT
            && self.version == STATE_VERSION
            && valid_identity(&self.profile_id)
            && invitation(&self.identity).is_ok_and(|value| verify_invitation(&value))
            && unique_versions(&self.versions)
            && self.versions.len() <= MAX_TRACKED_KEYS
            && self.versions.iter().all(StoredVersion::valid)
            && valid_applied(&self.applied)
            && valid_operations(&self.operations)
            && self.trusted_devices.len() <= MAX_TRUSTED_DEVICES
            && trusted_valid(&self.trusted_devices, &self.identity)
            && self.pending.as_ref().is_none_or(|pending| {
                operation_valid(&pending.operation_id)
                    && valid_sync_digest(&pending.request_digest)
                    && pending.binding.profile_id == self.profile_id
                    && valid_identity(&pending.binding.profile_id)
                    && valid_identity(&pending.binding.generation)
                    && pending.binding.owner_epoch >= 0
                    && pending.expected_commit >= 0
                    && unique_versions(&pending.versions)
                    && pending.versions.len() <= MAX_TRACKED_KEYS
                    && pending.versions.iter().all(StoredVersion::valid)
                    && valid_applied(&pending.applied)
                    && pending.command.as_ref().is_none_or(|command| {
                        valid_pending_command(command)
                            && match command {
                                Command::ApplyPersonalSync {
                                    bundle_id,
                                    device_id,
                                    expected_commit,
                                    changes,
                                    ..
                                } => {
                                    *expected_commit == pending.expected_commit
                                        && device_id == &self.identity.device_id
                                        && pending
                                            .applied
                                            .iter()
                                            .any(|entry| entry.bundle_id == *bundle_id)
                                        && changes.iter().all(|change| {
                                            pending.versions.iter().any(|version| {
                                                version.key == change.key
                                                    && change.digest()
                                                        == Some(version.digest.as_str())
                                            })
                                        })
                                }
                                _ => false,
                            }
                    })
                    && self.operations.iter().any(|operation| {
                        operation.operation_id == pending.operation_id
                            && operation.request_digest == pending.request_digest
                            && operation.commit_sequence.is_none()
                            && !operation.completed
                    })
            })
            && self
                .operations
                .iter()
                .filter(|operation| !operation.completed)
                .count()
                == usize::from(self.pending.is_some())
    }

    pub(super) fn status(&self) -> PersonalSyncStatus {
        PersonalSyncStatus {
            initialized: true,
            owner_fingerprint: Some(self.identity.owner_fingerprint.clone()),
            device_id: Some(self.identity.device_id.clone()),
            trusted_devices: self.trusted_devices.clone(),
            tracked_keys: self.versions.len(),
            tombstones: self
                .versions
                .iter()
                .filter(|version| version.digest == tombstone_digest(&version.key))
                .count(),
            applied_entries: self.applied.len(),
            pending_operation: self
                .pending
                .as_ref()
                .map(|pending| pending.operation_id.clone()),
        }
    }

    pub(super) fn trust(
        &mut self,
        device: PersonalSyncDeviceStatus,
        expected_owner_fingerprint: &str,
        now_ms: u64,
    ) -> Result<()> {
        if expected_owner_fingerprint != self.identity.owner_fingerprint
            || device.invitation.owner_id != self.identity.owner_id
            || device.invitation.owner_fingerprint != self.identity.owner_fingerprint
            || device.invitation.device_id == self.identity.device_id
            || !verify_invitation(&device.invitation)
            || now_ms == 0
        {
            return Err(Error::InvalidRequest);
        }
        if let Some(index) = self
            .trusted_devices
            .iter()
            .position(|known| known.invitation.device_id == device.invitation.device_id)
        {
            let existing = &self.trusted_devices[index];
            if !existing.revoked
                && (existing.invitation.signing_public != device.invitation.signing_public
                    || existing.invitation.age_recipient != device.invitation.age_recipient)
            {
                return Err(Error::Conflict);
            }
            self.trusted_devices[index] = PersonalSyncDeviceStatus {
                invitation: device.invitation,
                trusted_at_ms: now_ms,
                revoked: false,
            };
            return Ok(());
        }
        if self.trusted_devices.len() >= MAX_TRUSTED_DEVICES {
            return Err(Error::Capacity);
        }
        self.trusted_devices.push(PersonalSyncDeviceStatus {
            invitation: device.invitation,
            trusted_at_ms: now_ms,
            revoked: false,
        });
        self.trusted_devices
            .sort_by(|a, b| a.invitation.device_id.cmp(&b.invitation.device_id));
        Ok(())
    }

    pub(super) fn revoke(&mut self, device_id: &str) -> Result<()> {
        let device = self
            .trusted_devices
            .iter_mut()
            .find(|known| known.invitation.device_id == device_id)
            .ok_or(Error::NotFound)?;
        device.revoked = true;
        Ok(())
    }

    pub(super) fn trusted_sender(&self, device_id: &str) -> Option<&PersonalSyncDeviceStatus> {
        self.trusted_devices
            .iter()
            .find(|known| known.invitation.device_id == device_id && !known.revoked)
    }

    pub(super) fn known_sender(&self, device_id: &str) -> Option<&PersonalSyncDeviceStatus> {
        self.trusted_devices
            .iter()
            .find(|known| known.invitation.device_id == device_id)
    }

    pub(super) fn require_idle(&self) -> Result<()> {
        if self.pending.is_some() {
            Err(Error::Conflict)
        } else {
            Ok(())
        }
    }

    pub(super) fn rotate(&mut self, identity: GeneratedIdentity) -> Result<()> {
        if identity.owner_id != self.identity.owner_id
            || identity.device_id != self.identity.device_id
            || identity.owner_fingerprint != self.identity.owner_fingerprint
            || self.pending.is_some()
        {
            return Err(Error::Conflict);
        }
        self.identity = identity;
        Ok(())
    }

    pub(super) fn reconcile(
        &mut self,
        current: &BTreeMap<String, PersonalSyncContent>,
    ) -> Result<()> {
        let device = self.identity.device_id.clone();
        let mut versions: BTreeMap<String, StoredVersion> = self
            .versions
            .iter()
            .cloned()
            .map(|version| (version.key.clone(), version))
            .collect();
        for (key, content) in current {
            let digest = content.digest();
            match versions.get_mut(key) {
                Some(stored) => {
                    if stored.digest == tombstone_digest(key) || stored.digest != digest {
                        stored.clock = clock_tick(&stored.clock, &device).ok_or(Error::Capacity)?;
                        stored.digest = digest;
                    }
                }
                None => {
                    versions.insert(
                        key.clone(),
                        StoredVersion {
                            key: key.clone(),
                            clock: clock_tick(&VectorClock::new(), &device)
                                .ok_or(Error::Capacity)?,
                            digest,
                        },
                    );
                }
            }
        }
        for stored in versions.values_mut() {
            if !current.contains_key(&stored.key) && stored.digest != tombstone_digest(&stored.key)
            {
                stored.clock = clock_tick(&stored.clock, &device).ok_or(Error::Capacity)?;
                stored.digest = tombstone_digest(&stored.key);
            }
        }
        if versions.len() > MAX_TRACKED_KEYS {
            return Err(Error::Capacity);
        }
        self.versions = versions.into_values().collect();
        Ok(())
    }

    pub(super) fn version(
        &self,
        key: &str,
        current: &BTreeMap<String, PersonalSyncContent>,
    ) -> Option<PersonalSyncVersion> {
        let stored = self.versions.iter().find(|version| version.key == key)?;
        if stored.digest == tombstone_digest(key) {
            Some(PersonalSyncVersion {
                key: key.to_owned(),
                clock: stored.clock.clone(),
                digest: stored.digest.clone(),
                content: None,
            })
        } else {
            current.get(key).and_then(|content| {
                (content.digest() == stored.digest).then(|| PersonalSyncVersion {
                    key: key.to_owned(),
                    clock: stored.clock.clone(),
                    digest: stored.digest.clone(),
                    content: Some(content.clone()),
                })
            })
        }
    }

    pub(super) fn begin(&mut self, pending: PendingApply) -> Result<()> {
        if self.pending.is_some() {
            return Err(Error::Conflict);
        }
        if let Some(operation) = self
            .operations
            .iter()
            .find(|operation| operation.operation_id == pending.operation_id)
        {
            return Err(if operation.request_digest == pending.request_digest {
                Error::Conflict
            } else {
                Error::IdempotencyConflict
            });
        }
        self.operations.push(OperationRecord {
            operation_id: pending.operation_id.clone(),
            request_digest: pending.request_digest.clone(),
            commit_sequence: None,
            completed: false,
            applied: 0,
            changed: 0,
        });
        if self.operations.len() > MAX_OPERATIONS {
            self.operations.remove(0);
        }
        self.pending = Some(pending);
        Ok(())
    }

    pub(super) fn operation(&self, operation_id: &str) -> Option<&OperationRecord> {
        self.operations
            .iter()
            .find(|operation| operation.operation_id == operation_id)
    }

    pub(super) fn pending(&self) -> Option<&PendingApply> {
        self.pending.as_ref()
    }

    pub(super) fn finish(
        &mut self,
        operation_id: &str,
        commit_sequence: Option<i64>,
    ) -> Result<()> {
        let pending = self.pending.clone().ok_or(Error::Conflict)?;
        if pending.operation_id != operation_id {
            return Err(Error::Conflict);
        }
        let changed = pending
            .command
            .as_ref()
            .and_then(|command| match command {
                Command::ApplyPersonalSync { changes, .. } => Some(changes.len()),
                _ => None,
            })
            .unwrap_or(0);
        let applied_count = pending.applied.len();
        let mut versions: BTreeMap<String, StoredVersion> = self
            .versions
            .iter()
            .cloned()
            .map(|version| (version.key.clone(), version))
            .collect();
        for version in pending.versions {
            versions.insert(version.key.clone(), version);
        }
        if versions.len() > MAX_TRACKED_KEYS {
            return Err(Error::Capacity);
        }
        self.pending = None;
        self.versions = versions.into_values().collect();
        for applied in pending.applied {
            if !self.applied.contains(&applied) {
                self.applied.push(applied);
            }
        }
        if self.applied.len() > MAX_APPLIED_ENTRIES {
            self.applied
                .drain(..self.applied.len() - MAX_APPLIED_ENTRIES);
        }
        let operation = self
            .operations
            .iter_mut()
            .find(|operation| operation.operation_id == operation_id)
            .ok_or(Error::Conflict)?;
        operation.commit_sequence = commit_sequence;
        operation.completed = true;
        operation.applied = applied_count;
        operation.changed = changed;
        Ok(())
    }

    pub(super) fn abort(&mut self, operation_id: &str) -> Result<()> {
        if self.pending.as_ref().map(|p| p.operation_id.as_str()) != Some(operation_id) {
            return Err(Error::Conflict);
        }
        self.pending = None;
        self.operations
            .retain(|operation| operation.operation_id != operation_id);
        Ok(())
    }

    pub(super) fn plan_versions(
        &self,
        plan: &PersonalSyncApplyPlan,
    ) -> Result<(Vec<StoredVersion>, Vec<PersonalSyncAppliedEntry>)> {
        let versions: Vec<StoredVersion> = plan
            .versions
            .iter()
            .map(StoredVersion::from_version)
            .collect();
        let applied = plan.applied.clone();
        if !unique_versions(&versions) || !valid_applied(&applied) {
            return Err(Error::InvalidRequest);
        }
        Ok((versions, applied))
    }
}

fn unique_versions(versions: &[StoredVersion]) -> bool {
    let mut keys = BTreeSet::new();
    versions
        .iter()
        .all(|version| keys.insert(version.key.as_str()))
}

fn valid_applied(applied: &[PersonalSyncAppliedEntry]) -> bool {
    let mut seen = BTreeSet::new();
    applied.len() <= MAX_APPLIED_ENTRIES
        && applied.iter().all(|entry| {
            valid_identity(&entry.bundle_id)
                && PersonalSyncVersion::absent(&entry.key).valid()
                && valid_sync_digest(&entry.digest)
                && seen.insert((
                    entry.bundle_id.as_str(),
                    entry.key.as_str(),
                    entry.digest.as_str(),
                ))
        })
}

fn valid_pending_command(command: &Command) -> bool {
    match command {
        Command::ApplyPersonalSync {
            bundle_id,
            bundle_digest,
            device_id,
            expected_commit,
            changes,
            ..
        } => {
            valid_identity(bundle_id)
                && valid_sync_digest(bundle_digest)
                && valid_identity(device_id)
                && *expected_commit >= 0
                && !changes.is_empty()
                && changes.len() <= precept_core::operator::MAX_SYNC_ITEMS
                && changes.iter().all(|change| change.valid())
        }
        _ => false,
    }
}

fn valid_operations(operations: &[OperationRecord]) -> bool {
    let mut seen = BTreeSet::new();
    operations.len() <= MAX_OPERATIONS
        && operations.iter().all(|operation| {
            operation_valid(&operation.operation_id)
                && valid_sync_digest(&operation.request_digest)
                && operation
                    .commit_sequence
                    .is_none_or(|sequence| sequence > 0)
                && (operation.completed || operation.commit_sequence.is_none())
                && operation.applied <= precept_core::operator::MAX_SYNC_ITEMS
                && operation.changed <= precept_core::operator::MAX_SYNC_ITEMS
                && seen.insert(operation.operation_id.as_str())
        })
}

fn trusted_valid(devices: &[PersonalSyncDeviceStatus], identity: &GeneratedIdentity) -> bool {
    let mut seen = BTreeSet::new();
    devices.iter().all(|device| {
        device.trusted_at_ms > 0
            && device.invitation.device_id != identity.device_id
            && device.invitation.owner_id == identity.owner_id
            && device.invitation.owner_fingerprint == identity.owner_fingerprint
            && verify_invitation(&device.invitation)
            && seen.insert(device.invitation.device_id.as_str())
    })
}

pub(super) fn file_name(profile_id: &str) -> Result<String> {
    valid_identity(profile_id)
        .then(|| format!("personal-sync-{profile_id}.json"))
        .ok_or(Error::InvalidRequest)
}

pub(super) fn load(state_dir: &StateDir, profile_id: &str) -> Result<Option<SyncState>> {
    let Some(text) = state_dir.read_bounded(&file_name(profile_id)?, MAX_STATE_BYTES)? else {
        return Ok(None);
    };
    let state: SyncState = serde_json::from_str(&text).map_err(|_| Error::InvalidRequest)?;
    if state.profile_id != profile_id || !state.valid() {
        return Err(Error::InvalidRequest);
    }
    Ok(Some(state))
}

pub(super) fn save(state_dir: &StateDir, state: &SyncState) -> Result<()> {
    if !state.valid() {
        return Err(Error::InvalidRequest);
    }
    let bytes = serde_json::to_vec(state).map_err(|_| Error::Unavailable)?;
    state_dir.write_bounded(&file_name(&state.profile_id)?, &bytes, MAX_STATE_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use precept_core::operator::{PersonalSyncContent, RuleOptions};

    fn content(value: &str) -> PersonalSyncContent {
        PersonalSyncContent {
            key: "tone".into(),
            value: value.into(),
            tags: vec!["style".into()],
            rule: RuleOptions::default(),
            enabled: true,
        }
    }

    #[test]
    fn reconciliation_tracks_local_edits_deletions_and_new_keys() {
        let identity = precept_sync::generate_identity().unwrap();
        let mut state = SyncState::new(&identity.owner_id.clone(), identity);
        let mut current = BTreeMap::from([("tone".to_owned(), content("one"))]);
        state.reconcile(&current).unwrap();
        assert_eq!(state.versions[0].clock[&state.identity.device_id], 1);
        current.insert("tone".into(), content("two"));
        state.reconcile(&current).unwrap();
        assert_eq!(state.versions[0].clock[&state.identity.device_id], 2);
        current.clear();
        state.reconcile(&current).unwrap();
        assert_eq!(state.versions[0].digest, tombstone_digest("tone"));
        assert_eq!(state.versions[0].clock[&state.identity.device_id], 3);
        current.insert("tone".into(), content("three"));
        state.reconcile(&current).unwrap();
        assert_eq!(state.versions[0].clock[&state.identity.device_id], 4);
        assert!(state.valid());
    }

    #[test]
    fn sidecar_only_operations_complete_without_a_profile_receipt() {
        let identity = precept_sync::generate_identity().unwrap();
        let mut state = SyncState::new(&identity.owner_id.clone(), identity);
        state
            .begin(PendingApply {
                operation_id: "sidecar-only".into(),
                request_digest: "0".repeat(64),
                binding: Binding {
                    profile_id: state.profile_id.clone(),
                    generation: "a".repeat(32),
                    owner_epoch: 0,
                },
                expected_commit: 0,
                versions: Vec::new(),
                applied: Vec::new(),
                command: None,
            })
            .unwrap();
        assert!(state.valid());
        state.finish("sidecar-only", None).unwrap();
        assert!(state.valid());
        assert!(state.pending().is_none());
        assert!(state.operation("sidecar-only").unwrap().completed);
    }

    #[test]
    fn private_sidecars_round_trip_and_refuse_malformed_or_oversized_files() {
        let temp = tempfile::tempdir().unwrap();
        let dir = StateDir::open(&temp.path().join("state")).unwrap();
        let identity = precept_sync::generate_identity().unwrap();
        let state = SyncState::new(&identity.owner_id.clone(), identity);
        save(&dir, &state).unwrap();
        let loaded = load(&dir, &state.profile_id).unwrap().unwrap();
        assert_eq!(loaded.status(), state.status());
        precept_ipc::transport::private_file(
            &dir.path().join(file_name(&state.profile_id).unwrap()),
        )
        .unwrap();
        dir.write_bounded(
            &file_name(&state.profile_id).unwrap(),
            b"not json",
            MAX_STATE_BYTES,
        )
        .unwrap();
        assert_eq!(
            load(&dir, &state.profile_id).err(),
            Some(Error::InvalidRequest)
        );
        assert_eq!(
            dir.write_bounded(&file_name(&state.profile_id).unwrap(), b"too large", 4,)
                .err(),
            Some(Error::TooLarge)
        );
    }

    #[test]
    fn trust_requires_the_owner_fingerprint_and_revocation_is_explicit() {
        let identity = precept_sync::generate_identity().unwrap();
        let peer = precept_sync::rotate_identity(&identity.owner_id, &"d".repeat(32)).unwrap();
        let mut state = SyncState::new(&identity.owner_id.clone(), identity);
        let device = PersonalSyncDeviceStatus {
            invitation: precept_sync::invitation(&peer).unwrap(),
            trusted_at_ms: 1,
            revoked: false,
        };
        assert!(state.trust(device.clone(), &"0".repeat(32), 1).is_err());
        let fingerprint = state.identity.owner_fingerprint.clone();
        state.trust(device, &fingerprint, 1).unwrap();
        assert!(state.trusted_sender(&peer.device_id).is_some());

        let rotated = precept_sync::rotate_identity(&peer.owner_id, &peer.device_id).unwrap();
        let replacement = PersonalSyncDeviceStatus {
            invitation: precept_sync::invitation(&rotated).unwrap(),
            trusted_at_ms: 2,
            revoked: false,
        };
        assert!(state.trust(replacement.clone(), &fingerprint, 2).is_err());
        state.revoke(&peer.device_id).unwrap();
        state.trust(replacement.clone(), &fingerprint, 2).unwrap();
        assert_eq!(
            state
                .trusted_sender(&peer.device_id)
                .unwrap()
                .invitation
                .signing_public,
            replacement.invitation.signing_public
        );
    }
}
