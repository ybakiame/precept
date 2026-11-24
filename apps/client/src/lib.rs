//! Operator app transport. Native IPC to the daemon (`native`), the Windows -> WSL2
//! stdio bridge (`bridge`), and the operator request paths over either.
//! Authorization stays daemon-side; this crate never opens SQLite or the Writer.
pub mod bridge;
pub mod discover;
pub mod native;
pub mod operator;
mod paths;
pub mod settings;
