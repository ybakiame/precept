//! MCP client configuration for one project: what to paste into Claude Code,
//! Codex or any MCP client so it starts `precept-mcp` (a stdio program: no port,
//! no listener). The daemon supplies the executable and exact arguments
//! (`project.mcp`); this only formats them. Paths only, never a token: the
//! capability file is read by `precept-mcp` itself, inside the daemon's system.
use serde::Deserialize;

/// The daemon's launch line for a project (`project.mcp`).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Launch {
    pub name: String,
    pub shared: bool,
    pub executable: Option<String>,
    pub args: Vec<String>,
    /// The session-start hook beside `precept-mcp`, when installed.
    #[serde(default)]
    pub hook: Option<String>,
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

/// The MCP server name a client lists: `precept-<project>`.
pub fn server_name(project: &str) -> String {
    format!("precept-{project}")
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
/// `precept-mcp` executable.
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

/// The session-start hook configuration for `client` (Claude Code and Codex
/// read the same shape), or `None` when the daemon reported no hook or the
/// client has no session-start hook (other clients read the AGENTS.md pointer).
pub fn hook_snippet(client: Client, launch: &Launch, host: &Host) -> Option<String> {
    let (vendor, file) = match client {
        Client::ClaudeCode => ("claude", "~/.claude/settings.json"),
        Client::Codex => ("codex", "~/.codex/hooks.json"),
        Client::Json => return None,
    };
    let hook = Launch {
        executable: launch.hook.clone(),
        args: std::iter::once("--vendor".to_owned())
            .chain(std::iter::once(vendor.to_owned()))
            .chain(launch.args.iter().cloned())
            .collect(),
        ..launch.clone()
    };
    let (cmd, args) = command(&hook, host)?;
    let line = std::iter::once(cmd.as_str())
        .chain(args.iter().map(String::as_str))
        .map(word)
        .collect::<Vec<_>>()
        .join(" ");
    let json = serde_json::to_string_pretty(&serde_json::json!({
        "hooks": { "SessionStart": [ { "hooks": [ { "type": "command", "command": line } ] } ] }
    }))
    .unwrap_or_default();
    Some(format!("// {file}: merge into its \"hooks\"\n{json}"))
}

/// The AGENTS.md pointer of a project as the daemon reports it
/// (`integration.pointer`): its state, the file digest the operator previews,
/// and the exact block Precept would write.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Pointer {
    pub path: String,
    pub exists: bool,
    pub sha256: Option<String>,
    /// `absent`, `current` or `outdated`; `None` with `malformed`.
    pub state: Option<String>,
    pub malformed: bool,
    pub block: String,
    #[serde(default)]
    pub action: Option<String>,
}

/// One harness's session-start hook for a project (`integration.hooks`).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct HookState {
    /// `claude` or `codex`.
    pub vendor: String,
    /// Where it lives, `~/`-relative for display.
    pub file: String,
    pub exists: bool,
    pub sha256: Option<String>,
    pub malformed: bool,
    pub installed: bool,
    /// Installed exactly as this daemon would write it.
    pub current: bool,
    /// The exact command the entry runs.
    pub command: String,
    #[serde(default)]
    pub action: Option<String>,
}

/// Every hook of a project, or `available: false` without `precept-hook`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Hooks {
    pub available: bool,
    pub hooks: Vec<HookState>,
}
impl Hooks {
    pub fn get(&self, vendor: &str) -> Option<&HookState> {
        self.hooks.iter().find(|h| h.vendor == vendor)
    }
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
    /// The project's session-start hooks (`integration.hooks`).
    pub hooks: Option<Hooks>,
    /// The harness whose hook is being switched.
    pub hook_busy: Option<String>,
    pub hooks_error: Option<String>,
    /// The project's AGENTS.md pointer (`integration.pointer`).
    pub pointer: Option<Pointer>,
    /// The pointer dialog: `Some(true)` add/update, `Some(false)` remove.
    pub pointer_dialog: Option<bool>,
    pub pointer_busy: bool,
    pub pointer_error: Option<String>,
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
            executable: Some("/usr/bin/precept-mcp".into()),
            args: [
                "--runtime-dir",
                "/home/u/.local/state/precept/runtime",
                "--endpoint",
                "/home/u/.local/state/precept/runtime/control.sock",
                "--project",
                "alpha",
                "--capability-file",
                "/home/u/.local/state/precept/runtime/agent-alpha.cap",
            ]
            .map(String::from)
            .to_vec(),
            hook: Some("/usr/bin/precept-hook".into()),
        }
    }
    #[test]
    fn hooks_name_the_vendor_wrap_wsl_and_skip_other_clients() {
        let same = hook_snippet(Client::ClaudeCode, &launch(), &Host::Same).unwrap();
        assert!(same.starts_with("// ~/.claude/settings.json"));
        let json: serde_json::Value =
            serde_json::from_str(same.split_once('\n').unwrap().1).unwrap();
        let cmd = json["hooks"]["SessionStart"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(cmd.starts_with("/usr/bin/precept-hook --vendor claude --runtime-dir "));
        assert!(cmd.ends_with("agent-alpha.cap"));
        let host = Host::WindowsToWsl {
            distribution: "archlinux".into(),
            user: "u".into(),
        };
        let win = hook_snippet(Client::Codex, &launch(), &host).unwrap();
        assert!(
            win.contains("wsl.exe -d archlinux -u u --exec /usr/bin/precept-hook --vendor codex")
        );
        assert!(hook_snippet(Client::Json, &launch(), &Host::Same).is_none());
        let none = Launch {
            hook: None,
            ..launch()
        };
        assert!(hook_snippet(Client::ClaudeCode, &none, &Host::Same).is_none());
    }
    #[test]
    fn claude_code_gets_one_command_and_windows_wraps_wsl_exe() {
        let same = snippet(Client::ClaudeCode, &launch(), &Host::Same).unwrap();
        assert!(same.starts_with(
            "claude mcp add --scope user precept-alpha -- /usr/bin/precept-mcp --runtime-dir "
        ));
        assert!(
            same.ends_with(
                "--capability-file /home/u/.local/state/precept/runtime/agent-alpha.cap"
            )
        );
        let host = Host::WindowsToWsl {
            distribution: "archlinux".into(),
            user: "u".into(),
        };
        let win = snippet(Client::ClaudeCode, &launch(), &host).unwrap();
        assert!(
            win.contains("-- wsl.exe -d archlinux -u u --exec /usr/bin/precept-mcp --runtime-dir")
        );
    }
    #[test]
    fn codex_toml_and_json_carry_the_same_command() {
        let host = Host::WindowsToWsl {
            distribution: "Ubuntu 24.04".into(),
            user: "u".into(),
        };
        let toml = snippet(Client::Codex, &launch(), &host).unwrap();
        assert!(toml.contains("[mcp_servers.\"precept-alpha\"]"));
        assert!(toml.contains("command = \"wsl.exe\""));
        assert!(toml.contains("\"Ubuntu 24.04\""));
        let json: serde_json::Value =
            serde_json::from_str(&snippet(Client::Json, &launch(), &Host::Same).unwrap()).unwrap();
        assert_eq!(
            json["mcpServers"]["precept-alpha"]["command"],
            "/usr/bin/precept-mcp"
        );
        assert_eq!(json["mcpServers"]["precept-alpha"]["args"][5], "alpha");
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
