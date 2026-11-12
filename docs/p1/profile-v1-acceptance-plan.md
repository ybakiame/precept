# P1-M2 — Profile v1 runtime acceptance plan

- Status: Stage 1 merged in PR #42; [S1 exact-source results](../evaluation/p1-m2-s1-storage.md) retain their historical sources. S2's [contract-first plan](daemon-profile-ownership.md) and [native results](../evaluation/p1-m2-s2-ownership.md) cover daemon ownership/write IPC only, as a Draft candidate. CLI/NSH and Python retirement remain separately gated.
- Contract: [Rust-native Profile v1 and runtime contract](profile-v1-runtime-contract.md).
- Issue: [#40](https://github.com/ybakiame/agent-control-plane/issues/40).
- First implementation slice: [#43 — Profile v1 storage foundation](https://github.com/ybakiame/agent-control-plane/issues/43).
- Baseline after ADR merge: `d8d48fa582f60f1dd0e00b22276e2fbb29ab5e53`.
- Target platforms: native Windows and Linux/WSL. Hosted CI is reported separately from local acceptance.

## 1. Test discipline

This document describes all of M2. Issue #43 implements sections 2–4 and their
direct schema/domain/storage negatives, plus removal of production-reachable M1
migration paths. Its exact-source Rust gates and native Windows/Linux/WSL storage
evidence are required. Sections 5–7 and complete Python retirement in section 8
belong to later slices; they must not be reported as delivered by Stage 1. The
full completion gate in section 10 remains the parent Issue #40 gate.

- Every test uses a fresh temporary root and synthetic data. No test reads, migrates, overwrites or deletes a user's existing profile.
- No preference values, Handoff bodies, credentials, raw SQL diagnostics or private paths are printed in test output.
- Every acceptance report names the exact source SHA, OS/version, Rust toolchain, commands and PASS/FAIL/BLOCKED/NOT_RUN status.
- Tests distinguish a unit-level injected fault from an actual independent process, OS lock, process kill or platform ACL behavior.
- A test passing on one platform is not evidence for the other platform.

## 2. Schema creation and strict reader

| ID | Scenario | Required assertion |
| --- | --- | --- |
| S01 | Create a new profile at an explicit absent path | Exact v1 DDL, `user_version=1`, one metadata row, random profile/generation IDs, epoch 0, next ID 1, commit sequence 0. |
| S02 | Create at an existing file or directory | Refuse without truncating, changing or replacing the existing object. |
| S03 | Create under unsafe path conditions | Refuse relative paths, symlink/reparse parents, hard-linked destinations, unsafe permissions/ACLs and path/control-file collisions. |
| S04 | Open valid v1 read-only | All supported project/preference/Handoff reads work; query-only and read-only flags deny writes even if query_only is toggled off. |
| S05 | Open old Python v1 fixture | Refuse as incompatible schema even though `user_version` is numerically 1. |
| S06 | Open M1 synthetic v2 fixture | Refuse as unsupported version; no migration or fallback is attempted. |
| S07 | Mutate schema shape | Refuse extra/missing tables, indexes, views, triggers, columns, changed DDL, type/nullability/PK/FK/check drift and unknown objects. |
| S08 | Corrupt records/metadata | Refuse missing/duplicate singleton metadata, invalid IDs, bad counters, malformed JSON, invalid enum/scope, broken FK and integrity errors. |
| S09 | Reopen after close/process restart | Same profile identity/generation and logical data remain readable; no implicit creation, repair or epoch increment occurs in read-only mode. |
| S10 | Concurrent fresh creators and publication failures | At most one no-replace publisher; loser preserves winning file. Pre-publish cleanup removes only owned temp files; post-publish failure preserves DB and reports OutcomeUnknown. |
| S11 | Actual storage types and pinned verification | Reject NULL text keys and REAL counters despite SQLite affinity; schema/rows/receipts are checked within one read snapshot under concurrent commits. |

## 3. Domain transaction behavior

| ID | Scenario | Required assertion |
| --- | --- | --- |
| D01 | Register project | Canonical allowlisted root, exact name/root uniqueness and atomic receipt. |
| D02 | Suggest preference | Monotonic allocated ID, suggested status, revision 1; reject invalid scope/project, blank/NUL/oversized fields. |
| D03 | Full transition matrix | Confirm/reject only from suggested; deprecate only from confirmed; every other edge returns InvalidTransition with no state change. |
| D04 | Confirm same-scope peers | Target confirmation and peer deprecation are one transaction; other scope/project peers remain unchanged. |
| D05 | Delete preference | Expected revision required; content row disappears; receipt reports deletion; next allocated ID is greater and never reused. |
| D06 | Put Handoff | Create with Absent, update with exact revision; stale revision, wrong project and malformed validation JSON are denied. |
| D07 | CAS conflict | Two independent writers/read snapshots racing on one revision produce at most one commit; loser gets Conflict and no partial changes. |
| D08 | Counter overflow/corruption | Checked overflow and inconsistent next-ID/commit-sequence state fail closed before mutation. |
| D09 | Transaction atomicity | Inject failure before commit and compare all logical rows, revisions, counters and receipts byte/logically; none change. |
| D10 | Bounded contention | Hold a real SQLite write lock; command returns Busy within the configured bound, with no unbounded retry. |

## 4. Idempotency and receipts

| ID | Scenario | Required assertion |
| --- | --- | --- |
| R01 | First operation | Data, revisions, sequence and receipt commit together. |
| R02 | Same operation ID and same canonical request | Return byte/logically identical original receipt; no second mutation or timestamp change. |
| R03 | Same operation ID with changed payload | IdempotencyConflict; no mutation. |
| R04 | Lost response after commit | Drop the response after actual commit, retry the exact operation ID/request, recover the original receipt. |
| R05 | Stale generation/epoch replay | Reject before receipt lookup; old credentials cannot replay against a new owner/generation. |
| R06 | Receipt capacity | At 4096 receipts, an existing operation still replays; a new operation fails Capacity with complete state unchanged. |
| R07 | Receipt integrity | Reject malformed digest/result, duplicate/non-monotonic commit sequence, generation mismatch and oversized receipt. |
| R08 | Delete and allocation history | Historical receipts remain valid after deletion; latest versions match live rows or terminal deletes; next ID covers deleted IDs; missing history/resurrection/counter rollback is refused. |
| R09 | Canonical bytes and privacy | Outer JSON reordering replays; raw validation JSON byte changes conflict; byte/scalar limits and redacted Debug/errors hold. |

## 5. Profile identity, write grant and revocation

| ID | Scenario | Required assertion |
| --- | --- | --- |
| A01 | No write grant | Operator read capability cannot invoke a mutation. |
| A02 | Agent/MCP/session/read-only roles | All mutation methods denied before write-capable storage open. |
| A03 | Explicit grant | Operator performs the explicit confirm action; daemon obtains profile lease, increments epoch and issues a separate profile-bound write credential. |
| A04 | Wrong profile/generation/epoch | Deny before mutation; compare complete logical state. |
| A05 | Revocation before admission | New writes are denied immediately after revocation gate closes. |
| A06 | Revocation racing an admitted transaction | A transaction admitted before revocation may commit; revocation waits for drain and reports no false rollback. |
| A07 | Daemon restart | Ephemeral write credential is invalid/removed; new explicit grant increments epoch; old token cannot write. |
| A08 | Grant issue failure | Lock collision, bad profile, failed epoch increment or credential file failure leaves no active grant and no partial token. |
| A09 | Sanitization | Errors/logs contain no SQL, path, token, preference value, Handoff body or SQLite diagnostics. |

## 6. Cross-process writer lease

| ID | Scenario | Required assertion |
| --- | --- | --- |
| L01 | Two independent daemon processes, same profile | Only one obtains the writer lease; second returns bounded Busy/OwnerMismatch and cannot mutate. |
| L02 | Canonical path aliases | Aliases resolve to the same file/lock identity; no competing lease. |
| L03 | Symlink/reparse/hardlink ambiguity | Refuse safely without opening a writer or changing the database. |
| L04 | Kill lease owner | OS releases lock; fresh daemon revalidates file/profile identity and authorization before acquiring a new epoch. |
| L05 | Lock file remains after exit | Reopen and lock the same validated file; never force-delete or infer stale ownership from timestamps/PID. |
| L06 | Lock file substitution | File identity/ACL/link validation detects replacement and refuses. |
| L07 | Revocation and shutdown | New admissions stop, admitted transactions drain, credential is removed, then lease is released. |

## 7. IPC, CLI, NSH and MCP

| ID | Scenario | Required assertion |
| --- | --- | --- |
| I01 | Unselected daemon | Only health and explicit profile create/select are available; no project/session/MCP/write operation succeeds. |
| I02 | Profile create/select | Typed IPC only; create is no-overwrite; select is strict read-only open; one selected profile per process. |
| I03 | IPC v2 DTOs | Unknown fields, unknown methods, wrong version, malformed enums, oversized fields/frames and wrong bindings fail closed. |
| I04 | CLI | Profile and write commands use IPC; CLI has no acp-write dependency and no read-write SQLite connection. |
| I05 | NSH | Minimum profile/preference/Handoff operator workflows use the same typed client; no UI database or writer dependency. |
| I06 | MCP | Exactly two project-bound read tools remain; no write method or OperatorWrite credential accepted. |
| I07 | Sessions | Project scope, explicit start/stop, context opt-in, expiry and revocation still work after coordinated IPC changes. |
| I08 | End-to-end | Create/select profile, register project, enable write grant, perform operator actions, read via CLI/NSH/MCP, revoke grant, and verify all denied paths. |

## 8. Python and legacy retirement inventory

The final implementation PR must include a machine-checkable inventory and prove:

- No product-owned Python package, CLI, Store, MCP server, Python test dependency, Python build dependency or Python CI job remains.
- No product binary/runtime path invokes Python, migration/rehearsal code or a compatibility adapter.
- No active quickstart, install or development instructions refer to Python product entrypoints.
- Any retained `.py` file is individually justified as non-product maintenance and is not imported, packaged, required by CI or reachable at runtime.
- M1's report and schema document remain historical evidence; no historical PASS claim is rewritten as product v1 acceptance.

## 9. Required commands and reporting

Run the repository's exact AGENTS-required gates on each native host, including:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- Targeted Profile v1 schema, writer transaction, grant/revocation, real IPC and two-process lease tests.
- Affected NSH/shared UI build and verification commands discovered from the current workspace scripts.
- Python/package/CI inventory checks proving removal.

Record separately: local Windows, local Linux/WSL, hosted GitHub Actions, and any unavailable host. Never label a skipped host or mocked process test as PASS.

## 10. Completion gate

P1-M2 is complete only when:

1. Exact Rust-native Profile v1 create/open/read/write is implemented and verified.
2. The daemon is the only write owner and the separate profile-bound grant/OS lease/fencing/revocation contract is proven.
3. CLI and minimum NSH operator workflows use typed IPC only; MCP/agents remain read-only.
4. Python product runtime/tests/dependencies/CI and active legacy migration paths are removed.
5. Required native Windows and Linux/WSL tests pass on identified clean SHAs.
6. A Draft PR links Issue #40 and reports all unrun checks honestly.

This plan does not authorize production rollout, release, real-profile migration or writes to unrelated local data.
