use super::super::parser::{
    Check, Guard, GuardTool, MAX_BYTES, MAX_CHECKS, MAX_GUARDS, RuleFile, parse, relative_path,
    valid_id, valid_key,
};
use super::location::{Locations, Scope};
use super::position::{Position, Positions, diagnostic};
use super::report::{
    Diagnostic, LintFile, RuleFacts, Severity, display_file, sort, sort_and_bound,
};
use crate::operator::{AppliesTo, Enforcement, PRIORITY_MAX, PRIORITY_MIN, canonical_tags};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Front {
    key: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    allowed: Vec<String>,
    #[serde(default)]
    enforcement: Enforcement,
    #[serde(default)]
    priority: i64,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    applies_to: AppliesTo,
    #[serde(default)]
    check: Vec<Check>,
    #[serde(default)]
    guard: Vec<Guard>,
}

pub(super) struct Linted {
    pub(super) id: String,
    pub(super) file: LintFile,
    pub(super) rule: Option<RuleFile>,
    pub(super) facts: Option<RuleFacts>,
    pub(super) key_position: Option<Position>,
    pub(super) paths_position: Option<Position>,
}

impl Linted {
    pub(super) fn failure(id: &str, code: &'static str) -> Self {
        Self {
            id: id.to_owned(),
            file: LintFile {
                file: display_file(id),
                status: "issues".into(),
                diagnostics: vec![diagnostic(code, Severity::Error, Position::start(), None)],
            },
            rule: None,
            facts: None,
            key_position: None,
            paths_position: None,
        }
    }

    pub(super) fn new(id: &str, raw: &[u8]) -> Self {
        let mut diagnostics = vec![];
        if !valid_id(id) {
            diagnostics.push(diagnostic(
                "id.invalid",
                Severity::Error,
                Position::start(),
                None,
            ));
        }
        if raw.len() > MAX_BYTES {
            diagnostics.push(diagnostic(
                "file.too_large",
                Severity::Error,
                Position::start(),
                None,
            ));
            return Self::finished(id, diagnostics, None, None, None, None);
        }
        let Ok(source) = std::str::from_utf8(raw) else {
            diagnostics.push(diagnostic(
                "encoding.invalid",
                Severity::Error,
                Position::start(),
                None,
            ));
            return Self::finished(id, diagnostics, None, None, None, None);
        };
        // A leading BOM is accepted by the strict parser and has no width.
        let source = source.strip_prefix('\u{feff}').unwrap_or(source);
        let positions = Positions::new(source);
        let parts = match split_source(source) {
            Ok(parts) => parts,
            Err(failure) => {
                let (code, offset) = match failure {
                    SplitFailure::Opening => ("format.opening_marker", 0),
                    SplitFailure::Closing(offset) => ("format.closing_marker", offset),
                };
                diagnostics.push(diagnostic(
                    code,
                    Severity::Error,
                    positions.span(offset..offset),
                    None,
                ));
                return Self::finished(id, diagnostics, None, None, None, None);
            }
        };
        let locations = Locations::scan(parts.front);
        let mut key_position = locations.field(Scope::Root, "key").map(|span| {
            positions.span(parts.front_start + span.start..parts.front_start + span.end)
        });
        let paths_position = locations
            .field(Scope::AppliesTo, "paths")
            .or_else(|| locations.field(Scope::Root, "applies_to"))
            .map(|span| {
                positions.span(parts.front_start + span.start..parts.front_start + span.end)
            });

        let front = match toml::from_str::<Front>(parts.front) {
            Ok(front) => Some(front),
            Err(error) => {
                let span = error.span().unwrap_or(0..0);
                let start = span.start.min(parts.front.len());
                let end = span.end.clamp(start, parts.front.len());
                diagnostics.push(diagnostic(
                    "frontmatter.invalid",
                    Severity::Error,
                    positions.span(parts.front_start + start..parts.front_start + end),
                    None,
                ));
                None
            }
        };
        if let Some(front) = front.as_ref() {
            semantic_diagnostics(
                front,
                &locations,
                &positions,
                parts.front_start,
                &mut diagnostics,
            );
        }
        body_diagnostics(parts.body, parts.body_start, &positions, &mut diagnostics);
        if key_position.is_none() {
            key_position = Some(positions.span(parts.front_start..parts.front_start));
        }
        let facts = front.as_ref().and_then(|front| {
            (valid_id(id) && valid_key(&front.key)).then(|| RuleFacts {
                key: front.key.clone(),
                applies_to: front.applies_to.canonical(),
                priority: (PRIORITY_MIN..=PRIORITY_MAX)
                    .contains(&front.priority)
                    .then_some(front.priority),
            })
        });
        let rule = parse(id, raw).ok();
        Self::finished(id, diagnostics, rule, facts, key_position, paths_position)
    }

