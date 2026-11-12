# Agent Control Plane

**A local-first, developer-centric control plane for engineering context, project rules and continuity across coding agents and harnesses.** Make the right context available to the right agent, explain why it applies, preserve it across Handoffs, and distinguish reported work from observed or independently verified evidence. See the [product charter](docs/product.md).

> Implementation maturity: `0.0.x-rc` engineering prototype. [ADR-0006](docs/adr/0006-rust-only-runtime.md) is accepted: Rust is the only target product runtime; the first Rust-native Profile schema is v1 (`PRAGMA user_version=1`). This is a fresh schema, not compatible with the retired Python prototype's legacy v1 or M1's synthetic v2. Both are disposable validation artifacts; no migration or backward-compatibility promise is made. The Python P0 prototype is scheduled for removal in [P1-M2 / Issue #40](https://github.com/ybakiame/agent-control-plane/issues/40). The current main tree has not completed that cutover. Breaking changes are expected during validation; do not preserve old Python commands, local prototype data, legacy schemas, or obsolete wire interfaces for compatibility. [NSH Rust is the accepted single Desktop/TUI UI direction](docs/adr/0003-ui-framework-selection.md). No production readiness, OS sandbox or universal vendor compatibility is claimed.

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

## Runtime status

There is no supported end-user installation or quickstart until the Rust-only
runtime cutover is complete. The Python CLI/MCP examples from the P0 prototype are
intentionally no longer documented as product entrypoints. P1-M2 will deliver
explicit Rust-native Profile v1 creation, native operator commands, daemon-owned writes, and
the Rust MCP/NSH integration. See [ADR-0006](docs/adr/0006-rust-only-runtime.md)
and [Issue #40](https://github.com/ybakiame/agent-control-plane/issues/40).

## Data / trust rules

- The accepted target uses an explicitly operator-created, user-local Rust-native Profile v1. It is not compatible with the old Python v1 schema or M1's synthetic v2. Startup does not scan for, adopt, or migrate existing databases; an explicit development reset may delete disposable ACP prototype data.
- All preference proposals start as **suggested**. Confirmation requires a deliberate operator action on the exact record/revision; agents and MCP cannot confirm or mutate profile data.
- Project-scoped confirmed values mask global confirmed values with the same key. **Neither overrides the project's rules.**
- Project files are re-read live when requested; default output includes only allowlisted document paths, sizes and hashes. Document contents require explicit opt-in. Symlinks escaping the registered checkout are blocked.
- A Handoff is historical continuity, not proof that a check passed on the current HEAD. Resume compares revision, branch, worktree and bounded fingerprint state.
- Do not store secrets, credentials, personal device identifiers, raw transcripts, private customer data, or unredacted logs. MCP clients run under the user account and can read exposed data; only attach to trusted local harnesses.

## Repository structure

```text
crates/
  acp-core/       # domain and typed context resolution
  acp-store/      # strict read-only Rust-native Profile v1 access
  acp-write/      # private transactional writer; daemon-only
  acp-daemon/     # local service and sole write owner
  acp-ipc/        # versioned local capabilities and transport
  acp-query/      # native operator CLI
  acp-mcp/        # project-bound read-only MCP stdio
  acp-session/    # optional native ACP session supervisor
spikes/p1-e/      # shared NSH Desktop/TUI presentation
docs/             # architecture, ADRs, interfaces, risks and roadmap
```

The Python P0 package, tests, legacy Python v1 profiles and M1 synthetic v2 profiles are disposable validation artifacts. P1-M2 removes the product paths and establishes a fresh Rust-native Profile v1; no migration, backward-compatibility or data-retention guarantee is made.

## Design and roadmap

- [Product charter and capability boundaries](docs/product.md)
- [Domain, human review and evidence trust ADR](docs/adr/0004-product-domain-trust.md)
- [ADR-0006: Rust-only runtime and Python prototype retirement](docs/adr/0006-rust-only-runtime.md)
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

M1's [synthetic Rust writer evidence](docs/evaluation/p1-m1-synthetic-writer.md)
records the historical synthetic v2 transaction, CAS/receipt and strict-transition
checks for PR #39. That schema is not the product schema. Rust core and real IPC/MCP process tests cover the
current Rust boundaries. The [Python-to-Rust migration matrix](docs/p1/rust-test-migration.md)
is historical evidence, not a commitment to retain Python. P1-M2 removes the Python
runtime/tests and establishes the Rust-only current-schema acceptance matrix. Old
local prototype databases may be deleted during an explicit development reset. Local evidence
does not imply every harness/version, complete MCP conformance, production security
or universal platform support.

## Optional native agent session prototype

The Rust daemon can explicitly opt into one configured ACP v1 executable, controlled
through the NSH Desktop/TUI Sessions panel. Start requires operator confirmation;
prompts, single-request permission decisions, cancel and stop are separate actions.
Closing a UI does not stop the daemon-owned child. Default execution is disabled;
no shell, automatic startup, persistent resume or Windows→WSL execution bridge.
The agent runs with your user privileges, without an OS sandbox. Profile write IPC is not yet enabled; P1-M2 delivers the Rust-only operator write path. MCP remains project-bound and read-only, but its wire schema may evolve in lockstep during validation. See
[configuration and boundaries](docs/p1/agent-session-runtime.md) and
[local evidence / interoperability limits](docs/evaluation/p1-g-local-agent-session.md).


## Native agent authentication follow-up

P1-H adds bounded auth/capability observations and an explicit auth-required recovery
path. Prepare private agent authentication outside the app, then review Start again;
there is no login/credential widget or automatic retry. The optional local session
extension currently uses API v2; IPC, session and MCP interfaces may change together during validation, with no old-version compatibility promise.
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
access even on an established peer. These are current security semantics, not a promise to preserve old credential or wire formats.

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
