//! Profile export and import: how preferences move between Profiles (a new
//! schema, a new machine) without migration code. Built on the ordinary
//! operator reads and the reviewed write path: import only ever creates
//! suggestions, which the operator confirms again. Project memory travels only
//! when asked for (this machine's environment notes only when asked for too).
//! Handoffs are not carried.

mod export;
mod format;
mod import;
#[cfg(test)]
mod tests;

pub use export::collect;
// Keep the established transfer API available even when the binary does not
// name every exported format type or bound directly.
#[allow(unused_imports)]
pub use format::{
    Export, Exported, ExportedNote, FORMAT, MAX_BYTES, MAX_RECORDS, MemoryChoice, VERSION, read,
    valid_prefix, write_new,
};
pub use import::{import, preflight};

#[cfg(test)]
use format::{file_indexes, validate_export};
#[cfg(test)]
use import::{ExistingNote, Observation, memory_plan, preference_identity, preference_plan};
