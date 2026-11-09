# Agent Control Plane

Local-first, developer-centric context control plane for coding agents. **P0 prototype**: a working SQLite context hub, explicit preference review, task handoffs, and a project-scoped read-only MCP stdio server. No GUI or external memory backend is required.

> Current status: `0.1.0a0` engineering prototype. Python 3.11+ remains the **stdlib-only** operator write path. A Rust read-only core, local daemon/IPC/MCP, and isolated NSH desktop/TUI console are implemented. Framework selection remains **Proposed**; this is not an agent process orchestrator or production security boundary.

## Why

- Your coding agents should share **confirmed** developer habits without automatically inferring permanent preferences from chat history.
- Current project rules, source files, tests, and accepted ADRs have priority over personal preferences and old handoffs.
- A task can change harness without losing objective, revision, next action, and *which checks actually ran*.
- MCP provides scoped, inspectable context; it cannot silently change profile data in P0.

## Quickstart

```bash
# Python >= 3.11; no third-party runtime dependencies
python -m pip install --no-deps -e .

# Register an existing checked-out project
acpctl project add lanpath /absolute/path/to/lanpath

# Explicitly suggest, review, then confirm a preference
acpctl preference suggest --key interaction.language --value zh-CN --source 'user-confirmed-request'
acpctl preference list --all
acpctl preference confirm 1

# Inspect effective context (no project document content by default)
acpctl context lanpath
acpctl context lanpath --include-documents

# Create handoff from the CURRENT project HEAD/branch; statuses must be honest
acpctl handoff put --project lanpath --task-id fix-001 \
  --objective 'Investigate startup issue' --next-action 'Run selected tests on target platform' \
  --validation-json '[{"check":"Android device","status":"NOT_RUN"}]'
acpctl handoff get --project lanpath --task-id fix-001

# Run a project-scoped read-only MCP stdio process (e.g. as a harness subprocess)
acpctl mcp --project lanpath
```

### Run without installation

```bash
PYTHONPATH=src python -m agent_control_plane.cli project list
PYTHONPATH=src python -m unittest discover -s tests -v
```

### MCP client configuration (illustrative)

Point a compatible coding agent's MCP **stdio** server configuration to:

```json
{
  "command": "acpctl",
  "args": ["mcp", "--project", "lanpath"]
}
```

The actual configuration file and syntax are harness-specific. The server reads newline-delimited JSON-RPC on standard input and writes only JSON-RPC on standard output. Use a **separate configured process for each project**: an agent attached to `lanpath` cannot request another project's context through this server. This is a cooperative local process boundary; it does not sandbox an agent that already has direct disk access.

**Implemented tools**: `get_effective_context(include_documents?: boolean)`, `get_handoff(task_id: string)`. There are no MCP mutation tools, background scanning, web APIs, or automatic habit-learning hooks.

The proof-of-concept supports MCP 2025-11-25 legacy initialization and basic 2026-07-28 stateless metadata on stdio. It does **not** claim complete MCP conformance, HTTP transport, authentication, sampling, resources, prompts, task augmentation, or ACP agent control. See [protocol limitations](docs/interfaces.md).

## Data / trust rules

- SQLite data directory defaults to `AGENT_CP_HOME`, otherwise platform-local user state (`XDG_STATE_HOME` or `~/.local/state/agent-control-plane`; Windows `LOCALAPPDATA`).
- All preference proposals start as **suggested** and are **not** sent to an agent. `confirm` requires deliberate CLI operator action. Confirmed preferences can be deprecated or deleted.
- Project-scoped confirmed values mask global confirmed values with the same key. **Neither overrides the project's rules.**
- Project files are re-read live when requested; default output includes only allowlisted document paths, sizes and hashes. `--include-documents` explicitly includes small allowlisted files. Symlinks escaping the registered checkout are blocked.
- A handoff is **evidence about past work**, not proof a check passed on the current HEAD. Resuming checks git revision/branch/worktree changes and reports warnings. New handoffs also include a bounded worktree fingerprint to detect uncommitted drift; older snapshots remain readable and explicitly unverified.
- Do not store secrets, credentials, personal device identifiers, raw transcripts, private customer data, or unredacted logs. MCP clients run under the user account and can read exposed data; only attach to trusted local harnesses.

## Repository structure

```text
src/agent_control_plane/
  store.py       # SQLite, statuses, project-scoped data
  context.py     # live project facts / git / resolver
  mcp.py         # read-only MCP stdio
  cli.py         # operator-controlled writes and inspection
tests/           # stdlib unittest integration / security checks
docs/            # architecture, interfaces, risks, roadmap, UI ADR
```

## Design and roadmap

- [Architecture](docs/architecture.md)
- [Wire interfaces and conformance boundaries](docs/interfaces.md)
- [Threat model](docs/security.md)
- [P0/P1/P2 roadmap and acceptance](docs/roadmap.md)
- [Codex/Kimi context and Handoff E2E evidence (2026-11-08)](docs/evaluation/2026-11-08-codex-kimi-handoff.md)
- [P0 MCP negative-boundary test plan](docs/evaluation/p0-mcp-negative-boundaries.md)
- [Local NSH console, Windows/WSL ownership and verification quickstart](docs/p1/local-codex-e2.md)
- [P1-E2 sanitized local validation](docs/evaluation/p1-e2-local-validation.md)
- [ADR-0001: Core before GPUI / NSH interface](docs/adr/0001-core-before-ui.md)

The standalone [agent-development-protocol](https://github.com/ybakiame/agent-development-protocol) remains authoritative for agent-neutral engineering procedures. This repository **consumes** the protocol; it does not rewrite a project's `AGENTS.md`, dependencies, or CI gates.

## Test status

`python -m unittest discover -s tests -v` runs the P0 checks. Two real-harness, read-only Codex/Kimi trials were reported by the operator; they do not establish independently measured end-to-end interoperability with every version of Codex/Pi/OpenCode, production-grade MCP conformance, multi-process reliability, or native desktop support. Seven isolated MCP negative-boundary tests are included, but real-harness negative trials and platform security boundaries remain unverified. Those are explicit next milestones.
