//! The session-start briefing: the effective context of one project rendered
//! as short Markdown an agent reads before it works. Built only from the
//! projected context (confirmed preferences, effective contracts, warnings),
//! so it carries nothing an agent could not read through MCP. Bounded; a cut
//! briefing says so and points to the MCP tool for the rest.
use serde_json::Value;

/// The longest briefing, in bytes.
pub const MAX_BYTES: usize = 12 * 1024;
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
            .filter_map(Value::as_str)
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
    let contracts = context
        .get("effective_contracts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !contracts.is_empty() {
        out.push_str("\n## Project contracts\n\n");
        for c in &contracts {
            let key = c.get("key").and_then(Value::as_str).unwrap_or("?");
            let allowed: Vec<&str> = c
                .get("allowed_values")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            out.push_str(&format!("- **{key}**: {}\n", allowed.join(" | ")));
        }
    }
    let (environment, preferences): (Vec<Value>, Vec<Value>) = context
        .get("preferences")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .partition(|p| p.get("scope").and_then(Value::as_str) == Some("environment"));
    out.push_str("\n## Preferences\n");
    if preferences.is_empty() {
        out.push_str("\nNo confirmed preferences apply.\n");
    }
    let mut truncated = false;
    for p in &preferences {
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
        entry.push_str(value);
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
    if !environment.is_empty() && !truncated {
        out.push_str("\n## Work environment (this machine)\n\n");
        for e in &environment {
            let key = e.get("key").and_then(Value::as_str).unwrap_or("?");
            let (value, _) = cut(
                e.get("value").and_then(Value::as_str).unwrap_or("").trim(),
                MAX_VALUE,
            );
            let line = format!("- **{key}**: {value}\n");
            if out.len() + line.len() > MAX_BYTES - 200 {
                truncated = true;
                break;
            }
            out.push_str(&line);
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
    if let Some(status) = context
        .pointer("/contract_resolution/status")
        .and_then(Value::as_str)
        && !matches!(status, "ready" | "not_configured")
    {
        out.push_str(&format!(
            "\nContract source status: `{status}` (some contract keys may be unresolved).\n"
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
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_contracts_preferences_and_compaction_within_bounds() {
        let context = json!({
            "effective_contracts":[{"key":"formatter","allowed_values":["rustfmt"]}],
            "preferences":[
                {"key":"style","scope":"global","tags":["style","docs"],"value":"Short sentences."},
                {"key":"build","scope":"project","tags":[],"value":"Use mise."}
            ],
            "contract_resolution":{"status":"partial"}
        });
        let b = render("alpha", &context, Moment::Compact);
        assert!(b.starts_with("# Precept: context for alpha"));
        assert!(b.contains("was compacted"));
        assert!(b.contains("- **formatter**: rustfmt"));
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
             "applies_to":{"agents":[],"languages":["rust"],"paths":["apps/"],"task_tags":[]}},
            {"key":"shell","scope":"environment","tags":[],"value":"Agents run inside WSL."}
        ]});
        let b = render("alpha", &scoped, Moment::Start);
        assert!(b.contains("_personal · hard_"));
        assert!(b.contains("Applies to languages rust; paths apps/ only."));
        assert!(
            b.contains("## Work environment (this machine)\n\n- **shell**: Agents run inside WSL.")
        );
        assert!(!b.contains("### shell"));
        let with_work = json!({"work_sessions":[
            {"id":"ab12","status":"open","agent":"claude-code 2.1","updated_ms":1_763_640_000_000u64,
             "goal":"Add import","doing":["tests"],"todo":["docs","ship"],"done":[]},
            {"id":"cd34","status":"closed","updated_ms":0,"goal":"Rename","doing":[],"todo":[],"done":["all"],"summary":"Renamed."}
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
