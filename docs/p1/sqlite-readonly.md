# P1-C1: read-only SQLite adapter

Issue #6; base main `afce07fed08df03b9fe66e308fb83220a0ef15f3`.
The implementation lives in `crates/acp-store`; `acp-core` has no SQLite dependency.
Python P0 application code, CLI and MCP remain unchanged and retain all writes.

## Ownership and connection contract

`SqliteStore::open(&Path)` requires an explicit existing database file. There is
no default profile lookup, use of `AGENT_CP_HOME`, directory creation, migration,
DDL, backup/checkpoint command or exposed connection/write method. Errors are
sanitized enum diagnostics, without database paths, SQL or record contents.
The store implements `ProjectRegistry`, `PreferenceRepository` and
`HandoffRepository`; storage-specific diagnostics map to safe core errors.

The dependency is exact `rusqlite = 0.41.1` with `bundled`, also pinned in
`Cargo.lock`. A maintained binding avoids hand-written SQLite FFI; bundling pins
the native library via `libsqlite3-sys` and avoids different system SQLite APIs
across CI machines. Building requires a native C compiler (provided in CI).
Python's SQLite library is independent: parity checks cross that boundary.

Connections use `SQLITE_OPEN_READ_ONLY` without CREATE or URI flags. The library
additionally enables connection-local `query_only`, disables trusted schema and
sets a five-second busy timeout. There is no extension-loading feature/API.
The raw connection is private. A test disables query_only on the **actual**
adapter connection and verifies an UPDATE still fails with SQLite ReadOnly.
Read-only mode is a database access mode, not an OS security sandbox; another
same-user process can still access local files directly.

## Existing schema detection

Both existing P0 formats have `PRAGMA user_version = 0`. The adapter inspects it
without setting it and distinguishes these shapes:

| Shape | Handoff fingerprint | Rust representation |
| --- | --- | --- |
| Legacy | No column | `None` |
| Fingerprinted | Nullable TEXT column, regardless of column order | NULL => `None`, otherwise recorded string |

P1-C2A extends read support to user_version=1 with the fingerprinted shape;
opening still never migrates. Other versions are rejected as UnsupportedVersion. An uninitialized,
corrupt or incompatible database yields IncompatibleSchema. Detection requires
exact known table/column sets, declared types and primary-key positions; extra
views/triggers/tables/columns and generated columns are rejected. Internal SQLite
objects and indexes are not application schema additions. This is structural
compatibility checking, not a byte-for-byte DDL/constraint attestation. Rows are
still validated; unknown review states, negative preference IDs, invalid field
types, invalid validation JSON/statuses and nonabsolute project roots fail closed.
Malformed scope/project combinations are excluded before returning preferences.
Schema/version inspection repeats before reads so an external schema change is
not accepted indefinitely using a cached opening result. This does not make a
multi-method resolver request a single atomic snapshot against concurrent DDL.

Bound project/task queries use parameters. Missing tasks never fall back to a
different project; all review states are typed, and the core applies confirmed
filtering, project precedence and newest-ID ordering. Original source and IDs
are preserved. Validation evidence remains opaque nested JSON, including keys
named `id` and `updated_at`; only check/status are typed.

## WAL and concurrent readers

This adapter does not force journal mode or use `immutable=1`: immutable mode
could hide live committed WAL data. Independent reader connections see committed
updates on their next query; uncommitted writes remain invisible. SQLite may
need existing/readable WAL/shared-memory sidecars or permission to initialize
read-side WAL bookkeeping. It can report a clean failure if those prerequisites
are unavailable. We do not promise unchanged atime, shared-memory bytes, sidecar
existence, or OS-wide checkpoint behavior. No adapter code requests a checkpoint.

Tests compare main database bytes and SQL dumps before/after reads while no
external writer mutates the data. This establishes those observed invariants in
the tested scenarios, not immutability of every filesystem metadata field.

## Production P1-C2B design only (not implemented)

The operator owns the database and backup location. An explicit later write tool
must identify the source version, reject unknown future versions, stop/quiesce
all writers, and create a consistent backup through the SQLite backup API.
Copying the main file without its live WAL is insufficient. Verify the backup
by reopening it separately, checking integrity and comparing known synthetic
records before authorizing migration. Backups need the same private permissions
and retention policy as the database; no backup contents belong in Git or logs.

A future migration should use one transaction for additive DDL/data changes and
version update, with failure rollback before commit. Preflight the exact schema
and available disk space; do not start from an unknown version. Validate the
migrated copy with both readers before switching ownership. For recovery after
a committed migration, quiesce processes, close connections, verify the retained
backup and restore consistently without orphaned WAL/SHM files. Crash injection,
lock contention, disk-full, interrupted backup/migration/restore, idempotence and
rollback tests on synthetic data are prerequisites to any real write rollout.
No real-data migrations are authorized here. P1-C2A implements only an
[owned temporary synthetic rehearsal](synthetic-migration.md), in a separate crate.

## Reproduction and observed local evidence

Linux x86_64, Python 3.12.14, rustc/Cargo 1.99.1. Baseline main: 10 Rust tests and
21 Python tests passed before changes. From the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo build --locked --example contract_reader
cargo build --locked -p acp-store --example sqlite_reader
PYTHONPATH=src python tools/check_p1_parity.py
PYTHONPATH=src python tools/check_sqlite_parity.py
PYTHONPATH=src python -m unittest discover -s tests -v
```

Local results: all commands PASS; 16 Rust tests (10 core, 1 store unit, 5 store
integration), 21 Python regression tests, 120 existing JSON contract comparisons,
and 78 new exact SQLite comparisons across 13 scenarios. Comparison counts are
request executions, not independent tests. The SQLite runner compares the same
stored IDs, timestamps and entire nested evidence without normalization. A small
read-only Python facade uses existing Store read methods without running the
schema-writing constructor, mapping a missing legacy fingerprint to logical NULL.

`tools/sqlite_fixture.py` creates disposable Python databases from the existing
v1 golden preference fixture. The legacy fixture drops the nullable column only
in synthetic setup; neither reader migrates it. Rust integration tests spawn
Python for fixture creation (`ACP_TEST_PYTHON` can select the interpreter).
No generated SQLite files or actual host paths/data are committed or logged.
The SQLite reader example is a bounded synthetic JSON test harness, not a new
operator CLI or MCP implementation. Supply `--reader PATH` to parity scripts
when executables are elsewhere (Windows requires the `.exe` suffix).

Coverage includes both shapes, NULL/present/absent fingerprints, original scopes
and IDs, all review/validation states, duplicates/newest confirmed records,
shared task IDs, unknown projects/tasks, invalid scope/status/JSON, nonexistent,
empty and corrupt files, future/extra/generated schemas, read denial for writes,
four concurrent reader threads, uncommitted isolation and committed WAL updates.
The CI matrix runs all existing gates plus SQLite parity on Linux/macOS/Windows;
its actual results and exact tested commit are recorded in the Draft PR.

NOT_RUN: real profile databases, permission behavior under other OS accounts,
network filesystems, corruption recovery, process-kill/disk-full/long-running
contention, production backup/migration/rollback, native Git adapter, real MCP
clients, daemon/GUI/ACP sessions. Cross-platform results must come from CI rather
than inference from the local Linux run.
