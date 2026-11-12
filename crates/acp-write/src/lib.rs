//! Synthetic-only profile v2 transactions. No arbitrary path opening, profile
//! discovery, production authority, network, subprocess or agent mutation surface.
mod command;
mod schema;
mod workspace;
mod writer;
pub use command::{
    Binding, Command, Expected, FixtureRoot, Receipt, RecordVersion, Request, Scope, Snapshot,
};
pub use workspace::{FixtureShape, SyntheticWorkspace};
pub use writer::Writer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidCommand,
    InvalidTransition,
    IdempotencyConflict,
    Conflict,
    ScopeMismatch,
    StaleGeneration,
    OwnerMismatch,
    UnsupportedVersion,
    IncompatibleSchema,
    InvalidRecord,
    Collision,
    UnsafeWorkspace,
    Busy,
    Capacity,
    StorageFull,
    IoFailure,
    Verification,
    BeforeCommit,
    OutcomeUnknown,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub(crate) fn sql_error(error: rusqlite::Error) -> Error {
    match error.sqlite_error_code() {
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
            Error::Busy
        }
        Some(rusqlite::ErrorCode::DiskFull) => Error::StorageFull,
        Some(rusqlite::ErrorCode::TooBig) => Error::Capacity,
        _ => Error::IoFailure,
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
    None,
    #[cfg(test)]
    BeforeCommit,
    #[cfg(test)]
    AfterCommit,
    #[cfg(test)]
    AfterData,
    #[cfg(test)]
    AfterRevisions,
    #[cfg(test)]
    BackupPartial,
    #[cfg(test)]
    StorageFull,
    #[cfg(test)]
    PauseBeforeCommit,
    #[cfg(test)]
    PauseAfterCommit,
}
#[cfg(test)]
mod tests;
