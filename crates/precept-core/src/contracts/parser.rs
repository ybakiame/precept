//! One repository rule file: TOML front matter between `+++` lines, then the
//! rule's Markdown text. Parsing is strict (unknown fields refuse the file)
//! and bounded; nothing here touches the filesystem.
use crate::operator::{AppliesTo, Enforcement, PRIORITY_MAX, PRIORITY_MIN, canonical_tags};
use serde::Deserialize;
use serde_json::Value;

/// The repository directory that holds rule files (`<name>.md`).
pub const DIR: &str = ".precept/rules";
/// The most rule files read from [`DIR`].
pub const MAX_FILES: usize = 64;
/// The largest rule file, front matter included.
pub const MAX_BYTES: usize = 16_384;
/// The most checks and the most guards in one rule.
pub const MAX_CHECKS: usize = 16;
pub const MAX_GUARDS: usize = 16;

/// Sanitized refusal of one file; parser diagnostics never leave this boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidSource;

/// A check against a repository file. `file` alone: it exists. With
/// `contains` / `absent`: its text has (or lacks) that substring. With `toml`
/// or `json` (a dotted path) and `equals`: that value equals `equals`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub file: String,
    #[serde(default)]
    pub contains: Option<String>,
    #[serde(default)]
    pub absent: Option<String>,
    #[serde(default)]
    pub toml: Option<String>,
    #[serde(default)]
    pub json: Option<String>,
    #[serde(default, deserialize_with = "any_value")]
    pub equals: Option<Value>,
}
fn any_value<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}
/// What the pre-tool gate denies under a hard rule: a shell command containing
/// `deny`, or a file write to one of `paths` (one ending in `/` covers its
/// whole directory).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum GuardTool {
    Shell,
    Write,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Guard {
    pub tool: GuardTool,
    #[serde(default)]
    pub deny: Option<String>,
    #[serde(default)]
    pub paths: Vec<String>,
}

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

/// One parsed rule file. `id` is the file name without `.md`.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleFile {
    pub id: String,
    pub key: String,
    pub title: Option<String>,
    pub allowed: Vec<String>,
    pub enforcement: Enforcement,
    pub priority: i64,
    pub tags: Vec<String>,
    pub applies_to: AppliesTo,
    pub checks: Vec<Check>,
    pub guards: Vec<Guard>,
    pub body: String,
}

fn label(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && !s.chars().any(|c| c.is_control() || c.is_whitespace())
}
fn text(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control)
}
/// A rule key: 1–120 characters, no control characters, no surrounding space.
pub fn valid_key(s: &str) -> bool {
    !s.is_empty() && s.len() <= 120 && s.trim() == s && !s.chars().any(char::is_control)
}
/// A repository-relative file: no absolute path, `..`, `.`, empty part,
/// backslash, colon or glob character.
pub fn relative_path(s: &str) -> bool {
    let body = s.strip_suffix('/').unwrap_or(s);
    !body.is_empty()
        && s.len() <= 256
        && !s.contains(['\\', ':', '*', '?', '[', ']', '{', '}'])
        && !s.chars().any(char::is_control)
        && body.split('/').all(|p| !matches!(p, "" | "." | ".."))
}
/// A file name stem usable as a rule id: lowercase ASCII letters, digits,
/// `-`, `_` and `.`, not starting with `.`.
pub fn valid_id(s: &str) -> bool {
    label(s, 120)
        && !s.starts_with('.')
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_.".contains(&b))
}

impl Check {
    fn valid(&self) -> bool {
        let path_query = self.toml.as_deref().or(self.json.as_deref());
        let forms = [
            self.contains.is_some(),
            self.absent.is_some(),
            path_query.is_some(),
        ]
        .into_iter()
        .filter(|b| *b)
        .count();
        relative_path(&self.file)
            && !self.file.ends_with('/')
            && forms <= 1
            && !(self.toml.is_some() && self.json.is_some())
            && path_query.is_some() == self.equals.is_some()
            && path_query.is_none_or(|p| text(p, 256) && p.split('.').all(|s| !s.is_empty()))
            && [&self.contains, &self.absent]
                .into_iter()
                .flatten()
                .all(|s| text(s, 1024))
    }
}
impl Guard {
    fn valid(&self) -> bool {
        match self.tool {
            GuardTool::Shell => {
                self.paths.is_empty() && self.deny.as_deref().is_some_and(|d| text(d, 512))
            }
            GuardTool::Write => {
                self.deny.is_none()
                    && !self.paths.is_empty()
                    && self.paths.len() <= 32
                    && self.paths.iter().all(|p| relative_path(p))
            }
        }
    }
}

