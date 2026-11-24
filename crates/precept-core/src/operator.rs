//! Data-only Profile operator DTOs. No authorization, storage or writer implementation.
mod command;
mod record;
mod review;
mod rule;
mod sync;
#[cfg(test)]
mod tests;

pub use command::{
    BatchDecision, BatchItem, Binding, Command, Expected, MAX_BATCH, Mutation, Receipt,
    RecordVersion, Scope, Snapshot, SuggestItem,
};
pub use record::{
    HandoffList, HandoffPreview, HandoffRecord, HandoffSummary, PreferencePage, PreferencePreview,
    PreferenceRecord, ProfileInfo,
};
pub use review::{
    Channel, DecisionAction, MAX_NOTE, PeerDemotion, ReviewCount, ReviewDecisionRecord,
    ReviewEvidence, ReviewInspect, ReviewQueue, ReviewRelated, ReviewRelation, canonical_note,
    digest_from_hex, digest_hex, proposal_digest,
};
pub use rule::{
    AppliesTo, Enforcement, MAX_APPLIES, MAX_TAG_CHARS, MAX_TAGS, Origin, PRIORITY_MAX,
    PRIORITY_MIN, RuleOptions, canonical_tags, normalize_tag, tags_canonical, tags_json,
};
pub use sync::{
    MAX_SYNC_CLOCK_DEVICES, MAX_SYNC_COUNTER, MAX_SYNC_ENVELOPE_BYTES, MAX_SYNC_ITEMS,
    MAX_SYNC_PAYLOAD_BYTES, PERSONAL_SYNC_FORMAT, PERSONAL_SYNC_VERSION, PersonalSyncAbortRequest,
    PersonalSyncAbortResult, PersonalSyncAppliedEntry, PersonalSyncApplyPlan,
    PersonalSyncApplyRequest, PersonalSyncApplyResult, PersonalSyncApplySubmission,
    PersonalSyncBundle, PersonalSyncChange, PersonalSyncContent, PersonalSyncDeviceStatus,
    PersonalSyncEntry, PersonalSyncEnvelope, PersonalSyncExportRequest, PersonalSyncHeader,
    PersonalSyncInvitation, PersonalSyncLocalVersion, PersonalSyncMerge, PersonalSyncOpenRequest,
    PersonalSyncPreview, PersonalSyncPreviewItem, PersonalSyncResetRequest, PersonalSyncResolution,
    PersonalSyncRevokeRequest, PersonalSyncSelection, PersonalSyncStatus, PersonalSyncTombstone,
    PersonalSyncTrustRequest, PersonalSyncVersion, VectorClock, classify_merge, clock_cmp,
    clock_join, clock_tick, plan_personal_sync_apply, tombstone_digest, valid_clock,
    valid_identity, valid_sync_digest,
};
