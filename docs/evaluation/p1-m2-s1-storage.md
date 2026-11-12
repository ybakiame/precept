# P1-M2 / Stage 1 — fresh Profile v1 storage evidence

Authority: [Issue #43](https://github.com/ybakiame/agent-control-plane/issues/43),
parent [Issue #40](https://github.com/ybakiame/agent-control-plane/issues/40),
Accepted [ADR-0006](../adr/0006-rust-only-runtime.md), and the owner-confirmed
[Profile v1 contract](../p1/profile-v1-runtime-contract.md).
Delivery: [Draft PR #42](https://github.com/ybakiame/agent-control-plane/pull/42),
branch `feat/p1-m2-rust-only-runtime`. Base main is
`d8d48fa582f60f1dd0e00b22276e2fbb29ab5e53`.
This is storage-layer validation on fresh disposable fixtures, not completion of
M2 or production write activation. Owner confirmed the Phase 1 recommendations
and authorized this same-PR implementation after the contract-first commit.

## Exact sources

| Source | Scope |
| --- | --- |
| `f99acde` | Contract freeze precedes product implementation: explicit NOT NULL keys, strict stored types, receipt/deletion/counter consistency, 4096 limit, no-replace publication, private ACLs, Stage 1 epoch-zero boundary and canonical bytes |
| `089ff0368f4311e28fbee80480e27d84e3d59630` | Initial current-v1 reader, private writer, creation and Rust fixtures; native Windows compilation/Clippy passed but writer tests exposed extended-prefix refusal |
| `e274c3d80bef92e65019d23961f7cad981fabc75` | Windows prefix correction, allocation-history negatives and registration replay; clean full matrices passed 84 tests on each host |
| `dc791761966750fc919aec9092ebd23082844f50` | Final runtime: sidecar disappearance handling, isolated identity replacement test, pre-publication fault and UTC vectors. Clean full matrices pass 86 tests on each host |

Later changes are documentation only. Runtime results are not relabeled as tests
of a later documentation SHA. Historical M1/C2A/C2B1 outcomes retain their original
sources; no legacy parity/migration/recovery result is presented as v1 acceptance.

## Delivered boundary

- [Exact v1 schema and verification](../../crates/acp-store/src/profile.rs): five
  tables and one user index, with exact SQLite-generated unique indexes checked;
  no AUTOINCREMENT, sqlite_sequence, separate revisions or schema fingerprint.
  The frozen document DDL is compared with implementation constants by a test.
- [Strict reader](../../crates/acp-store/src/lib.rs): absolute existing-path open,
  SQLite READ_ONLY, foreign_keys/query_only on, trusted_schema off, bounded limits
  and complete schema/domain/type/receipt checks inside one read snapshot.
  Disabling query_only in a private test still cannot enable SQLite writes.
  Operator revision reads remain separate from core/agent context records.
- [Fresh creator](../../crates/acp-write/src/profile.rs) and
  [file boundary](../../crates/acp-write/src/files.rs): selected existing private
  parent, exclusive private staging file, complete initialization/independent RO
  verification, closed SQLite handles and no-replace same-filesystem publication.
  Linux uses renameat2 with RENAME_NOREPLACE; Windows uses MoveFileExW without
  replacement/cross-volume copying. No existing-profile writer adoption API exists.
  The owned Profile handle can reopen only its own created identity for tests;
  daemon ownership/restart activation is not implemented here.
- Windows explicitly creates protected current-user-only DACLs and checks owner,
  ACE rights, reparse flags, link count and volume/file identity. Unix checks UID,
  private modes, symlinks, device/inode and hardlinks. Checks fail closed; they are
  cooperative controls, not isolation from a hostile same-user process.
- [Seven typed commands](../../crates/acp-write/src/writer.rs): one immediate
  transaction, in-transaction binding/state/scope/CAS checks, checked counters,
  row revisions, exact strict transitions, atomic same-scope peer deprecation,
  deletion without content history, Handoff upsert and minimal committed receipts.
  Requests/receipts/handles and errors do not print payloads or paths.
- Receipt sequences/count, latest live/deleted versions and retained allocated IDs
  are checked together. Deletes keep their minimal historical receipts; deleting
  rows cannot reset IDs. Same-ID replay is checked before CAS/capacity, returns
  the original receipt and leaves timestamps unchanged. Directory availability
  changes do not invalidate a committed registration replay.
- File observation/canonicalization happens before the write transaction; no
  filesystem, Git or network work occurs inside mutation transactions. Snapshot
  fields are bounded trusted-caller data, not claims of fresh Git/model evidence.
  Initial epoch zero is a storage binding, not a grant. No production caller links
  the writer; no authorization/lease/epoch-advancement shortcut is provided.
- `acp-rehearsal`, M1 v2 schema/migration/backup machinery and its writer fixture
  adoption surface were removed from the workspace and source tree. Old Python
  schemas and M1 v2 are rejected; there is no converter or fallback.

MCP product code, daemon, IPC, CLI, session and UI code are unchanged. The existing
MCP process suite now includes a test-only Rust builder for fresh current-v1
fixtures instead of starting Python to build an old schema. Its original seven
scope/role/transport assertions remain. The builder has no executable entrypoint
and is not linked into any product binary. The test data is synthetic.

## Native hosts and commands

| Host | Environment |
| --- | --- |
| Linux/WSL | Arch Linux, x86_64, WSL2 kernel 6.18.54.1-microsoft-standard-WSL2 |
| Native Windows | Windows 11 10.0.26300, AMD64; isolated owned checkout |
| Both | Rust 1.99.1 (58cf8f9c3), Cargo 1.99.1 (5f94df478), locked existing caches; CARGO_NET_OFFLINE=true |

The exact-source private runner requires clean HEAD before starting. Native
Windows uses process-local MSVC/SDK/PATH and the previously checksum-verified
owned MinGit tool; no installer or persistent system configuration change occurs.
New Profile/ACL tests touch only their newly allocated disposable roots.

At clean **dc791761966750fc919aec9092ebd23082844f50**:

| Command | Linux/WSL | Native Windows |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | PASS |
| `cargo build --workspace --bins --locked` | PASS | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | PASS |
| `cargo test --workspace --all-targets --locked` | PASS, 86 | PASS, 86 |
| `git diff origin/main --check` | PASS | PASS |

Native binaries are built before the existing process tests; a previous host's
binary cache is not substituted. The totals have different platform makeup:
`acp-store` has 9 tests per host; `acp-write` has 27 on Linux and 28 on Windows
(the additional Windows extended-prefix case). One existing core test is Unix-only.
Both hosts independently run all seven real daemon/IPC/MCP process cases. There
are no ignored acceptance tests in these final storage suites. Zero-test targets
and historical counts do not inflate totals.

Existing Python baseline: 29 unchanged tests PASS on Linux only, separately from
Rust acceptance. This honors the standing prototype baseline check; it adds no
Python feature/test, compatibility promise or new product dependency. Existing
legacy fingerprint checks in the Rust workspace still run; their complete Python
retirement belongs to the later M2 slice.

## Direct adversarial coverage

| Boundary | Observed assertions |
| --- | --- |
| Fresh schema | Explicit absent destination; exactly one metadata row; independent random identities; epoch 0/next ID 1/sequence 0; strict DDL matches contract |
| Creation refusal | Existing file/directory and sidecar sentinels preserved; relative/control paths refused; concurrent creators have one no-replace winner |
| Creation failure | Before-COMMIT injection removes only the owned staging artifacts and preserves a neighbor; after-publication injection preserves the valid DB, returns OutcomeUnknown and refuses overwriting retry |
| Read-only/schema | SQLite write refusal even with query_only disabled; missing/empty/non-SQLite, old-version/old-layout and drifted schemas refuse unchanged; extra tables/indexes/views/triggers and missing/changed columns refuse |
| Stored validity | REAL/invalid counters, missing metadata/history, inconsistent revisions, malformed JSON/status/snapshot/date/digest, foreign-key failures and generation mismatch refuse without repair |
| Seven commands | Positive registration/suggestion/confirm/reject/deprecate/delete/Handoff; full 12-cell transition matrix; invalid inputs/unknown projects, scope mismatch and stale CAS are logical no-ops |
| Atomicity and races | Independent connections racing a revision have one winner; failed peer confirmation rolls back target and peers; injected data/precommit/full failures preserve all rows/counters/receipts |
| Busy and capacity | Real SQLite write contention returns bounded Busy; a read snapshot blocking COMMIT gets an explicitly rolled-back Busy; fresh request retry is caller-controlled. Engine max_page_count produces actual SQLITE_FULL with logical rollback; this is not physical disk exhaustion |
| Receipts and deletion | Lost-response replay after connection reopen; canonical outer JSON replay; changed payload/raw JSON bytes conflict; at 4096 receipts exact replay works and new writes refuse; deleted IDs never reused; erased receipt history/counter drift/resurrection markers refuse |
| Files and privacy | Reparse/symlink/private-mode/ACL/hardlink refusal; replacement copy has the same valid schema/binding and correct private permissions before identity substitution is rejected; Windows extended paths resolve the same parent; Debug/errors/results exclude payloads and paths |
| UTC | Rust calendar vectors include epoch/leap days; malformed dates/seconds/offset forms refuse |

Development failures are retained privately and not relabeled PASS. The first
Windows run compiled and passed Clippy but refused legitimate extended-prefix
paths; the filesystem walk was corrected without relaxing DACL/link checks.
The first read-lock test exposed COMMIT Busy incorrectly reported as unknown;
successful explicit rollback now establishes Busy. An overflow test initially
modified an otherwise pristine profile counter inconsistently and was corrected
to test overflow after a valid committed allocation. Routine compile/format/lint
iterations were resolved; final gates above pass without disabling assertions.

## Scope and remaining work

Current normal-dependency inspection confirms no production consumer of
`acp-write`, no `acp-rehearsal` workspace member, and no writable connection exposed
through `SqliteStore`. No registry package version was upgraded; removing the old
rehearsal also removes its otherwise unused HMAC dependency. No new schema,
compatibility shim, catalog or lifecycle subsystem was added after contract freeze.
Historical M1 reports retain their results and point to their preserved source tree.

NOT_RUN / NOT_IMPLEMENTED for this slice: GitHub Actions; daemon bootstrap/select,
OS writer lease, epoch advancement, grants/revocation, write IPC and native client
workflows; complete Python/package/CI retirement; GUI/WSLg/TUI/10k interaction,
niri/mobile/macOS, clean-machine packaging, physical disk-full/power-loss/OS I/O
faults, network or cross-OS shared-profile guarantees, hostile same-user or
cross-account attacks, model/vendor calls, backup/restore and production writes.
The receipt ceiling is a validation-stage total-operation limit, not indefinite
product operation or a 10k-write acceptance claim. Structural receipts/counters
cannot authenticate a coordinated external rewrite of the whole database.
Windows parent-directory power-loss durability is not certified by file flushing.

Python packaging and old migration/parity CI/tool invocations still present in
the tree are deferred retirement work under Issue #40. They are not run or treated
as supported v1 validation paths; the removed rehearsal cannot be invoked by them.
No hosted workflow was edited or dispatched. All new pushed commits use [skip ci].
No real ACP DB, retained authentication, external checkout, conversation or model
service was selected. Raw logs and checkout paths stay under ignored private
`artifacts/local-p1-m2/`; nothing from those directories is committed or uploaded.

Issue #43 is ready for human review at this storage scope. Parent Issue #40 remains
incomplete. PR #42 stays Draft; no automatic merge, profile adoption, production
activation or later-slice authorization follows from these test results.