/// Parse one rule file named `<id>.md`.
pub fn parse(id: &str, raw: &[u8]) -> Result<RuleFile, InvalidSource> {
    if raw.len() > MAX_BYTES || !valid_id(id) {
        return Err(InvalidSource);
    }
    let source = std::str::from_utf8(raw).map_err(|_| InvalidSource)?;
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let rest = source
        .strip_prefix("+++\n")
        .or_else(|| source.strip_prefix("+++\r\n"))
        .ok_or(InvalidSource)?;
    let mut front = String::new();
    let mut body = None;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        offset += line.len();
        if line.trim_end_matches(['\r', '\n']) == "+++" {
            body = Some(&rest[offset..]);
            break;
        }
        front.push_str(line);
    }
    let body = body.ok_or(InvalidSource)?.trim().replace("\r\n", "\n");
    let f: Front = toml::from_str(&front).map_err(|_| InvalidSource)?;
    let tags = if f.tags.is_empty() {
        vec![]
    } else {
        canonical_tags(&f.tags).ok_or(InvalidSource)?
    };
    let applies_to = f.applies_to.canonical().ok_or(InvalidSource)?;
    let valid = valid_key(&f.key)
        && f.title.as_deref().is_none_or(|t| text(t, 200))
        && f.allowed.len() <= 64
        && f.allowed.iter().all(|v| text(v, 4096))
        && (PRIORITY_MIN..=PRIORITY_MAX).contains(&f.priority)
        && f.check.len() <= MAX_CHECKS
        && f.check.iter().all(Check::valid)
        && f.guard.len() <= MAX_GUARDS
        && f.guard.iter().all(Guard::valid)
        && (f.guard.is_empty() || f.enforcement == Enforcement::Hard)
        && !body.is_empty()
        && !body
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t');
    if !valid {
        return Err(InvalidSource);
    }
    Ok(RuleFile {
        id: id.to_owned(),
        key: f.key,
        title: f.title,
        allowed: f.allowed,
        enforcement: f.enforcement,
        priority: f.priority,
        tags,
        applies_to,
        checks: f.check,
        guards: f.guard,
        body,
    })
}

/// The text of a rule file: its front matter (only what differs from the
/// defaults) and its Markdown below. Parsing the result yields the same rule.
pub fn render(r: &RuleFile) -> String {
    fn quote(s: &str) -> String {
        toml::Value::String(s.to_owned()).to_string()
    }
    fn list(v: &[String]) -> String {
        let items: Vec<String> = v.iter().map(|s| quote(s)).collect();
        format!("[{}]", items.join(", "))
    }
    let mut out = String::from("+++\n");
    out.push_str(&format!("key = {}\n", quote(&r.key)));
    if let Some(title) = &r.title {
        out.push_str(&format!("title = {}\n", quote(title)));
    }
    if !r.allowed.is_empty() {
        out.push_str(&format!("allowed = {}\n", list(&r.allowed)));
    }
    if !r.tags.is_empty() {
        out.push_str(&format!("tags = {}\n", list(&r.tags)));
    }
    if r.priority != 0 {
        out.push_str(&format!("priority = {}\n", r.priority));
    }
    if r.enforcement != Enforcement::Advisory {
        out.push_str(&format!("enforcement = \"{}\"\n", r.enforcement.as_str()));
    }
    if !r.applies_to.is_empty() {
        out.push_str("\n[applies_to]\n");
        for (name, v) in [
            ("agents", &r.applies_to.agents),
            ("languages", &r.applies_to.languages),
            ("paths", &r.applies_to.paths),
            ("task_tags", &r.applies_to.task_tags),
        ] {
            if !v.is_empty() {
                out.push_str(&format!("{name} = {}\n", list(v)));
            }
        }
    }
    for c in &r.checks {
        out.push_str(&format!("\n[[check]]\nfile = {}\n", quote(&c.file)));
        for (name, v) in [
            ("contains", &c.contains),
            ("absent", &c.absent),
            ("toml", &c.toml),
            ("json", &c.json),
        ] {
            if let Some(v) = v {
                out.push_str(&format!("{name} = {}\n", quote(v)));
            }
        }
        if let Some(equals) = &c.equals
            && let Ok(v) = toml::Value::try_from(equals)
        {
            out.push_str(&format!("equals = {v}\n"));
        }
    }
    for g in &r.guards {
        let tool = match g.tool {
            GuardTool::Shell => "shell",
            GuardTool::Write => "write",
        };
        out.push_str(&format!("\n[[guard]]\ntool = \"{tool}\"\n"));
        if let Some(deny) = &g.deny {
            out.push_str(&format!("deny = {}\n", quote(deny)));
        }
        if !g.paths.is_empty() {
            out.push_str(&format!("paths = {}\n", list(&g.paths)));
        }
    }
    out.push_str("+++\n\n");
    out.push_str(r.body.trim());
    out.push('\n');
    out
}

