//! Data-only Profile operator DTOs. No authorization, storage or writer implementation.
mod command;
mod record;
mod review;
mod rule;
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
    ReviewInspect, ReviewQueue, canonical_note, digest_from_hex, digest_hex, proposal_digest,
};
pub use rule::{
    AppliesTo, Enforcement, MAX_APPLIES, MAX_TAG_CHARS, MAX_TAGS, Origin, PRIORITY_MAX,
    PRIORITY_MIN, RuleOptions, canonical_tags, normalize_tag, tags_canonical, tags_json,
};
