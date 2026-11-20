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
    let preferences = context
        .get("preferences")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
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
        entry.push_str("_\n\n");
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
    }
}