    fn finished(
        id: &str,
        mut diagnostics: Vec<Diagnostic>,
        rule: Option<RuleFile>,
        facts: Option<RuleFacts>,
        key_position: Option<Position>,
        paths_position: Option<Position>,
    ) -> Self {
        sort(&mut diagnostics);
        Self {
            id: id.to_owned(),
            file: LintFile {
                file: display_file(id),
                status: if diagnostics.is_empty() {
                    "ok"
                } else {
                    "issues"
                }
                .into(),
                diagnostics,
            },
            rule,
            facts,
            key_position,
            paths_position,
        }
    }

    pub(super) fn diagnostics(mut self) -> Vec<Diagnostic> {
        sort_and_bound(&mut self.file.diagnostics);
        self.file.diagnostics
    }

    pub(super) fn push_at(
        &mut self,
        code: &'static str,
        severity: Severity,
        position: Position,
        related_file: Option<String>,
    ) {
        self.file
            .diagnostics
            .push(diagnostic(code, severity, position, related_file));
    }
}

struct Parts<'a> {
    front: &'a str,
    body: &'a str,
    front_start: usize,
    body_start: usize,
}

enum SplitFailure {
    Opening,
    Closing(usize),
}

/// The opening and closing marker rules intentionally mirror the parser.
fn split_source(source: &str) -> Result<Parts<'_>, SplitFailure> {
    let opening = source
        .strip_prefix("+++\n")
        .or_else(|| source.strip_prefix("+++\r\n"))
        .ok_or(SplitFailure::Opening)?;
    let front_start = source.len() - opening.len();
    let mut offset = 0;
    for line in opening.split_inclusive('\n') {
        offset += line.len();
        if line.trim_end_matches(['\r', '\n']) == "+++" {
            return Ok(Parts {
                front: &opening[..offset - line.len()],
                body: &opening[offset..],
                front_start,
                body_start: front_start + offset,
            });
        }
    }
    Err(SplitFailure::Closing(front_start))
}

