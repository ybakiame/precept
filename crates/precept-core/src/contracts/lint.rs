//! Deterministic, privacy-preserving diagnostics for repository rule files.
//!
//! The pure entry point never touches the filesystem. [`Snapshot`](super::Snapshot)
//! adds inventory failures and conservative checks for currently absent selectors.
mod location;
mod parse;
mod position;
mod report;
#[cfg(test)]
mod tests;

pub use report::{Diagnostic, LintFile, LintReport, LintStatus, LintSummary, Severity};

use super::file_of;
use super::parser::{DIR, valid_id};
use crate::repo_fs::Repository;
use parse::Linted;
use position::{Position, diagnostic};
use report::{LintFile as ReportFile, PreviousRule, Severity as ReportSeverity, sort_and_bound};
use std::collections::BTreeSet;

/// Lint one in-memory rule file. This function is pure and filesystem-free.
pub fn lint(id: &str, raw: &[u8]) -> Vec<Diagnostic> {
    Linted::new(id, raw).diagnostics()
}

pub(super) fn report(status: LintStatus, files: Vec<LintFile>) -> LintReport {
    report::report(status, files)
}

pub(super) fn lint_files<'a>(
    sources: impl Iterator<Item = (&'a str, &'a [u8], Option<&'static str>)>,
    root: Option<&Repository>,
) -> LintReport {
    let mut files: Vec<Linted> = sources
        .map(|(id, raw, failure)| match failure {
            Some(code) => Linted::failure(id, code),
            None => Linted::new(id, raw),
        })
        .collect();
    files.sort_by(|a, b| a.file.file.cmp(&b.file.file).then_with(|| a.id.cmp(&b.id)));
    disambiguate_invalid_files(&mut files);
    files.sort_by(|a, b| a.file.file.cmp(&b.file.file).then_with(|| a.id.cmp(&b.id)));

    let mut previous: Vec<PreviousRule> = vec![];
    for file in &mut files {
        if let Some(facts) = file.facts.clone() {
            if let Some(position) = file.key_position {
                if let Some(first) = previous.iter().find(|p| p.facts.key == facts.key) {
                    file.push_at(
                        "rule.duplicate_key",
                        Severity::Warning,
                        position,
                        Some(first.file.clone()),
                    );
                }
                if let Some(first) = previous.iter().find(|p| {
                    p.facts.key == facts.key
                        && p.facts.applies_to == facts.applies_to
                        && p.facts.priority == facts.priority
                        && facts.applies_to.is_some()
                        && facts.priority.is_some()
                }) {
                    file.push_at(
                        "rule.priority_tie",
                        Severity::Warning,
                        position,
                        Some(first.file.clone()),
                    );
                }
            }
            previous.push(PreviousRule {
                file: file.file.file.clone(),
                facts,
            });
        }
        if let (Some(root), Some(position), Some(rule)) =
            (root, file.paths_position, file.rule.clone())
        {
            for path in &rule.applies_to.paths {
                if root.selector_missing(path) == Some(true) {
                    file.push_at(
                        "selector.path_not_present",
                        Severity::Warning,
                        position,
                        None,
                    );
                }
            }
        }
    }

    let files: Vec<ReportFile> = files
        .into_iter()
        .map(|mut file| {
            sort_and_bound(&mut file.file.diagnostics);
            file.file.status = if file.file.diagnostics.is_empty() {
                "ok"
            } else {
                "issues"
            }
            .to_owned();
            file.file
        })
        .collect();
    let status = if files.iter().any(|f| f.status == "unavailable") {
        LintStatus::Unavailable
    } else if files.iter().any(|f| !f.diagnostics.is_empty()) {
        LintStatus::Issues
    } else {
        LintStatus::Ok
    };
    report(status, files)
}

pub(super) fn source_report(status: LintStatus, code: &'static str) -> LintReport {
    let file_status = if status == LintStatus::Unavailable {
        "unavailable"
    } else {
        "issues"
    };
    report(
        status,
        vec![ReportFile {
            file: DIR.to_owned(),
            status: file_status.to_owned(),
            diagnostics: vec![diagnostic(
                code,
                ReportSeverity::Error,
                Position::start(),
                None,
            )],
        }],
    )
}

fn disambiguate_invalid_files(files: &mut [Linted]) {
    let mut used: BTreeSet<String> = files
        .iter()
        .filter(|file| valid_id(&file.id))
        .map(|file| file_of(&file.id))
        .collect();
    for file in files.iter_mut().filter(|file| !valid_id(&file.id)) {
        let mut candidate = file.file.file.clone();
        let Some(stem) = candidate.strip_suffix(".md").map(str::to_owned) else {
            continue;
        };
        let mut suffix = 2;
        while !used.insert(candidate.clone()) {
            candidate = format!("{stem}-{suffix}.md");
            suffix += 1;
        }
        file.file.file = candidate;
    }
}
