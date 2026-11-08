# P1-C2A: synthetic-only backup, migration and recovery rehearsal

Issue #8; base main `a7a630dd318d79fd07f4af415cb45380287d7796`.
This is **not approval to migrate or back up a personal/local profile**. Python
continues owning writes. No CLI/MCP application source, auto-start behavior,
daemon, GUI, session API or default state-directory discovery is changed.

## Component and path boundary

`acp-rehearsal` is separate from `acp-store` and its private read-only connection.
`SyntheticWorkspace::new()` owns a fresh temporary directory and cannot adopt an
existing workspace. Its source, backup, migration output and recovery paths must
be explicit descendants with normal components. Missing parents, traversal,
symlinks, Windows reparse points and existing targets/sidecars are rejected.
Unix source/sidecar hardlinks are rejected. Backup/output files are exclusively
reserved (`create_new`); Unix workspace/files are 0700/0600. Windows relies on the
temporary directory's inherited ACL; it does not claim Unix-mode equivalents.

Sources are always opened read-only. Migration only writes an output created by
this workspace from a verified backup. Retry requires registered output/backup
paths and compares retained logical records; restore rechecks the original
verified backup's schema/records and always creates another file. Changed backups
are rejected. All handles close before cleanup; there is no overwrite recovery.
The workspace owns and deletes these synthetic artifacts when dropped. This is
not persistent production backup storage, a hostile same-user sandbox or a proof
that a malicious caller placed synthetic content in its directory. Do not import
real files into the owned workspace. Path checks do not claim to eliminate all
adversarial same-user race conditions or Windows hardlink attacks.

The example accepts **only** `--synthetic-only`, no source/profile path arguments.
It invokes the repository's synthetic Python builder under the owned workspace,
chooses explicit descendant paths and runs the entire rehearsal. The Python
compatibility helper also requires the guard and a temporary rehearsal root;
it is a test helper, not a user migration command. Diagnostics contain stage/error
enums, not host paths, SQL, records or evidence. Schema/record comparison data stays
in process memory and temporary SQLite files, never in Git or public logs.

## Backup and integrity contract

The source is a pinned read transaction. SQLite's backup API copies that snapshot,
including committed WAL data and excluding uncommitted changes. We never copy a
live main file using filesystem copy. Copying has bounded steps and fails promptly
on busy/locked state; no unbounded retry loop runs. Fault injection can interrupt
after a one-page step or simulate a backup write error. Unverified partial output
is removed after its connections close; it is not registered as a usable backup.

Verification closes the destination writer, reopens it independently read-only,
runs integrity/foreign-key checks, validates schema and compares deterministic raw
records with the source snapshot. Comparison includes every project, all preference
states, IDs/source/timestamps, handoff IDs, raw validation JSON, nested evidence
and sqlite_sequence. Missing legacy fingerprint is compared as logical NULL.
The verified backup is not migrated or modified. Tests preserve its bytes through
retry/restore. We do not promise unchanged atime, SHM bytes, sidecar existence or
OS-wide checkpoint behavior. No code requests a checkpoint. A snapshot need not
include commits made after its read transaction began; production writer
quiescence/ownership remains a separate requirement.

## Exact schema and transaction

The read adapter now accepts both v0 shapes and v1; opening it still never writes.
Unknown versions fail. V1 without fingerprint is incompatible. For migration,
inspection is deliberately stricter: known P0 table/index definitions (including
CHECK/FK/UNIQUE/nullability/defaults), exact columns and primary keys are required.
Both Python's initial fingerprint placement and SQLite's appended-column layout
are accepted. Unknown tables, views, triggers, generated/extra columns, changed
constraints/indexes, corruption and mismatched versions are rejected.

Migration takes BEGIN IMMEDIATE on the **output**, validates schema and expected
records under that writer lock, and performs only:

1. Legacy v0: add `worktree_fingerprint TEXT` with implicit NULL.
2. Either v0: set user_version=1 in the same transaction.
3. Verify v1 shape and exact records before commit, then verify after commit.

There is no data rewrite. Exceptions before commit drop/roll back the transaction,
including DDL and version. Valid v1 retry returns AlreadyV1 without changes.
An injected AfterCommit error means the output **is v1**, not a rolled-back v0;
the backup stays v0 and restoration produces a separate v0 file. Retry closes and
reopens connections, revalidates evidence and either migrates v0 or confirms v1.
There is intentionally no API to resume/adopt arbitrary profiles after process
restart; actual process-kill recovery/persistent manifests belong to later work.

