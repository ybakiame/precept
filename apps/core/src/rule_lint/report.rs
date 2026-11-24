//! Strict, bounded DTOs for the daemon's privacy-preserving rule diagnostics.
//! The daemon's diagnostic text is accepted only when it is the fixed English
//! message for its known code; source text and transport errors never enter it.
use serde::{Deserialize, Serialize};

const FORMAT: &str = "precept-rules-lint";
const VERSION: u32 = 1;
const MAX_FILES: usize = 64;
const MAX_DIAGNOSTICS_PER_FILE: usize = 32;
const MAX_REPORT_BYTES: usize = 768 * 1024;
const MAX_POSITION: u32 = 1_000_000;
const MAX_COPY_DIAGNOSTICS: usize = 80;
const MAX_COPY_CHARS: usize = 24 * 1024;
pub(super) const INVALID: &str = "Invalid rule lint report";

/// The overall result of checking one project's repository rules.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LintStatus {
    Ok,
    Issues,
    NotConfigured,
    Unavailable,
}

/// Whether a diagnostic rejects a rule or asks for attention.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

/// The closed diagnostic vocabulary. Unknown codes refuse the whole reply.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum DiagnosticCode {
    #[serde(rename = "id.invalid")]
    IdInvalid,
    #[serde(rename = "file.too_large")]
    FileTooLarge,
    #[serde(rename = "encoding.invalid")]
    EncodingInvalid,
    #[serde(rename = "format.opening_marker")]
    FormatOpeningMarker,
    #[serde(rename = "format.closing_marker")]
    FormatClosingMarker,
    #[serde(rename = "frontmatter.invalid")]
    FrontmatterInvalid,
    #[serde(rename = "key.invalid")]
    KeyInvalid,
    #[serde(rename = "title.invalid")]
    TitleInvalid,
    #[serde(rename = "allowed.invalid")]
    AllowedInvalid,
    #[serde(rename = "tags.invalid")]
    TagsInvalid,
    #[serde(rename = "applies_to.invalid")]
    AppliesToInvalid,
    #[serde(rename = "priority.invalid")]
    PriorityInvalid,
    #[serde(rename = "check.invalid")]
    CheckInvalid,
    #[serde(rename = "guard.invalid")]
    GuardInvalid,
    #[serde(rename = "guard.requires_hard")]
    GuardRequiresHard,
    #[serde(rename = "body.empty")]
    BodyEmpty,
    #[serde(rename = "body.control_characters")]
    BodyControlCharacters,
    #[serde(rename = "file.unsafe_path")]
    FileUnsafePath,
    #[serde(rename = "source.unsafe_path")]
    SourceUnsafePath,
    #[serde(rename = "source.too_many_files")]
    SourceTooManyFiles,
    #[serde(rename = "source.unavailable")]
    SourceUnavailable,
    #[serde(rename = "enforcement.no_verification")]
    EnforcementNoVerification,
    #[serde(rename = "rule.duplicate_key")]
    RuleDuplicateKey,
    #[serde(rename = "rule.priority_tie")]
    RulePriorityTie,
    #[serde(rename = "selector.path_not_present")]
    SelectorPathNotPresent,
    #[serde(rename = "diagnostics.truncated")]
    DiagnosticsTruncated,
}

