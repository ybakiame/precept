//! Project memory: what agents learn while working in one project — this
//! machine's environment, cautions, lessons and attempts — written and kept
//! current by the agents themselves, so it survives a change of agent, harness
//! or model. Reference only: reviewed rules and the current code outrank it.
//! Kept in the daemon's private state (one file per project), never in the
//! Profile, bounded; the operator edits, pins, deletes or clears it at will.
//! Agent writes need the operator's per-project permission for agent writes.

mod model;
mod ranking;
mod store;
#[cfg(test)]
mod tests;
mod validation;

// Preserve the original module's public surface at its existing path.
#[allow(unused_imports)]
pub use model::{
    BRIEF_BYTES, BRIEF_ENVIRONMENT, BRIEF_RELEVANT, Earlier, Entry, FADE_AFTER_DAYS, Kind,
    MAX_BASIS, MAX_BODY, MAX_ENTRIES, MAX_HISTORY, MAX_TITLE, NOTICE_AFTER_DAYS, PROPOSE_AFTER,
    Remembered,
};
pub use store::{Memory, now_ms};

#[cfg(test)]
use model::DAY_MS;
#[cfg(test)]
use ranking::terms;
