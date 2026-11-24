//! Agent-owned work coordination: a session goal plus flat, structured work
//! items. The daemon owns persistence and CAS; agents may update only their
//! project-bound session state where the operator allows agent writes.
pub(crate) mod api;
mod model;
mod store;
#[cfg(test)]
mod tests;

pub use store::Sessions;
