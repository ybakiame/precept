//! `precept skill`: the `precept-connect` skill, which guides an agent in a
//! harness Precept does not set up itself (anything but Claude Code and Codex)
//! to connect through MCP and, where that harness reads Claude Code's hook
//! format, through hooks. Rendered with this installation's values: the MCP
//! command and each registered project's hook commands, read from the daemon
//! (`project.list`, `project.mcp`). Printed, or written once to a new file.
use precept_ipc::{Error, Location, Result, operator::control};
use serde_json::{Value, json};
use std::{io::Write, path::Path};

const TEMPLATE: &str = include_str!("../../../skills/precept-connect/SKILL.md");
const SECTION: &str = "## This installation\n";

/// A shell word: plain when safe, else quoted (single quotes for POSIX
/// shells, double quotes on Windows).
fn word(s: &str) -> String {
    let plain = !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./:=@+,".contains(&b));
    if plain {
        s.to_owned()
    } else if cfg!(windows) {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}
/// A path as a shell reads it (Windows: no verbatim prefix, forward slashes).
fn path(v: &Value) -> Option<String> {
    let s = v.as_str()?;
    Some(if cfg!(windows) {
        s.strip_prefix(r"\\?\").unwrap_or(s).replace('\\', "/")
    } else {
        s.to_owned()
    })
}
fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| path(a).or_else(|| a.as_u64().map(|n| n.to_string())))
        .collect()
}

/// The "This installation" section for `projects` (name, root).
fn installation(mcp: &Value, projects: &[(String, String, Value)]) -> String {
    let mut out = String::new();
    let exe = path(&mcp["executable"]);
    let args = strings(&mcp["args"]);
    match &exe {
        Some(exe) => {
            let entry = json!({"mcpServers":{"precept":{"command":exe,"args":args}}});
            out.push_str(&format!(
                "MCP server (every registered project):\n\n```json\n{}\n```\n",
                serde_json::to_string_pretty(&entry).unwrap_or_default()
            ));
        }
        None => out.push_str(
            "MCP server: `precept-mcp` was not found beside the daemon; MCP cannot be connected on this installation.\n",
        ),
    }
    if projects.is_empty() {
        out.push_str(
            "\nNo project is registered and shared with agents yet: the developer registers one in Precept first; then run `precept skill` again for its hook commands.\n",
        );
        return out;
    }
    for (name, root, launch) in projects {
        out.push_str(&format!("\nProject `{name}` (folder `{root}`):\n\n"));
        let Some(hook) = path(&launch["hook"]) else {
            out.push_str(
                "- Hooks: `precept-hook` was not found beside the daemon; use MCP only.\n",
            );
            continue;
        };
        let mut base = vec![
            word(&hook),
            "--vendor".into(),
            "claude".into(),
            "--client".into(),
            "<harness>".into(),
        ];
        let project_args = strings(&launch["project_args"]);
        // --project NAME --capability-file PATH, then --root and the location.
        base.extend(project_args.iter().map(|a| word(a)));
        base.extend(["--root".into(), word(root)]);
        base.extend(args.iter().map(|a| word(a)));
        let line = |event: Option<&str>| {
            let mut w = base.clone();
            if let Some(e) = event {
                w.extend(["--event".into(), e.into()]);
            }
            w.join(" ")
        };
        out.push_str(&format!(
            "- `SessionStart`: `{}`\n- `PreToolUse`: `{}`\n- `PostToolUse` (`Read`): `{}`\n- `PreCompact`: `{}`\n",
            line(None),
            line(Some("pre-tool")),
            line(Some("post-read")),
            line(Some("pre-compact"))
        ));
    }
    out
}

/// The skill rendered for this installation (all shared projects, or `only`).
pub async fn render(location: &Location, only: Option<&str>) -> Result<String> {
    let list = control(location, "project.list", json!({})).await?;
    let mut projects = Vec::new();
    let mut mcp = Value::Null;
    for p in list.as_array().ok_or(Error::Unavailable)? {
        let Some(name) = p["name"].as_str() else {
            continue;
        };
        if only.is_some_and(|o| o != name) || p["state"] != "ready" || p["agents"] != true {
            continue;
        }
        let launch = control(location, "project.mcp", json!({"name":name})).await?;
        let root = path(&p["root"]).unwrap_or_default();
        mcp = launch.clone();
        projects.push((name.to_owned(), root, launch));
    }
    if let Some(name) = only
        && projects.is_empty()
    {
        let _ = name;
        return Err(Error::NotFound);
    }
    if mcp.is_null() {
        // No shared project: the MCP launch is the same for any project name
        // the daemon knows; ask with the first listed one, if any.
        if let Some(name) = list
            .as_array()
            .and_then(|l| l.first())
            .and_then(|p| p["name"].as_str())
        {
            mcp = control(location, "project.mcp", json!({"name":name}))
                .await
                .unwrap_or(Value::Null);
        }
    }
    let at = TEMPLATE.find(SECTION).ok_or(Error::Unavailable)?;
    Ok(format!(
        "{}{SECTION}\n{}",
        &TEMPLATE[..at],
        installation(&mcp, &projects)
    ))
}

/// Writes `text` as `<dir>/SKILL.md`, creating `dir`; never overwrites.
pub fn write_new(dir: &Path, text: &str) -> Result<std::path::PathBuf> {
    std::fs::create_dir_all(dir).map_err(|_| Error::Unavailable)?;
    let file = dir.join("SKILL.md");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&file)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => Error::Conflict,
            _ => Error::Unavailable,
        })?;
    f.write_all(text.as_bytes())
        .map_err(|_| Error::Unavailable)?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_section_is_filled_with_the_installation_values() {
        assert!(TEMPLATE.contains(SECTION));
        let mcp = json!({"executable":"/usr/bin/precept-mcp","hook":"/usr/bin/precept-hook",
            "args":["--runtime-dir","/run/p","--endpoint","/run/p/c.sock"],
            "project_args":["--project","alpha","--capability-file","/run/p/agent-alpha.cap"]});
        let text = installation(&mcp, &[("alpha".into(), "/w/my repo".into(), mcp.clone())]);
        assert!(text.contains("\"command\": \"/usr/bin/precept-mcp\""));
        let root = if cfg!(windows) {
            "\"/w/my repo\""
        } else {
            "'/w/my repo'"
        };
        assert!(text.contains(&format!(
            "`SessionStart`: `/usr/bin/precept-hook --vendor claude --client <harness> --project alpha --capability-file /run/p/agent-alpha.cap --root {root} --runtime-dir /run/p --endpoint /run/p/c.sock`"
        )), "{text}");
        assert!(text.contains("--event pre-tool`"));
        assert!(text.contains("--event post-read`"));
        assert!(installation(&mcp, &[]).contains("No project is registered"));
        let no_hook = json!({"executable":"/x","args":[]});
        assert!(
            installation(&no_hook, &[("a".into(), "/w".into(), no_hook.clone())])
                .contains("use MCP only")
        );
    }

    #[test]
    fn writing_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("precept-skill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = write_new(&dir.join("precept-connect"), "x").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "x");
        assert_eq!(
            write_new(&dir.join("precept-connect"), "y"),
            Err(Error::Conflict)
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
