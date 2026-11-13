# P1 Implementation Plan — Rust Core, Local Daemon and UI Spike

> **Historical record — not a current runbook.** Original dates, source revisions
> and outcomes below are preserved. S4 retired the executable prototype and probes;
> use [Rust-only development](rust-only-development.md) for current commands.

Status: **historical P1-A implementation proposal**. The core, local service,
NSH UI and optional native sessions now exist. Use the current
[architecture](../architecture.md), [product roadmap](../roadmap.md) and
[Accepted NSH ADR](../adr/0003-ui-framework-selection.md) for current decisions.
The original slice names/statuses below are retained as planning history;
production Rust writes/migration remain unimplemented.

## Preconditions: stabilize P0 on main

PR #1 (Handoff worktree fingerprint) and PR #2 (MCP negative tests) were both merged into `main` after updating PR #2 against PR #1 and passing combined CI run #46. This implementation plan now targets that combined P0 baseline. Before implementation, run `python -m unittest discover -s tests -v` on the actual target revision and record current CI separately.

Retain the Python P0 as the executable reference until a Rust implementation passes the same behavioral checks. Do not change the public MCP names, confirmed-preference semantics, task handoff states, or project scope while replacing internals.

## Delivery slices

| Slice | Deliverable | Acceptance |
| --- | --- | --- |
| P1-A (this PR) | Versioned contract fixture, Python oracle, migration/IPC plan and UI spike criteria | Runs under existing stdlib unittest; old P0 unchanged |
| P1-B | Rust acp-core crate, read-only project/profile/handoff resolver | Consumes the same fixture; matches Python normalized response for every case |
| P1-C | Versioned SQLite migration and Rust-owned write path behind explicit CLI | Tested backup + rollback; old local profiles and handoffs remain intact |
| P1-D | Local acp-daemon with project-scoped read-only IPC and an MCP compatibility adapter | GUI exit does not end daemon; no network listener or cross-project switching |
| P1-E | Compare GPUI Kit and NSH in a throwaway UI spike | Measured target-OS, accessibility, state handling and packaging evidence; ADR selects UI |
| P1-F | Optional ACP/PTY session adapters after boundaries pass | Explicit capability matrix, no invented session state |

P1-A supplies the design and reference fixture. P1-B now has a read-only `acp-core`
library and cross-language checks; see [implementation and validation evidence](p1-b-validation.md).
P1-C1 adds a [read-only SQLite adapter](sqlite-readonly.md); P1-C2A adds an
[owned temporary migration rehearsal](synthetic-migration.md). P1-C2B1 adds
[test-only persistent process recovery](process-recovery.md). Production P1-C2B2
write/migration rollout remains unimplemented. P1-D provides a
[runnable local read-only service](local-service.md). At the original P1-E checkpoint,
the [isolated comparative UI experiment](ui-spike-evaluation.md) had not accepted a
framework or production UI rollout, and the planned P1-F session-adapter slice was
not implemented. Those are historical plan statuses; the current
[roadmap](../roadmap.md) records the later UI/session work, and
[ADR-0003](../adr/0003-ui-framework-selection.md) now accepts NSH Rust Desktop/TUI.
Remote CI/platform results must be reported separately from local validation.

P1-E2 developed the [read-only NSH operator console](local-codex-e2.md), prioritizing
Windows desktop/ConPTY and explicit Windows-owner access to WSL. Verification at
that checkpoint was local-only while Actions quota was exhausted; see the
[sanitized measured evidence](../evaluation/p1-e2-local-validation.md).
ADR-0003 was Proposed at that checkpoint; its current status is
[Accepted for NSH Rust Desktop/TUI](../adr/0003-ui-framework-selection.md).
Mobile remains outside current product targets.

## Crate layout (core/store/rehearsal and local service crates exist)

```text
crates/
  acp-core/       # portable domain types, precedence and validation
  acp-store/      # read-only SQLite adapter; migrations deferred
  acp-ipc/        # local transport, DTOs and capability boundary
  acp-daemon/     # supervisor process, local endpoint and audit events
  acp-mcp/        # project-scoped MCP compatibility surface
apps/
  desktop-spike/  # separate, disposable GPUI Kit or NSH experiments
```

Do not replace Python's `acpctl` command until migration and recovery have been rehearsed on synthetic and backed-up existing data. UI calls APIs rather than querying SQLite directly.

## P1-A validation

```bash
PYTHONPATH=src python -m unittest discover -s tests -v
PYTHONPATH=src python -m unittest discover -s tests -p test_p1_reference_contract.py -v
```

The fixture and its Python test are portable reference behavior. Git metadata, timestamps, preference IDs and filesystem paths are excluded from golden equality because those change per checkout. No fixture contains real user information.

## Evidence to preserve

Record Rust/Python version, OS, Git revision, SQLite schema version, tool/IPC response deltas, bounded failures, denied operations, the exact project scope and any manual device/GUI tests. Keep only sanitized synthetic evidence in Git.

See [Rust parity and migration](rust-core-migration.md), [daemon and IPC](daemon-ipc.md) and [GUI/TUI spike](gui-tui-spike.md).