/// A rule text's own title: its first line when that is a Markdown heading
/// (at most 200 characters), else none.
pub fn heading_title(body: &str) -> Option<String> {
    let first = body.lines().map(str::trim).find(|l| !l.is_empty())?;
    let hashes = first.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&hashes) || !first[hashes..].starts_with(' ') {
        return None;
    }
    let title = first[hashes..].trim();
    (!title.is_empty() && title.len() <= 200 && !title.chars().any(char::is_control))
        .then(|| title.to_owned())
}

/// A rule id (file stem) for `key`: lowercase, other characters as `-`.
pub fn id_for(key: &str) -> String {
    let mut id = String::new();
    for c in key.trim().chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.' {
            id.push(c);
        } else if !id.ends_with('-') {
            id.push('-');
        }
    }
    let id = id.trim_matches(['-', '.']).to_owned();
    let id: String = id.chars().take(100).collect();
    if valid_id(&id) { id } else { "rule".into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = "+++\nkey = \"formatter\"\ntitle = \"Rust formatting\"\nallowed = [\"rustfmt\"]\nenforcement = \"hard\"\ntags = [\"rust\"]\n[applies_to]\nlanguages = [\"Rust\"]\n[[check]]\nfile = \"rustfmt.toml\"\n[[check]]\nfile = \"Cargo.toml\"\ntoml = \"package.edition\"\nequals = \"2024\"\n[[guard]]\ntool = \"shell\"\ndeny = \"git push --force\"\n[[guard]]\ntool = \"write\"\npaths = [\"Cargo.lock\"]\n+++\nRun `cargo fmt`.\n";

    #[test]
    fn a_full_rule_parses_canonically() {
        let r = parse("formatter", FULL.as_bytes()).unwrap();
        assert_eq!(r.key, "formatter");
        assert_eq!(r.applies_to.languages, ["rust"]);
        assert_eq!(r.checks.len(), 2);
        assert_eq!(r.checks[1].equals, Some(Value::from("2024")));
        assert_eq!(r.guards[0].tool, GuardTool::Shell);
        assert_eq!(r.body, "Run `cargo fmt`.");
        assert!(parse("formatter", FULL.replace('\n', "\r\n").as_bytes()).is_ok());
    }

    #[test]
    fn malformed_files_are_refused() {
        for bad in [
            "key = \"x\"\n+++\nbody",
            "+++\nkey = \"x\"\n",
            "+++\nkey = \"x\"\nunknown = 1\n+++\nbody",
            "+++\nkey = \" padded\"\n+++\nbody",
            "+++\nkey = \"x\"\n+++\n   ",
            "+++\nkey = \"x\"\n[[check]]\nfile = \"../etc/passwd\"\n+++\nbody",
            "+++\nkey = \"x\"\n[[check]]\nfile = \"/abs\"\n+++\nbody",
            "+++\nkey = \"x\"\n[[check]]\nfile = \"a\"\ncontains = \"b\"\nabsent = \"c\"\n+++\nbody",
            "+++\nkey = \"x\"\n[[check]]\nfile = \"a\"\ntoml = \"b\"\n+++\nbody",
            "+++\nkey = \"x\"\n[[guard]]\ntool = \"shell\"\ndeny = \"rm\"\n+++\nbody",
            "+++\nkey = \"x\"\nenforcement = \"hard\"\n[[guard]]\ntool = \"write\"\n+++\nbody",
            "+++\nkey = \"x\"\npriority = 1000\n+++\nbody",
        ] {
            assert_eq!(parse("x", bad.as_bytes()), Err(InvalidSource), "{bad}");
        }
        assert_eq!(parse("Bad Name", FULL.as_bytes()), Err(InvalidSource));
        let big = format!("+++\nkey = \"x\"\n+++\n{}", "a".repeat(MAX_BYTES));
        assert_eq!(parse("x", big.as_bytes()), Err(InvalidSource));
    }

    #[test]
    fn rendered_rules_parse_back() {
        let applies = AppliesTo {
            languages: vec!["rust".into()],
            paths: vec!["src/".into()],
            ..Default::default()
        };
        let full = parse("formatter", FULL.as_bytes()).unwrap();
        assert_eq!(parse("formatter", render(&full).as_bytes()).unwrap(), full);
        let simple = RuleFile {
            id: id_for("commit \"style\""),
            key: "commit \"style\"".into(),
            title: heading_title("# Small commits\n\nOne change each."),
            allowed: vec![],
            enforcement: Enforcement::Verify,
            priority: 5,
            tags: vec!["git".into(), "style".into()],
            applies_to: applies,
            checks: vec![],
            guards: vec![],
            body: "# Small commits\n\nOne change each.".into(),
        };
        let raw = render(&simple);
        assert!(raw.contains("title = \"Small commits\"\n"));
        assert_eq!(parse(&simple.id, raw.as_bytes()).unwrap(), simple);
        assert_eq!(id_for("Commit Style!"), "commit-style");
        assert_eq!(id_for("???"), "rule");
        assert_eq!(heading_title("Plain text"), None);
        assert_eq!(heading_title("#nospace"), None);
    }
}
