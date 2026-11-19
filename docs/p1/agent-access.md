# Agent access: per-project sharing and MCP client configuration

Status: **implemented on `dev` (2026-11-19), owner-requested.** Builds on the
[project-management plan](project-management-plan.md).

## Decisions (owner, 2026-11-19)

- **Stdio, no port.** `acp-mcp` stays a stdio program that the MCP client starts
  itself. There is no network listener and no port to configure; a listener would
  need its own ADR (AGENTS.md: no network listener).
- **Copyable configuration, no writes outside ACP.** The GUI shows, per project and
  per client, the exact text to paste: a `claude mcp add` command for Claude Code,
  a `~/.codex/config.toml` table for Codex, and generic MCP JSON. ACP never writes
  another tool's configuration and never runs a shell.
- **Sharing is per project.** Each project in the daemon's set has `agents`
  (shared with external agents), on when a project is added.

## Daemon

- `projects.json` entries carry `agents` (absent reads as shared). Startup
  publishes an agent credential (`agent-<name>.cap`) only for shared projects.
- `project.agents {name, enabled}` (operator): persists the choice, then
  withdraws or republishes that project's agent credential. A token held by a
  running client is refused on its next connection. No Profile write; ACP-started
  sessions and their context leases are unaffected. Agents cannot call it.
- `project.mcp {name}` (operator): the executable (the configured `--context-mcp`,
  else the `acp-mcp` beside the daemon) and the exact arguments
  (`--runtime-dir`, `--endpoint`, `--project`, `--capability-file`). Paths only,
  never a token; `shared` says whether a client would be admitted now.
- `project.list` reports `agents`. CLI: `acp-query project share|unshare|mcp NAME`.

## Clients

- A client on the daemon's own system runs the executable directly.
- A client on Windows with a WSL daemon runs
  `wsl.exe -d <distribution> -u <user> --exec <acp-mcp> <args>`: stdio crosses the
  boundary, the capability file is read inside Linux and never leaves it.
- The capability file is re-issued when the daemon restarts; restart the MCP server
  in the client afterwards. The configuration itself does not change.

## GUI

Set-up step 4, **Agents**: each project with Shared / Not shared and Share / Stop
sharing; for the chosen project, Claude Code / Codex / JSON tabs, "client on
Windows / inside WSL" for a WSL connection, the text and **Copy**. It explains
that agents only read one project's confirmed preferences, repository facts and
contracts.
