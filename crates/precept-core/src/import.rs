//! Import from AGENTS.md: the instructions a repository already writes down
//! for agents, one candidate suggestion per section. Pure: the daemon reads the
//! file; this only splits its text. Nothing here decides anything: a candidate
//! becomes a suggestion only when the operator picks it, and a rule only when
//! the operator confirms it. (`.precept/rules` is Precept's own output, never
//! an import source.)
use crate::operator::RuleOptions;
use serde::Serialize;

/// The file an import reads, relative to the project root.
pub const FILE: &str = "AGENTS.md";
/// The longest value a candidate may carry (the writer's bound).
pub const MAX_VALUE: usize = 4096;
/// The most candidates one scan returns.
pub const MAX_CANDIDATES: usize = 200;

/// One section of AGENTS.md, as a project convention to suggest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Candidate {
    /// The file, relative to the project root.
    pub source: String,
    /// The section heading it came from.
    pub title: String,
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub rule: RuleOptions,
}

/// A part of a file that was not turned into a candidate, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Skipped {
    pub source: String,
    pub title: String,
    /// `too_long`, `empty` or `unreadable`.
    pub reason: &'static str,
}

/// A key from a title: lowercase ASCII letters, digits and hyphens.
fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out.truncate(80);
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Removes Precept's own block, so it is never imported back.
fn without_pointer(text: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("<!-- precept:begin") {
            inside = true;
            continue;
        }
        if inside {
            if t == crate::pointer::END {
                inside = false;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Adds `c` (its value trimmed, its key made unique in the scan), or
/// records why it was skipped.
fn push(out: &mut (Vec<Candidate>, Vec<Skipped>), mut c: Candidate) {
    c.value = c.value.trim().to_owned();
    // Tags as the writer stores them: normalized, sorted, one to ten.
    c.tags = crate::operator::canonical_tags(&c.tags).unwrap_or_else(|| vec!["imported".into()]);
    let skip = |reason| Skipped {
        source: c.source.clone(),
        title: c.title.clone(),
        reason,
    };
    if c.value.is_empty() {
        out.1.push(skip("empty"));
    } else if c.value.len() > MAX_VALUE {
        out.1.push(skip("too_long"));
    } else if out.0.len() < MAX_CANDIDATES {
        let base = c.key.clone();
        let mut n = 2;
        while out.0.iter().any(|o| o.key == c.key) {
            c.key = format!("{base}-{n}");
            n += 1;
        }
        out.0.push(c);
    }
}

/// One candidate per `##` section (or per `###` inside a section that has
/// them, keyed `section-subsection`). Text before the first `##` is the
/// file's introduction and is not a rule. Fenced code stays with its section.
pub fn agents_md(source: &str, text: &str, out: &mut (Vec<Candidate>, Vec<Skipped>)) {
    let tag = "agents-md".to_owned();
    let prefix = "agents";
    let text = without_pointer(text);
    // (level, title, body)
    let mut sections: Vec<(usize, String, String)> = Vec::new();
    let mut fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
        }
        let level = if fence {
            0
        } else if line.starts_with("### ") {
            3
        } else if line.starts_with("## ") {
            2
        } else {
            0
        };
        if level > 0 {
            sections.push((level, line[level + 1..].trim().to_owned(), String::new()));
        } else if let Some(last) = sections.last_mut() {
            last.2.push_str(line);
            last.2.push('\n');
        }
    }
    let mut parent = String::new();
    for (index, (level, title, body)) in sections.iter().enumerate() {
        let part = |t: &str| {
            let s = slug(t);
            if s.is_empty() {
                format!("section-{}", index + 1)
            } else {
                s
            }
        };
        let (full, key) = if *level == 2 {
            parent = title.clone();
            (title.clone(), format!("{prefix}-{}", part(title)))
        } else {
            (
                format!("{parent} {title}"),
                format!("{prefix}-{}-{}", part(&parent), part(title)),
            )
        };
        let mut key = key;
        key.truncate(110);
        // A `##` whose own text is empty and that has `###` parts is only a
        // heading for them.
        let has_children = *level == 2
            && sections
                .get(index + 1)
                .is_some_and(|(next, _, _)| *next == 3);
        if has_children && body.trim().is_empty() {
            continue;
        }
        push(
            out,
            Candidate {
                source: source.into(),
                title: full.to_owned(),
                key,
                value: body.to_owned(),
                tags: vec!["imported".into(), tag.clone()],
                rule: RuleOptions::default(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str, text: &str) -> (Vec<Candidate>, Vec<Skipped>) {
        let mut out = Default::default();
        agents_md(source, text, &mut out);
        out
    }

    #[test]
    fn instructions_split_by_section_and_skip_precepts_block() {
        let text = "# AGENTS.md — Demo\n\nIntro text.\n\n<!-- precept:begin v1 -->\n## Precept\nCall the tool.\n<!-- precept:end -->\n\n## Repository records\n\nWrite records in English.\n\n```text\n## not a heading\n```\n\n## Engineering\n\n### Validation\n\nRun the gates.\n\n### 安全\n\n不要记录密钥。\n\n## Empty\n\n";
        let (c, s) = run("AGENTS.md", text);
        let keys: Vec<&str> = c.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "agents-repository-records",
                "agents-engineering-validation",
                "agents-engineering-section-4"
            ]
        );
        assert!(c[0].value.contains("## not a heading"), "fences stay");
        assert!(!c.iter().any(|c| c.value.contains("Call the tool")));
        assert_eq!(c[0].tags, ["agents-md", "imported"]);
        assert_eq!(c[1].title, "Engineering Validation");
        assert_eq!(c[2].value, "不要记录密钥。");
        assert_eq!(
            s,
            [Skipped {
                source: "AGENTS.md".into(),
                title: "Empty".into(),
                reason: "empty"
            }]
        );
        let long = format!("## Big\n\n{}", "x".repeat(MAX_VALUE + 1));
        assert_eq!(run("AGENTS.md", &long).1[0].reason, "too_long");
    }
}
