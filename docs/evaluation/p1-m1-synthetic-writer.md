# P1-M1 — synthetic profile v2 writer evidence

Authority: [Issue #38](https://github.com/ybakiame/agent-control-plane/issues/38),
Accepted [ADR-0005](../adr/0005-local-storage-and-write-ownership.md) and
[ADR-0004](../adr/0004-product-domain-trust.md). Base latest main
`aef3a501288133549afbd85edfcf5c2cef50f7dd`, clean before branching.
This is data/transaction-layer acceptance on disposable fixtures, not production
Rust writes, operator authentication, ownership fencing or real-profile migration.

## Contract and implementation

The [exact schema/command contract](../p1/profile-v2-schema-and-writer.md) was
committed as M1-A at `b9a1559` before writer implementation. It freezes profile v2,
legacy shape allowlists, revision/tombstone keys, receipt limits/canonical requests,
strict transitions and the independently versioned **planned** catalog DDL/interface.
Catalog physical-identity bindings are specified for future M3 reconciliation;
no usable catalog, manifest or publication service is implemented here.

[`acp-write`](../../crates/acp-write/src/lib.rs) is a separate workspace library.
Its only workspace constructor creates a fresh private TempDir and Rust-generated
fixtures; no public `open(path)`, path adoption, profile defaults, executable/SQL/
connection callback, CLI or normal-process write entrypoint exists. Its binding
DTO is not authority; the non-serializable parent workspace owns the files and
borrowed writers cannot outlive it. No production crate depends on `acp-write`
(checked with `cargo tree --locked --invert acp-write`). Existing `acp-store`
continues to reject v2 and keeps its public read-only behavior unchanged.

The source opens RO. A pinned SQLite backup, independent backup reopen and full
legacy-record comparison precede a separate new-candidate migration. Fingerprint
addition, technical tables, imported revision 1 and user_version=2 are one transaction.
Independent candidate verification checks exact schema, logical fields/raw JSON,
identities and pristine technical metadata before enabling the synthetic writer.
Source and verified backup remain intact through every ordinary operation.

Seven typed commands implement explicit scope/CAS, transactional current-state
validation, strict suggested→confirmed/rejected and confirmed→deprecated, atomic
confirmed-peer deprecation, deletion with minimal tombstones, registration and
Handoff upsert. Fresh suggested→deprecated is denied. Historical deprecated rows
and unknown attribution are preserved without replay/repair. This intentional
legacy Python CLI difference does not change current Python behavior.

Data, revisions, commit sequence and SHA-256-bound minimal receipt commit together.
Replay precedes CAS; a different request under the same ID conflicts. Generation/
epoch mismatch, capacity, stale state and invalid scope refuse without logical
changes. No silent receipt pruning, raw response cache, automatic confirmation or
new human ReviewDecision history is introduced. M2 must enforce real operator
binding and live Git/fingerprint recapture; M1 snapshots are fixed synthetic labels.

Only existing registry packages are used (rusqlite 0.41.1 bundled/backup, serde,
serde_json, sha2, getrandom, tempfile); Cargo.lock adds the new local package without
upgrading a third-party package or toolchain. No Python/core/store/rehearsal/daemon/
IPC/MCP/session/UI application code, legacy fixture or workflow is changed.

## Local hosts and clean sources

| Host | OS / architecture | Toolchain / interpreter |
| --- | --- | --- |
| Linux/WSL | Arch Linux, WSL2 `6.18.54.1-microsoft-standard-WSL2`, x86_64 | Rust 1.99.1 (`58cf8f9c3`), Cargo 1.99.1 (`5f94df478`), Python 3.14.9, Git 2.56.0 |
| Native Windows | Windows 11 `10.0.26300`, AMD64; isolated disposable checkout | Same Rust/Cargo, Python 3.14.5, Git 2.56.1.windows.1 |

All retained runs checked the exact full SHA and `worktree_dirty=false` before
commands. Source commits remain in this PR for reproduction:

- `c9c7cd9e87b002ef20f565bb355a5f7e2860b253`: initial implementation; Linux full
  matrix and Windows dedicated M1 suite (26 passing cases each).
- `e27cd737b18c16c91d2043abb44ee92f28f2054a`: stricter initial-candidate metadata
  admission and regression; Windows full matrix, Linux focused writer/required
  Python checks. Both hosts pass all **27 final M1 cases** at this source.
- The earlier Linux full matrix is not relabeled as an e27cd73 run. A later
  one-page backup-interruption/reservation-error correction awaits its separately
  recorded focused checks; prior full matrices are not promoted to that source.

Cargo used `CARGO_NET_OFFLINE=true`, `--locked` and existing caches. `PYTHONPATH`
contains the checkout's `src` and `tools`; `ACP_TEST_PYTHON` names the selected test
interpreter. Commands below spell it `python` without a private absolute path.
The private runner reuses `tools/local_verify.py::run_gate` for bounded gates/logs.
No generic UI, vendor-model, external checkout or real ACP integration was selected.

Windows initially lacked native Git. A process-local
[official MinGit 2.56.1 archive](https://github.com/git-for-windows/git/releases/tag/v2.56.1.windows.1)
was downloaded and extracted inside the ignored disposable validation directory;
SHA-256 matched the official release/API digest
`4a92850b6e917da4fd010669b614b55dccf5b0ec333a534cc4be4700dfefdafd`.
There was no installer, registry write or permanent PATH change. SDK/MSVC and
MinGit PATH setup occurred only in the test PowerShell process; Windows Git used
an owned empty global config with system config disabled. No Rust package download
or system setting change was needed. This test-tool download is not a hosted test.

## Commands and observed outcomes

| Command | Linux/WSL at c9c7cd9 | Native Windows at e27cd73 |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | PASS |
| `cargo test --workspace --all-targets --locked` | PASS, 94 tests | PASS, 94 tests |
| `cargo build --locked --example contract_reader` | PASS | PASS |
| `cargo build --locked -p acp-store --example sqlite_reader` | PASS | PASS |
| `python -m unittest discover -s tests -v` | PASS, 29 tests | PASS, 29 tests |
| `python tools/check_p1_parity.py` | PASS, 120 comparisons | PASS, 120 comparisons |
| `python tools/check_sqlite_parity.py` | PASS, 78 comparisons / 13 scenarios | PASS, 78 comparisons / 13 scenarios |
| `cargo run --locked -p acp-rehearsal --example synthetic_rehearsal -- --synthetic-only` | PASS | PASS |
| `cargo test --locked -p acp-rehearsal --features recovery-tests --test process_recovery -- --nocapture` | PASS, 11 tests | PASS, 11 tests |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS | PASS |
| `git diff origin/main --check` | PASS | PASS |

Native Windows additionally ran `cargo build --workspace --bins --locked` before
workspace tests, as the existing process-security tests require those binaries.
Linux had its existing native binaries available; their sources are unchanged by
M1. Windows parity commands append `--reader target/debug/examples/contract_reader.exe`
and `--reader target/debug/examples/sqlite_reader.exe`, respectively.

The identical full totals have different scoped makeup: Linux's earlier M1 suite
had 26 tests; final Windows M1 has 27. The existing Unix-only legacy core test
accounts for the platform difference outside M1; neither count is inferred.

Linux follow-up at clean **e27cd73**, without repeating the unaffected legacy
workspace/parity/rehearsal/recovery matrix:

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy -p acp-write --all-targets --locked -- -D warnings` | PASS |
| `cargo test --locked -p acp-write --lib` | PASS, 27 tests |
| `python -m unittest discover -s tests -v` | PASS, 29 tests |
| `git diff origin/main --check` | PASS |

Each M1 suite reports one intentionally ignored **private worker entrypoint**.
The parent process test invokes that helper explicitly six times with `--ignored`;
it is not an omitted acceptance scenario. No tests/policies/assertions were disabled
to pass a gate. Ordinary workspace PASS is not substituted for the separate
existing 11-test C2B1 process target.

## Adversarial M1 evidence

[`tests.rs`](../../crates/acp-write/src/tests.rs) runs 27 acceptance tests per host,
with table-driven subcases rather than inflating the top-level count:

| Boundary | Actual assertions |
| --- | --- |
| Frozen schema and history | DDL matches the frozen profile contract; all three source shapes independently migrate, preserving every legacy field, raw nested/non-ASCII JSON, timestamp, status, root/ID and sqlite_sequence; first new preference ID is 1001 |
| Schema/record refusal | Future version, missing fingerprint in v1, extra/missing/incorrect tables/columns/types/indexes/constraints/views/triggers, invalid status/JSON/allocation and corrupt technical metadata fail without mutation |
| Strict review | All 12 state×command cells across all three shapes; only three transitions succeed; every denial leaves legacy rows, allocation, revisions, receipts and sequence unchanged; no Withdraw |
| Scope and atomicity | Confirm deprecates only same-key/scope/project confirmed peers and advances all revisions together; global/foreign-project peers remain untouched; partial data/revision/precommit faults roll back the entire operation |
| CAS and fencing labels | Stale/missing/occupied expectations, scope, generation and epoch negatives; simultaneous independent SQLite connections produce one CAS winner and one conflict, never two commits |
| Idempotency | Lost-response fault after commit, connection close/reopen and exact receipt replay; same-ID changed canonical content conflicts; raw opaque JSON string bytes matter; replay still works at receipt capacity; no pruning |
| Allocation/capacity | Revision overflow, actual SQLite AUTOINCREMENT exhaustion and oversized peer/receipt sets refuse atomically; injected StorageFull is a simulation, not physical disk-full |
| WAL and busy | Live-source committed WAL included/uncommitted changes excluded by backup; v2 RO snapshot sees no partial data/revision/receipt; real writer lock contention returns bounded Busy, then succeeds after release |
| Migration outcome | Before-commit schema/version rollback, partial backup/full injection and committed-before-response OutcomeUnknown; candidate remains ineligible until independent explicit verification; original/verified backup intact |
| New metadata regression | Changed imported revision or initial receipt limit is rejected despite otherwise valid v2 data; the test failed against c9c7cd9's verifier and passed after the correction at e27cd73 |
| Genuine child termination | Three source shapes × before/after command COMMIT: real child kill/wait (SIGKILL / TerminateProcess), fresh SQLite reopen and v2 verification; precommit state entirely old, postcommit data/revisions/sequence/receipt entirely committed; prior copies unchanged |
| Cross-language preservation | Separate Python process opens source/backup/v2 RO without Store, compares full logical rows/raw JSON and initial technical state, emits only PASS; all three main files unchanged after this read |
| Rollback representability | After new writes/delete, a separate owned synthetic v1 projection retains current legacy rows/allocation and is readable by old Rust; sealed v2 remains valid. No production downgrade/cutover API is added |
| Privacy / filesystem | Minimal receipt/tombstone columns contain no command values/key/source/body/path/SQL; request/workspace/writer Debug and errors are sanitized; target sidecars, replaced identity and actual hardlinks refused on both hosts; Unix symlink/mode and Windows readonly-attribute cases run on their respective hosts |

The process helper can create only another **fresh** fixture beneath its parent's
owned temporary root, using a bounded private stdin message. It cannot adopt a DB.
It emits a content-free READY marker; no paths/capabilities go in argv or logs.
The parent retains its TempDir, bounds readiness, kills/reaps children and owns
cleanup. This proves v2 SQLite transaction boundaries, not production owner restart,
parent-loss recovery, a public fixture-adoption API or a durable catalog.

Development failures remain private and are not relabeled PASS: the new metadata
regression first failed against the earlier verifier; Windows orchestration first
failed before gates because Git was missing, then seven process-security cases
failed because its fresh checkout lacked prebuilt binaries. The private runner's
Git environment/build order was corrected; all final Windows gates above passed
with application tests and assertions unchanged. A clean-source SHA preflight
refusal also occurred before any test command and is not a test result.

## Scope, limits and completion

M1 changes only its new crate, workspace membership/lock entry, artifact ignore,
contract, roadmap and this sanitized report. Final diff/API/dependency inspection
confirms no production entrypoint or writable acp-store handle. No user profile,
default ACP state path, retained authentication, external development repository,
real conversation, production backend or system configuration was accessed/changed.
Only synthetic fixtures are generated. The new crate does not persist manifests,
credentials, device inventory or command payload caches.

NOT_RUN: GitHub Actions (all pushes use `[skip ci]`), model/vendor turns, GUI/WSLg
rendering/TUI/10k interactions, niri/mobile/macOS, physical disk-full/OS I/O fault
injection, memory-pressure/OOM and power-loss/hardware durability, network filesystems,
cross-account/privileged ACL attacks, hostile same-user/TOCTOU, live operator IPC/
profile ownership fences, real migration/restore, persistent catalog publication,
parent-capability loss and full P1-O decision history. Windows inherits private
temp-root ACLs; reparse/identity/hardlink checks do not prove cross-account isolation.
SQLite free pages, WAL and backups may retain deleted bytes; no forensic erasure.

C2A/C2B1 historical hosted evidence retains its original scope. The local C2B1 rerun
above and the new v2 child-kill test are distinct evidence, neither a power-loss
or real-profile recovery guarantee. Checksums/digests are comparison metadata,
not identity attestation, authenticated operator attribution or proof of model behavior.

Private ignored artifacts: `artifacts/local-p1-m1/wsl-1/`, `wsl-final/`,
`windows-1/` (failed build-order attempt), `windows-final/`, `development/` and
private runner/tool metadata; native output was copied locally for inspection.
Raw diagnostics/private checkout paths are not committed or uploaded. The final
backup fault-point refinement needs its separately recorded focused evidence.
Final documentation checks passed for three documents and 32 relative
links/anchors, balanced fences, the exact 14-file scope, private-path/credential
patterns and whitespace. Protected application/workflow paths and third-party
lock entries are unchanged. Runtime changes after e27cd73 are confined to the
synthetic writer backup-fault/reservation path and its regression assertion.
The candidate will be submitted for Draft PR review after focused follow-up
verification, not automatic merge or permission for M2–M5. Production writes and every real-profile operation remain separately gated.