fn semantic_diagnostics(
    front: &Front,
    locations: &Locations,
    positions: &Positions,
    front_start: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let span = |scope, name| {
        locations
            .field(scope, name)
            .or_else(|| locations.table(scope))
            .or_else(|| match scope {
                Scope::AppliesTo => locations.field(Scope::Root, "applies_to"),
                Scope::Check(_) => locations.field(Scope::Root, "check"),
                Scope::Guard(_) => locations.field(Scope::Root, "guard"),
                _ => None,
            })
            .unwrap_or_default()
    };
    let push = |code, scope, name, diagnostics: &mut Vec<Diagnostic>| {
        let range = span(scope, name);
        diagnostics.push(diagnostic(
            code,
            Severity::Error,
            positions.span(front_start + range.start..front_start + range.end),
            None,
        ));
    };

    if !valid_key(&front.key) {
        push("key.invalid", Scope::Root, "key", diagnostics);
    }
    if front
        .title
        .as_deref()
        .is_some_and(|title| !valid_text(title, 200))
    {
        push("title.invalid", Scope::Root, "title", diagnostics);
    }
    if front.allowed.len() > 64 || front.allowed.iter().any(|value| !valid_text(value, 4096)) {
        push("allowed.invalid", Scope::Root, "allowed", diagnostics);
    }
    if !front.tags.is_empty() && canonical_tags(&front.tags).is_none() {
        push("tags.invalid", Scope::Root, "tags", diagnostics);
    }
    if front.applies_to.canonical().is_none() {
        let field = applies_issue(&front.applies_to).unwrap_or("applies_to");
        let scope = if field == "applies_to" {
            Scope::Root
        } else {
            Scope::AppliesTo
        };
        push("applies_to.invalid", scope, field, diagnostics);
    }
    if !(PRIORITY_MIN..=PRIORITY_MAX).contains(&front.priority) {
        push("priority.invalid", Scope::Root, "priority", diagnostics);
    }
    if front.check.len() > MAX_CHECKS {
        let range = locations
            .table(Scope::Check(MAX_CHECKS))
            .or_else(|| locations.last_table(Scope::Check(MAX_CHECKS)))
            .or_else(|| locations.field(Scope::Root, "check"))
            .unwrap_or_default();
        diagnostics.push(diagnostic(
            "check.invalid",
            Severity::Error,
            positions.span(front_start + range.start..front_start + range.end),
            None,
        ));
    }
    for (index, check) in front.check.iter().enumerate() {
        if let Some(field) = check_issue(check) {
            push("check.invalid", Scope::Check(index), field, diagnostics);
        }
    }
    if front.guard.len() > MAX_GUARDS {
        let range = locations
            .table(Scope::Guard(MAX_GUARDS))
            .or_else(|| locations.last_table(Scope::Guard(MAX_GUARDS)))
            .or_else(|| locations.field(Scope::Root, "guard"))
            .unwrap_or_default();
        diagnostics.push(diagnostic(
            "guard.invalid",
            Severity::Error,
            positions.span(front_start + range.start..front_start + range.end),
            None,
        ));
    }
    for (index, guard) in front.guard.iter().enumerate() {
        if let Some(field) = guard_issue(guard) {
            push("guard.invalid", Scope::Guard(index), field, diagnostics);
        }
    }
    if !front.guard.is_empty() && front.enforcement != Enforcement::Hard {
        let range = locations
            .table(Scope::Guard(0))
            .or_else(|| locations.field(Scope::Root, "guard"))
            .unwrap_or_default();
        diagnostics.push(diagnostic(
            "guard.requires_hard",
            Severity::Error,
            positions.span(front_start + range.start..front_start + range.end),
            None,
        ));
    }
    if front.enforcement == Enforcement::Verify && front.check.is_empty() {
        push(
            "enforcement.no_verification",
            Scope::Root,
            "enforcement",
            diagnostics,
        );
        if let Some(last) = diagnostics.last_mut() {
            last.severity = Severity::Warning;
        }
    }
}

fn body_diagnostics(
    body: &str,
    body_start: usize,
    positions: &Positions,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let normalized = body.trim().replace("\r\n", "\n");
    if normalized.is_empty() {
        diagnostics.push(diagnostic(
            "body.empty",
            Severity::Error,
            positions.span(body_start..body_start),
            None,
        ));
        return;
    }
    let trim_start = body.len() - body.trim_start().len();
    let trim_end = body.trim_end().len();
    for (offset, ch) in body[trim_start..trim_end].char_indices() {
        let crlf = ch == '\r'
            && body
                .as_bytes()
                .get(trim_start + offset + 1)
                .is_some_and(|next| *next == b'\n');
        if ch.is_control() && ch != '\n' && ch != '\t' && !crlf {
            let start = body_start + trim_start + offset;
            diagnostics.push(diagnostic(
                "body.control_characters",
                Severity::Error,
                positions.span(start..start + ch.len_utf8()),
                None,
            ));
            break;
        }
    }
}

fn valid_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

