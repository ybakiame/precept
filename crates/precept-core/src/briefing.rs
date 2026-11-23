//! The session-start briefing: the effective context of one project rendered
//! as short Markdown an agent reads before it works. Built only from the
//! projected context (repository rules, confirmed preferences, warnings),
//! so it carries nothing an agent could not read through MCP. Bounded; a cut
//! briefing says so and points to the MCP tool for the rest.
use serde_json::Value;

/// The longest briefing, in bytes.
pub const MAX_BYTES: usize = 12 * 1024;
/// The most the project-memory section adds, in bytes, on top of
/// [`MAX_BYTES`]: its own budget, so notes never displace rules.
pub const MEMORY_BYTES: usize = 3 * 1024;
/// The longest preference value shown before it is cut.
const MAX_VALUE: usize = 1200;

fn cut(s: &str, max: usize) -> (&str, bool) {
    if s.len() <= max {
        return (s, false);
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    (&s[..end], true)
}

/// Why the briefing is shown: a new session, or a session that resumed after
/// the agent compacted its conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moment {
    Start,
    Compact,
}

/// Where a rule applies, in words (`None` for everywhere).
fn applies(p: &Value) -> Option<String> {
    let a = p.get("applies_to")?;
    let parts: Vec<String> = [
        ("agents", "agents"),
        ("languages", "languages"),
        ("paths", "paths"),
        ("task_tags", "tasks tagged"),
    ]
    .iter()
    .filter_map(|(field, word)| {
        let v: Vec<&str> = a
            .get(*field)?
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .collect();
        (!v.is_empty()).then(|| format!("{word} {}", v.join(", ")))
    })
    .collect();
    (!parts.is_empty()).then(|| parts.join("; "))
}

/// `YYYY-MM-DD HH:MM UTC` of a Unix millisecond time.
fn when(ms: u64) -> String {
    let days = (ms / 86_400_000) as i64;
    // Civil date from days since the epoch (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        (ms / 3_600_000) % 24,
        (ms / 60_000) % 60
    )
}

/// The project's work sessions (agents' own notes, not rules): where the
/// work stands, so it continues across agents and compaction.
fn work(context: &Value) -> String {
    let sessions = context
        .get("work_sessions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if sessions.is_empty() {
        return String::new();
    }
    let list = |s: &Value, field: &str| -> Vec<String> {
        s.get(field)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|i| i.get("text").and_then(Value::as_str))
            .map(|t| cut(t, 200).0.to_owned())
            .collect()
    };
    let mut out = String::from(
        "\n## Work in progress\n\nAgents' own notes on this project's work (not rules). Continue an open one where it fits, and keep yours current with `update_work_state`.\n",
    );
    for s in &sessions {
        let open = s.get("status").and_then(Value::as_str) == Some("open");
        let goal = cut(s.get("goal").and_then(Value::as_str).unwrap_or(""), 300).0;
        let agent = s.get("agent").and_then(Value::as_str).unwrap_or("an agent");
        let updated = s.get("updated_ms").and_then(Value::as_u64).unwrap_or(0);
        out.push_str(&format!(
            "\n- **{goal}** ({}, {agent}, {}; session `{}`)\n",
            if open { "open" } else { "closed" },
            when(updated),
            s.get("id").and_then(Value::as_str).unwrap_or("?")
        ));
        for (field, label) in [("doing", "Doing"), ("todo", "Next"), ("done", "Done")] {
            let items = list(s, field);
            if !items.is_empty() {
                out.push_str(&format!("  - {label}: {}\n", items.join("; ")));
            }
        }
        if let Some(sum) = s.get("summary").and_then(Value::as_str) {
            out.push_str(&format!("  - Summary: {}\n", cut(sum, 400).0));
        }
    }
    out
}

