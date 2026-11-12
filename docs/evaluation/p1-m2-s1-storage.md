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
| `1751c5bbeefab3d33659db798dbf0821aa3c91de` | Evidence/documentation update after the original full matrices |
| `5e47349586b750ded4e8aaeb95a919e8fba05c06` | Review revision: Windows reserved spelling fix, test-only verification counts/measurement and explicit later selection identity gate. Clean targeted storage gates and release measurements pass on both native hosts |
| `c6c9052595ee8240495266cbc1172c5d01571870` | Documentation-only relative to `5e47349`; subsequent clean full Workspace validation passes 87 tests on Linux/WSL and 88 on native Windows, including seven real daemon/IPC/MCP process cases on each |

The latest evidence update after `5e47349` is documentation only. Runtime results
are not relabeled as tests of a later documentation SHA. Historical M1/C2A/C2B1 outcomes retain their original
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

## PR #42 small review revision

Tested source: **5e47349586b750ded4e8aaeb95a919e8fba05c06**, clean on each
native host. This revision changes only Windows creation-name reservations,
test-only instrumentation/fixtures and documentation. Every existing integrity
check and negative assertion remains; there is no new runtime verification cache.
The test counter is thread-local under `cfg(test)` and absent from product builds.

### Windows filename checks and later selection

All ASCII reservations use one Windows case-folded comparison: the four control
basenames, `.acp-create-` prefix and SQLite `-wal`, `-shm`, `-journal` suffixes.
The new Windows regression checks absent mixed-case reserved targets, existing
mixed-case control/sidecar/database aliases with preserved sentinel contents,
and a permitted mixed-case name whose spelling must be preserved. Unix naming
semantics and permission/identity checks are unchanged.

