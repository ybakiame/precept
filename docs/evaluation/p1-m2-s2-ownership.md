# P1-M2/S2 — daemon ownership and write IPC evidence

> **Historical record — not a current runbook.** Original dates, source revisions
> and outcomes below are preserved. S4 retired the executable prototype and probes;
> use [Rust-only development](../p1/rust-only-development.md) for current commands.

Authority: [Issue #44](https://github.com/ybakiame/agent-control-plane/issues/44),
parent [#40](https://github.com/ybakiame/agent-control-plane/issues/40), Accepted
[ADR-0006](../adr/0006-rust-only-runtime.md), the frozen
[Profile v1 contract](../p1/profile-v1-runtime-contract.md) and
[S2 contract-first plan](../p1/daemon-profile-ownership.md).
Branch: `feat/p1-m2-s2-daemon-write-owner`; delivery remains Draft for review.
Base main: **be2c4a0c9b9ef7d0bf8dc9e952dd2f931b6ce046**, after S1 merge
`1976d6f4d373db737b3be27d2781c191253a467a`.
This is disposable-profile ownership validation, not real-profile activation,
release, CLI/NSH write UX or completion of parent M2.

## Exact sources and implemented boundary

| Commit | Scope |
| --- | --- |
| `e958c1985acc3df8f8943836d696e953632cb20a` | Contract and native acceptance plan committed before product changes |
| `b258b4f062a73cc0a0ee0b2af9c1d5f53d9f44fb` | Initial native guard/selection, lease/epoch owner, distinct grant, strict IPC v2 dispatch and independent-process tests |
| `a790390b22b4462791191048aa5fa3f62e79f430` | Deterministic admission/drain test and Windows native-handle type correction |
| `e5ef1a1b90b712fbfe5b7f382bb1b3ff2343403d` | Expanded refusals/aliases/envelope tests and corrected Windows locked-file fixture; Workspace passes 102 Linux/103 Windows, but extra Windows shared-client Clippy fails |
| **`371e57784e0abc1edd987c243178e7aee1c269f4`** | Final tested code: epoch-bound status/refusal regression and Windows shared-client lint correction; full gates below pass on both hosts |

Later report/status changes are documentation only. Do not relabel tested code
as a later evidence commit. All new commits use `[skip ci]`.

- The daemon can start control-only without `--database`. Create/select are
  explicit operator methods; startup selection opens an existing exact v1 only.
  Selection is single-assignment, with `RestartRequired` rather than hot switching.
- [Selection and ownership](../../crates/acp-write/src/owner.rs) retain a native
  DB guard and validate path components, canonical parent/path, owner/private
  ACLs/modes, reparse/symlink/hardlink safety, native identity and profile ID/
  generation. A copied DB with identical metadata cannot replace a selected file.
  [SQLite native binding](../../crates/acp-store/src/native.rs) uses the actual
  Windows SQLite handle; Unix checks the VFS moved-file observation plus path/
  guard identity. Unsupported identity controls fail closed; no custom VFS.
- Only the daemon is a normal consumer of `acp-write`, enabling `daemon-owner`.
  Existing-v1 attachment requires a previously validated selection, native
  profile-specific OS lease and checked epoch advancement. No arbitrary existing
  path Writer/SQL/connection API is added. The feature/dependency boundary is an
  auditable product integration boundary, not code-origin attestation or an OS
  sandbox against another program running as the same user.
- The lease's identity-derived private name is in the canonical Profile parent;
  aliases use the same OS lock. Contention is immediate/bounded Busy. The native
  handle remains held until revocation drains admitted work and matching token
  cleanup completes. Lease files are never unlinked, force-unlocked or repaired.
- The separate format-3 `OperatorWrite` credential binds profile ID/generation/
  epoch and authorizes only `profile.mutate`. Control, agent and session formats
  cannot become write authority. No token is returned in an IPC result. Explicit
  enable advances epoch; selection/startup never grants writes. Failed token
  publication can consume an epoch but creates no grant or domain receipt.
- [Daemon dispatch](../../crates/acp-daemon/src/profiles.rs) rechecks grant before
  RW opening and at the serialized transaction admission point. Disable closes
  admissions, drains admitted work, removes only a matching private token and
  releases ownership. An admitted commit returns its actual receipt despite a
  later revocation. A computation timeout with uncertain write outcome is
  OutcomeUnknown; bounded blocking work retains its admission slot.
- IPC is v2 throughout Rust transport/client/MCP/session integration and the
  shared health reader. Strict lifecycle DTOs plus the seven S1 typed commands
  are used; no v1 compatibility path. Profile paths are absolute UTF-8, bounded
  to 32768 bytes with NUL rejected; S1 mutation requests remain bounded to 128 KiB.
- Registration must match both `--allow-project` and independently configured
  `--allow-root NAME=PATH` for a new root. Existing roots are pinned at selection;
  every context/Handoff read role compares current root with that binding.
  Runtime/config/executable control files cannot be under shared roots. Native
  CLI/NSH write command workflows are not implemented by this slice.
- Profile DDL, receipt ceiling, strict transitions, CAS, scoped peer deprecation,
  monotonic IDs/revisions and receipt digest/replay semantics remain S1's contract.
  No external filesystem/Git/network work is moved inside mutation transactions.
  Handoff snapshot fields remain bounded operator input; they are not certified
  as new Git observations or fresh validation evidence by this write endpoint.

## Native hosts and final commands

| Host | Environment |
| --- | --- |
| Linux/WSL | Arch Linux, x86_64, WSL2 kernel 6.18.54.1-microsoft-standard-WSL2 |
| Native Windows | Windows 11 10.0.26300, AMD64; isolated owned checkout |
| Both | Rust 1.99.1 (58cf8f9c3), Cargo 1.99.1 (5f94df478); existing local locked caches, CARGO_NET_OFFLINE=true |

Both runners require clean tracked worktrees and exact HEAD before execution.
Windows uses process-local MSVC/SDK/PATH and the previously checksum-verified
owned MinGit, without installing tools or changing persistent system settings.
Tests create only owned temporary roots, explicit private directories and
synthetic Profile v1 files. Raw logs and checkout paths remain ignored/private.

At clean **371e57784e0abc1edd987c243178e7aee1c269f4**:

| Command | Linux/WSL | Native Windows |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | PASS |
| `cargo build --workspace --bins --locked` | PASS | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | PASS |
| `cargo test --workspace --all-targets --locked` | **PASS, 103** | **PASS, 104** |
| `cargo fmt --manifest-path spikes/p1-e/shared/Cargo.toml -- --check` | PASS | PASS |
| `cargo clippy --manifest-path spikes/p1-e/shared/Cargo.toml --all-targets --features native --locked -- -D warnings` | PASS | PASS |
| `cargo test --manifest-path spikes/p1-e/shared/Cargo.toml --features native --locked` | **PASS, 25** | **PASS, 25** |
| `git diff origin/main --check` | PASS | PASS |

The Workspace totals include **13 new independent-process ownership tests**, the
**7 existing independent daemon/IPC/MCP security cases**, and the deterministic
framed admission/drain test. Store totals are 11 per host; IPC 6; daemon unit
tests 8 plus 13 ownership tests; writer 27 Linux/29 Windows. The existing Unix-only
core case explains the remaining platform difference. Zero-test targets do not
inflate totals. The shared-model 25 are reported separately, including existing
10k-row/data/presentation checks; this is not GUI/TUI interaction acceptance.

Exactly one Workspace case remains intentionally ignored: S1's opt-in
`synthetic_profile_read_cost`. No security/acceptance case is disabled. Its prior
numbers retain their original S1 source, not an S2 latency claim. S2 adds selected
binding and independent root admission checks: a successful current context path
has seven full verifications (two in selected-reader admission, one root read,
four resolver reads) for both static and session roles. The old 5/6 measurement
fixture is explicitly labeled S1-only. No snapshot optimization or cross-request
trusted verification cache is introduced.

Unchanged legacy Python baseline: 29 PASS on Linux at this exact source, separately
from Rust acceptance. No new Python test/feature/adapter or runtime dependency was
added; Python product retirement remains a later M2 slice.

## What the negative and process tests establish

| Boundary | Observed assertions |
| --- | --- |
| Bootstrap/selection | Control-only health/info; unselected project/session reads unavailable; no implicit create; strict DTO/absolute path; selected restart requirement; no startup/selection write token |
| Native file safety | Valid safe selection; missing/legacy Python v1/M1 v2 startup refuses unchanged; native main/ancestor symlink or Windows junction, hardlink and unsafe mode/inherited ACL refuse; opened SQLite file differs from an identical copy's guard |
| Replacement | An otherwise valid private DB copy has matching metadata before substitution; selected reads/writes then refuse native identity change. Replacing a held lease with another private empty file refuses writes without altering logical DB state |
| Lease refusal | Hard-linked and unsafe mode/ACL lease files cannot advance epoch; Unix symlink lease refusal; no credential is issued and all logical state remains unchanged |
| Roles | Control/agent/session cannot mutate; agent/session/write roles cannot enable a grant; write role cannot read/control/session. Forged local role metadata fails handshake, and a write credential cannot launch the MCP bridge |
| Epoch/lifecycle | Explicit opt-in advances epoch, stale binding/token refuses, epoch drift makes info non-current and denies writes, overflow preserves state, publication collision preserves the foreign file and issues no authority |
| Independent owners | Two real daemons in distinct runtimes contend on one Profile through lexical/canonical aliases; loser is bounded Busy with unchanged state. Kill idle owner, reacquire with advanced epoch, restart old daemon read-only, remove only validated stale token and reject old credentials |
| Seven commands | Real IPC registration/suggestion/confirm/reject/deprecate/delete/Handoff, atomic peer deprecation, stale CAS, strict states, exact project/global scope, independent root refusal, malformed DTO and idempotency conflict leave hashed complete logical state unchanged |
| Lost response/version | Native client sends mutation then disconnects without consuming response; receipt persists, same-epoch retry returns original receipt without changing rows/counters/timestamps. IPC v1 handshake refuses; no dual-version shim |
| Contention | Real SQLite BEGIN IMMEDIATE contention blocks epoch acquisition or mutation with Busy, no logical changes, and only explicit retry after releasing lock succeeds |
| Session/MCP | Real configured deterministic fake agent starts under IPC v2, launches the MCP bridge and reads both tools on synthetic data. Session credential cannot write/grant; its established read peer works before Stop and fails after revocation; operator write authority remains independent |
| Root binding | Structurally valid out-of-band synthetic root replacement cannot redirect operator/static agent context outside the pinned root; session admission/publication protections remain |

The drain proof is deliberately **in-process framed IPC with actual production
dispatch and real SQLite**, using a `cfg(test)` barrier at final admission. It
observes a new handshake refused after gate closure, disable waiting, the admitted
write committing and returning its receipt, then disable completing and later
requests refusing. The barrier does not exist in a product binary. It is not
misreported as a process-kill or multi-process timing experiment. The separate
13 process tests use real native daemon processes and OS transports/locks.

## Development failures, scope and remaining work

Private development logs retain failures honestly. Initial native Windows
compilation exposed an untyped HANDLE pointer; the explicit native type fixed
it. The first lease-replacement fixture tried reading a held empty lease, which
Windows byte-range locking refuses. It now constructs an empty replacement and
retains all identity-refusal assertions; no product lock/check was relaxed.
At `e5ef1a1` both full Workspaces passed, but the extra Windows shared-client
Clippy found a pre-existing Unix-only error map compiled into an identity map on
Windows. Conditional compilation now preserves Unix error refinement and native
Windows errors with lint checks still enabled. A new epoch-status regression is
included in the final rerun; no stale source result is relabeled as final PASS.

Normal dependency inspection shows only `acp-daemon` consumes `acp-write`.
The exact Profile DDL/verifier and the 4096 receipt policy remain unchanged from
merged S1; no registry dependency version was upgraded. Python prototype/package/
CI files, core resolver/fingerprint and agent/session execution permissions are
not expanded. Shared changes only coordinate the wire health version/fixture and
the Windows lint fix; no screens, interactions or write UX are added.

NOT_RUN: hosted Actions; GUI/WSLg/TUI/niri/mobile/macOS; models/vendor/authentication
services; real ACP data; native CLI/NSH write UX; production write activation or
release; cross-OS/shared-network Profile filesystems; hostile same-user/other-account
attacks; packaging/clean-machine tests; power-loss, physical disk-full and kill
during an in-flight COMMIT. The kill test proves idle writer ownership recovery,
not automatic hot-journal repair or recovery beyond S1's fail-closed storage.
No backup, restore, pruning, migration, catalog, Python fallback or schema change.
Same-operation replay remains same-epoch; no cross-epoch digest conversion is added.

Stage 2 is ready for human review at this scope. Parent #40 remains incomplete:
native CLI/NSH workflows, complete Python/package/CI retirement and final cutover
acceptance remain later work. Keep the PR Draft; do not merge or activate a real
Profile automatically.