impl DiagnosticCode {
    /// The stable code sent by the daemon.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IdInvalid => "id.invalid",
            Self::FileTooLarge => "file.too_large",
            Self::EncodingInvalid => "encoding.invalid",
            Self::FormatOpeningMarker => "format.opening_marker",
            Self::FormatClosingMarker => "format.closing_marker",
            Self::FrontmatterInvalid => "frontmatter.invalid",
            Self::KeyInvalid => "key.invalid",
            Self::TitleInvalid => "title.invalid",
            Self::AllowedInvalid => "allowed.invalid",
            Self::TagsInvalid => "tags.invalid",
            Self::AppliesToInvalid => "applies_to.invalid",
            Self::PriorityInvalid => "priority.invalid",
            Self::CheckInvalid => "check.invalid",
            Self::GuardInvalid => "guard.invalid",
            Self::GuardRequiresHard => "guard.requires_hard",
            Self::BodyEmpty => "body.empty",
            Self::BodyControlCharacters => "body.control_characters",
            Self::FileUnsafePath => "file.unsafe_path",
            Self::SourceUnsafePath => "source.unsafe_path",
            Self::SourceTooManyFiles => "source.too_many_files",
            Self::SourceUnavailable => "source.unavailable",
            Self::EnforcementNoVerification => "enforcement.no_verification",
            Self::RuleDuplicateKey => "rule.duplicate_key",
            Self::RulePriorityTie => "rule.priority_tie",
            Self::SelectorPathNotPresent => "selector.path_not_present",
            Self::DiagnosticsTruncated => "diagnostics.truncated",
        }
    }

    /// The only message accepted for this code. It contains no repository data.
    pub fn message(self) -> &'static str {
        match self {
            Self::IdInvalid => "The rule file id is invalid.",
            Self::FileTooLarge => "The rule file exceeds the size limit.",
            Self::EncodingInvalid => "The rule file is not valid UTF-8.",
            Self::FormatOpeningMarker => "The rule file must open with a +++ marker.",
            Self::FormatClosingMarker => "The front matter must end with a +++ marker.",
            Self::FrontmatterInvalid => "The front matter is invalid.",
            Self::KeyInvalid => "The rule key is invalid.",
            Self::TitleInvalid => "The rule title is invalid.",
            Self::AllowedInvalid => "The allowed values are invalid.",
            Self::TagsInvalid => "The rule tags are invalid.",
            Self::AppliesToInvalid => "The applicability selector is invalid.",
            Self::PriorityInvalid => "The rule priority is out of range.",
            Self::CheckInvalid => "The repository check is invalid.",
            Self::GuardInvalid => "The tool guard is invalid.",
            Self::GuardRequiresHard => "Tool guards require hard enforcement.",
            Self::BodyEmpty => "The rule body must not be empty.",
            Self::BodyControlCharacters => "The rule body contains unsupported control characters.",
            Self::FileUnsafePath => "The rule file is not a safe regular file.",
            Self::SourceUnsafePath => "The rule source contains an unsafe directory or link.",
            Self::SourceTooManyFiles => "The rule source contains too many files.",
            Self::SourceUnavailable => "The rule source is unavailable.",
            Self::EnforcementNoVerification => "Verify enforcement has no repository checks.",
            Self::RuleDuplicateKey => "Another rule file uses the same key.",
            Self::RulePriorityTie => {
                "Another rule file has the same key, applicability and priority."
            }
            Self::SelectorPathNotPresent => {
                "The selected path is not present in the repository; future files may still match."
            }
            Self::DiagnosticsTruncated => {
                "Additional diagnostics were omitted by the report limit."
            }
        }
    }

    fn severity(self) -> Severity {
        match self {
            Self::EnforcementNoVerification
            | Self::RuleDuplicateKey
            | Self::RulePriorityTie
            | Self::SelectorPathNotPresent
            | Self::DiagnosticsTruncated => Severity::Warning,
            _ => Severity::Error,
        }
    }
}

/// One sanitized finding. Positions are 1-based and end-exclusive.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub message: String,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related_file: Option<String>,
}

/// The state of one reported repository-relative file.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    Ok,
    Issues,
    Unavailable,
}

/// Diagnostics for one file, or a sanitized synthetic source entry.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LintFile {
    pub file: String,
    pub status: FileStatus,
    pub diagnostics: Vec<Diagnostic>,
}

/// Aggregate counts for the diagnostics actually returned.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LintSummary {
    pub files: u32,
    pub errors: u32,
    pub warnings: u32,
}

/// The stable serialized lint contract, validated before the UI retains it.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LintReport {
    pub format: String,
    pub version: u32,
    pub status: LintStatus,
    pub summary: LintSummary,
    pub files: Vec<LintFile>,
}

fn safe_rule_path(value: &str) -> bool {
    let Some(name) = value.strip_prefix(".precept/rules/") else {
        return false;
    };
    let Some(id) = name.strip_suffix(".md") else {
        return false;
    };
    !id.is_empty()
        && id.len() <= 120
        && !id.starts_with('.')
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_.".contains(&byte)
        })
}