## Python compatibility is executed

The guarded example tests both starting shapes. A read-only facade first compares
all original records in source/backup/output/restore. It then opens the **actual
current Python Store constructor** on the v1 output and checks schema and old
records remain intact. Explicit synthetic writes exercise suggested→confirmed
→deprecated, suggested→rejected and handoff writes with nested evidence. Python's
version remains 1. The existing compiled Rust SQLite reader then exactly matches
Python context/handoff responses from that same Python-written database (four
requests per shape). No claim is based solely on the facade avoiding initialization.

## Fault evidence and limitations

| Scenario | Injection/condition | Expected observed recovery |
| --- | --- | --- |
| Interrupted backup | After first one-page backup step | No published backup; retry succeeds |
| Backup write failure | Deterministic InjectedWrite before copy | Own reserved file removed; source unchanged |
| After DDL | Error after nullable column addition | DDL and version rolled back to original legacy v0 |
| Before commit | Error after user_version=1 inside transaction | Column/version rolled back; verified backup retained |
| After commit, before verification | AfterCommit error | Output v1, backup v0; restore v0 and retry no-op |
| Writer contention | Real BEGIN IMMEDIATE held by another connection | Locked; backup intact; retry after release succeeds |
| Schema drift | External synthetic extra column | Refused without mutation; backup remains restorable |
| Backup tamper | External synthetic row change | Restore/retry refused by retained record comparison |
| WAL snapshot | Real writer with committed plus uncommitted updates | Backup includes only committed state |
| Independent readers | Four threads, separate read-only v1 connections | Read successfully; output bytes unchanged |
| Paths/locks | Collisions, sidecars, symlinks and unknown versions | No caller-owned file is overwritten |

InjectedWrite is a simulation, **not** an OS disk-full or I/O fault. Interrupted
backup and postcommit failures are deterministic software injection, not process
kill or power loss. Connection close/reopen retry was executed; actual process
restart recovery, physical disk-full/read-only-volume failures, long-running lock
stress, crash/durability/fsync behavior, network filesystems, other-account ACLs,
Windows hardlink attacks and adversarial race resistance are NOT_RUN. Windows
file/directory symlink refusal and cross-platform SQLite locking run in CI; Unix
hardlink/mode tests are compiled only on Unix. No production data was used.

## Reproduction and local results

Linux x86_64 / Python 3.12.14 / Rust and Cargo 1.99.1. Baseline main passed
16 Rust + 21 Python tests before edits. Use the pinned toolchain/lockfile:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo build --locked --example contract_reader
cargo build --locked -p acp-store --example sqlite_reader
PYTHONPATH=src python tools/check_p1_parity.py
PYTHONPATH=src python tools/check_sqlite_parity.py
PYTHONPATH=src python -m unittest discover -s tests -v
cargo run --locked -p acp-rehearsal --example synthetic_rehearsal -- --synthetic-only
```

Observed local PASS: fmt/Clippy, 28 Rust tests (10 core, 7 store, 11 rehearsal),
21 Python regressions, existing 120+78 comparisons and two complete guarded flows
with eight additional exact v1 read comparisons after actual Python writes.
No local tests failed or were ignored in the final run. The Windows-specific test
is compile-disabled locally; it is not a local pass. Remote matrix status and
exact tested SHA are recorded in the Draft PR. Earlier Clippy findings about
module order/style were corrected and Clippy rerun successfully.

## Production rollout gate (future P1-C2B, separate authorization)

Before any production migration: require explicit operator opt-in for exact
profile and backup paths, exclusive writer ownership/quiescence across all
Python/Rust/MCP processes, authenticated lock ownership and stale-lock recovery,
verified backup with private permissions/retention and tested disk-space/failure
handling. Version recovery manifests and audit stages without copying records to
logs. Rehearse backup verification, transaction failure and committed-state
restore under process-kill/disk-full tests on each supported OS. Restore must close
all handles and account for WAL/SHM, verify backup integrity and version before
replacing any destination, and preserve a recoverable prior copy. Stage rollout
on copies with both readers before any write-owner switch. V1 itself confers no
consent, and this rehearsal does not authorize real-profile migration or restore.
