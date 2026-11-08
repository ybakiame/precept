# P1 Implementation Plan — Rust Core, Local Daemon and UI Spike

Status: **implementation proposal** (not a claim that Rust code or GUI exists).

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

Only P1-A is started here. P1-B onward require a native Rust toolchain and CI builds; **do not mark complete from documentation**.

## Suggested new crate layout (not created yet)

```text
crates/
  acp-core/       # portable domain types, precedence and validation
  acp-store/      # SQLite adapters and versioned migrations
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