fn safe_lint_path(value: &str) -> bool {
    value == ".precept/rules" || safe_rule_path(value)
}

fn ordered_position(d: &Diagnostic) -> bool {
    let start = (d.line, d.column);
    let end = (d.end_line, d.end_column);
    d.line > 0
        && d.column > 0
        && d.end_line > 0
        && d.end_column > 0
        && start.0 <= MAX_POSITION
        && start.1 <= MAX_POSITION
        && end.0 <= MAX_POSITION
        && end.1 <= MAX_POSITION
        && end >= start
}

impl LintReport {
    /// Parses one daemon value and refuses unknown, oversized, unsafe or
    /// internally inconsistent output. No caller-supplied error text is kept.
    pub fn parse(value: serde_json::Value) -> Result<Self, &'static str> {
        if serde_json::to_vec(&value).map_or(true, |bytes| bytes.len() > MAX_REPORT_BYTES) {
            return Err(INVALID);
        }
        let report: Self = serde_json::from_value(value).map_err(|_| INVALID)?;
        report.validate()?;
        Ok(report)
    }

    /// Rechecks every structural and cross-field invariant.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.format != FORMAT
            || self.version != VERSION
            || self.files.len() > MAX_FILES
            || !self
                .files
                .windows(2)
                .all(|pair| pair[0].file < pair[1].file)
            || self.files.iter().any(|file| !safe_lint_path(&file.file))
        {
            return Err(INVALID);
        }
        let mut errors = 0_u32;
        let mut warnings = 0_u32;
        let mut unavailable = false;
        let mut issues = false;
        for file in &self.files {
            if file.diagnostics.len() > MAX_DIAGNOSTICS_PER_FILE {
                return Err(INVALID);
            }
            let synthetic = file.file == ".precept/rules";
            match file.status {
                FileStatus::Ok if !synthetic && file.diagnostics.is_empty() => {}
                FileStatus::Issues
                    if !file.diagnostics.is_empty()
                        && if synthetic {
                            file.diagnostics.len() == 1
                                && matches!(
                                    file.diagnostics[0].code,
                                    DiagnosticCode::SourceUnsafePath
                                        | DiagnosticCode::SourceTooManyFiles
                                )
                        } else {
                            file.diagnostics.iter().all(|diagnostic| {
                                !matches!(
                                    diagnostic.code,
                                    DiagnosticCode::SourceUnsafePath
                                        | DiagnosticCode::SourceTooManyFiles
                                        | DiagnosticCode::SourceUnavailable
                                )
                            })
                        } =>
                {
                    issues = true;
                }
                FileStatus::Unavailable
                    if synthetic
                        && file.diagnostics.len() == 1
                        && file.diagnostics[0].code == DiagnosticCode::SourceUnavailable
                        && file.diagnostics[0].severity == Severity::Error =>
                {
                    unavailable = true;
                }
                _ => return Err(INVALID),
            }
            for diagnostic in &file.diagnostics {
                if diagnostic.severity != diagnostic.code.severity()
                    || diagnostic.message != diagnostic.code.message()
                    || !ordered_position(diagnostic)
                    || (diagnostic.code == DiagnosticCode::SourceUnavailable
                        && file.status != FileStatus::Unavailable)
                    || diagnostic.related_file.as_deref().is_some_and(|path| {
                        !safe_rule_path(path)
                            || !self.files.iter().any(|reported| reported.file == path)
                    })
                {
                    return Err(INVALID);
                }
                match diagnostic.severity {
                    Severity::Error => errors += 1,
                    Severity::Warning => warnings += 1,
                }
            }
        }
        let expected = LintSummary {
            files: self.files.len() as u32,
            errors,
            warnings,
        };
        if self.summary != expected {
            return Err(INVALID);
        }
        let consistent = match self.status {
            LintStatus::Ok => !issues && !unavailable && errors == 0 && warnings == 0,
            LintStatus::Issues => issues && !unavailable && errors + warnings > 0,
            LintStatus::NotConfigured => self.files.is_empty(),
            LintStatus::Unavailable => unavailable && !issues && self.files.len() == 1,
        };
        consistent.then_some(()).ok_or(INVALID)
    }

    /// A bounded English report for Copy. It uses only validated fixed messages
    /// and safe relative paths, and names any diagnostics omitted from the copy.
    pub fn plain_text(&self) -> String {
        if self.validate().is_err() {
            return INVALID.to_owned();
        }
        let mut out = CopyText::default();
        out.line("Precept rule diagnostics");
        out.line(match self.status {
            LintStatus::Ok => "Status: no issues",
            LintStatus::Issues => "Status: issues found",
            LintStatus::NotConfigured => "Status: not configured",
            LintStatus::Unavailable => "Status: unavailable",
        });
        out.line(format!(
            "Summary: {} file(s), {} error(s), {} warning(s)",
            self.summary.files, self.summary.errors, self.summary.warnings
        ));
        if self.files.is_empty() {
            out.line("No rule files were reported.");
            return out.text;
        }
        let total = self
            .files
            .iter()
            .filter(|file| file.status != FileStatus::Ok)
            .map(|file| file.diagnostics.len())
            .sum::<usize>();
        let mut shown = 0_usize;
        'files: for file in &self.files {
            if file.status == FileStatus::Ok {
                continue;
            }
            let header = vec![
                String::new(),
                format!(
                    "{} [{}]",
                    file.file,
                    match file.status {
                        FileStatus::Ok => "OK",
                        FileStatus::Issues => "Issues",
                        FileStatus::Unavailable => "Unavailable",
                    }
                ),
            ];
            if !out.fits_lines(&header) {
                out.truncated = true;
                break;
            }
            for line in header {
                out.line(line);
            }
            for diagnostic in &file.diagnostics {
                if shown == MAX_COPY_DIAGNOSTICS {
                    break 'files;
                }
                let span = if (diagnostic.end_line, diagnostic.end_column)
                    == (diagnostic.line, diagnostic.column)
                {
                    format!("{}:{}", diagnostic.line, diagnostic.column)
                } else {
                    format!(
                        "{}:{}-{}:{}",
                        diagnostic.line,
                        diagnostic.column,
                        diagnostic.end_line,
                        diagnostic.end_column
                    )
                };
                let mut lines = vec![format!(
                    "[{}] {} {}: {}",
                    match diagnostic.severity {
                        Severity::Error => "Error",
                        Severity::Warning => "Warning",
                    },
                    diagnostic.code.as_str(),
                    span,
                    diagnostic.message
                )];
                if let Some(related) = &diagnostic.related_file {
                    lines.push(format!("Related file: {related}"));
                }
                if !out.fits_lines(&lines) {
                    out.truncated = true;
                    break 'files;
                }
                for line in lines {
                    out.line(line);
                }
                shown += 1;
            }
        }
        out.finish(total, shown)
    }
}

