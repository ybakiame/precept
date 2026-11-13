# Rust Core Migration and Behavioral Parity

> **Historical record — not a current runbook.** Original dates, source revisions
> and outcomes below are preserved. S4 retired the executable prototype and probes;
> use [Rust-only development](rust-only-development.md) for current commands.

## Preserve P0 semantics first

Python P0 is the initial behavior oracle, not a permanent runtime dependency. Implement the Rust project/profile/handoff domain without selecting a UI library. The shared fixture at `tests/fixtures/p1-preference-contract.v1.json` defines version 1 of a deliberately small, stable application-level behavior contract.

- Only `confirmed` preferences reach agent-facing effective context. Never return `suggested`, `rejected` or `deprecated` data through read-only MCP.
- Project-scoped confirmed preferences mask global confirmed values with the same key; they never override the target project's contract/ADRs.
- Each MCP process is bound to one registered project. Tools reject injected `project`/`project_id` selectors and unsupported mutations.
- `get_handoff` returns only the bound project's task. Validation statuses are PASS, FAIL, SKIPPED or NOT_RUN; earlier PASS never certifies a later revision or device.
- Live project docs are allowlisted and size-bounded. Reject symlinks escaping the project root; do not load all repo files or follow arbitrary URLs.
- New handoffs include a best-effort worktree fingerprint when PR #1 is merged; legacy NULL fingerprints remain readable but unverified. A matching hash is *not* full-state attestation.

## Versioned API / DTO boundaries

Use serializable domain DTOs with an explicit `api_version` in **new internal IPC messages**, not by silently changing the existing MCP JSON payload. Keep public fields and tool names unchanged until an intentional versioned migration. Rust may add internal detail but must normalize to the Python response for the fixture.

Separate interfaces: `ProjectRegistry`, `PreferenceRepository`, `HandoffRepository`, `WorktreeInspector`, `ContextResolver`, `PermissionEvaluator`. Use trait-based adapters; the core should not depend on GPUI, NSH, MCP transport, the terminal or OS window APIs.

## SQLite migration safety

1. Before a write-capable Rust migration, identify every schema variant encountered in operator installations (initial P0 and nullable-fingerprint PR #1).
2. Introduce `PRAGMA user_version` or a schema migration table *with explicit backwards checks*. Existing databases without it should be treated as legacy version 0; never reset them.
3. Quiesce writers and MCP processes, checkpoint WAL, take a verified copy using the SQLite backup API, and validate the copy in a fresh temp directory.
4. Run additive, transactional migrations; make migrations idempotent and reject unknown future schema versions.
5. Reopen with both reference and Rust readers, compare synthetic known records, then test operator-confirmed writes in Rust.
6. Prove rollback from backup and handling of interrupted migration before switching the CLI default. Never run a migration merely because a GUI was opened.

No personal preference data, host paths, raw transcripts or real private project docs enter fixtures or test logs. SQLite file permissions are best-effort privacy protection, not isolation from same-user programs.

## Tests / exit criteria

- Rust crate builds and tests with its own pinned toolchain on targeted Linux, macOS and Windows platforms when available. Mark unavailable platforms UNVERIFIED.
- Rust reads exactly the fixture's effective preference maps, forbidden markers, rejected MCP calls, and project-specific documents.
- Legacy SQLite files from pre-fingerprint P0 and post-fingerprint P0 are readable without losing records.
- Python `acpctl` and Rust CLI match on deterministic contract fields. Report drift rather than silently correcting data.
- No dual-writer migration until cross-process SQLite locking and crash tests pass.

Do not assume a compiler is installed in a documentation-only environment. This plan authorizes no automatic source rewrite.
