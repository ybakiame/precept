//! Daemon-private personal sync identities, trust, clocks and apply recovery.
mod service;
mod state;

pub(crate) use service::{
    abort, apply, current_globals, export, get_invitation, init, pair, preview, reset, revoke,
    rotate, status, trust,
};

pub(crate) const MAX_STATE_BYTES: u64 = 4 * 1024 * 1024;
pub(crate) const MAX_TRACKED_KEYS: usize = 500;
pub(crate) const MAX_TRUSTED_DEVICES: usize = precept_core::operator::MAX_SYNC_CLOCK_DEVICES - 1;
pub(crate) const MAX_APPLIED_ENTRIES: usize = 5000;
pub(crate) const MAX_OPERATIONS: usize = 4096;
