use super::RuleOptions;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub const PERSONAL_SYNC_FORMAT: &str = "precept-personal-sync";
pub const PERSONAL_SYNC_VERSION: u32 = 1;
pub const MAX_SYNC_ITEMS: usize = 50;
pub const MAX_SYNC_CLOCK_DEVICES: usize = 16;
pub const MAX_SYNC_COUNTER: u64 = 1_000_000;
pub const MAX_SYNC_PAYLOAD_BYTES: usize = 512 * 1024;
pub const MAX_SYNC_ENVELOPE_BYTES: usize = 768 * 1024;

pub type VectorClock = BTreeMap<String, u64>;

fn hash(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
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

pub fn valid_identity(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn valid_sync_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn valid_clock(clock: &VectorClock) -> bool {
    !clock.is_empty()
        && clock.len() <= MAX_SYNC_CLOCK_DEVICES
        && clock.iter().all(|(device, counter)| {
            valid_identity(device) && (1..=MAX_SYNC_COUNTER).contains(counter)
        })
}

pub fn clock_cmp(a: &VectorClock, b: &VectorClock) -> Option<Ordering> {
    let mut less = false;
    let mut greater = false;
    for device in a.keys().chain(b.keys()) {
        match (a.get(device), b.get(device)) {
            (Some(left), Some(right)) if left < right => less = true,
            (Some(left), Some(right)) if left > right => greater = true,
            (Some(_), None) => greater = true,
            (None, Some(_)) => less = true,
            _ => (),
        }
    }
    match (less, greater) {
        (true, false) => Some(Ordering::Less),
        (false, true) => Some(Ordering::Greater),
        (false, false) => Some(Ordering::Equal),
        (true, true) => None,
    }
}

pub fn clock_join(a: &VectorClock, b: &VectorClock) -> Option<VectorClock> {
    let mut joined = a.clone();
    for (device, counter) in b {
        joined
            .entry(device.clone())
            .and_modify(|local| *local = (*local).max(*counter))
            .or_insert(*counter);
    }
    (joined.len() <= MAX_SYNC_CLOCK_DEVICES).then_some(joined)
}

pub fn clock_tick(clock: &VectorClock, device: &str) -> Option<VectorClock> {
    if !valid_identity(device) || clock.len() > MAX_SYNC_CLOCK_DEVICES {
        return None;
    }
    let mut next = clock.clone();
    let counter = next.get(device).copied().unwrap_or(0).checked_add(1)?;
    if counter > MAX_SYNC_COUNTER {
        return None;
    }
    if next.len() >= MAX_SYNC_CLOCK_DEVICES && !next.contains_key(device) {
        return None;
    }
    next.insert(device.to_owned(), counter);
    Some(next)
}

fn sync_text(value: &str, max: usize) -> bool {
    !value.contains('\0')
        && value.chars().count() <= max
        && value.chars().any(|c| !c.is_control() && !c.is_whitespace())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncContent {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub rule: RuleOptions,
    pub enabled: bool,
}

impl PersonalSyncContent {
    pub fn valid(&self) -> bool {
        sync_text(&self.key, 120)
            && sync_text(&self.value, 4096)
            && super::tags_canonical(&self.tags)
            && self.rule.valid()
    }

    pub fn digest(&self) -> String {
        hash(&serde_json::to_vec(self).expect("personal sync content serializes"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncEntry {
    #[serde(flatten)]
    pub content: PersonalSyncContent,
    pub clock: VectorClock,
    pub digest: String,
}

impl PersonalSyncEntry {
    pub fn valid(&self) -> bool {
        self.content.valid()
            && valid_clock(&self.clock)
            && valid_sync_digest(&self.digest)
            && self.digest == self.content.digest()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncTombstone {
    pub key: String,
    pub clock: VectorClock,
    pub digest: String,
}

pub fn tombstone_digest(key: &str) -> String {
    let value = (PERSONAL_SYNC_FORMAT, PERSONAL_SYNC_VERSION, "delete", key);
    hash(&serde_json::to_vec(&value).expect("tombstone serializes"))
}

impl PersonalSyncTombstone {
    pub fn valid(&self) -> bool {
        sync_text(&self.key, 120)
            && valid_clock(&self.clock)
            && valid_sync_digest(&self.digest)
            && self.digest == tombstone_digest(&self.key)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncBundle {
    pub format: String,
    pub version: u32,
    pub bundle_id: String,
    pub owner_id: String,
    pub created_at_ms: u64,
    pub entries: Vec<PersonalSyncEntry>,
    pub tombstones: Vec<PersonalSyncTombstone>,
}

impl PersonalSyncBundle {
    pub fn valid(&self) -> bool {
        self.format == PERSONAL_SYNC_FORMAT
            && self.version == PERSONAL_SYNC_VERSION
            && valid_identity(&self.bundle_id)
            && valid_identity(&self.owner_id)
            && self.created_at_ms > 0
            && self.entries.len().saturating_add(self.tombstones.len()) <= MAX_SYNC_ITEMS
            && self.entries.iter().all(PersonalSyncEntry::valid)
            && self.tombstones.iter().all(PersonalSyncTombstone::valid)
            && unique_keys(
                self.entries
                    .iter()
                    .map(|e| e.content.key.as_str())
                    .chain(self.tombstones.iter().map(|t| t.key.as_str())),
            )
    }

    pub fn digest(&self) -> String {
        hash(&serde_json::to_vec(self).expect("personal sync bundle serializes"))
    }
}

fn unique_keys<'a>(keys: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = std::collections::BTreeSet::new();
    keys.into_iter().all(|key| seen.insert(key))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncHeader {
    pub format: String,
    pub version: u32,
    pub bundle_id: String,
    pub sender_device_id: String,
    pub signing_public: String,
    pub owner_fingerprint: String,
}

impl PersonalSyncHeader {
    pub fn valid(&self) -> bool {
        self.format == PERSONAL_SYNC_FORMAT
            && self.version == PERSONAL_SYNC_VERSION
            && valid_identity(&self.bundle_id)
            && valid_identity(&self.sender_device_id)
            && self.signing_public.len() == 64
            && self
                .signing_public
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && self.owner_fingerprint.len() == 32
            && self
                .owner_fingerprint
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncEnvelope {
    pub format: String,
    pub version: u32,
    pub ciphertext: String,
}

impl PersonalSyncEnvelope {
    pub fn valid(&self) -> bool {
        self.format == PERSONAL_SYNC_FORMAT
            && self.version == PERSONAL_SYNC_VERSION
            && !self.ciphertext.is_empty()
            && self.ciphertext.len() <= MAX_SYNC_ENVELOPE_BYTES
            && (self
                .ciphertext
                .starts_with("-----BEGIN AGE ENCRYPTED FILE-----\n")
                || self
                    .ciphertext
                    .starts_with("-----BEGIN AGE ENCRYPTED FILE-----\r\n"))
            && (self
                .ciphertext
                .ends_with("-----END AGE ENCRYPTED FILE-----\n")
                || self
                    .ciphertext
                    .ends_with("-----END AGE ENCRYPTED FILE-----\r\n"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncInvitation {
    pub format: String,
    pub version: u32,
    pub owner_id: String,
    pub device_id: String,
    pub age_recipient: String,
    pub signing_public: String,
    pub owner_fingerprint: String,
    pub signature: String,
}

impl PersonalSyncInvitation {
    pub fn valid(&self) -> bool {
        self.format == PERSONAL_SYNC_FORMAT
            && self.version == PERSONAL_SYNC_VERSION
            && valid_identity(&self.owner_id)
            && valid_identity(&self.device_id)
            && !self.age_recipient.is_empty()
            && self.age_recipient.len() <= 256
            && self.signing_public.len() == 64
            && self
                .signing_public
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && self.owner_fingerprint.len() == 32
            && self
                .owner_fingerprint
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && self.signature.len() == 128
            && self
                .signature
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncChange {
    pub key: String,
    pub expected_digest: String,
    pub entry: Option<PersonalSyncEntry>,
    pub tombstone: Option<PersonalSyncTombstone>,
}

impl PersonalSyncChange {
    pub fn valid(&self) -> bool {
        valid_sync_digest(&self.expected_digest)
            && match (&self.entry, &self.tombstone) {
                (Some(entry), None) => entry.valid() && entry.content.key == self.key,
                (None, Some(tombstone)) => tombstone.valid() && tombstone.key == self.key,
                _ => false,
            }
    }

    pub fn digest(&self) -> Option<&str> {
        self.entry
            .as_ref()
            .map(|entry| entry.digest.as_str())
            .or_else(|| {
                self.tombstone
                    .as_ref()
                    .map(|tombstone| tombstone.digest.as_str())
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "resolution", rename_all = "snake_case", deny_unknown_fields)]
pub enum PersonalSyncResolution {
    Accept,
    KeepLocal,
    Manual(PersonalSyncContent),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncSelection {
    pub key: String,
    pub incoming_digest: String,
    pub expected_local_digest: String,
    pub resolution: PersonalSyncResolution,
}

impl PersonalSyncSelection {
    pub fn valid(&self) -> bool {
        sync_text(&self.key, 120)
            && valid_sync_digest(&self.incoming_digest)
            && valid_sync_digest(&self.expected_local_digest)
            && match &self.resolution {
                PersonalSyncResolution::Manual(content) => {
                    content.valid() && content.key == self.key
                }
                _ => true,
            }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncApplyRequest {
    pub bundle: PersonalSyncBundle,
    pub local_versions: Vec<PersonalSyncVersion>,
    pub sender_device_id: String,
    pub device_id: String,
    pub selections: Vec<PersonalSyncSelection>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncLocalVersion {
    pub key: String,
    pub clock: VectorClock,
    pub digest: String,
}

impl PersonalSyncLocalVersion {
    pub fn valid(&self) -> bool {
        sync_text(&self.key, 120)
            && (self.clock.is_empty() || valid_clock(&self.clock))
            && valid_sync_digest(&self.digest)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncProfileScope {
    pub profile_id: String,
    pub generation: String,
}

impl PersonalSyncProfileScope {
    pub fn valid(&self) -> bool {
        super::valid_identity(&self.profile_id) && super::valid_identity(&self.generation)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncApplySubmission {
    pub operation_id: String,
    pub expected_commit: i64,
    pub channel: super::Channel,
    pub envelope: PersonalSyncEnvelope,
    pub passphrase: Option<String>,
    pub local_versions: Vec<PersonalSyncLocalVersion>,
    pub sender_device_id: String,
    pub device_id: String,
    pub selections: Vec<PersonalSyncSelection>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncAppliedEntry {
    pub bundle_id: String,
    pub key: String,
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncApplyPlan {
    pub bundle_id: String,
    pub bundle_digest: String,
    pub changes: Vec<PersonalSyncChange>,
    pub versions: Vec<PersonalSyncVersion>,
    pub applied: Vec<PersonalSyncAppliedEntry>,
}

fn incoming_version(bundle: &PersonalSyncBundle, key: &str) -> Option<PersonalSyncVersion> {
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

fn resolution_version(
    key: &str,
    incoming: &PersonalSyncVersion,
    local: &PersonalSyncVersion,
    device_id: &str,
    resolution: &PersonalSyncResolution,
) -> Option<PersonalSyncVersion> {
    let clock = clock_tick(&clock_join(&incoming.clock, &local.clock)?, device_id)?;
    let content = match resolution {
        PersonalSyncResolution::Accept => incoming.content.clone(),
        PersonalSyncResolution::KeepLocal => local.content.clone(),
        PersonalSyncResolution::Manual(content) => Some(content.clone()),
    };
    Some(match content {
        Some(content) => PersonalSyncVersion {
            key: key.to_owned(),
            digest: content.digest(),
            clock,
            content: Some(content),
        },
        None => PersonalSyncVersion {
            key: key.to_owned(),
            digest: tombstone_digest(key),
            clock,
            content: None,
        },
    })
}

pub fn plan_personal_sync_apply(
    request: &PersonalSyncApplyRequest,
) -> Option<PersonalSyncApplyPlan> {
    if !request.bundle.valid()
        || !super::valid_identity(&request.sender_device_id)
        || !super::valid_identity(&request.device_id)
        || request.selections.is_empty()
        || request.selections.len() > MAX_SYNC_ITEMS
        || request.local_versions.len() > MAX_SYNC_ITEMS
        || !request.selections.iter().all(PersonalSyncSelection::valid)
        || !request
            .local_versions
            .iter()
            .all(PersonalSyncVersion::valid)
    {
        return None;
    }
    let mut local_keys = std::collections::BTreeSet::new();
    if !request
        .local_versions
        .iter()
        .all(|version| local_keys.insert(version.key.as_str()))
    {
        return None;
    }
    let mut selected_keys = std::collections::BTreeSet::new();
    if !request
        .selections
        .iter()
        .all(|selection| selected_keys.insert(selection.key.as_str()))
    {
        return None;
    }
    let mut changes = Vec::new();
    let mut versions = Vec::new();
    let mut applied = Vec::new();
    for selection in &request.selections {
        let incoming = incoming_version(&request.bundle, &selection.key)?;
        let local = request
            .local_versions
            .iter()
            .find(|version| version.key == selection.key)?;
        if selection.incoming_digest != incoming.digest
            || selection.expected_local_digest != local.digest
        {
            return None;
        }
        let incoming_digest = incoming.digest.clone();
        let resulting = match classify_merge(&incoming, local) {
            PersonalSyncMerge::Accept => resolution_version(
                &selection.key,
                &incoming,
                local,
                &request.device_id,
                &PersonalSyncResolution::Accept,
            )?,
            PersonalSyncMerge::Stale => local.clone(),
            PersonalSyncMerge::Noop if clock_cmp(&incoming.clock, &local.clock).is_none() => {
                resolution_version(
                    &selection.key,
                    &incoming,
                    local,
                    &request.device_id,
                    &PersonalSyncResolution::KeepLocal,
                )?
            }
            PersonalSyncMerge::Noop => local.clone(),
            PersonalSyncMerge::Conflict => resolution_version(
                &selection.key,
                &incoming,
                local,
                &request.device_id,
                &selection.resolution,
            )?,
        };
        if resulting.digest != local.digest {
            let change = match &resulting.content {
                Some(content) => PersonalSyncChange {
                    key: selection.key.clone(),
                    expected_digest: local.digest.clone(),
                    entry: Some(PersonalSyncEntry {
                        content: content.clone(),
                        clock: resulting.clock.clone(),
                        digest: resulting.digest.clone(),
                    }),
                    tombstone: None,
                },
                None => PersonalSyncChange {
                    key: selection.key.clone(),
                    expected_digest: local.digest.clone(),
                    entry: None,
                    tombstone: Some(PersonalSyncTombstone {
                        key: selection.key.clone(),
                        clock: resulting.clock.clone(),
                        digest: resulting.digest.clone(),
                    }),
                },
            };
            changes.push(change);
        }
        versions.push(resulting);
        applied.push(PersonalSyncAppliedEntry {
            bundle_id: request.bundle.bundle_id.clone(),
            key: selection.key.clone(),
            digest: incoming_digest,
        });
    }
    Some(PersonalSyncApplyPlan {
        bundle_id: request.bundle.bundle_id.clone(),
        bundle_digest: request.bundle.digest(),
        changes,
        versions,
        applied,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncVersion {
    pub key: String,
    pub clock: VectorClock,
    pub digest: String,
    pub content: Option<PersonalSyncContent>,
}

impl PersonalSyncVersion {
    pub fn absent(key: &str) -> Self {
        Self {
            key: key.to_owned(),
            clock: BTreeMap::new(),
            digest: tombstone_digest(key),
            content: None,
        }
    }

    pub fn reference(&self) -> PersonalSyncLocalVersion {
        PersonalSyncLocalVersion {
            key: self.key.clone(),
            clock: self.clock.clone(),
            digest: self.digest.clone(),
        }
    }

    pub fn valid(&self) -> bool {
        match &self.content {
            Some(content) => {
                content.key == self.key
                    && content.valid()
                    && valid_clock(&self.clock)
                    && self.digest == content.digest()
            }
            None => {
                sync_text(&self.key, 120)
                    && (self.clock.is_empty() || valid_clock(&self.clock))
                    && self.digest == tombstone_digest(&self.key)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonalSyncMerge {
    Accept,
    Stale,
    Noop,
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncDeviceStatus {
    pub invitation: PersonalSyncInvitation,
    pub trusted_at_ms: u64,
    pub revoked: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncStatus {
    pub initialized: bool,
    pub owner_fingerprint: Option<String>,
    pub device_id: Option<String>,
    pub trusted_devices: Vec<PersonalSyncDeviceStatus>,
    pub tracked_keys: usize,
    pub tombstones: usize,
    pub applied_entries: usize,
    pub pending_operation: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncPreviewItem {
    pub key: String,
    pub merge: PersonalSyncMerge,
    pub selected: bool,
    pub applied: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncPreview {
    pub bundle: PersonalSyncBundle,
    pub local_contents: Vec<PersonalSyncContent>,
    pub local_versions: Vec<PersonalSyncLocalVersion>,
    pub sender_device_id: String,
    pub device_id: String,
    pub expected_commit: i64,
    pub selections: Vec<PersonalSyncSelection>,
    pub items: Vec<PersonalSyncPreviewItem>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncExportRequest {
    pub recipient: Option<String>,
    pub passphrase: Option<String>,
    pub keys: Option<Vec<String>>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncOpenRequest {
    pub envelope: PersonalSyncEnvelope,
    pub passphrase: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncTrustRequest {
    pub invitation: PersonalSyncInvitation,
    pub expected_owner_fingerprint: String,
    pub expected_invitation_fingerprint: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncRevokeRequest {
    pub device_id: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncResetRequest {
    pub confirm: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncAbortRequest {
    pub operation_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncAbortResult {
    pub operation_id: String,
    pub settled_commit: Option<i64>,
    pub aborted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalSyncApplyResult {
    pub operation_id: String,
    pub commit_sequence: Option<i64>,
    pub applied: usize,
    pub changed: usize,
}

pub fn classify_merge(
    incoming: &PersonalSyncVersion,
    local: &PersonalSyncVersion,
) -> PersonalSyncMerge {
    match clock_cmp(&incoming.clock, &local.clock) {
        Some(Ordering::Greater) => PersonalSyncMerge::Accept,
        Some(Ordering::Less) => PersonalSyncMerge::Stale,
        Some(Ordering::Equal) if incoming.digest == local.digest => PersonalSyncMerge::Noop,
        None if incoming.digest == local.digest => PersonalSyncMerge::Noop,
        _ => PersonalSyncMerge::Conflict,
    }
}

#[cfg(test)]
mod tests;