const COPY_NOTICE_RESERVE: usize = 256;
const COPY_SIZE_NOTICE: &str = "Copy truncated at the 24,576-character limit.";

#[derive(Default)]
struct CopyText {
    text: String,
    truncated: bool,
}

impl CopyText {
    fn content_limit(&self) -> usize {
        MAX_COPY_CHARS - COPY_NOTICE_RESERVE
    }

    fn fits_lines(&self, lines: &[String]) -> bool {
        let mut used = self.text.chars().count();
        for line in lines {
            let separator = usize::from(used != 0);
            if used + separator + line.chars().count() > self.content_limit() {
                return false;
            }
            used += separator + line.chars().count();
        }
        true
    }

    fn line(&mut self, line: impl AsRef<str>) {
        let line = line.as_ref();
        if !self.fits_lines(&[line.to_owned()]) {
            self.truncated = true;
            return;
        }
        if !self.text.is_empty() {
            self.text.push('\n');
        }
        self.text.push_str(line);
    }

    fn finish(mut self, total: usize, shown: usize) -> String {
        if shown < total {
            self.line(format!(
                "Copy includes {shown} of {total} diagnostics; {} omitted.",
                total - shown
            ));
        }
        if self.truncated {
            self.line(COPY_SIZE_NOTICE);
        }
        self.text
    }
}
