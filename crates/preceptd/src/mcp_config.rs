//! MCP server entries in the agents' own user configuration, switched by the
//! operator: one `precept-<project>` entry per client, nothing else touched.
//! Like the hooks, a change names the digest of the file the operator saw; a
//! file that changed since is refused. The previous file is kept as
//! `<file>.precept.bak` and writes are atomic. Codex's TOML is edited as text
//! (only Precept's own table goes or comes) and checked by parsing the result.
use crate::integration::{read_config, write_atomic};
use crate::ojson::{self, J};
use precept_core::pointer;
use precept_ipc::Error;
use serde_json::{Value, json};
use std::{fs, path::Path};

/// An MCP client whose user configuration Precept can add its server to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Client {
    /// Claude Code: `~/.claude.json`, `mcpServers` (user scope).
    Claude,
    /// Codex: `~/.codex/config.toml`, `[mcp_servers.<name>]`.
    Codex,
    /// Antigravity: `~/.gemini/config/mcp_config.json`, `mcpServers`.
    Antigravity,
}
impl Client {
    pub const ALL: [Client; 3] = [Client::Claude, Client::Codex, Client::Antigravity];
    fn file(self) -> &'static str {
        match self {
            Client::Claude => ".claude.json",
            Client::Codex => ".codex/config.toml",
            Client::Antigravity => ".gemini/config/mcp_config.json",
        }
    }
}

/// How the client starts `precept-mcp` for the project.
pub struct Launch {
    pub command: String,
    pub args: Vec<String>,
}

pub fn server(project: &str) -> String {
    format!("precept-{project}")
}

/// The command and arguments of a JSON server entry.
fn json_launch(entry: &J) -> Option<(String, Vec<String>)> {
    let command = entry.get("command")?.as_str()?.to_owned();
    let J::Arr(args) = entry.get("args")? else {
        return None;
    };
    let args = args
        .iter()
        .map(|a| a.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()?;
    Some((command, args))
}

fn json_entry(client: Client, launch: &Launch) -> J {
    let args = J::Arr(launch.args.iter().map(|a| J::str(a.as_str())).collect());
    match client {
        // What `claude mcp add --scope user` writes.
        Client::Claude => J::Obj(vec![
            ("type".into(), J::str("stdio")),
            ("command".into(), J::str(launch.command.as_str())),
            ("args".into(), args),
            ("env".into(), J::Obj(vec![])),
        ]),
        _ => J::Obj(vec![
            ("command".into(), J::str(launch.command.as_str())),
            ("args".into(), args),
        ]),
    }
}

/// What the file holds for `name`: `None` when unreadable as the client's
/// format; else whether the entry exists and its launch.
type Found = Option<Option<Option<(String, Vec<String>)>>>;

fn found(client: Client, text: Option<&str>, name: &str) -> Found {
    let Some(text) = text.filter(|t| !t.trim().is_empty()) else {
        return Some(None);
    };
    match client {
        Client::Codex => {
            let table: toml::Table = toml::from_str(text).ok()?;
            let Some(servers) = table.get("mcp_servers") else {
                return Some(None);
            };
            let entry = servers.as_table()?.get(name);
            Some(entry.map(|e| {
                let command = e.get("command")?.as_str()?.to_owned();
                let args = e
                    .get("args")?
                    .as_array()?
                    .iter()
                    .map(|a| a.as_str().map(str::to_owned))
                    .collect::<Option<Vec<_>>>()?;
                Some((command, args))
            }))
        }
        _ => {
            let config = ojson::parse(text)?;
            if !matches!(config, J::Obj(_)) {
                return None;
            }
            let Some(servers) = config.get("mcpServers") else {
                return Some(None);
            };
            if !matches!(servers, J::Obj(_)) {
                return None;
            }
            Some(servers.get(name).map(json_launch))
        }
    }
}

/// One client's MCP entry for `project`.
pub fn status(home: &Path, client: Client, project: &str, launch: &Launch) -> Result<Value, Error> {
    let current = read_config(&home.join(client.file()))?;
    let name = server(project);
    let state = found(client, current.as_deref(), &name);
    let installed = matches!(state, Some(Some(_)));
    let current_entry = matches!(
        &state,
        Some(Some(Some((c, a)))) if *c == launch.command && *a == launch.args
    );
    Ok(json!({
        "client": client,
        "file": format!("~/{}", client.file()),
        "server": name,
        "exists": current.is_some(),
        "sha256": current.as_ref().map(|t| pointer::digest(t.as_bytes())),
        "malformed": state.is_none(),
        "installed": installed,
        "current": current_entry,
    }))
}

/// Whether a TOML table header line names `name`'s table or one inside it.
fn ours(line: &str, name: &str) -> bool {
    let t = line.trim();
    if !t.starts_with('[') || t.starts_with("[[") {
        return false;
    }
    let inner: String = t
        .trim_start_matches('[')
        .split(']')
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    [
        format!("mcp_servers.\"{name}\""),
        format!("mcp_servers.{name}"),
    ]
    .iter()
    .any(|h| inner == *h || inner.starts_with(&format!("{h}.")))
}

/// The TOML text without `name`'s tables (comments before a following table
/// stay with it), plus the new table when `launch` is given.
fn toml_edit(text: &str, name: &str, launch: Option<&Launch>) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut inside = false;
    let mut pending: Vec<&str> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            if ours(line, name) {
                // The blank lines before the removed table go with it.
                while out.last().is_some_and(|l| l.trim().is_empty()) {
                    out.pop();
                }
                inside = true;
                pending.clear();
                continue;
            }
            if inside {
                if !out.is_empty() {
                    out.push("");
                }
                out.extend(pending.drain(..).filter(|l| !l.trim().is_empty()));
                inside = false;
            }
            out.push(line);
        } else if inside {
            if t.is_empty() || t.starts_with('#') {
                pending.push(line);
            } else {
                pending.clear();
            }
        } else {
            out.push(line);
        }
    }
    out.extend(pending.into_iter().filter(|l| !l.trim().is_empty()));
    let mut next = out.join("\n");
    let trimmed = next.trim_end().len();
    next.truncate(trimmed);
    if let Some(l) = launch {
        let quote = |s: &str| toml::Value::String(s.to_owned()).to_string();
        let args: Vec<String> = l.args.iter().map(|a| quote(a)).collect();
        if !next.is_empty() {
            next.push_str("\n\n");
        }
        next.push_str(&format!(
            "[mcp_servers.\"{name}\"]\ncommand = {}\nargs = [{}]",
            quote(&l.command),
            args.join(", ")
        ));
    }
    if !next.is_empty() {
        next.push('\n');
    }
    next
}

