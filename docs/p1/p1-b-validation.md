# P1-B read-only Rust core — validation evidence

Implemented for issue #4 on `design/p1-rust-daemon-contract`, not the older P0
revision. Python P0 source, CLI, MCP responses, existing fixture and existing tests
are unchanged. No user SQLite files were opened, migrated or written by Rust.

## Boundary and usage

`acp-core` is a library with typed project, preference, review state, worktree,
validation and handoff records. `ContextResolver` pins a project at construction,
rechecks adapter record scope, returns confirmed preferences in Python ordering
(project before global, newest ID within scope), and preserves P0 handoff warning
semantics. `ProjectOverride`, `UnknownTool` and malformed-argument errors deny
selectors and unsupported operations before reading data. No mutation methods exist.

`ProjectRegistry`, `PreferenceRepository`, `HandoffRepository`,
`WorktreeInspector`, and `DocumentRepository` are injected read-only interfaces.
The built-in permission evaluator accepts only the two existing read tool names;
it is not an MCP transport implementation. The optional `FileDocuments` adapter
reads only the five P0 allowlisted document paths, with a 16 KiB limit, SHA-256,
lossy UTF-8 and explicit content opt-in. It rereads files on every request and
denies canonical paths outside the project. It does not scan directories.
Canonical path checks are best-effort, not protection against hostile same-user
filesystem races. Errors omit record values, selectors and host paths.

The synthetic `contract_reader` example consumes JSON adapters for validation only.
It does not open SQLite or replace `acpctl`. The parity runner creates disposable
Python state using the existing v1 fixture, feeds those records to Rust, compares
normalized responses and asserts forbidden markers never appear in alpha.
It removes storage IDs, timestamps and host roots while retaining relative
document paths, content hashes, preference provenance and warning order.
Worktree evidence is injected identically into both resolvers, explicitly testing
matching, changed and unavailable evidence rather than pretending to inspect Git
in Rust. All PASS/FAIL/SKIPPED/NOT_RUN statuses remain recorded evidence, never
permission to act or proof that prior checks remain valid.

## Local results (2026-11-08, UTC+8)

- Base commit: `dbc5006e03d107ea599d3d3260a4a68648af6e67` (fetched P1 design branch HEAD).
- OS: Linux x86_64; Python 3.12.14; rustc 1.99.1; Cargo 1.99.1.
- Python synthetic SQLite uses the existing P0 schema with nullable fingerprint;
  Rust has no SQLite adapter or schema migration in this slice.
- `cargo fmt --all -- --check`: PASS.
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
- `cargo test --workspace`: PASS, 10 integration tests; library/doc targets have
  no additional tests. Includes the unchanged golden fixture, denied requests,
  invalid records, precedence, scope, live documents, symlink escape and handoffs.
- `cargo build --locked --example contract_reader`: PASS.
- `PYTHONPATH=src python tools/check_p1_parity.py`: PASS, 120 request comparisons
  across six batches of 20 request cases (not 120 distinct tests): golden scope/default vs included documents,
  illegal selectors/writes/malformed arguments, project-scoped tasks (including
  the same task ID in two projects), legacy NULL fingerprints, current evidence
  changes/unavailability, validation provenance (including nested evidence), file size boundaries, invalid UTF-8 and escaping symlinks.
- `PYTHONPATH=src python -m unittest discover -s tests -v`: PASS, 21 tests.
  Baseline before implementation also passed the same 21 tests.

`Cargo.lock` and `rust-toolchain.toml` pin dependencies and compiler. Reproduce
from the repository root with the commands above; optionally add `--locked` to
all Cargo build/test/lint commands. `tools/check_p1_parity.py --reader PATH`
selects the built example (use `.exe` on Windows).

## NOT_RUN / limitations

macOS and Windows execution, Python 3.11/3.13, real client/harness integration,
permission-denied file access under other OS accounts, adversarial filesystem
races, real SQLite legacy files/locking/migrations, a native Rust Git fingerprint
adapter and cross-process concurrency were NOT_RUN. Traits require trusted
adapters and do not constitute an OS sandbox. Core JSON serialization parity is
not complete MCP protocol conformance. CI defines Linux/macOS/Windows jobs; local
results do not establish those remote jobs passed. No daemon, network listener,
IPC credentials, GUI/TUI or ACP session management is implemented.

## Review follow-up

The Windows legacy SQLite fixture now uses `contextlib.closing` as well as a
transaction context, releasing the connection before reopening the store and
cleaning up its temporary directory. This test-only fix is isolated in commit
`e7f6c6a`; Python P0 application source remains unchanged.

Rust validation DTOs now flatten additional JSON keys, preserving Python-permitted
provenance without relaxing typed `check`/`status` validation. A Rust handoff
round-trip regression and a new parity request test `environment`, `revision`,
nested evidence, nulls, arrays, booleans and numeric metadata. Validation records
are compared exactly before normalization can hide `id` or `updated_at` inside
opaque evidence. Local fmt/Clippy/build, 10 Rust tests, 21 Python tests and all
120 comparisons passed again. Remote matrix status is reported on PR #5.
