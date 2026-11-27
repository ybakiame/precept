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
    /// The one `precept` server's arguments (no project: it serves the
    /// project of the session's folder).
    pub args: Vec<String>,
    /// What a project's hook command adds (`--project`, `--capability-file`).
    #[serde(default)]
    pub project_args: Vec<String>,
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
/// The one server name every client lists.
pub const SERVER: &str = "precept";

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
    let name = SERVER;
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
                toml_string(name),
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
            .chain(launch.project_args.iter().cloned())
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
    /// What is installed now (it may predate a move of the programs).
    #[serde(default)]
    pub installed_command: Option<String>,
    /// The installed entry starts a program that is gone: it cannot run.
    #[serde(default)]
    pub broken: bool,
    #[serde(default)]
    pub action: Option<String>,
}

/// Every hook of a project, or `available: false` without `precept-hook`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Hooks {
    pub available: bool,
    /// Session-start entries.
    pub hooks: Vec<HookState>,
    /// Pre-tool gate entries.
    #[serde(default)]
    pub gate_hooks: Vec<HookState>,
    /// Stale-document warning entries (after a file read; Claude Code only).
    #[serde(default)]
    pub read_hooks: Vec<HookState>,
    /// Compaction reminder entries (before the conversation is compacted).
    #[serde(default)]
    pub compact_hooks: Vec<HookState>,
    /// Whether the operator turned the gate on for the project.
    #[serde(default)]
    pub gate: bool,
}
impl Hooks {
    pub fn get(&self, vendor: &str) -> Option<&HookState> {
        self.hooks.iter().find(|h| h.vendor == vendor)
    }
    /// The entry of `vendor` for `event` (`session_start`, `pre_tool`,
    /// `post_read` or `pre_compact`).
    pub fn of(&self, vendor: &str, event: &str) -> Option<&HookState> {
        let list = match event {
            "pre_tool" => &self.gate_hooks,
            "post_read" => &self.read_hooks,
            "pre_compact" => &self.compact_hooks,
            _ => &self.hooks,
        };
        list.iter().find(|h| h.vendor == vendor)
    }
}

/// One client's `precept-<project>` MCP entry (`integration.mcp`).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct McpEntry {
    /// `claude` or `codex`.
    pub client: String,
    /// Where it lives, `~/`-relative for display.
    pub file: String,
    pub server: String,
    pub exists: bool,
    pub sha256: Option<String>,
    pub malformed: bool,
    pub installed: bool,
    /// Installed exactly as this daemon would write it.
    pub current: bool,
    /// The installed entry starts a program that is gone.
    #[serde(default)]
    pub broken: bool,
    #[serde(default)]
    pub action: Option<String>,
}

/// One outdated hook entry (`integration.refresh`).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct RefreshHook {
    pub vendor: String,
    pub project: String,
    pub event: String,
    pub file: String,
    /// The installed command and the one this installation writes.
    pub from: Option<String>,
    pub to: String,
    pub broken: bool,
}
/// One outdated MCP entry.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct RefreshMcp {
    pub client: String,
    pub file: String,
    pub broken: bool,
    pub sha256: Option<String>,
}
/// Every Precept entry in the agents' configuration that is not what this
/// installation writes (typically after its programs moved).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Refresh {
    pub hooks: Vec<RefreshHook>,
    /// Each changed hook file's digest, by vendor.
    pub hook_files: std::collections::BTreeMap<String, Option<String>>,
    pub mcp: Vec<RefreshMcp>,
}
impl Refresh {
    pub fn count(&self) -> usize {
        self.hooks.len() + self.mcp.len()
    }
    pub fn broken(&self) -> bool {
        self.hooks.iter().any(|h| h.broken) || self.mcp.iter().any(|m| m.broken)
    }
    /// The program a command starts (its first word, unquoted).
    pub fn program(command: &str) -> &str {
        match command.chars().next() {
            Some(q @ ('\'' | '"')) => command[1..].split(q).next().unwrap_or(""),
            _ => command.split_whitespace().next().unwrap_or(""),
        }
    }
}

/// Every client's entry for a project, or `available: false` without
/// `precept-mcp` beside the daemon.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct McpEntries {
    pub available: bool,
    pub clients: Vec<McpEntry>,
}
impl McpEntries {
    pub fn get(&self, client: &str) -> Option<&McpEntry> {
        self.clients.iter().find(|c| c.client == client)
    }
}

/// How an agent is connected (docs/product.md, "Connecting agents").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// The session-start hook and MCP: the recommended setup.
    Full,
    /// The briefing only: read-only, nothing given back.
    Hooks,
    /// MCP only: the agent must call it.
    Mcp,
    None,
}
/// The agents a level is shown for: (hook vendor and MCP client name).
pub const AGENTS: [&str; 2] = ["claude", "codex"];
impl AgentsPage {
    /// `agent`'s level from its session-start hook and MCP entry; `None`
    /// until both were read (never guessed).
    pub fn level(&self, agent: &str) -> Option<Level> {
        let hook = self.hooks.as_ref()?;
        let hook = hook.available && hook.get(agent)?.installed;
        let mcp = self.mcp.as_ref()?;
        let mcp = mcp.available && mcp.get(agent)?.installed;
        Some(match (hook, mcp) {
            (true, true) => Level::Full,
            (true, false) => Level::Hooks,
            (false, true) => Level::Mcp,
            (false, false) => Level::None,
        })
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
    /// The project's MCP entries in the clients' user configuration.
    pub mcp: Option<McpEntries>,
    /// The client whose MCP entry is being switched.
    pub mcp_busy: Option<String>,
    /// Why the MCP entries could not be read (a fixed code).
    pub mcp_error: Option<String>,
    /// Outdated Precept entries across projects (`integration.refresh`).
    pub refresh: Option<Refresh>,
    pub refresh_busy: bool,
    pub refresh_error: Option<String>,
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
            ]
            .map(String::from)
            .to_vec(),
            project_args: [
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
            "claude mcp add --scope user precept -- /usr/bin/precept-mcp --runtime-dir "
        ));
        assert!(same.ends_with("--endpoint /home/u/.local/state/precept/runtime/control.sock"));
        assert!(!same.contains("--project"));
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
        assert!(toml.contains("[mcp_servers.\"precept\"]"));
        assert!(toml.contains("command = \"wsl.exe\""));
        assert!(toml.contains("\"Ubuntu 24.04\""));
        let json: serde_json::Value =
            serde_json::from_str(&snippet(Client::Json, &launch(), &Host::Same).unwrap()).unwrap();
        assert_eq!(
            json["mcpServers"]["precept"]["command"],
            "/usr/bin/precept-mcp"
        );
        assert_eq!(
            json["mcpServers"]["precept"]["args"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
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
