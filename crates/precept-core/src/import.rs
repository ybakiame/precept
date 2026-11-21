//! Importers: rules an existing repository already writes down for agents
//! (AGENTS.md and its relatives, Cursor rules, Agent Control rules), turned
//! into candidate suggestions. Pure: the daemon reads the files; this only
//! splits and maps their text. Nothing here decides anything: a candidate
//! becomes a suggestion only when the operator picks it, and a rule only when
//! the operator confirms it.
use crate::operator::{
    AppliesTo, Enforcement, PRIORITY_MAX, PRIORITY_MIN, RuleOptions, normalize_tag,
};
use serde::Serialize;

/// The longest value a candidate may carry (the writer's bound).
pub const MAX_VALUE: usize = 4096;
/// The most candidates one scan returns.
pub const MAX_CANDIDATES: usize = 200;

/// Which kind of file a candidate came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    /// Markdown instructions for agents: one candidate per `##`/`###` section.
    Instructions,
    /// A Cursor rule (`.mdc`): one candidate per file.
    CursorRule,
    /// An Agent Control rule (front matter + body): one candidate per file.
    AgentControl,
}

/// The files a scan reads, relative to the project root: exact files, then
/// directories whose files of one extension are read.
pub const FILES: [(&str, Format); 4] = [
    ("AGENTS.md", Format::Instructions),
    ("CLAUDE.md", Format::Instructions),
    ("GEMINI.md", Format::Instructions),
    (".github/copilot-instructions.md", Format::Instructions),
];
pub const DIRECTORIES: [(&str, &str, Format); 2] = [
    (".cursor/rules", "mdc", Format::CursorRule),
    (".agent-control/rules", "md", Format::AgentControl),
];

/// Where an imported rule belongs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// This project's conventions.
    Project,
    /// The developer's personal preferences (an Agent Control `global` rule).
    Personal,
}

/// One rule found in a file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Candidate {
    /// The file, relative to the project root.
    pub source: String,
    /// The section heading or rule title it came from.
    pub title: String,
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub rule: RuleOptions,
    pub target: Target,
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

/// The file's own tag: `agents-md`, `claude-md`, `cursor-rules`, ...
fn file_tag(source: &str, format: Format) -> String {
    match format {
        Format::CursorRule => "cursor-rules".into(),
        Format::AgentControl => "agent-control".into(),
        Format::Instructions => {
            let name = source.rsplit('/').next().unwrap_or(source);
            normalize_tag(&name.replace('.', "-")).unwrap_or_else(|| "instructions".into())
        }
    }
}

/// `prefix-key`, bounded to the writer's 120 characters.
fn key_for(prefix: &str, title: &str, index: usize) -> String {
    let base = slug(title);
    let base = if base.is_empty() {
        format!("section-{}", index + 1)
    } else {
        base
    };
    let mut key = format!("{prefix}-{base}");
    key.truncate(120);
    key
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
    // Tags as the writer stores them: normalized, sorted, one to five.
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
pub fn instructions(source: &str, text: &str, out: &mut (Vec<Candidate>, Vec<Skipped>)) {
    let tag = file_tag(source, Format::Instructions);
    let prefix = tag.trim_end_matches("-md").to_owned();
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
                target: Target::Project,
            },
        );
    }
}

/// Splits `---` front matter from the body.
fn front_matter(text: &str) -> (Vec<&str>, &str) {
    let Some(rest) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return (Vec::new(), text);
    };
    let Some(end) = rest.find("\n---") else {
        return (Vec::new(), text);
    };
    let head = rest[..end].lines().collect();
    let body = rest[end + 4..].trim_start_matches(['\r', '\n']);
    (head, body)
}

/// A scalar YAML value: unquoted, `'single'` ('' escapes) or `"double"`.
fn scalar(raw: &str) -> String {
    let raw = raw.trim();
    if let Some(inner) = raw.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        inner.replace("''", "'")
    } else if let Some(inner) = raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        inner.replace("\\\"", "\"")
    } else {
        raw.to_owned()
    }
}

