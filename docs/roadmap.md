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
