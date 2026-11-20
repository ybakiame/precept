//! MCP client configuration for one project: what to paste into Claude Code,
//! Codex or any MCP client so it starts `acp-mcp` (a stdio program: no port,
//! no listener). The daemon supplies the executable and exact arguments
//! (`project.mcp`); this only formats them. Paths only, never a token: the
//! capability file is read by `acp-mcp` itself, inside the daemon's system.
use serde::Deserialize;

/// The daemon's launch line for a project (`project.mcp`).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Launch {
    pub name: String,
    pub shared: bool,
    pub executable: Option<String>,
    pub args: Vec<String>,
}

/// Which client the snippet is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Client {
    #[default]
    ClaudeCode,
    Codex,
    Json,
}
impl Client {
    pub const ALL: [Self; 3] = [Self::ClaudeCode, Self::Codex, Self::Json];
}

/// Where the MCP client runs relative to the daemon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Host {
    /// On the daemon's own system (Linux/WSL shell, or the same Windows user).
    Same,
    /// On Windows, with the daemon inside this WSL distribution and user:
    /// `wsl.exe` carries stdio; the credential never leaves Linux.
    WindowsToWsl { distribution: String, user: String },
}

/// The MCP server name a client lists: `acp-<project>`.
pub fn server_name(project: &str) -> String {
    format!("acp-{project}")
}

/// The command and arguments the client runs.
pub fn command(launch: &Launch, host: &Host) -> Option<(String, Vec<String>)> {
    let executable = launch.executable.clone()?;
    Some(match host {
        Host::Same => (executable, launch.args.clone()),
        Host::WindowsToWsl { distribution, user } => {
            let mut args = vec![
                "-d".to_owned(),
                distribution.clone(),
                "-u".to_owned(),
                user.clone(),
                "--exec".to_owned(),
                executable,
            ];
            args.extend(launch.args.iter().cloned());
            ("wsl.exe".to_owned(), args)
        }
    })
}

/// A shell word: plain when safe, otherwise single-quoted (POSIX shells and
/// PowerShell both read single quotes literally).
fn word(s: &str) -> String {
    let plain = !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=@\\".contains(c));
    if plain {
        s.to_owned()
    } else {
        format!("'{}'", s.replace('\'', "''"))
    }
}
/// A TOML basic string (JSON string escapes are valid TOML).
fn toml_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// The text to copy for `client`, or `None` when the daemon did not report an
/// `acp-mcp` executable.
pub fn snippet(client: Client, launch: &Launch, host: &Host) -> Option<String> {
    let (cmd, args) = command(launch, host)?;
    let name = server_name(&launch.name);
    let line = std::iter::once(cmd.as_str())
        .chain(args.iter().map(String::as_str))
        .map(word)
        .collect::<Vec<_>>()
        .join(" ");
    Some(match client {
        Client::ClaudeCode => format!("claude mcp add --scope user {name} -- {line}"),
        Client::Codex => {
            let args = args
                .iter()
                .map(|a| toml_string(a))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "# ~/.codex/config.toml\n[mcp_servers.{}]\ncommand = {}\nargs = [{args}]\n",
                toml_string(&name),
                toml_string(&cmd)
            )
        }
        Client::Json => serde_json::to_string_pretty(&serde_json::json!({
            "mcpServers": { name: { "command": cmd, "args": args } }
        }))
        .unwrap_or_default(),
    })
}

/// The Agents page state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentsPage {
    /// The project whose configuration is shown.
    pub project: Option<String>,
    pub client: Client,
    /// For a WSL connection: the client runs on Windows (default) or inside WSL.
    pub inside_wsl: bool,
    pub launch: Option<Launch>,
    pub busy: bool,
    /// The last refusal (a fixed code).
    pub error: Option<String>,
    /// Which snippet was copied last (feedback only).
    pub copied: bool,
    /// Recent agent activity for `project` (`activity.list`).
    pub activity: crate::activity::Activity,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn launch() -> Launch {
        Launch {
            name: "alpha".into(),
            shared: true,
            executable: Some("/usr/bin/acp-mcp".into()),
            args: [
                "--runtime-dir",
                "/home/u/.local/state/acp/runtime",
                "--endpoint",
                "/home/u/.local/state/acp/runtime/control.sock",
                "--project",
                "alpha",
                "--capability-file",
                "/home/u/.local/state/acp/runtime/agent-alpha.cap",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
    #[test]
    fn claude_code_gets_one_command_and_windows_wraps_wsl_exe() {
        let same = snippet(Client::ClaudeCode, &launch(), &Host::Same).unwrap();
        assert!(same.starts_with(
            "claude mcp add --scope user acp-alpha -- /usr/bin/acp-mcp --runtime-dir "
        ));
        assert!(
            same.ends_with("--capability-file /home/u/.local/state/acp/runtime/agent-alpha.cap")
        );
        let host = Host::WindowsToWsl {
            distribution: "archlinux".into(),
            user: "u".into(),
        };
        let win = snippet(Client::ClaudeCode, &launch(), &host).unwrap();
        assert!(win.contains("-- wsl.exe -d archlinux -u u --exec /usr/bin/acp-mcp --runtime-dir"));
    }
    #[test]
    fn codex_toml_and_json_carry_the_same_command() {
        let host = Host::WindowsToWsl {
            distribution: "Ubuntu 24.04".into(),
            user: "u".into(),
        };
        let toml = snippet(Client::Codex, &launch(), &host).unwrap();
        assert!(toml.contains("[mcp_servers.\"acp-alpha\"]"));
        assert!(toml.contains("command = \"wsl.exe\""));
        assert!(toml.contains("\"Ubuntu 24.04\""));
        let json: serde_json::Value =
            serde_json::from_str(&snippet(Client::Json, &launch(), &Host::Same).unwrap()).unwrap();
        assert_eq!(
            json["mcpServers"]["acp-alpha"]["command"],
            "/usr/bin/acp-mcp"
        );
        assert_eq!(json["mcpServers"]["acp-alpha"]["args"][5], "alpha");
        // A shell word with a space is quoted.
        let claude = snippet(Client::ClaudeCode, &launch(), &host).unwrap();
        assert!(claude.contains("-d 'Ubuntu 24.04' -u u"));
    }
    #[test]
    fn no_executable_means_no_snippet() {
        let mut l = launch();
        l.executable = None;
        assert!(snippet(Client::Codex, &l, &Host::Same).is_none());
    }
}