/// The top-level `key: value` pairs and `key:` lists of simple front matter
/// (what Agent Control and Cursor write); nested maps keep their indented
/// `child: [..]`/`- item` lines under `parent.child`.
fn fields(head: &[&str]) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let mut parent = String::new();
    for line in head {
        let indent = line.len() - line.trim_start().len();
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some(item) = t.strip_prefix("- ") {
            if let Some(last) = out.last_mut() {
                last.1.push(scalar(item));
            }
            continue;
        }
        let Some((k, v)) = t.split_once(':') else {
            continue;
        };
        let name = if indent == 0 {
            parent = k.trim().to_owned();
            parent.clone()
        } else {
            format!("{parent}.{}", k.trim())
        };
        let v = v.trim();
        let values = if let Some(list) = v.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            list.split(',')
                .map(scalar)
                .filter(|s| !s.is_empty())
                .collect()
        } else if v.is_empty() || v == "{}" {
            Vec::new()
        } else {
            vec![scalar(v)]
        };
        out.push((name, values));
    }
    out
}

fn field<'a>(f: &'a [(String, Vec<String>)], name: &str) -> &'a [String] {
    f.iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_slice())
        .unwrap_or(&[])
}

/// Up to five canonical tags: `imported`, the file's tag, then its own.
fn tags(base: &str, own: &[String]) -> Vec<String> {
    let mut out = vec!["imported".to_owned(), base.to_owned()];
    for t in own.iter().filter_map(|t| normalize_tag(t)) {
        if out.len() >= crate::operator::MAX_TAGS {
            break;
        }
        if !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// An Agent Control rule: its id as key, title then body as value, and its
/// tags, priority, enforcement, scope and `applies_to` where Precept has them.
pub fn agent_control(source: &str, text: &str, out: &mut (Vec<Candidate>, Vec<Skipped>)) {
    let (head, body) = front_matter(text);
    let f = fields(&head);
    let first = |name: &str| field(&f, name).first().cloned().unwrap_or_default();
    let stem = source
        .rsplit('/')
        .next()
        .unwrap_or(source)
        .trim_end_matches(".md");
    let title = first("title");
    let id = first("id");
    let key = {
        let k = slug(if id.is_empty() { stem } else { &id });
        if k.is_empty() {
            key_for("agent-control", &title, 0)
        } else {
            k
        }
    };
    let content = first("content");
    let mut value = String::new();
    if !title.is_empty() {
        value.push_str(&format!("**{title}**\n\n"));
    }
    value.push_str(if body.trim().is_empty() {
        &content
    } else {
        body
    });
    let applies = |name: &str| field(&f, &format!("applies_to.{name}")).to_vec();
    let applies_to = AppliesTo {
        agents: applies("agents"),
        languages: applies("languages"),
        paths: applies("paths"),
        task_tags: applies("task_tags"),
    }
    .canonical()
    .unwrap_or_default();
    let priority = first("priority")
        .parse::<i64>()
        .map(|p| p.clamp(PRIORITY_MIN, PRIORITY_MAX))
        .unwrap_or(0);
    let enforcement = Enforcement::parse(&first("enforcement")).unwrap_or_default();
    let target = if first("scope") == "global" {
        Target::Personal
    } else {
        Target::Project
    };
    let title = if title.is_empty() {
        stem.to_owned()
    } else {
        title
    };
    push(
        out,
        Candidate {
            source: source.into(),
            title,
            key,
            value,
            tags: tags("agent-control", field(&f, "tags")),
            rule: RuleOptions {
                applies_to,
                priority,
                enforcement,
            },
            target,
        },
    );
}

/// A Cursor rule: its file name as key, `description` then body as value.
/// Its `globs` are glob patterns Precept's paths do not take, so they are
/// kept as words in the value instead of narrowing the rule.
pub fn cursor_rule(source: &str, text: &str, out: &mut (Vec<Candidate>, Vec<Skipped>)) {
    let (head, body) = front_matter(text);
    let f = fields(&head);
    let stem = source
        .rsplit('/')
        .next()
        .unwrap_or(source)
        .trim_end_matches(".mdc");
    let description = field(&f, "description")
        .first()
        .cloned()
        .unwrap_or_default();
    let globs = field(&f, "globs").join(", ");
    let mut value = String::new();
    if !description.is_empty() {
        value.push_str(&format!("**{description}**\n\n"));
    }
    if !globs.is_empty() {
        value.push_str(&format!("Applies to files matching: {globs}\n\n"));
    }
    value.push_str(body);
    push(
        out,
        Candidate {
            source: source.into(),
            title: stem.to_owned(),
            key: key_for("cursor", stem, 0),
            value,
            tags: vec!["imported".into(), "cursor-rules".into()],
            rule: RuleOptions::default(),
            target: Target::Project,
        },
    );
}

/// Candidates from one file's text.
pub fn parse(source: &str, format: Format, text: &str, out: &mut (Vec<Candidate>, Vec<Skipped>)) {
    match format {
        Format::Instructions => instructions(source, text, out),
        Format::CursorRule => cursor_rule(source, text, out),
        Format::AgentControl => agent_control(source, text, out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str, format: Format, text: &str) -> (Vec<Candidate>, Vec<Skipped>) {
        let mut out = Default::default();
        parse(source, format, text, &mut out);
        out
    }

    #[test]
    fn instructions_split_by_section_and_skip_precepts_block() {
        let text = "# AGENTS.md — Demo\n\nIntro text.\n\n<!-- precept:begin v1 -->\n## Precept\nCall the tool.\n<!-- precept:end -->\n\n## Repository records\n\nWrite records in English.\n\n```text\n## not a heading\n```\n\n## Engineering\n\n### Validation\n\nRun the gates.\n\n### 安全\n\n不要记录密钥。\n\n## Empty\n\n";
        let (c, s) = run("AGENTS.md", Format::Instructions, text);
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
        assert_eq!(
            run("CLAUDE.md", Format::Instructions, &long).1[0].reason,
            "too_long"
        );
    }

    #[test]
    fn agent_control_rules_keep_their_options() {
        let text = "---\nid: ac-definition-of-done\nversion: 2\ntitle: 'Definition of done: fmt -> clippy'\ncontent: ''\nkind: contract\nscope: project\nsource:\n  kind: project\n  author: someone\npriority: 100\napplies_to:\n  languages: [Rust]\n  paths:\n  - crates/\ntags:\n- verification\n- gate\n- mise\n- Extra Tag\nenabled: true\nenforcement: verify\n---\nRun the full gate.\n";
        let (c, _) = run(
            ".agent-control/rules/ac-definition-of-done.md",
            Format::AgentControl,
            text,
        );
        let c = &c[0];
        assert_eq!(c.key, "ac-definition-of-done");
        assert_eq!(
            c.value,
            "**Definition of done: fmt -> clippy**\n\nRun the full gate."
        );
        assert_eq!(
            c.tags,
            ["agent-control", "gate", "imported", "mise", "verification"]
        );
        assert_eq!(c.rule.priority, 100);
        assert_eq!(c.rule.enforcement, Enforcement::Verify);
        assert_eq!(c.rule.applies_to.languages, ["rust"]);
        assert_eq!(c.rule.applies_to.paths, ["crates/"]);
        assert_eq!(c.target, Target::Project);
        let global = text.replace("scope: project", "scope: global");
        assert_eq!(
            run("r.md", Format::AgentControl, &global).0[0].target,
            Target::Personal
        );
    }

    #[test]
    fn cursor_rules_keep_globs_as_words() {
        let text = "---\ndescription: TypeScript style\nglobs: src/**/*.ts\nalwaysApply: false\n---\nUse strict mode.\n";
        let (c, _) = run(".cursor/rules/ts-style.mdc", Format::CursorRule, text);
        assert_eq!(c[0].key, "cursor-ts-style");
        assert!(
            c[0].value
                .contains("Applies to files matching: src/**/*.ts")
        );
        assert!(
            c[0].value.ends_with("Use strict mode.\n") || c[0].value.ends_with("Use strict mode.")
        );
        assert!(c[0].rule.applies_to.is_empty());
    }
}