[Contract section 3.1](../p1/profile-v1-runtime-contract.md#31-later-daemon-selection-path-identity-acceptance-gate)
now makes the existing selection boundary explicit for later startup/select:
validate canonical path and actual opened native file identity, owner/private
permissions, links/reparse points and profile ID/generation; reject replacement
by a valid copy even with identical metadata. Record positive and negative native
tests for that later slice. `SqliteStore::open` alone is not that daemon filesystem
selection boundary. No selection code, grant or lease is implemented here.

### Complete context.get verification call graph

For a successful ordinary request, `peer` dispatches to `Service::query`, which
opens one store and constructs the existing `ContextResolver`:

| Step | Full Profile verifications |
| --- | --- |
| `SqliteStore::open` -> `metadata` -> `snapshot` | 1 |
| `ContextResolver::new` -> `bound_project` -> `read_project` | 1 |
| `context` -> `bound_project` -> `read_project` | 1 |
| `resolve_preferences` -> `bound_project` -> `read_project` | 1 |
| `resolve_preferences` -> `read_preferences` | 1 |
| **Ordinary operator/project-agent context.get total** | **5** |
| Session credential: additional `Service::query` root-binding `read_project` | **+1, total 6** |

Each call checks the complete schema, `integrity_check`, foreign keys, actual
stored types/domains, metadata, every receipt and historical/live revision/counter
consistency in its own pinned snapshot. `read_preferences` uses the private
`project_record` query internally; that query does not add a full verification.
`bound_root` belongs to session control, not this request. Startup validation,
health and separate MCP calls are not counted as part of one context request.
Document inclusion and Git inspection do not open the Profile.

The new normal unit regression invokes the actual store and resolver with the
same storage sequence as `Service::query`, asserting 5/6 using a thread-local
counter at `profile::verify`. The daemon call graph is source-audited; the test
does not instrument or execute the daemon service/IPC. Git is explicitly
unavailable in this fixture, not represented as clean evidence.

### Synthetic near-capacity release measurements

Command on both native hosts:

```text
cargo test -p acp-store --release --locked --lib synthetic_profile_read_cost -- --ignored --nocapture
```

The measurement is deliberately opt-in, with no timing PASS threshold. Normal
storage regression skips only this manual performance test; it was then explicitly
executed and passed on both hosts. No acceptance/security test is skipped.

Each mode discards two warmups and records 21 serial samples, median and nearest-rank
p95. A fresh owned test-only fixture has two projects, eight preferences and four
Handoffs, initially 14 receipts / 49152 bytes. Test-only direct SQL extends coherent
history on `alpha/shared` to 4095 and 4096 receipts, with one corresponding live
revision; full validation succeeds before measurement. Both near-capacity files
are 1490944 bytes on each host. Digests in this read-only fixture are shape-valid
synthetic zero blobs, not writer replay evidence. Setup is excluded from timing.
No real Profile, agent, authentication or external project is used.

`verify` measures `metadata` on an already open store, including its read snapshot;
`open` measures fresh RO connection setup plus full verification. `context` and
`session-context` measure open, the actual resolver, allowlisted synthetic document
reads and JSON construction, with the storage root check for the session variant.
They exclude IPC/authentication/transport and real Git process cost, so these are
**storage/resolver costs, not end-to-end daemon latency**. Every sample also asserts
the exact 1/1/5/6 verification count. OS cache/scheduling and other local work are
not controlled; the 4095/4096 difference is not evidence of a scaling improvement.
This small fixture is not a worst-case bound for maximum-sized receipts/affected sets.

Values below are **median / p95 milliseconds** at the exact source above:

| Receipts | Mode (full checks) | Linux/WSL | Native Windows |
| --- | --- | --- | --- |
| 14 | verify (1) | 0.175 / 0.303 | 0.204 / 0.247 |
| 14 | open (1) | 0.287 / 0.389 | 0.482 / 0.619 |
| 14 | context (5) | 0.921 / 1.153 | 1.524 / 2.351 |
| 14 | session-context (6) | 1.033 / 1.428 | 1.734 / 1.892 |
| 4095 | verify (1) | 8.152 / 17.556 | 7.784 / 8.636 |
| 4095 | open (1) | 8.297 / 10.862 | 9.109 / 10.878 |
| 4095 | context (5) | 29.380 / 67.277 | 39.777 / 42.813 |
| 4095 | session-context (6) | 31.164 / 39.974 | 46.140 / 49.238 |
| 4096 | verify (1) | 4.882 / 5.417 | 6.812 / 7.465 |
| 4096 | open (1) | 5.560 / 6.916 | 7.674 / 8.921 |
| 4096 | context (5) | 25.305 / 27.419 | 33.703 / 35.134 |
| 4096 | session-context (6) | 31.250 / 35.158 | 41.076 / 42.970 |

There is measurable repeated validation overhead: at 4096, the ordinary read path
is about five times a single verification, with a sixth scan for session context.
This does not establish an end-to-end timeout or a product latency budget failure.

**Minimal optimization proposal, not implemented; requires owner review:** load
all Profile records needed for one context request inside one RO snapshot after
one unchanged full `verify`, then resolve immutable captured records and release
that transaction before Git/document work. Preserve project/session root checks,
permission and selection/file identity admission separately. Revalidate fully on
every new request; do not trust mtime, size, `commit_sequence`, `data_version` or
cached success as an integrity proof. No schema, receipt pruning, reduced checks,
background repair or cross-request cache is proposed. A future scoped storage/caller
change would need concurrent-writer snapshot tests, malformed-next-request refusal,
per-request verification counts and the existing negative suite unchanged. This
revision does not implement the new snapshot integration or change daemon/IPC.

### Targeted revision gates

The native hosts/toolchains are the ones recorded above. At clean `5e47349`:

| Command | Linux/WSL | Native Windows |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | PASS |
| `cargo clippy -p acp-store -p acp-write --all-targets --locked -- -D warnings` | PASS | PASS |
| `cargo test -p acp-store -p acp-write --locked` | PASS, 37 | PASS, 39 |
| Opt-in release measurement command above | PASS, 1 | PASS, 1 |
| `git diff origin/main --check` | PASS | PASS |

Counts: store 10 per host; writer 27 Linux / 29 Windows (extended-path and new
mixed-case cases). The manually run performance case is reported separately,
not added to normal regression totals. The unchanged Python baseline passes 29
on Linux, separately scoped. The original 86-test full workspace and seven real
process results remain evidence of `dc79176`, not relabeled as this revision.
At that targeted-validation stage the full Workspace matrix was not rerun; the
subsequent owner-requested full rerun is recorded below. UI/model matrices remain
outside this localized change.

PR #42 remains Draft for re-review. No IPC, daemon write authorization, CLI, NSH,
Python retirement or schema/DDL implementation scope is added by this revision.

## Latest-source full Workspace rerun

At the owner's request, the entire original Workspace gate set was repeated on
both native hosts at clean **c6c9052595ee8240495266cbc1172c5d01571870**. This is
the latest PR source before this evidence-only update. Its code includes the
`5e47349` Windows fix and count regression; the only difference from that commit
is the preceding evidence document. These are new executions, not a relabeling
of `dc79176` or reuse of the targeted package results.

| Command at clean c6c9052 | Linux/WSL | Native Windows |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | PASS |
| `cargo build --workspace --bins --locked` | PASS | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | PASS |
| `cargo test --workspace --all-targets --locked` | PASS, 87 | PASS, 88 |
| `git diff origin/main --check` | PASS | PASS |

The hosts/toolchains and isolated Windows checkout are as recorded above. Each
runner verifies exact HEAD and a clean tracked worktree before execution, uses
local locked caches with `CARGO_NET_OFFLINE=true`, and builds native binaries
before running tests. Both full runs independently pass all seven real
daemon/IPC/MCP process regressions, with zero failures. Workspace test execution
took 3.982 seconds on Linux/WSL and 92.662 seconds on native Windows; these are
local gate durations, not a performance comparison or latency acceptance claim.

Compared with the original 86 per host at `dc79176`, the count regression adds one
on both hosts; the new mixed-case collision test adds one more on Windows. Store
totals are now 10 per host, writer totals 27 Linux / 29 Windows, with the existing
Unix-only core case accounting for the remaining platform difference. Zero-test
targets do not inflate totals.

Each normal Workspace run reports exactly one ignored case: the explicitly opt-in
`synthetic_profile_read_cost` measurement. No acceptance/security regression is
ignored or disabled. That measurement's separate release PASS and numerical
results remain attributed to `5e47349`; it was not remeasured during this full
rerun. No test, product code, assertion or integrity check changed for this rerun.

Only this evidence document and PR metadata are updated after the tested source;
the next documentation commit is not claimed as an independently tested runtime
SHA. Raw reports/logs stay private under ignored `artifacts/local-p1-m2/`.
No GitHub Actions, real Profile, retained authentication, model call, external
checkout, GUI/NSH, selection/lease/authorization or Python retirement work was
performed. PR #42 remains Draft, unmerged and awaiting human re-review.

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
