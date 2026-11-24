//! Pure validation and model state for Desktop repository-rule diagnostics.
mod report;
mod state;
#[cfg(test)]
mod tests;

pub use report::{
    Diagnostic, DiagnosticCode, FileStatus, LintFile, LintReport, LintStatus, LintSummary, Severity,
};
pub use state::{RuleLintRequest, RuleLintState};
