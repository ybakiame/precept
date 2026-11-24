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
    pub header: PersonalSyncHeader,
    pub ciphertext: String,
    pub signature: String,
}

impl PersonalSyncEnvelope {
    pub fn valid(&self) -> bool {
        self.format == PERSONAL_SYNC_FORMAT
            && self.version == PERSONAL_SYNC_VERSION
            && self.header.valid()
            && !self.ciphertext.is_empty()
            && self.ciphertext.len() <= MAX_SYNC_ENVELOPE_BYTES
            && self.signature.len() == 128
            && self
                .signature
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
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
            PersonalSyncMerge::Accept => incoming,
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

    pub fn valid(&self) -> bool {
        match &self.content {
            Some(content) => {
                content.key == self.key
                    && content.valid()
                    && valid_clock(&self.clock)
                    && self.digest == content.digest()
            }
            None => self.clock.is_empty() && self.digest == tombstone_digest(&self.key),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersonalSyncMerge {
    Accept,
    Stale,
    Noop,
    Conflict,
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
mod tests {
    use super::*;

    fn version(device: &str, counter: u64, value: &str) -> PersonalSyncVersion {
        let content = PersonalSyncContent {
            key: "tone".into(),
            value: value.into(),
            tags: vec!["style".into()],
            rule: RuleOptions::default(),
            enabled: true,
        };
        let digest = content.digest();
        PersonalSyncVersion {
            key: content.key.clone(),
            clock: BTreeMap::from([(device.to_owned(), counter)]),
            digest,
            content: Some(content),
        }
    }

    #[test]
    fn vector_clocks_compare_join_and_tick_without_losing_a_device() {
        let a = BTreeMap::from([("a".repeat(32), 2), ("b".repeat(32), 1)]);
        let b = BTreeMap::from([("a".repeat(32), 1), ("c".repeat(32), 3)]);
        assert_eq!(clock_cmp(&a, &b), None);
        assert_eq!(
            clock_cmp(&clock_tick(&a, &"a".repeat(32)).unwrap(), &a),
            Some(std::cmp::Ordering::Greater)
        );
        let joined = clock_join(&a, &b).unwrap();
        assert_eq!(joined.len(), 3);
        assert_eq!(joined[&"c".repeat(32)], 3);
        let full: VectorClock = (0..MAX_SYNC_CLOCK_DEVICES)
            .map(|index| (format!("{index:032x}"), 1))
            .collect();
        let other: VectorClock = BTreeMap::from([("1".repeat(32), 1)]);
        assert!(clock_join(&full, &other).is_none());
        assert!(clock_tick(&full, &"1".repeat(32)).is_none());
    }

    #[test]
    fn merge_refuses_equal_clock_drift_and_concurrent_edits() {
        let local = version(&"a".repeat(32), 2, "local");
        let equal_drift = version(&"a".repeat(32), 2, "remote");
        assert_eq!(
            classify_merge(&equal_drift, &local),
            PersonalSyncMerge::Conflict
        );
        let newer = version(&"a".repeat(32), 3, "remote");
        assert_eq!(classify_merge(&newer, &local), PersonalSyncMerge::Accept);
        assert_eq!(classify_merge(&local, &newer), PersonalSyncMerge::Stale);
        let mut concurrent = version(&"b".repeat(32), 1, "remote");
        concurrent.clock.insert("a".repeat(32), 1);
        assert_eq!(
            classify_merge(&concurrent, &local),
            PersonalSyncMerge::Conflict
        );
        assert_eq!(classify_merge(&local, &local), PersonalSyncMerge::Noop);
        assert_eq!(
            classify_merge(
                &version(&"b".repeat(32), 1, "new"),
                &PersonalSyncVersion::absent("tone")
            ),
            PersonalSyncMerge::Accept
        );
        let deletion = PersonalSyncVersion {
            key: "tone".into(),
            clock: BTreeMap::from([("b".repeat(32), 1)]),
            digest: tombstone_digest("tone"),
            content: None,
        };
        assert_eq!(
            classify_merge(&deletion, &PersonalSyncVersion::absent("tone")),
            PersonalSyncMerge::Accept
        );
        assert_eq!(
            classify_merge(&deletion, &deletion),
            PersonalSyncMerge::Noop
        );
    }

    fn sync_bundle(versions: &[PersonalSyncVersion]) -> PersonalSyncBundle {
        let mut entries = Vec::new();
        let mut tombstones = Vec::new();
        for version in versions {
            match &version.content {
                Some(content) => entries.push(PersonalSyncEntry {
                    content: content.clone(),
                    clock: version.clock.clone(),
                    digest: version.digest.clone(),
                }),
                None => tombstones.push(PersonalSyncTombstone {
                    key: version.key.clone(),
                    clock: version.clock.clone(),
                    digest: version.digest.clone(),
                }),
            }
        }
        PersonalSyncBundle {
            format: PERSONAL_SYNC_FORMAT.into(),
            version: PERSONAL_SYNC_VERSION,
            bundle_id: "b".repeat(32),
            owner_id: "c".repeat(32),
            created_at_ms: 1,
            entries,
            tombstones,
        }
    }

    fn sync_selection(
        incoming: &PersonalSyncVersion,
        local: &PersonalSyncVersion,
        resolution: PersonalSyncResolution,
    ) -> PersonalSyncSelection {
        PersonalSyncSelection {
            key: incoming.key.clone(),
            incoming_digest: incoming.digest.clone(),
            expected_local_digest: local.digest.clone(),
            resolution,
        }
    }

    #[test]
    fn apply_planning_accepts_newer_versions_and_records_each_selection() {
        let local = version(&"a".repeat(32), 1, "local");
        let incoming = version(&"a".repeat(32), 2, "remote");
        let request = PersonalSyncApplyRequest {
            bundle: sync_bundle(std::slice::from_ref(&incoming)),
            local_versions: vec![local.clone()],
            device_id: "d".repeat(32),
            selections: vec![sync_selection(
                &incoming,
                &local,
                PersonalSyncResolution::Accept,
            )],
        };
        let plan = plan_personal_sync_apply(&request).unwrap();
        assert_eq!(plan.changes.len(), 1);
        assert_eq!(plan.versions[0].clock, incoming.clock);
        assert_eq!(plan.applied[0].digest, incoming.digest);
        assert_eq!(plan.bundle_digest, request.bundle.digest());
    }

    #[test]
    fn conflict_resolutions_dominate_both_clocks_and_keep_local_requires_no_rewrite() {
        let local = version(&"a".repeat(32), 1, "local");
        let incoming = version(&"b".repeat(32), 1, "remote");
        let mut request = PersonalSyncApplyRequest {
            bundle: sync_bundle(std::slice::from_ref(&incoming)),
            local_versions: vec![local.clone()],
            device_id: "d".repeat(32),
            selections: vec![sync_selection(
                &incoming,
                &local,
                PersonalSyncResolution::KeepLocal,
            )],
        };
        let kept = plan_personal_sync_apply(&request).unwrap();
        assert_eq!(kept.versions[0].digest, local.digest);
        assert_eq!(kept.versions[0].clock[&"d".repeat(32)], 1);
        assert!(kept.changes.is_empty());
        request.selections[0].resolution = PersonalSyncResolution::Manual(PersonalSyncContent {
            key: "tone".into(),
            value: "merged".into(),
            tags: vec!["style".into()],
            rule: RuleOptions::default(),
            enabled: true,
        });
        let merged = plan_personal_sync_apply(&request).unwrap();
        assert_eq!(merged.versions[0].content.as_ref().unwrap().value, "merged");
        assert_eq!(merged.versions[0].clock.len(), 3);
    }

    #[test]
    fn concurrent_identical_content_joins_clocks_without_rewriting_the_profile() {
        let local = version(&"a".repeat(32), 1, "same");
        let incoming = version(&"b".repeat(32), 1, "same");
        let request = PersonalSyncApplyRequest {
            bundle: sync_bundle(std::slice::from_ref(&incoming)),
            local_versions: vec![local],
            device_id: "d".repeat(32),
            selections: vec![sync_selection(
                &incoming,
                &version(&"a".repeat(32), 1, "same"),
                PersonalSyncResolution::KeepLocal,
            )],
        };
        let plan = plan_personal_sync_apply(&request).unwrap();
        assert!(plan.changes.is_empty());
        assert_eq!(plan.versions[0].clock.len(), 3);
    }

    #[test]
    fn apply_planning_refuses_stale_digests_duplicate_keys_and_unknown_items() {
        let local = version(&"a".repeat(32), 1, "local");
        let incoming = version(&"a".repeat(32), 2, "remote");
        let mut request = PersonalSyncApplyRequest {
            bundle: sync_bundle(std::slice::from_ref(&incoming)),
            local_versions: vec![local.clone()],
            device_id: "d".repeat(32),
            selections: vec![sync_selection(
                &incoming,
                &local,
                PersonalSyncResolution::Accept,
            )],
        };
        request.selections[0].expected_local_digest = "0".repeat(64);
        assert!(plan_personal_sync_apply(&request).is_none());
        request.selections[0].expected_local_digest = local.digest;
        request.selections.push(request.selections[0].clone());
        assert!(plan_personal_sync_apply(&request).is_none());
        request.selections.pop();
        request.selections[0].key = "unknown".into();
        assert!(plan_personal_sync_apply(&request).is_none());
    }

    #[test]
    fn malformed_clocks_digests_and_duplicate_keys_are_refused() {
        let mut entry = version(&"a".repeat(32), 1, "value");
        let clock = entry.clock.clone();
        let digest = entry.digest.clone();
        let content = entry.content.clone().unwrap();
        let bundle = PersonalSyncBundle {
            format: PERSONAL_SYNC_FORMAT.into(),
            version: PERSONAL_SYNC_VERSION,
            bundle_id: "b".repeat(32),
            owner_id: "c".repeat(32),
            created_at_ms: 1,
            entries: vec![PersonalSyncEntry {
                content: content.clone(),
                clock: clock.clone(),
                digest: digest.clone(),
            }],
            tombstones: vec![],
        };
        assert!(bundle.valid());
        let mut bad_clock = bundle.clone();
        bad_clock.entries[0].clock.insert("d".repeat(32), 0);
        assert!(!bad_clock.valid());
        let mut bad_digest = bundle.clone();
        bad_digest.entries[0].digest = "0".repeat(64);
        assert!(!bad_digest.valid());
        let mut duplicate = bundle;
        duplicate.entries.push(PersonalSyncEntry {
            content,
            clock,
            digest,
        });
        assert!(!duplicate.valid());
        assert!(entry.valid());
        entry.digest = "0".repeat(64);
        assert!(!entry.valid());
    }
}
