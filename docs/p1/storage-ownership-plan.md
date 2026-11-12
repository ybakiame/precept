# P1-M0 storage ownership and acceptance plan

Design-only companion to [Proposed ADR-0005](../adr/0005-local-storage-and-write-ownership.md).
Authority: [Issue #36](https://github.com/ybakiame/agent-control-plane/issues/36).
Reviewed source baseline: main `2a5949120aacd0523cd7fa5e417548e1e648bc81`.
Every proposed mechanism below is **NOT_IMPLEMENTED / NOT_RUN** unless explicitly
identified as historical C2A/C2B1 evidence. No profile is selected or accessed by M0.

## Source review and ownership matrix

Source review includes [AGENTS](../../AGENTS.md), [product](../product.md),
[architecture](../architecture.md), [interfaces](../interfaces.md),
[security](../security.md), [roadmap](../roadmap.md),
[Accepted ADR-0004](../adr/0004-product-domain-trust.md), and the
[P1-L typed contract](typed-context-v1.md). The implementation anchors below explain
why a compatibility/fencing release must precede cutover.

| Component / data | Implemented now | Proposed owner / restriction |
| --- | --- | --- |
| [Python Store](../../src/agent_control_plane/store.py) | Constructor opens RW, enables WAL, creates schema and may add fingerprint; owns all P0 mutations; no version check | Legacy direct writes only while explicitly Python-owned and guarded; retire after rollout |
| [Python CLI](../../src/agent_control_plane/cli.py) | Uses Store for reads, writes and MCP | Thin authenticated frontend; reads avoid Store; no fallback writer |
| [Python MCP](../../src/agent_control_plane/mcp.py) | Two read tools, but launched by CLI with initialized Store | Genuinely RO adapter or read IPC; no initialization writes, operator credentials or mutations |
| [acp-store](../../crates/acp-store/src/lib.rs) | Explicit existing-path RO connection; structural/version inspection before reads | Remains RO; add reviewed v2 reader support before activation |
| [daemon](../../crates/acp-daemon/src/main.rs) | Uses SqliteStore; operator sessions and project/lease-scoped reads, no profile mutation | Sole ordinary writer only when explicitly opted in; separate authenticated write extension/library |
| [acp-query](../../crates/acp-query/src/main.rs) / NSH | Operator reads/session control; UI has no direct DB | Typed operator client of same daemon; no direct SQL or second writer |
| Proposed maintenance owner | Absent | Exclusive offline mode for backup/migration/restore; no ordinary mutation service |
| [acp-rehearsal](../../crates/acp-rehearsal/src/lib.rs) | Owned disposable synthetic roots; cannot adopt profiles | Remains synthetic-only; production machinery separate |
| Profiles, registration, preferences, Handoff | User-local SQLite; legacy source/validation fields | Same authority; exact legacy preservation; later generalized models separately reviewed |
| Contract sources / cache | Repository AGENTS/docs/accepted ADRs; bounded live reads | Repository remains authority; any DB index disposable and version-bound; no new cache/parser now |
| Proposed control catalog | Absent | User-local SQLite, separate catalog format; opaque identities/relative artifact roles/owner epoch/stage only |
| Session credentials | Private runtime files and daemon memory | Stay ephemeral and outside profile/backup/manifest; session grants never become write authority |

## Reader and writer compatibility

“Current Python” includes its initializing constructor; it is not a genuine RO
reader. A compatible replacement must use read-only SQLite open and fail closed
on unsupported version/shape **before any write-capable initialization**.

| Profile shape | Current Python Store | Current Rust reader/daemon | Proposed RO Python/Rust readers | Proposed ordinary writer |
| --- | --- | --- | --- | --- |
| v0 legacy, no fingerprint | Opens RW and adds nullable fingerprint; not side-effect-free | Reads with unknown fingerprint | Read without initialization/DDL | Guarded Python only before cutover; Rust refuses without explicit migration |
| v0 fingerprinted | Opens RW / initializes | Reads | Reads | Same pre-cutover restriction |
| v1 fingerprinted | Actual constructor/writes compatible in historical C2A | Reads | Reads | Guarded Python owner; Rust ordinary writer requires active v2 |
| v1 without fingerprint or malformed supported version | Constructor may repair; must not be used as validator | Refuses | Refuse without repair | Refuse |
| Proposed exact v2 | Version-blind: may initialize/write; **unsafe, never launch** | UnsupportedVersion | Reads after separately implemented exact-schema upgrade | Opted-in Rust daemon only, matching active generation/epoch |
| Unknown future version / unexpected schema | No version fence: **unsafe** | Refuses | Refuse | Refuse |

v0→v1 remains the already-implemented synthetic migration. Proposed real migration
uses a new candidate: v0→fingerprinted v1→v2 within one reviewed transaction (or
v1→v2), preserving all legacy logical records; only absent fingerprint maps to NULL.
Exact v2 metadata DDL and independent catalog DDL/version are an M1 review gate.
No real DB upgrade, v2 support or authenticated write extension exists today.

All clients on the profile must upgrade before a v2 cutover; “old reader supports
v1” is not permission to leave it on a retired path. Python compatibility reads
remain supported after cutover with the new RO adapter/read IPC. Old binaries are
retired, not retroactively made safe by `user_version`, ACLs or cooperative locks.

The current `suggested → deprecated` Python edge is outside ADR-0004's described
graph. Existing rows/states are preserved; no approval is inferred. Full new-writer
transition compatibility is blocked until the owner resolves the discrepancy
recorded in ADR-0005. No M0 code or Accepted ADR text is changed to choose for them.

## Coordination and durable stage model

The following is the proposed **future** protocol, not C2B1's implemented JSON
test manifest. Use a private versioned SQLite control catalog: immutable stage
rows plus a transactional active-generation/owner pointer. Catalog and profile
commits are distinct; every transition validates both before proceeding.

Each stage binds operation/profile identity, fixed relative roles, source and
target schema, generation/owner epoch, previous/desired/verified state, backup
verification digest, commit watermark and sanitized outcome. Absolute location
selection stays in the private local operator interaction, not stage/log exports.
These are operational manifests in SQLite, not a signature or evidence chain.
No metadata contains preference/Handoff/evidence bodies or credentials.

The stable coordination lock has two roles: a shared profile gate for all open
connections, exclusive maintenance gate for cutover/restore, plus a separate
exclusive writer lease. Lock acquisition order is profile gate → writer lease →
catalog transaction → profile transaction; do not hold transactions across an
inverse acquisition. Drain/close shared holders before acquiring the exclusive
gate. Maintenance prevents new admission; all timeouts fail boundedly, without
force-unlocking. Catalog publication and profile transactions never overlap as an
assumed atomic two-DB commit. A fresh process locks first, then reconciles.
The control root/lock namespace is fixed to the canonical profile identity; passing
another control directory cannot create a second owner. Managed generations have
fixed relative roles. For original/backup files outside that root, recovery needs
operator reselection and identity verification, not an absolute-path manifest or
automatic search. Missing reselection is a safe blocked outcome.

```mermaid
stateDiagram-v2
    [*] --> PythonOwned
    PythonOwned --> Consented: Exact profile and backup selection
    Consented --> Quiesced: Close all clients and take exclusive gates
    Quiesced --> BackupVerified: Pinned SQLite backup and independent verification
    BackupVerified --> CandidateIntent: Persist new candidate identity and target
    CandidateIntent --> CandidateCommitted: New-copy schema transaction commits
    CandidateCommitted --> CandidateVerified: Independent schema and full-row verification
    CandidateVerified --> RustSelected: Catalog atomically publishes pointer and epoch
    RustSelected --> RustActive: Compatible readers pass and explicit write activation
    RustActive --> RustActive: Atomic command plus revision and receipt
    RustSelected --> RollbackIntent: No Rust writes and operator chooses rollback
    RustActive --> RollbackIntent: Quiesce and preserve latest Rust state
    RollbackIntent --> PythonSelected: Verify unchanged prior or lossless new v1 copy
    PythonSelected --> PythonOwned: Explicit guarded Python activation
    RustActive --> RestoreIntent: New consent including lost commit interval
    PythonOwned --> RestoreIntent: New consent and verified current backup
    RestoreIntent --> RestoreVerified: Restore to new generation and verify
    RestoreVerified --> CompatibleSelected: Catalog publishes schema-compatible owner
    CompatibleSelected --> PythonOwned: Explicit Python activation for v0 or v1
    CompatibleSelected --> RustActive: Explicit Rust activation for v2
    CandidateIntent --> RecoveryRequired: Interruption or ambiguous artifacts
    CandidateCommitted --> RecoveryRequired: Commit or manifest uncertainty
    RustSelected --> RecoveryRequired: Startup or owner mismatch
    RestoreIntent --> RecoveryRequired: Incomplete restore
    RecoveryRequired --> Quiesced: Explicit authorized reconciliation only
```

`Consented` is permission for that selected operation, not persistent write
activation. `RustSelected` records selection while admission remains closed;
it is not a running writer. Fresh daemon restart revalidates prior explicit
enablement and generation; an incomplete maintenance operation never auto-resumes.
If reader checks fail, keep write admission closed and offer explicit rollback.
Maintenance stops managed sessions/revokes context leases through existing controls;
resumption requires fresh context binding, not cached pre-cutover authority.

### Failure and recovery decisions

| Boundary / failure | Required future decision | Retained state / visible outcome |
| --- | --- | --- |
| Held lock, long-lived MCP reader, unknown old process | Close admission, bound wait, refuse; no kill or force-unlock | Original owner/generation unchanged; Busy |
| Identity/consent/path/permissions mismatch | Refuse before write-capable DB open | No guessed profile; Unauthorized or RecoveryRequired |
| Insufficient space preflight | Do not start backup/migration | Original generation intact; StorageFull |
| Backup API interruption, FULL/I/O/flush error or failed full-row verification | Ineligible partial copy; never migrate/publish it | Preserve source and any earlier verified backup; sanitized failure |
| Crash during candidate copy / before migration commit | Reopen only an authorized owned candidate for SQLite recovery; independently verify; never trust stage alone | Original remains active; v0/v1 rollback or incomplete candidate, explicit retry into new candidate if needed |
| Schema commit succeeds, catalog stage not updated | Inspect candidate exact v2 metadata/epoch, full legacy records and verified backup; matching intent may be recorded after explicit reconciliation | Original pointer unchanged; no automatic Rust writer; mismatch refuses |
| Catalog publication transaction interrupted | SQLite yields old or new pointer; independently reconcile profile and manifest before activation | No dual owner, no “latest filename” guess; RecoveryRequired if inconsistent |
| Crash after pointer/owner switch, before write activation | Reopen selected generation with closed admission; check readers, epoch and committed state | Selected v2 preserved; explicit activation or prewrite rollback |
| Writer crash / response lost after mutation commit | Release OS lease; reconcile active generation; query same operation ID/receipt | Committed mutation reported exactly once; unknown result never becomes automatic new command |
| Return Python ownership after new Rust writes | Back up current v2; new lossless v1 projection, verify all current records and compatible Python reads/writes; publish new epoch under gate | Current data preserved; sealed v2 retained; no stale command/revision reuse |
| Downgrade would drop new domain fields or legacy metadata | Refuse lossless rollback; no implicit old-backup restore | Current owner remains stopped/unchanged pending operator choice |
| Crash during restore or incomplete restore verification | Keep current selected generation and verified backups; partial restore ineligible, no overwrite/reuse by guess | RecoveryRequired; explicit new restore candidate after inspection |
| Crash after restored pointer publication | Inspect selected restored generation and matching catalog intent/epoch; require owner-compatible activation | Prior current generation retained, never resurrect two writers |
| Missing/tampered/unknown-version catalog, changed backup, unexplained WAL/journal | Refuse rather than reconstruct authority from artifacts alone | All existing artifacts retained; RecoveryRequired |
| Retention cleanup interruption | Delete only explicitly selected, inactive, unreferenced owned artifacts; never active or sole recovery copy | Interrupted cleanup reported; keep catalog references consistent before resuming |

For logical verification compare all project/preference/Handoff fields plus
`sqlite_sequence`, raw validation JSON and unknown nested evidence keys, scopes,
states, timestamps and nullable fingerprint. Checksums help detect change; they do
not authenticate data origin. Unexpected records/objects or provenance changes
refuse. Read-only SQLite may still need WAL/SHM bookkeeping; do not promise zero
filesystem metadata effects. No `immutable=1` for live WAL profiles.

## Evidence ledger: historical versus future

M0 reviews source/docs and existing PR evidence only; it runs **no code tests**,
opens no database and does not rerun hosted Actions or local platform matrices.

| Evidence | Exact historical source / result | What remains outside that result |
| --- | --- | --- |
| C2A, #8 / [PR #9](https://github.com/ybakiame/agent-control-plane/pull/9) | Head `538611243e5c0cd3ab138825528a5fe6cf49ee1e`; fresh owned synthetic roots; two v0 shapes, pinned committed-WAL backup, full-record verification, transactional v1, rollback/restore/retry, actual Python writes on v1 and Rust parity | No adopted profile, owner switch, v2, production ACL/retention or durable restart protocol in C2A itself |
| C2A hosted platform evidence | [Run 42496573449](https://github.com/ybakiame/agent-control-plane/actions/runs/42496573449), recorded SUCCESS on Linux/macOS/Windows including guarded rehearsal at that exact head | Historical recorded result, not rerun or fresh support certification |
| C2B1, #10 / [PR #11](https://github.com/ybakiame/agent-control-plane/pull/11) | Corrected head `3d2a7973bc4b9e3a02d761b50bdbb704e3eca7ec`; 11 process tests with 36 main combinations, real child kill/reap/fresh worker, independent reconciliation, copy/restore/manifest faults and bounded refusal | Parent retains capability; not parent death/key-loss recovery, a production catalog, profile adoption or owner cutover |
| C2B1 corrected hosted process recovery | [Run 42500175669](https://github.com/ybakiame/agent-control-plane/actions/runs/42500175669), recorded SUCCESS: three separate Linux/macOS/Windows process-recovery jobs plus three ordinary core jobs | Process-target success is explicit historical evidence, not inferred from unit-test success or rerun by M0 |
| P1-L, #34 / [PR #35](https://github.com/ybakiame/agent-control-plane/pull/35) | Merged `2a5949120aacd0523cd7fa5e417548e1e648bc81`; [scoped local evidence](../evaluation/p1-l-typed-context-resolver.md) retains original full checks and narrower review correction | Pure typed resolver only; no persisted schema, permission engine or storage-writer acceptance |
| M0 documentation review | Source, contract, link, privacy and exact-diff checks only | No Rust/Python/platform/model/DB test PASS claimed |

C2B1's manifest authenticates a synthetic session using parent-held private-pipe
capability and HMAC; valid-prefix replay still lacks an external monotonic anchor.
Parent death/capability loss remains NOT_RUN. The future catalog instead needs
durable local identity/recovery authorization; it cannot inherit a stronger guarantee
from that harness. Neither is a hostile same-user sandbox or proof of event origin.

Retain NOT_RUN for sudden power loss, physical OS disk-full/I/O failures, hardware
flush guarantees, network filesystems, cross-account/privileged ACL attacks,
hostile same-user races, Windows directory flush/hardlink guarantees and parent
loss. C2A's injected write error is simulation. C2B1 has Unix directory-sync checks;
Windows records file-sync-only, not directory durability. Windows temporary ACL
inheritance is not proof of the proposed protected-DACL policy. M0 makes no new
macOS/Windows/WSLg/TUI/UI acceptance claim.

## Phased follow-up briefs

These are proposed issue briefs only: **not opened, implemented or authorized by
M0**. ADR acceptance and resolution of the transition discrepancy precede code.

| Brief / phase | Scope | Required acceptance before progressing |
| --- | --- | --- |
| M1 — synthetic writer and schema contract | Freeze profile-v2/control-catalog exact DDL; separate writer library; fresh synthetic roots only; typed command/revision/receipt semantics | Owner-reviewed transition matrix; complete legacy-row/JSON preservation; unknown-version/schema refusal; atomic confirm/deprecate; competing revisions; duplicate/reused operation IDs; lost commit response; sanitized errors; no writable reader/core/MCP handle; no profile-path API |
| M2 — compatibility and exclusive-owner integration | Genuine Python RO adapter; guard every old CLI/MCP path; authenticated native daemon command opt-in; CLI compatibility; shared connection gates and writer lease | Local Windows and Linux/WSL tests of legacy reads and new v2 readers, authorization negatives before DB access, idle MCP shutdown/drain, simultaneous processes/aliases, bounded Busy, lease death/restart, read/write grant separation and unchanged IPC/MCP/session revocation; retire version-blind Python before rollout |
| M3 — synthetic cutover/rollback/restore and privacy | Private SQLite control manifests, generations, explicit consent; latest-state v2→v1 rollback and separately consented older restore | Reproduce every failure-table boundary with genuine killed/restarted processes, including control/profile commit gap and post-write downgrade; full Python/Rust logical parity and reader reopen; no sidecar orphan/overwrite; catalog/version/backup corruption refusal; Unix permissions and actual Windows ACL policy; bounded space/error injection and honest untested limits |
| M4 — per-profile operator rollout opt-in | Separately authorized real profile, exact source/control/backup identities and retention window; no bulk/default discovery | Owner approves exact operation/locations/data-loss implications after M1–M3 review; stop all old clients, verified backup and independent reader checks, explicit selection then activation; rollback rehearsal accepted; supported local filesystem/durability policy and sanitized local report |
| M5 — close rollback window and retire Python direct writes | Explicit operator decision after stable use; keep Python compatibility reads/frontend | Verify no remaining legacy writers; selected inactive-artifact deletion and receipt-retention policy; preserve required verified recovery point; no hidden deleted-content history; removal of direct-write fallback after explicit reviewed release |

The matrix is local-only; no GitHub Actions run is required or authorized by M0.
Later code changes must run AGENTS-required Python tests and targeted meaningful
Rust/security/compatibility tests. Reuse prior evidence with exact scope; run broad
matrices only when affected boundaries justify them. New provider/model turns are
irrelevant to storage acceptance and not implicitly authorized.

### No-go and separate authorization

No production adoption until: ADR accepted; transition discrepancy resolved;
synthetic writer/compatibility/recovery gates reviewed; all clients fenced; exact
source, profile/control root and backup destination selected; permissions/lock/
durability policy supported; independently verified backup retained; lossless
rollback supported; space and restore-point implications understood. Live handles,
unknown owner/version/epoch, unsafe alias/ACL, missing/changed backup, insufficient
space, network filesystem, partial catalog or inability to reconcile mean refusal.

Real-profile consent is per operation and expires when its bound identity or
operation changes. ADR merge, earlier integration permissions, agent auth, Start,
Handoff instructions, successful tests or a project being public do not supply it.
The owner must separately approve migration/cutover and any later lossy restore.
Future N/O/P/Q/R tasks retain their separate domain and acceptance boundaries.

## M0 review record

The deliverable consists only of this plan, Proposed ADR-0005, architecture and
roadmap updates. Source/PR review did not instantiate Store, inspect a profile,
read default state locations, access credentials or change system settings.
Local documentation checks passed: 58 relative links/anchors across the four
changed documents; balanced Markdown fences; exact four-document scope; no
private-path/credential patterns; `git diff --check` for whitespace. Eight unique
external references were checked against fetched repository issue/PR evidence,
without rerunning the historical Actions jobs. These checks are not application
tests or storage/runtime evidence.
No owner acceptance, code implementation, real-data authorization or automatic
merge is inferred from submitting the Draft PR.
