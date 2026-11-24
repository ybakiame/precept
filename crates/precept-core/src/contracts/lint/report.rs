use super::super::{DIR, file_of, valid_id};
use crate::operator::AppliesTo;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const FORMAT: &str = "precept-rules-lint";
const VERSION: u32 = 1;
pub(super) const MAX_DIAGNOSTICS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LintStatus {
    Ok,
    Issues,
    NotConfigured,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_file: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintFile {
    pub file: String,
    pub status: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintSummary {
    pub files: usize,
    pub errors: usize,
    pub warnings: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintReport {
    pub format: String,
    pub version: u32,
    pub status: LintStatus,
    pub summary: LintSummary,
    pub files: Vec<LintFile>,
}

#[derive(Clone)]
pub(super) struct RuleFacts {
    pub(super) key: String,
    pub(super) applies_to: Option<AppliesTo>,
    pub(super) priority: Option<i64>,
}

pub(super) struct PreviousRule {
    pub(super) file: String,
    pub(super) facts: RuleFacts,
}

pub(super) fn message(code: &str) -> &'static str {
    match code {
        "id.invalid" => "The rule file id is invalid.",
        "file.too_large" => "The rule file exceeds the size limit.",
        "encoding.invalid" => "The rule file is not valid UTF-8.",
        "format.opening_marker" => "The rule file must open with a +++ marker.",
        "format.closing_marker" => "The front matter must end with a +++ marker.",
        "frontmatter.invalid" => "The front matter is invalid.",
        "key.invalid" => "The rule key is invalid.",
        "title.invalid" => "The rule title is invalid.",
        "allowed.invalid" => "The allowed values are invalid.",
        "tags.invalid" => "The rule tags are invalid.",
        "applies_to.invalid" => "The applicability selector is invalid.",
        "priority.invalid" => "The rule priority is out of range.",
        "check.invalid" => "The repository check is invalid.",
        "guard.invalid" => "The tool guard is invalid.",
        "guard.requires_hard" => "Tool guards require hard enforcement.",
        "body.empty" => "The rule body must not be empty.",
        "body.control_characters" => "The rule body contains unsupported control characters.",
        "enforcement.no_verification" => "Verify enforcement has no repository checks.",
        "rule.duplicate_key" => "Another rule file uses the same key.",
        "rule.priority_tie" => "Another rule file has the same key, applicability and priority.",
        "selector.path_not_present" => {
            "The selected path is not present in the repository; future files may still match."
        }
        "source.unsafe_path" => "The rule source contains an unsafe directory or link.",
        "source.too_many_files" => "The rule source contains too many files.",
        "source.unavailable" => "The rule source is unavailable.",
        "file.unsafe_path" => "The rule file is not a safe regular file.",
        "diagnostics.truncated" => "Additional diagnostics were omitted by the report limit.",
        _ => "The rule source is invalid.",
    }
}

pub(super) fn sort(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|a, b| {
        (a.line, a.column, a.end_line, a.end_column, a.code.as_str()).cmp(&(
            b.line,
            b.column,
            b.end_line,
            b.end_column,
            b.code.as_str(),
        ))
    });
}

pub(super) fn sort_and_bound(diagnostics: &mut Vec<Diagnostic>) {
    sort(diagnostics);
    let truncated = diagnostics.len() > MAX_DIAGNOSTICS;
    if truncated {
        diagnostics.truncate(MAX_DIAGNOSTICS - 1);
        diagnostics.push(Diagnostic {
            code: "diagnostics.truncated".to_owned(),
            severity: Severity::Warning,
            message: message("diagnostics.truncated").to_owned(),
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
            related_file: None,
        });
        sort(diagnostics);
    }
}

pub(super) fn report(status: LintStatus, files: Vec<LintFile>) -> LintReport {
    let summary = LintSummary {
        files: files.len(),
        errors: files
            .iter()
            .flat_map(|file| &file.diagnostics)
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .count(),
        warnings: files
            .iter()
            .flat_map(|file| &file.diagnostics)
            .filter(|diagnostic| diagnostic.severity == Severity::Warning)
            .count(),
    };
    LintReport {
        format: FORMAT.to_owned(),
        version: VERSION,
        status,
        summary,
        files,
    }
}

pub(super) fn display_file(id: &str) -> String {
    if valid_id(id) {
        return file_of(id);
    }
    let hash = format!("{:x}", Sha256::digest(id.as_bytes()));
    format!("{DIR}/invalid-{}.md", &hash[..32])
}
