//! Doctor: one page of checks over what the app already observed (daemon,
//! Profile, the selected workspace, its integrations and agent activity). Each
//! check says what it saw, never more: a value not read yet is `Unknown`, not
//! a pass. Pure; the page renders it and the report copies it as text with no
//! paths, preference values or credentials.
mod presentation;
mod probe;
mod report;

pub use presentation::{Check, Fix, Level};
pub use probe::{HOOKS, MCP, checks};
pub use report::{level_word, report, tally};

#[cfg(test)]
mod tests;
