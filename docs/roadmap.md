# Roadmap and acceptance evidence

## P0 — context hub prototype (this repo)

- [x] Local SQLite project registry, explicit suggestion/review lifecycle, scope-based resolution.
- [x] Live narrow project-contract discovery, revision/dirty-worktree inspection.
- [x] Task Handoff snapshot and stale-state warning on resume.
- [x] Optional bounded worktree fingerprint on new handoffs; old snapshots remain readable without claiming verification.
- [x] Two read-only MCP tools through stdio, project scoped at process start.
- [x] CLI operator workflow and standard-library unit/integration tests.
- [x] Isolated negative tests for unconfirmed/rejected preference hiding, project-specific scope and read-only MCP; [evidence and real-harness limits](evaluation/p0-mcp-negative-boundaries.md).
- [x] User-reported read-only context and Handoff E2E across Codex + GPT-6.1 Sol and Kimi Code + Kimi K3.1 Pro; [evidence and limits](evaluation/2026-11-08-codex-kimi-handoff.md).
- [ ] Repeat under independently captured tool traces and add Pi/OpenCode and production task handoff trials; record versions, permissions and privacy boundaries.
- [ ] Versioned schema migrations, database backup/export/import, explicit user profile lifecycle and fuzz testing.
- [ ] Full 2026 MCP conformance and a tested backward-compatibility matrix.

## P1 — Rust daemon and interface spike

- Stable Rust domain core with migration tool and contract tests against the P0 behavior.
- Daemon detached from GUI lifecycle; local IPC and per-client access controls.
- ACP client where supported, PTY/CLI fallback with explicitly narrower observable state.
- Context Inspector, profile review, task handoff visualizations.
- Compare GPUI + GPUI Kit with `longbridge/nsh` (GPUI/TUI) using a measured spike on target OSs and keyboard/accessibility behavior; then select UI shell.

## P2 — durable engineering experience

- User-consented preference suggestions with explainable provenance and de-duplication.
- Optional semantic memory adapter with staleness/retention policies, never authoritative.
- Redacted session audit, protocol conformance benchmark against task completion and false PASS claims.
- Remote sync only with explicit encryption, account boundary and consent design.

## P0 acceptance matrix

1. A suggestion does not reach MCP until the user confirms it.
2. Confirmed project-scoped preferences override global preferences of the same key, without replacing project rules.
3. A client launched for project A cannot query project B using MCP tool arguments.
4. A symlink escaping the project root is not followed.
5. A changed HEAD/branch or dirty worktree is reported on handoff resume; old validation is not treated as current. New handoffs should detect bounded uncommitted content drift even at identical HEAD and dirty count; older handoffs report that the fingerprint is missing.
6. Legacy and stateless MCP clients can list/call supported tools in in-process tests; the actual stdio subprocess is exercised.

No passed test here proves production security or compatibility with a specific external harness. Real A/B evaluation belongs in the existing agent-development-protocol's cross-agent evaluation workflow.


## P1-G local single-agent slice

The optional Rust daemon supervisor and shared NSH Sessions panel implement explicit
operator start/prompt/cancel/permission/stop on one native local host. Context/MCP
remain read-only. This is an ephemeral prototype, not a fleet scheduler, sandbox,
persistent vendor resume or Windows→WSL execution bridge. Deterministic fixture
results and any real-agent gaps are recorded in
[local session evidence](evaluation/p1-g-local-agent-session.md). ADR-0003 stays Proposed.


## P1-H private authentication and real ACP turn

The existing native single-agent slice observes authentication/capabilities without
executing advertised login commands. Operator-prepared private authentication and
explicit Start review remain separate from permission approval. Session extension
API v2 rejects older session requests explicitly; core IPC/MCP/SQLite stay unchanged.
The optional context bridge is deferred until safe per-session credential revocation
exists; no operator or long-lived project credential is forwarded. See
[contract](p1/agent-authentication.md) and [local evidence](evaluation/p1-h-real-agent-e2e.md).


## P1-I revocable scoped session context

An explicit per-start native operator opt-in can attach the two-tool stdio bridge
using a separate ephemeral project/agent/generation credential. Every request and
response admission checks the lease; Stop, Cancel, expiry and lifecycle cleanup
revoke established peers too. Existing UI and non-session clients remain compatible.
Local deterministic security/context tests and Windows/WSLg regressions pass; real
Agent tool-call acceptance remains unverified after an incomplete original attempt.
See [contract](p1/session-context-bridge.md) and
[exact local evidence](evaluation/p1-i-context-bridge.md). No production isolation or
framework/brand acceptance is implied; Draft source review only.