fn applies_issue(applies_to: &AppliesTo) -> Option<&'static str> {
    for (name, candidate) in [
        (
            "agents",
            AppliesTo {
                agents: applies_to.agents.clone(),
                ..AppliesTo::default()
            },
        ),
        (
            "languages",
            AppliesTo {
                languages: applies_to.languages.clone(),
                ..AppliesTo::default()
            },
        ),
        (
            "paths",
            AppliesTo {
                paths: applies_to.paths.clone(),
                ..AppliesTo::default()
            },
        ),
        (
            "task_tags",
            AppliesTo {
                task_tags: applies_to.task_tags.clone(),
                ..AppliesTo::default()
            },
        ),
    ] {
        if candidate.canonical().is_none() {
            return Some(name);
        }
    }
    None
}

fn valid_check(check: &Check) -> bool {
    let path_query = check.toml.as_deref().or(check.json.as_deref());
    let forms = [
        check.contains.is_some(),
        check.absent.is_some(),
        path_query.is_some(),
    ]
    .into_iter()
    .filter(|present| *present)
    .count();
    relative_path(&check.file)
        && !check.file.ends_with('/')
        && forms <= 1
        && !(check.toml.is_some() && check.json.is_some())
        && path_query.is_some() == check.equals.is_some()
        && path_query.is_none_or(|path| {
            valid_text(path, 256) && path.split('.').all(|part| !part.is_empty())
        })
        && [&check.contains, &check.absent]
            .into_iter()
            .flatten()
            .all(|value| valid_text(value, 1024))
}

fn check_issue(check: &Check) -> Option<&'static str> {
    if !relative_path(&check.file) || check.file.ends_with('/') {
        return Some("file");
    }
    for (name, value) in [("contains", &check.contains), ("absent", &check.absent)] {
        if value
            .as_deref()
            .is_some_and(|value| !valid_text(value, 1024))
        {
            return Some(name);
        }
    }
    if check.toml.is_some() && check.json.is_some() {
        return Some("toml");
    }
    let query = check.toml.as_deref().or(check.json.as_deref());
    if let Some(path) = query
        && (!valid_text(path, 256) || path.split('.').any(|part| part.is_empty()))
    {
        return Some(if check.toml.is_some() { "toml" } else { "json" });
    }
    if query.is_some() != check.equals.is_some() {
        return Some(if check.equals.is_some() {
            "equals"
        } else if check.toml.is_some() {
            "toml"
        } else {
            "json"
        });
    }
    let forms = [
        check.contains.is_some(),
        check.absent.is_some(),
        query.is_some(),
    ]
    .into_iter()
    .filter(|present| *present)
    .count();
    if forms > 1 {
        return Some(if check.contains.is_some() {
            "contains"
        } else {
            "absent"
        });
    }
    (!valid_check(check)).then_some("file")
}

fn valid_guard(guard: &Guard) -> bool {
    match &guard.tool {
        GuardTool::Shell => {
            guard.paths.is_empty()
                && guard
                    .deny
                    .as_deref()
                    .is_some_and(|deny| valid_text(deny, 512))
        }
        GuardTool::Write => {
            guard.deny.is_none()
                && !guard.paths.is_empty()
                && guard.paths.len() <= 32
                && guard.paths.iter().all(|path| relative_path(path))
        }
    }
}

fn guard_issue(guard: &Guard) -> Option<&'static str> {
    match &guard.tool {
        GuardTool::Shell => {
            if !guard.paths.is_empty() {
                Some("paths")
            } else if guard
                .deny
                .as_deref()
                .is_none_or(|deny| !valid_text(deny, 512))
            {
                Some("deny")
            } else {
                None
            }
        }
        GuardTool::Write => {
            if guard.deny.is_some() {
                Some("deny")
            } else if guard.paths.is_empty()
                || guard.paths.len() > 32
                || !guard.paths.iter().all(|path| relative_path(path))
            {
                Some("paths")
            } else {
                None
            }
        }
    }
    .filter(|_| !valid_guard(guard))
}
