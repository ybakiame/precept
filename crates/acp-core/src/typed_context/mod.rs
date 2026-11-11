//! Pure, additive v1 context explanation. No authorization, adapters or I/O.
//! See docs/p1/typed-context-v1.md; this is not an MCP/IPC wire contract.
mod resolver;
mod types;

pub use resolver::resolve;
pub use types::*;
