//! Bounded, pure import parsers for known agent rule files. The daemon owns all
//! filesystem reads; this module only turns explicitly supplied text into
//! operator-review candidates. Import never confirms or activates a rule.
use crate::operator::RuleOptions;
use serde::{Deserialize, Serialize};

mod frontmatter;
mod markdown;
mod scan;
#[cfg(test)]
mod tests;

pub use scan::Scan;

pub const MAX_VALUE: usize = 4096;
pub const MAX_CANDIDATES: usize = 200;
pub const MAX_SKIPPED: usize = 255;
pub const MAX_REPLY_BYTES: usize = 640 * 1024;
const REPLY_RESERVE: usize = 1024;
const DAEMON_FIELD_RESERVE: usize = 160;
const MAX_KEY: usize = 120;
const MAX_DETAIL: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    AgentsMd,
    ClaudeMd,
    Cursor,
    Continue,
    Copilot,
    Claude,
}

impl SourceKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::AgentsMd => "AGENTS.md",
            Self::ClaudeMd => "Claude",
            Self::Cursor => "Cursor",
            Self::Continue => "Continue",
            Self::Copilot => "Copilot",
            Self::Claude => "Claude",
        }
    }

    const fn tag(self) -> &'static str {
        match self {
            Self::AgentsMd => "agents-md",
            Self::ClaudeMd => "claude-md",
            Self::Cursor => "cursor",
            Self::Continue => "continue",
            Self::Copilot => "copilot",
            Self::Claude => "claude",
        }
    }

    const fn prefix(self) -> &'static str {
        match self {
            Self::AgentsMd => "agents",
            Self::ClaudeMd => "claude",
            Self::Cursor => "cursor",
            Self::Continue => "continue",
            Self::Copilot => "copilot",
            Self::Claude => "claude",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontmatterKind {
    Cursor,
    Continue,
    Copilot,
    Claude,
}

#[derive(Clone, Copy, Debug)]
pub struct SourceSpec {
    pub source: &'static str,
    pub kind: SourceKind,
    pub sections: bool,
    pub frontmatter: Option<FrontmatterKind>,
}

#[derive(Clone, Copy, Debug)]
pub struct DirectorySpec {
    pub directory: &'static str,
    pub suffix: &'static str,
    pub kind: SourceKind,
    pub frontmatter: FrontmatterKind,
}

pub const FILES: [SourceSpec; 5] = [
    SourceSpec {
        source: "AGENTS.md",
        kind: SourceKind::AgentsMd,
        sections: true,
        frontmatter: None,
    },
    SourceSpec {
        source: "CLAUDE.md",
        kind: SourceKind::ClaudeMd,
        sections: true,
        frontmatter: None,
    },
    SourceSpec {
        source: ".claude/CLAUDE.md",
        kind: SourceKind::ClaudeMd,
        sections: true,
        frontmatter: None,
    },
    SourceSpec {
        source: ".cursorrules",
        kind: SourceKind::Cursor,
        sections: false,
        frontmatter: None,
    },
    SourceSpec {
        source: ".github/copilot-instructions.md",
        kind: SourceKind::Copilot,
        sections: false,
        frontmatter: None,
    },
];

pub const DIRECTORIES: [DirectorySpec; 4] = [
    DirectorySpec {
        directory: ".cursor/rules",
        suffix: "mdc",
        kind: SourceKind::Cursor,
        frontmatter: FrontmatterKind::Cursor,
    },
    DirectorySpec {
        directory: ".continue/rules",
        suffix: "md",
        kind: SourceKind::Continue,
        frontmatter: FrontmatterKind::Continue,
    },
    DirectorySpec {
        directory: ".github/instructions",
        suffix: "instructions.md",
        kind: SourceKind::Copilot,
        frontmatter: FrontmatterKind::Copilot,
    },
    DirectorySpec {
        directory: ".claude/rules",
        suffix: "md",
        kind: SourceKind::Claude,
        frontmatter: FrontmatterKind::Claude,
    },
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub source: String,
    pub kind: SourceKind,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub rule: RuleOptions,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Skipped {
    pub source: String,
    pub title: String,
    /// `empty`, `too_long`, `unreadable`, `malformed_frontmatter`,
    /// `unsupported_metadata`, `unsupported_applicability`, `duplicate` or
    /// `budget_exhausted`.
    pub reason: &'static str,
    #[serde(default)]
    pub detail: Option<String>,
}

pub fn parse(source: &str, spec: SourceSpec, text: &str, scan: &mut Scan) {
    if spec.sections {
        markdown::sections(source, spec.kind, text, scan);
    } else {
        frontmatter::single(source, spec, text, scan);
    }
}