/// Adds (`enabled`) or removes the project's server entry if the file is still
/// the one the status reported (`expected` digest, `None` for absent).
pub fn apply(
    home: &Path,
    client: Client,
    project: &str,
    launch: &Launch,
    enabled: bool,
    expected: Option<&str>,
) -> Result<Value, Error> {
    let path = home.join(client.file());
    let current = read_config(&path)?;
    let actual = current.as_ref().map(|t| pointer::digest(t.as_bytes()));
    if actual.as_deref() != expected {
        return Err(Error::Conflict);
    }
    let name = server(project);
    let present = found(client, current.as_deref(), &name).ok_or(Error::InvalidRequest)?;
    let exact = matches!(&present, Some(Some((c, a))) if *c == launch.command && *a == launch.args);
    let next = match client {
        // Already exactly as written: nothing moves.
        _ if enabled && exact => current.clone().unwrap_or_default(),
        Client::Codex => {
            let next = toml_edit(
                current.as_deref().unwrap_or(""),
                &name,
                enabled.then_some(launch),
            );
            // The edit must leave exactly the intended entry, else nothing is written.
            let wanted = enabled.then(|| Some((launch.command.clone(), launch.args.clone())));
            if found(client, Some(&next), &name) != Some(wanted) {
                return Err(Error::InvalidRequest);
            }
            next
        }
        _ => {
            let mut config = match current.as_deref() {
                Some(t) if !t.trim().is_empty() => ojson::parse(t).ok_or(Error::InvalidRequest)?,
                _ => J::Obj(vec![]),
            };
            let servers = config
                .entry("mcpServers", J::Obj(vec![]))
                .ok_or(Error::InvalidRequest)?;
            let J::Obj(entries) = servers else {
                return Err(Error::InvalidRequest);
            };
            let entry = json_entry(client, launch);
            match entries.iter_mut().find(|(k, _)| *k == name) {
                // Updated in place, keeping its position.
                Some((_, v)) if enabled => *v = entry,
                _ if enabled => entries.push((name.clone(), entry)),
                _ => entries.retain(|(k, _)| *k != name),
            }
            ojson::pretty(&config)
        }
    };
    let unchanged = current.as_deref() == Some(next.as_str()) || (current.is_none() && !enabled);
    let action = if unchanged {
        "unchanged"
    } else {
        if let Some(dir) = path.parent()
            && !dir.exists()
        {
            fs::create_dir_all(dir).map_err(|_| Error::Unavailable)?;
        }
        if let Some(old) = &current {
            let file = path.file_name().and_then(|f| f.to_str()).unwrap_or("file");
            write_atomic(&path.with_file_name(format!("{file}.precept.bak")), old)?;
        }
        write_atomic(&path, &next)?;
        #[cfg(unix)]
        if current.is_none() {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        if enabled { "added" } else { "removed" }
    };
    let mut out = status(home, client, project, launch)?;
    out["action"] = json!(action);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch(root: &str) -> Launch {
        Launch {
            command: "/usr/bin/precept-mcp".into(),
            args: vec![
                "--project".into(),
                "alpha".into(),
                "--runtime-dir".into(),
                root.into(),
            ],
        }
    }
    fn digest(path: &Path) -> Option<String> {
        fs::read_to_string(path)
            .ok()
            .map(|t| pointer::digest(t.as_bytes()))
    }

    #[test]
    fn json_clients_keep_everything_else_and_update_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let file = home.join(".claude.json");
        let theirs = r#"{"numStartups": 3, "mcpServers": {"other": {"command": "x", "args": []}, "precept-alpha": {"command": "old", "args": []}}, "projects": {}}"#;
        fs::write(&file, theirs).unwrap();
        let l = launch("/r");
        let s = status(home, Client::Claude, "alpha", &l).unwrap();
        assert_eq!(s["file"], "~/.claude.json");
        assert_eq!(
            (s["installed"].clone(), s["current"].clone()),
            (json!(true), json!(false))
        );
        assert_eq!(
            apply(home, Client::Claude, "alpha", &l, true, None),
            Err(Error::Conflict)
        );
        let on = apply(
            home,
            Client::Claude,
            "alpha",
            &l,
            true,
            digest(&file).as_deref(),
        )
        .unwrap();
        assert_eq!(on["current"], true);
        let v: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["precept-alpha"]["type"], "stdio");
        assert_eq!(v["mcpServers"]["other"]["command"], "x");
        assert_eq!(v["numStartups"], 3);
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.find("\"other\"") < text.find("precept-alpha"));
        assert!(text.find("precept-alpha") < text.find("\"projects\""));
        assert!(home.join(".claude.json.precept.bak").exists());
        let off = apply(
            home,
            Client::Claude,
            "alpha",
            &l,
            false,
            digest(&file).as_deref(),
        )
        .unwrap();
        assert_eq!(off["installed"], false);
        let v: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(v["mcpServers"], json!({"other":{"command":"x","args":[]}}));

        // A new file for Antigravity, without the `type` key.
        let ag = apply(home, Client::Antigravity, "alpha", &l, true, None).unwrap();
        assert_eq!(ag["action"], "added");
        let v: Value = serde_json::from_str(
            &fs::read_to_string(home.join(".gemini/config/mcp_config.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            v["mcpServers"]["precept-alpha"],
            json!({"command":"/usr/bin/precept-mcp","args":["--project","alpha","--runtime-dir","/r"]})
        );
        // Not JSON: reported, never written.
        fs::write(&file, "{nope").unwrap();
        assert_eq!(
            status(home, Client::Claude, "alpha", &l).unwrap()["malformed"],
            true
        );
        assert_eq!(
            apply(
                home,
                Client::Claude,
                "alpha",
                &l,
                true,
                digest(&file).as_deref()
            ),
            Err(Error::InvalidRequest)
        );
    }

    #[test]
    fn codex_toml_changes_only_precepts_table() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        fs::create_dir_all(home.join(".codex")).unwrap();
        let file = home.join(".codex/config.toml");
        let theirs = "model = \"gpt\"\n\n[mcp_servers.node]\ncommand = \"node\"\n\n[mcp_servers.\"precept-alpha\"]\ncommand = \"old\"\nargs = []\n\n# their comment\n[profiles.fast]\nmodel = \"mini\"\n";
        fs::write(&file, theirs).unwrap();
        let l = launch("/home/u/it's here");
        let on = apply(
            home,
            Client::Codex,
            "alpha",
            &l,
            true,
            digest(&file).as_deref(),
        )
        .unwrap();
        assert_eq!(on["current"], true);
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.starts_with("model = \"gpt\"\n\n[mcp_servers.node]\ncommand = \"node\"\n\n# their comment\n[profiles.fast]\nmodel = \"mini\"\n\n[mcp_servers.\"precept-alpha\"]\n"), "{text}");
        assert_eq!(text.matches("precept-alpha").count(), 1);
        let off = apply(
            home,
            Client::Codex,
            "alpha",
            &l,
            false,
            digest(&file).as_deref(),
        )
        .unwrap();
        assert_eq!(off["installed"], false);
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "model = \"gpt\"\n\n[mcp_servers.node]\ncommand = \"node\"\n\n# their comment\n[profiles.fast]\nmodel = \"mini\"\n"
        );
        // An inline table Precept cannot edit as text: refused, untouched.
        let inline = "mcp_servers = { node = { command = \"node\" } }\n";
        fs::write(&file, inline).unwrap();
        assert_eq!(
            apply(
                home,
                Client::Codex,
                "alpha",
                &l,
                true,
                digest(&file).as_deref()
            ),
            Err(Error::InvalidRequest)
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), inline);
    }
}
