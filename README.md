# Agent Control Plane

**A local-first, developer-centric control plane for engineering context, project rules and continuity across coding agents and harnesses.** Make the right context available to the right agent, explain why it applies, preserve it across Handoffs, and distinguish reported work from observed or independently verified evidence. See the [product charter](docs/product.md).

> Implementation maturity: `0.1.0a0` engineering prototype. The product charter and architecture decisions are documented under [P1-J / Issue #30](https://github.com/ybakiame/agent-control-plane/issues/30). Rust is the domain/core and daemon foundation; [NSH Rust is the accepted single Desktop/TUI UI direction](docs/adr/0003-ui-framework-selection.md). Python 3.11+ remains the **stdlib-only** persistent profile/Handoff write path. Formalizing the product does not implement structured contracts, generalized suggestions/review UI, frozen constraints or an evidence chain. No production readiness, OS sandbox or universal vendor compatibility is claimed.

## Implementation and support status

The Rust read-only core/store, local daemon/IPC/MCP, shared NSH console and optional
operator-controlled native Agent Client Protocol (ACP) session are implemented.
P1-I adds separately opted-in, revocable session context.
[Architecture](docs/architecture.md) separates this baseline
from the target domain; the [roadmap](docs/roadmap.md) gates later implementation.

Windows Desktop/ConPTY and native WSLg/TUI have scoped local regression evidence.
macOS has historical UI-spike build/window smoke, but current product behavior is
not revalidated. Mobile/browser are outside current product targets; full IME,
screen readers and clean-machine release support remain unverified. Theme contrast
and compilation do not establish accessibility or platform support.

P1-I independently observed Kimi 2.5.2 reading both MCP tools; OpenCode's official
DeepSeek model turn made zero MCP reads and is not an MCP pass. Broader client and
Handoff acceptance remains in [Issue #31](https://github.com/ybakiame/agent-control-plane/issues/31),
after product formalization. See [source-specific evidence](docs/evaluation/p1-i-context-bridge.md).

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

The proof-of-concept supports MCP 2025-11-25 legacy initialization and basic 2026-07-28 stateless metadata on stdio. It does **not** claim complete MCP conformance, HTTP transport, authentication, sampling, resources, prompts, task augmentation, or ACP agent control through MCP. See [protocol limitations](docs/interfaces.md).

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
crates/          # Rust domain/store, daemon/IPC/MCP, operator/session and rehearsal
spikes/p1-e/shared/ # framework-neutral typed Desktop/TUI presentation
spikes/p1-e/nsh/    # implemented NSH Rust UI; historical comparison paths remain
docs/            # architecture, interfaces, risks, roadmap, UI ADR
```

## Design and roadmap

- [Product charter and capability boundaries](docs/product.md)
- [Domain, human review and evidence trust ADR](docs/adr/0004-product-domain-trust.md)
- [Product UI information architecture](docs/product-ui.md)
- [agent-control design-reference assessment](docs/agent-control-assessment.md)
- [ADR-0003: accepted NSH Rust Desktop/TUI direction and limits](docs/adr/0003-ui-framework-selection.md)
- [Architecture](docs/architecture.md)
- [Wire interfaces and conformance boundaries](docs/interfaces.md)
- [Threat model](docs/security.md)
- [Product stages, acceptance and follow-up task briefs](docs/roadmap.md)
- [Codex/Kimi context and Handoff E2E evidence (2026-11-08)](docs/evaluation/2026-11-08-codex-kimi-handoff.md)
- [P0 MCP negative-boundary test plan](docs/evaluation/p0-mcp-negative-boundaries.md)
- [Local NSH console, Windows/WSL ownership and verification quickstart](docs/p1/local-codex-e2.md)
- [P1-E2 sanitized local validation](docs/evaluation/p1-e2-local-validation.md)
- [ADR-0001: Core before GPUI / NSH interface](docs/adr/0001-core-before-ui.md)

The standalone [agent-development-protocol](https://github.com/ybakiame/agent-development-protocol) remains authoritative for agent-neutral engineering procedures. This repository **consumes** the protocol; it does not rewrite a project's `AGENTS.md`, dependencies, or CI gates.

## Test status

`PYTHONPATH=src python -m unittest discover -s tests -v` runs the Python runtime/write/reference/local-runner compatibility checks. Rust core and real IPC/MCP process tests cover the migrated negative boundaries; see the [migration matrix](docs/p1/rust-test-migration.md). [P1-I evidence](docs/evaluation/p1-i-context-bridge.md) records later local Windows/WSLg checks and exact source provenance. These checks establish their documented scope, not every harness/version, complete MCP conformance, production security or universal platform support. J changes documentation only and makes no new client/device test claim.


## Optional native agent session prototype

The Rust daemon can explicitly opt into one configured ACP v1 executable, controlled
through the NSH Desktop/TUI Sessions panel. Start requires operator confirmation;
prompts, single-request permission decisions, cancel and stop are separate actions.
Closing a UI does not stop the daemon-owned child. Default execution is disabled;
no shell, automatic startup, persistent resume or Windows→WSL execution bridge.
The agent runs with your user privileges, without an OS sandbox. Python profile
writes and the two-tool read-only MCP stay unchanged. See
[configuration and boundaries](docs/p1/agent-session-runtime.md) and
[local evidence / interoperability limits](docs/evaluation/p1-g-local-agent-session.md).


## Native agent authentication follow-up

P1-H adds bounded auth/capability observations and an explicit auth-required recovery
path. Prepare private agent authentication outside the app, then review Start again;
there is no login/credential widget or automatic retry. The optional local session
extension uses API v2; existing IPC v1 and read-only two-tool MCP are unchanged.
The default authenticated turn keeps Context MCP off; P1-I adds the separate opt-in below. See the
[authentication contract](docs/p1/agent-authentication.md) and
[real-agent evidence](docs/evaluation/p1-h-real-agent-e2e.md) for verified results,
source provenance and remaining limits.

Local P1-H evidence records an opt-in Kimi Code CLI 2.5.2 authenticated streamed
turn and separate immediate-cancel acknowledgement. Windows/ConPTY and native WSLg
auth recovery are verified with deterministic agents; real-model GUI turns and
GUI context forwarding remain unverified. Native operator context opt-in is covered below.
This is an ephemeral prototype.


## Revocable native session context

P1-I optionally attaches the existing two read-only MCP tools to one native managed
ACP session. Configure a trusted absolute `--context-mcp` executable, then explicitly
run operator `session-start <project> <agent> --confirm-start --share-context` through
`acp-query`. Existing UI Start remains unshared by default; no layout/control changes.
`session-context` reports content-free counts and lease activity; `session-stop`
requires the exact generation. Stop, prompt Cancel, expiry, failure and restart revoke
access even on an established peer. Static non-session credentials stay compatible.

The local Windows/ConPTY and native WSLg regressions pass with explicit per-gate
provenance. Deterministic ACP-to-MCP calls and negative revocation tests pass;
real Kimi Code 2.5.2 completed both tools, followed by post-Stop peer/reconnect denial
and cleanup. OpenCode 2.2.7 accepted the session, but GLM/DeepSeek free-model calls were
rejected with quota errors in both keyed and anonymous isolated tests. Its model ID
was verified; the owner demonstrated regular WSL TUI success, whose difference remains
unexplained. A separate official DeepSeek Flash turn completed, but OpenCode MCP reads
remained zero under the strict Code Mode restriction. Earlier failures are retained. See the [lease contract](docs/p1/session-context-bridge.md)
and [evidence / remaining acceptance](docs/evaluation/p1-i-context-bridge.md).
This remains a local prototype, without OS sandboxing or Windows→WSL execution.
