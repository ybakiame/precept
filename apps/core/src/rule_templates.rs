//! Copy-only repository-rule starting points. Templates contain no project
//! data and never cause a repository write; the operator reviews and commits.
use crate::i18n::{Locale, Msg, tr};

/// One explicit, deterministic rule template.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleTemplate {
    Rust,
    Git,
    Security,
    Testing,
}

impl RuleTemplate {
    pub const ALL: [Self; 4] = [Self::Rust, Self::Git, Self::Security, Self::Testing];

    /// Stable id used in the target file name and test identity.
    pub fn id(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Git => "git",
            Self::Security => "security",
            Self::Testing => "testing",
        }
    }

    /// Localized visible name.
    pub fn label(self, locale: Locale) -> &'static str {
        tr(
            locale,
            match self {
                Self::Rust => Msg::TemplateRust,
                Self::Git => Msg::TemplateGit,
                Self::Security => Msg::TemplateSecurity,
                Self::Testing => Msg::TemplateTesting,
            },
        )
    }

    /// Localized one-line explanation of the starting point.
    pub fn description(self, locale: Locale) -> &'static str {
        tr(
            locale,
            match self {
                Self::Rust => Msg::TemplateRustHelp,
                Self::Git => Msg::TemplateGitHelp,
                Self::Security => Msg::TemplateSecurityHelp,
                Self::Testing => Msg::TemplateTestingHelp,
            },
        )
    }

    /// Exact repository-relative target shown before copying.
    pub fn path(self) -> String {
        format!(".precept/rules/{}.md", self.id())
    }

    /// Exact valid rule text placed on the clipboard.
    pub fn text(self) -> &'static str {
        match self {
            Self::Rust => include_str!("rule_templates/rust.md"),
            Self::Git => include_str!("rule_templates/git.md"),
            Self::Security => include_str!("rule_templates/security.md"),
            Self::Testing => include_str!("rule_templates/testing.md"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use precept_core::contracts;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn every_template_parses_and_core_lint_has_no_errors_or_warnings() {
        let root = std::env::temp_dir().join(format!(
            "precept-rule-template-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let rules = root.join(contracts::DIR);
        fs::create_dir_all(&rules).unwrap();
        for template in RuleTemplate::ALL {
            let parsed = contracts::parse(template.id(), template.text().as_bytes())
                .unwrap_or_else(|_| panic!("{} did not parse", template.id()));
            assert_eq!(parsed.id, template.id());
            assert_eq!(
                template.path(),
                format!(".precept/rules/{}.md", template.id())
            );
            fs::write(rules.join(format!("{}.md", template.id())), template.text()).unwrap();
        }
        let lint = contracts::Snapshot::read(&root).lint();
        assert_eq!(lint.summary.errors, 0, "{lint:?}");
        assert_eq!(lint.summary.warnings, 0, "{lint:?}");
        assert_eq!(lint.summary.files, RuleTemplate::ALL.len());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ids_and_paths_are_stable_and_localized_labels_are_nonempty() {
        let ids: Vec<_> = RuleTemplate::ALL
            .into_iter()
            .map(RuleTemplate::id)
            .collect();
        assert_eq!(ids, ["rust", "git", "security", "testing"]);
        for template in RuleTemplate::ALL {
            for locale in [Locale::En, Locale::ZhCn] {
                assert!(!template.label(locale).is_empty());
                assert!(!template.description(locale).is_empty());
            }
        }
    }
}