/// The rules the pre-tool gate denied in the last week, most frequent first:
/// what earlier agents were blocked from, so this one does not repeat it.
fn blocked(context: &Value) -> String {
    let denials = context
        .get("gate_denials")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if denials.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n## Recently blocked\n\nThe pre-tool gate denied these committed hard rules in the last 7 days. Follow them from the start instead of retrying variations; ask the developer if a rule is in the way.\n\n",
    );
    for d in &denials {
        let rule = cut(d.get("rule").and_then(Value::as_str).unwrap_or("?"), 120).0;
        let count = d.get("count").and_then(Value::as_u64).unwrap_or(0);
        let last = d.get("last_ms").and_then(Value::as_u64).unwrap_or(0);
        out.push_str(&format!(
            "- `{rule}`: {count} time{}, last {}\n",
            if count == 1 { "" } else { "s" },
            when(last)
        ));
    }
    out
}

/// The project's memory (agents' notes, unreviewed): counts by kind, every
/// environment note, then the heaviest others, within [`MEMORY_BYTES`].
fn memory(context: &Value) -> String {
    let Some(m) = context.get("memory") else {
        return String::new();
    };
    let count = |k: &str| {
        m.pointer(&format!("/counts/{k}"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let kinds = [
        ("environment", "Environment"),
        ("caution", "Cautions"),
        ("lesson", "Lessons"),
        ("attempt", "Attempts"),
    ];
    if kinds.iter().all(|(k, _)| count(k) == 0) {
        return String::new();
    }
    let counts: Vec<String> = kinds
        .iter()
        .map(|(k, label)| format!("{label} {}", count(k)))
        .collect();
    let mut out = format!(
        "\n## Project memory\n\nAgents' notes on this project, unreviewed: repository rules, confirmed preferences and the current code win. {}. Call `recall` for the rest, `reaffirm` a note that held, `remember` what you learn and `forget` what is no longer true.\n",
        counts.join(" · ")
    );
    let line = |e: &Value| {
        let one = |f: &str| e.get(f).and_then(Value::as_str).unwrap_or("");
        let mut flags = vec![];
        if e.get("pinned").and_then(Value::as_bool) == Some(true) {
            flags.push("pinned");
        }
        if e.get("unstable").and_then(Value::as_bool) == Some(true) {
            flags.push("changed often lately");
        }
        let flags = if flags.is_empty() {
            String::new()
        } else {
            format!(" _{}_", flags.join(", "))
        };
        format!(
            "- **{}**{flags} (`{}`): {}\n",
            cut(one("title"), 120).0,
            one("id"),
            cut(one("body"), 600).0.replace('\n', " ")
        )
    };
    for (field, heading) in [
        ("environment", "Environment"),
        ("relevant", "Most relevant"),
    ] {
        let entries = m
            .get(field)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if entries.is_empty() {
            continue;
        }
        let head = format!("\n### {heading}\n\n");
        if out.len() + head.len() > MEMORY_BYTES {
            break;
        }
        out.push_str(&head);
        for e in &entries {
            let l = if field == "relevant" {
                let kind = e.get("kind").and_then(Value::as_str).unwrap_or("note");
                line(e).replacen("- ", &format!("- [{kind}] "), 1)
            } else {
                line(e)
            };
            if out.len() + l.len() > MEMORY_BYTES {
                break;
            }
            out.push_str(&l);
        }
    }
    out
}

/// Markdown text placed under a `###` entry: its own headings move below
/// that level (`# A` becomes `#### A`), outside fenced code.
fn demote(text: &str) -> String {
    let mut fenced = false;
    text.lines()
        .map(|l| {
            if l.trim_start().starts_with("```") {
                fenced = !fenced;
            }
            let hashes = l.chars().take_while(|c| *c == '#').count();
            if !fenced && (1..=6).contains(&hashes) && l[hashes..].starts_with(' ') {
                format!("{}{l}", "#".repeat(3.min(6 - hashes)))
            } else {
                l.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A repository rule: its key, file, tags, declared enforcement and what its
/// checks found now, then its text.
fn rule_entry(r: &Value) -> String {
    let key = r.get("key").and_then(Value::as_str).unwrap_or("?");
    let file = r.get("file").and_then(Value::as_str).unwrap_or("?");
    let mut meta = vec![format!("repository · `{file}`")];
    let tags: Vec<String> = r
        .get("tags")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|t| format!("#{t}"))
        .collect();
    if !tags.is_empty() {
        meta.push(tags.join(" "));
    }
    if let Some(e) = r.get("enforcement").and_then(Value::as_str)
        && e != "advisory"
    {
        meta.push(e.to_owned());
    }
    let checks = r.get("checks");
    let total = checks
        .and_then(|c| c.get("total"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let passed = checks
        .and_then(|c| c.get("passed"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    match checks.and_then(|c| c.get("status")).and_then(Value::as_str) {
        Some("verified") => meta.push(format!("checks pass ({passed}/{total})")),
        Some("failing") => {
            let f = checks.and_then(|c| c.get("failure"));
            let file = f
                .and_then(|f| f.get("file"))
                .and_then(Value::as_str)
                .unwrap_or("?");
            let what = match f.and_then(|f| f.get("kind")).and_then(Value::as_str) {
                Some("exists") => "is missing".to_owned(),
                Some("contains") => "lacks the expected text".to_owned(),
                Some("absent") => "has text the rule excludes".to_owned(),
                _ => format!(
                    "`{}` differs",
                    f.and_then(|f| f.get("path"))
                        .and_then(Value::as_str)
                        .unwrap_or("?")
                ),
            };
            meta.push(format!(
                "check failing: `{file}` {what} ({passed}/{total} pass)"
            ));
        }
        Some("unavailable") => meta.push("checks could not run".into()),
        _ => {}
    }
    let mut entry = format!("\n### {key}\n\n_{}_\n\n", meta.join(" · "));
    if let Some(where_) = applies(r) {
        entry.push_str(&format!("Applies to {where_} only.\n\n"));
    }
    if let Some(allowed) = r.get("allowed_values").and_then(Value::as_array) {
        let v: Vec<&str> = allowed.iter().filter_map(Value::as_str).collect();
        entry.push_str(&format!("Allowed values: {}\n\n", v.join(" | ")));
    }
    let (text, cut_text) = cut(
        r.get("text").and_then(Value::as_str).unwrap_or("").trim(),
        MAX_VALUE,
    );
    entry.push_str(&demote(text));
    if cut_text {
        entry.push_str(" …");
    }
    entry.push('\n');
    entry
}

/// Renders `context` (the `context.get` projection) for `project`.
pub fn render(project: &str, context: &Value, moment: Moment) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Precept: context for {project}\n\n"));
    if moment == Moment::Compact {
        out.push_str(
            "Your conversation was compacted. These rules still apply; re-read them before continuing.\n\n",
        );
    }
    out.push_str(
        "Confirmed by the developer through Precept. Current repository files and \
         contracts outrank these preferences; if they disagree, say so instead of \
         choosing silently. Call the `get_effective_context` MCP tool for details, \
         selectors or documents.\n",
    );
    let mut truncated = false;
    let rules = context
        .get("repository_rules")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !rules.is_empty() {
        out.push_str("\n## Repository rules\n");
        for r in &rules {
            let entry = rule_entry(r);
            if out.len() + entry.len() > MAX_BYTES - 200 {
                truncated = true;
                break;
            }
            out.push_str(&entry);
        }
    }
    let pending = context
        .get("pending_rules")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    if pending > 0 {
        out.push_str(&format!(
            "\n{pending} rule file(s) under `.precept/rules/` are not committed yet; they apply once committed.\n"
        ));
    }
    let preferences: Vec<Value> = context
        .get("preferences")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !truncated {
        out.push_str("\n## Preferences\n");
        if preferences.is_empty() {
            out.push_str("\nNo confirmed preferences apply.\n");
        }
    }
    for p in preferences
        .iter()
        .take(if truncated { 0 } else { usize::MAX })
    {
        let key = p.get("key").and_then(Value::as_str).unwrap_or("?");
        let scope = match p.get("scope").and_then(Value::as_str) {
            Some("project") => "this project",
            _ => "personal",
        };
        let tags: Vec<String> = p
            .get("tags")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|t| format!("#{t}"))
            .collect();
        let (value, cut_value) = cut(
            p.get("value").and_then(Value::as_str).unwrap_or("").trim(),
            MAX_VALUE,
        );
        let mut entry = format!("\n### {key}\n\n_{scope}");
        if !tags.is_empty() {
            entry.push_str(&format!(" · {}", tags.join(" ")));
        }
        if let Some(e) = p.get("enforcement").and_then(Value::as_str) {
            entry.push_str(&format!(" · {e}"));
        }
        entry.push_str("_\n\n");
        if let Some(where_) = applies(p) {
            entry.push_str(&format!("Applies to {where_} only.\n\n"));
        }
        entry.push_str(&demote(value));
        if cut_value {
            entry.push_str(" …");
        }
        entry.push('\n');
        if out.len() + entry.len() > MAX_BYTES - 200 {
            truncated = true;
            break;
        }
        out.push_str(&entry);
    }
    if !truncated {
        let blocked = blocked(context);
        if out.len() + blocked.len() > MAX_BYTES - 200 {
            truncated = true;
        } else {
            out.push_str(&blocked);
        }
    }
    if !truncated {
        let work = work(context);
        if out.len() + work.len() > MAX_BYTES - 200 {
            truncated = true;
        } else {
            out.push_str(&work);
        }
    }
    out.push_str(&memory(context));
    if let Some(status) = context
        .pointer("/contract_resolution/status")
        .and_then(Value::as_str)
        && !matches!(status, "ready" | "not_configured")
    {
        let detail = match status {
            "partial" => "some rule keys conflict and are unresolved",
            "invalid_source" => {
                "a rule file under `.precept/rules/` is invalid; no rules or preferences are served until it is fixed"
            }
            _ => "rules could not be read",
        };
        out.push_str(&format!(
            "\nRepository rules status: `{status}` ({detail}).\n"
        ));
    }
    if truncated {
        out.push_str(
            "\n_Briefing cut to stay short: call `get_effective_context` for every rule._\n",
        );
    }
    out
}

#[cfg(test)]
mod memory_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn memory_lists_environment_then_relevant_within_its_budget() {
        let ctx = json!({"memory":{
            "counts":{"environment":1,"caution":1,"lesson":0,"attempt":3},
            "environment":[{"id":"a1","title":"Node","body":"Node 22 via mise","pinned":true}],
            "relevant":[{"id":"b2","kind":"caution","title":"WSL paths","body":"/mnt/c is slow","unstable":true}]
        }});
        let text = memory(&ctx);
        assert!(text.contains("Environment 1 · Cautions 1 · Lessons 0 · Attempts 3"));
        assert!(text.contains("- **Node** _pinned_ (`a1`): Node 22 via mise"));
        assert!(text.contains("- [caution] **WSL paths** _changed often lately_ (`b2`)"));
        assert!(text.find("### Environment") < text.find("### Most relevant"));
        assert!(memory(&json!({"memory":{"counts":{}}})).is_empty());
        let b = blocked(&json!({"gate_denials":[
            {"rule":"no-force-push","count":3,"last_ms":1_763_640_000_000u64},
            {"rule":"env-file","count":1,"last_ms":1_763_640_000_000u64}
        ]}));
        assert!(b.contains("## Recently blocked"));
        assert!(b.contains("- `no-force-push`: 3 times, last 2025-11-20"));
        assert!(b.contains("- `env-file`: 1 time, last"));
        assert!(blocked(&json!({"gate_denials":[]})).is_empty());
        let many: Vec<Value> = (0..40)
            .map(|i| json!({"id":format!("{i}"),"title":format!("t{i}"),"body":"x".repeat(500)}))
            .collect();
        let big = memory(&json!({"memory":{"counts":{"environment":40},"environment":many}}));
        assert!(big.len() <= MEMORY_BYTES);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_contracts_preferences_and_compaction_within_bounds() {
        let context = json!({
            "repository_rules":[{"key":"formatter","file":".precept/rules/formatter.md","text":"Use rustfmt.",
                "tags":["rust"],"enforcement":"hard","allowed_values":["rustfmt"],
                "checks":{"status":"failing","passed":1,"total":2,"failure":{"file":"rustfmt.toml","kind":"exists"}}}],
            "pending_rules":[{"id":"x","key":"x"}],
            "preferences":[
                {"key":"style","scope":"global","tags":["style","docs"],"value":"Short sentences."},
                {"key":"build","scope":"project","tags":[],"value":"Use mise."}
            ],
            "contract_resolution":{"status":"partial"}
        });
        let b = render("alpha", &context, Moment::Compact);
        assert!(b.starts_with("# Precept: context for alpha"));
        assert!(b.contains("was compacted"));
        assert!(b.contains("### formatter\n\n_repository · `.precept/rules/formatter.md` · #rust · hard · check failing: `rustfmt.toml` is missing (1/2 pass)_"));
        assert!(b.contains("Allowed values: rustfmt\n\nUse rustfmt."));
        assert_eq!(
            demote("# A\n```\n# not\n```\n### B"),
            "#### A\n```\n# not\n```\n###### B"
        );
        assert!(b.contains("1 rule file(s)"));
        assert!(b.contains("### style\n\n_personal · #style #docs_\n\nShort sentences."));
        assert!(b.contains("_this project_"));
        assert!(b.contains("`partial`"));
        assert!(!render("alpha", &context, Moment::Start).contains("compacted"));

        let many: Vec<Value> = (0..200)
            .map(|i| json!({"key":format!("k{i}"),"scope":"global","tags":[],"value":"合".repeat(800)}))
            .collect();
        let b = render("alpha", &json!({"preferences":many}), Moment::Start);
        assert!(b.len() <= MAX_BYTES);
        assert!(b.contains("Briefing cut"));
        assert!(render("alpha", &json!({}), Moment::Start).contains("No confirmed preferences"));
        let scoped = json!({"preferences":[
            {"key":"fmt","scope":"global","tags":[],"value":"rustfmt","enforcement":"hard",
             "applies_to":{"agents":[],"languages":["rust"],"paths":["apps/"],"task_tags":[]}}
        ]});
        let b = render("alpha", &scoped, Moment::Start);
        assert!(b.contains("_personal · hard_"));
        assert!(b.contains("Applies to languages rust; paths apps/ only."));
        let with_work = json!({"work_sessions":[
            {"id":"ab12","status":"open","agent":"claude-code 2.1","updated_ms":1_763_640_000_000u64,
             "goal":"Add import","doing":[{"id":"t1","text":"tests"}],
             "todo":[{"id":"d1","text":"docs"},{"id":"d2","text":"ship","depends_on":["d1"]}],
             "done":[],"rev":2},
            {"id":"cd34","status":"closed","updated_ms":0,"goal":"Rename","doing":[],"todo":[],
             "done":[{"id":"a1","text":"all"}],"summary":"Renamed.","rev":3}
        ]});
        let b = render("alpha", &with_work, Moment::Compact);
        assert!(b.contains("## Work in progress"));
        assert!(b.contains(
            "- **Add import** (open, claude-code 2.1, 2025-11-20 12:00 UTC; session `ab12`)"
        ));
        assert!(b.contains("  - Next: docs; ship"));
        assert!(b.contains("  - Summary: Renamed."));
        assert!(!render("alpha", &json!({}), Moment::Start).contains("Work in progress"));
    }
}
