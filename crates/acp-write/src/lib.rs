//! Private Profile v1 storage. The daemon alone enables guarded existing-v1
//! attachment through the native lease owner; clients never receive a Writer.
mod command;
mod files;
#[cfg(feature = "daemon-owner")]
pub mod owner;
mod profile;
mod writer;
pub use command::{Binding, Command, Expected, Receipt, RecordVersion, Request, Scope, Snapshot};
pub use profile::Profile;
pub use writer::Writer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Unauthorized,
    InvalidCommand,
    InvalidTransition,
    IdempotencyConflict,
    Conflict,
    ReviewMismatch,
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
    StorageFull,
}
#[cfg(test)]
mod tests;

impl From<acp_store::Error> for Error {
    fn from(e: acp_store::Error) -> Self {
        match e {
            acp_store::Error::UnsupportedVersion => Self::UnsupportedVersion,
            acp_store::Error::IncompatibleSchema => Self::IncompatibleSchema,
            _ => Self::InvalidRecord,
        }
    }
}
