# ADR-0005: local storage and write ownership

- Status: **Accepted, partially superseded by [ADR-0006](0006-rust-only-runtime.md)** — the daemon single-writer and security decisions remain; Python compatibility and legacy-schema migration decisions are superseded
- Date: 2026-11-12
- Authority: [P1-M0 / Issue #36](https://github.com/ybakiame/agent-control-plane/issues/36)
- Source baseline: main `2a5949120aacd0523cd7fa5e417548e1e648bc81` (PR #35 merged)
- Governing decisions: [Accepted ADR-0004](0004-product-domain-trust.md),
  [local IPC ADR](0002-local-ipc-capabilities.md) and [AGENTS.md](../../AGENTS.md)

> **Current authority:** Read [ADR-0006](0006-rust-only-runtime.md) for the accepted Rust-only product runtime and fresh Rust-native Profile schema v1. ADR-0006 supersedes this ADR's Python thin-frontend, Python read-only adapter, Python rollback-owner, legacy-schema compatibility/migration, compatibility-release, and schema-downgrade decisions. The daemon as single writer, operator-only write IPC, explicit profile-scoped grants, strict transactions/receipts, no agent/MCP writes, and safe single-writer/recovery principles remain. The detailed sections below preserve the original M0 decision record and are historical wherever ADR-0006 differs. The old Python v1 and M1 synthetic v2 schemas are not the new Rust-native v1 schema.

## Context and authorization

This accepted architecture establishes future boundaries, not permission to open a
real profile, perform migration, or activate a production Rust writer. M1 authorizes
only synthetic fixture work under a separately scoped Issue and reviewed PR.
Python `Store` currently owns writes. Its constructor creates directories, opens
read-write, enables WAL and initializes/extends schema, even for CLI reads and the
Python MCP launch path. It does not check `user_version`. Rust `acp-store` and the
daemon open explicit existing databases read-only and reject unsupported versions.
Thus “MCP has no mutation tools” does not yet mean the Python launch is free of
database initialization writes. No code changes here repair that distinction.

[C2A](../p1/synthetic-migration.md) already provides guarded synthetic backup,
v0→v1 migration, rollback, restore and actual Python/Rust compatibility.
[C2B1](../p1/process-recovery.md) already provides synthetic restart manifests and
genuine process-kill recovery; its corrected hosted Linux/macOS/Windows matrix
passed. These are prerequisites, not production migration machinery.
[P1-L](../p1/typed-context-v1.md) is merged pure domain resolution, not persisted
schema, review authorization or a new protocol. Exact historical evidence and
future acceptance gates are in the [execution plan](../p1/storage-ownership-plan.md).

## Decision: authoritative writer and clients

Choose the **native daemon as the single ordinary write owner for an explicitly
selected profile**, through a separately versioned operator-only local IPC
extension. Ownership is per profile/generation, not per UI window or project.
Daemon startup without a separately recorded owner opt-in remains read-only;
startup never migrates or discovers a profile to adopt.

| Alternative | Benefits | Cost / decision |
| --- | --- | --- |
| Dedicated Rust operator CLI writer per command | Small process lifetime; offline operation; no new daemon mutation transport | UI and multiple CLIs still need coordinated authority, fencing and receipts; repeated ownership acquisition and a second ordinary write route. Not selected |
| Authenticated operator-only daemon writes | One serialized transaction owner, shared CLI/UI path and one place to invalidate readers | Requires explicit write authorization, bounded admission and daemon availability. Selected; unavailable daemon fails closed |
| Python and Rust direct writes against the same profile | Superficially easier compatibility | Cannot coordinate old constructors or enforce revisions; rejected |

Future native Desktop/TUI, `acp-query` and Python `acpctl` send typed commands to
that owner. Python becomes a thin compatibility frontend, retaining supported
command names/read output, not a second SQLite writer. It may use a trusted native
helper with fixed argv and private framed stdin if needed; no shell, SQL, embedded
credential argv, arbitrary executable or fallback to `Store`. Missing authorization
or a stopped daemon returns an actionable sanitized refusal, not auto-start.

Operator role is necessary but insufficient: require an explicit profile-scoped
write enablement, current owner epoch and command-specific confirmation. Existing
operator read/session credentials do not silently acquire write access. Define the
new extension/version and opt-in grant separately from IPC v1 read DTOs, ACP Start,
model authentication, permission mediation and context sharing. Check role and
grant before storage access, and again at transaction admission; serialize revoke
with admission. A transaction admitted before revocation can commit and must be
reported as such. Revocation cannot recall a committed write.

Agent credentials (ordinary or session-bound), MCP, prompts, repository documents
and Handoffs cannot issue or confirm mutations. MCP keeps exactly its two read
tools. Python MCP must use a genuine read-only adapter, constructed without
`Store`, with explicit existing-path open, version/schema checks and no mkdir,
DDL, journal-mode change, checkpoint or migration. All Python read entrypoints
must avoid the write constructor. Compatibility reads after cutover remain
supported through this adapter or authenticated read IPC; neither grants a writer.

Maintenance is a mutually exclusive mode of the same ownership protocol: an
explicit operator maintenance executable may back up, migrate or restore while
the daemon and every reader/writer are closed. It cannot run ordinary preference
commands and cannot coexist with a daemon writer. No service auto-resumes after
maintenance until reconciliation and explicit owner activation succeed.
Windows→WSL owner-query support remains read-only; a native WSL owner must manage
its own WSL filesystem profile. This ADR adds no cross-OS write/execute bridge.

## Decision: crates, transactions and commands

Keep `acp-store` read-only. Propose a separate `acp-write` library, linked only by
the opted-in native owner/maintenance binaries, with private SQLite connections.
The library owns transactions; callers cannot pass a connection, SQL, table name,
arbitrary patch or transaction callback. Share schema descriptions/validation via
a small internal module without exposing a writable handle through reader traits.
`acp-core::typed_context` stays independent of storage, transport and authorization.
`acp-rehearsal` keeps its fresh-temp-root restriction; never add profile adoption.

The first command set mirrors existing operator operations only:
`RegisterProject`, `SuggestPreference`, `ConfirmPreference`, `RejectPreference`,
`DeprecatePreference`, `DeletePreference`, `PutHandoff`. No generic mutation,
confirmed-value edit, contract parser, general proposal API or snapshot writer.
Commands carry explicit profile/generation, operation ID, expected record revision
(or expected absence), and bounded typed fields. Handoff capture rechecks the
registered root and captures live Git/fingerprint state through existing trusted
adapters; an agent-supplied “verified” flag is never accepted.

Validate transport shape/unknown fields/bounds before admission; validate current
registration, exact scope, transition from the row's **current pre-command state**,
revision and owner epoch inside `BEGIN IMMEDIATE`. A stored terminal status alone
cannot establish which historical transition occurred or authorize a new one.
A confirmation changes its target and deprecates same-scope/key
confirmed peers atomically, advancing all affected revisions. Suggestion always
starts suggested; changing confirmed content/scope requires a new suggestion and
review. Delete removes content, not a shadow history of that content. Imported
legacy rows retain their original review state and unknown evidence attribution.

Each committed transaction atomically writes the mutation, monotonic commit
sequence, affected technical revisions and an idempotency receipt in the **same
profile DB**. Timestamps are display/history, never revision tokens. Imported rows
start technical revision 1; this is concurrency bookkeeping, not a P1-L RuleRef,
new review, provenance upgrade or validation. Tokens include generation so they
cannot survive restore or owner rollback. Missing/wrong expected revision returns
`Conflict` without changes; deletion keeps only an ID/revision tombstone.

Retry of the same operation ID and identical request returns its original minimal
receipt before CAS, even after a lost response; a different request is
`IdempotencyConflict`. Receipts store identifiers, outcome, revision and a private
request digest, never command values, Handoff text or full response caches. Digests
leak equality and can permit guessing low-entropy input; they stay private, are not
published evidence, and have the same explicit retention policy as the DB. The
initial design retains receipts for the active generation; size exhaustion refuses
new commands rather than silently evicting replay protection. An operation ID
includes its generation; an old/unknown generation is always refused. Later receipt
compaction needs a reviewed expiry protocol, not transparent replay.

These receipts/revisions are technical concurrency and commit bookkeeping, not
ADR-0004's domain-level human `ReviewDecision` with rationale, decision time and
attributed actor. A confirmation still requires the authenticated operator's
explicit action on the exact record/revision and scope; a receipt, request digest
or operator role label cannot supply that approval or independently verified actor
identity/evidence. Existing status/timestamp fields remain meaningful but do not
constitute a complete review/audit history. Full decision history and its provenance
remain P1-O work unless separately accepted; initial v2 does not claim to deliver it.

Typed results contain IDs/revisions/commit sequence, with content fetched through
authorized reads. Errors include `Unauthorized`, `ReadOnly`, `UnsupportedVersion`,
`IncompatibleSchema`, `InvalidCommand`, `InvalidTransition`, `Conflict`,
`IdempotencyConflict`, `Busy`, `StorageFull`, `IoFailure`, `RecoveryRequired` and
`OutcomeUnknown`. Logs expose only these classes and bounded non-sensitive
operation labels, never SQLite errors, raw SQL, values, paths or request bodies.
Timeout after possible commit is `OutcomeUnknown`; query/retry that same operation
ID, never automatically submit a new mutation. SQLite wait is at most five seconds
per command, with bounded queue/size and no background retry loop.

### Resolved owner decision: strict future transitions, preserved history

The [owner's PR #37 review](https://github.com/ybakiame/agent-control-plane/pull/37#pullrequestreview-5867130588)
on 2026-11-12 resolves the compatibility difference: future Rust writes must reject
`suggested → deprecated` with `InvalidTransition`. The allowed new transitions
are exactly `suggested → confirmed | rejected` and `confirmed → deprecated`, in
accordance with Accepted ADR-0004. Deletion remains a separately authorized existing
command, not a state transition. No new Withdraw state or synonym is introduced.
See the [transition matrix and synthetic gates](../p1/storage-ownership-plan.md#preference-transition-contract-and-compatibility-gates).

The current Python SQL still permits `suggested → deprecated`; M0 does not change
that code or claim a guard already exists. The legacy CLI may remain as-is only
during its explicitly Python-owned phase; its permissive/version-blind direct path
is temporary and unsafe for post-cutover use. M2 must fence/retire those entrypoints,
with no direct-write fallback, and enforce the chosen semantics in the compatibility
frontend and any guarded Python owner used for rollback. Rejecting that old CLI
edge is an intentional compatibility change, not exact write-behavior parity.

Migration preserves every historical `deprecated` row, including one possibly
created through the permissive Python edge, with its original ID, scope, review
status, timestamps, opaque JSON bytes and unknown provenance. Do not reconstruct
its prior state, reclassify it as rejected, repair it or retroactively judge the
transition. Preservation compares legacy logical fields/opaque bytes, not SQLite
page layout. New-command validation reads the current row in the writer transaction;
denied transitions change no records, revisions, commit sequence or receipts.
Previously committed identical operation retries still return the existing receipt,
rather than attempting a fresh transition. The owner resolved semantics only:
ADR-0005 was separately accepted by the owner after PR #37 merged; implementation
and synthetic compatibility tests remain NOT_IMPLEMENTED / NOT_RUN, with no code
or real-profile authorization from acceptance.

## Decision: schema and source ownership

Both v0 shapes and v1 are supported migration inputs. Reserve **profile schema v2**
for the first production-writer candidate: existing fingerprinted v1 tables plus
strictly specified private owner/generation metadata, record-revision/tombstone
and command-receipt tables. Their exact DDL, constraints and limits must be reviewed
and frozen in the synthetic implementation task before code. No tables are added
to v1 under the same version; current readers reject extra tables. The control
catalog has its own independent format version; neither version is P1-L DTO
`schema_version=1` or a future repository-contract file-format version.

Migration creates a new candidate and changes schema plus `user_version` in one
transaction. A legacy missing fingerprint becomes nullable NULL; no historical
fingerprint is fabricated. Every pre-existing ID, SQLite allocation sequence,
project/root, scope, key/value, source, review state, timestamp and Handoff field
is preserved. Preserve opaque validation JSON bytes and nested unknown keys;
do not parse/re-serialize, normalize timestamps, deduplicate rows or reinterpret
unknown provenance. New metadata must not make an old claim independently verified.
Malformed/unsupported input refuses before migration rather than repairing it.

New Rust/Python readers must support both v0 shapes, v1 and exact v2, rejecting
unknown future versions, malformed shapes and unexpected objects. Old Rust readers
reject v2; **old Python does not enforce version refusal** and is unsafe to launch
against it. A mandatory compatibility release must fence every Python entrypoint
before any owner cutover; an arbitrary old same-user script is outside that fence.
If old processes cannot be quiesced/retired, rollout is a no-go. See the
[compatibility matrix](../p1/storage-ownership-plan.md#reader-and-writer-compatibility).

Repository contracts remain reviewed repository files; SQLite can only index
source identity/version/digest and invalidation state as a disposable cache.
Missing/stale cache is unknown and prompts a live source read, never an authoritative
fallback. Cache tables, contract formats, parsers, discovery, scaffolding and export
are not part of initial v2. General review/evidence/snapshot persistence stays in
later N–Q tasks, not an implicit serialization of all P1-L DTOs.

## Decision: exclusive access and durable publication

Use a stable per-profile OS coordination lock plus a versioned private **SQLite
control catalog** for profile identity, writer epoch, active generation and
append-only stage manifests. This keeps application-owned durable state in SQLite
as required by ADR-0004; synthetic C2B1 JSON manifests remain test-only. Catalog
rows contain opaque identities, fixed relative artifact roles, schema versions,
verification digests, sequence/stage and sanitized outcomes, not profile contents,
credentials or absolute paths. They are recovery bookkeeping, not Q evidence-chain
or identity-attestation work. Do not transplant C2B1's parent-memory HMAC key into
a production secret store or claim its parent-loss recovery.

The control root is a fixed private child of the canonical profile directory,
with a deterministic per-database identity binding, not a caller-selectable second
lock namespace. An alternate control root for an already-bound DB is refused.
Canonical parent identity and OS file identity bind the lock/catalog/database;
reject symlink/reparse/hardlink aliases, unexpected files and identity changes.
Validate through opened handles where supported and retain the lock handle. Do
not use only a path spelling, PID or stale timestamp. The coordination lock file
is never unlinked/replaced while a holder might exist. Local filesystem only;
network/interop-mounted profiles and unsupported locking semantics are refused.
Managed candidate/active generations use fixed relative roles inside that root.
External original/backup destinations are bound by file identity and role; after
restart the operator must reselect them for reconciliation rather than storing or
discovering absolute paths in a manifest. Unavailable selections keep recovery
blocked. A catalog cannot claim another profile's original file or lock namespace.

All participating Python/Rust/MCP/daemon readers hold a shared profile gate for
their whole connection lifetime and bind one active generation. Ordinary readers
coexist with the one daemon writer, which additionally holds a separate exclusive
writer lease. Each command rechecks owner epoch; SQLite serialization is a second
layer, not ownership authority. Maintenance first closes admission, asks operators
to stop sessions/clients, drains requests and closes **all** DB handles, then
acquires the exclusive profile gate and writer lease. If a client cannot close,
return `Busy`; never kill it, force-unlock or unlink its files. Native shared/exclusive
lock behavior and starvation bounds must be demonstrated on each supported OS.

OS lease release on process death permits recovery inspection, not automatic
ownership transfer. A fresh process must reconcile catalog intent, active pointer,
candidate schema/owner metadata and verified prior copy before enabling writes.
Never break a held lock because its PID appears dead. Inconsistent/missing catalog,
unknown epoch/version, permission failure or ambiguous candidate means
`RecoveryRequired`, no writable profile open or guessed “newest file.” Old Python
does not participate today; compatible launchers and retirement of old processes
are a prerequisite, not something SQLite WAL or an advisory lock can enforce.

Use immutable **database generations in separate private directories**. Never
rename/overwrite an open SQLite file, detach its WAL/SHM/journal, or copy only the
main file as backup. Once candidate migration is committed, independently reopened
and verified, a single control-catalog transaction publishes the new active
generation, owner epoch and corresponding manifest. Readers reopen only after
maintenance releases the gate; cached paths/revisions/leases are invalidated.
Old direct-path readers must be stopped and explicitly reconfigured. Reader smoke
and reconciliation occur while write admission stays closed.

The candidate profile transaction and catalog publication are **two commits**,
not a cross-database atomic transaction. Persist intent first and reconcile that
gap from independently inspected artifacts, as specified in the plan. Catalog
publication is the sole logical cutover point; neither an unrecorded candidate
commit nor a manifest stage by itself enables a writer. Ordinary profile mutations
do not update the catalog, avoiding a second-DB atomicity dependency.

SQLite FULL synchronization, file flushes and Unix directory sync are planned
mechanisms, not power-loss proof. Windows directory durability and catalog creation
must have explicit local evidence/policy before rollout; file flush is not directory
flush. An incomplete catalog is a refusal, never authority reconstructed from PID
or filesystem modification time. A restart may identify a safe state without
automatically progressing migration, restore or activation.

## Decision: consent, backup, rollback and privacy

Before any future profile access, an operator must select the exact profile DB,
private control root, backup destination, operation, source/target versions,
expected owner/generation, retention and rollback window in a local consent record.
Bind it to canonical/file identities checked again under the gate; path or identity
change invalidates consent. No environment/default-profile scan, startup migration,
bulk adoption, UI checkbox inferred from Start, or inherited Handoff approval.
The local consent view may display selected paths for review; logs, public reports
and exports must not. Selection is not permission to access other installations.

Preflight checks source/backup/candidate separation, same-filesystem publication
assumptions, private permissions, exact schema, free-space estimate and reserved
headroom for source, verified backup, candidate, restore and journals. Space can
change after preflight: every allocation/SQLite write/flush/commit must handle
FULL/I/O failure, preserve the verified prior copy and stop safely. No best-effort
partial candidate becomes active. No large-data or physical disk-full claim is made.

Pin a read transaction and use the SQLite backup API, including committed WAL and
excluding uncommitted changes. Independently reopen the completed backup read-only;
verify integrity, foreign keys, exact schema, all logical rows/fields and allocation
sequence against that pinned snapshot. Only a verified backup is eligible for use.
Backup, candidate and restore destinations are exclusively reserved new files;
collisions/sidecars refuse. Frozen backups have no unexpected live sidecars. Never
delete a source sidecar as “cleanup”; authorized SQLite recovery/checkpoint/close
under the gate must finish or the operation stops. Keep each retired generation
and its sidecars together until explicitly eligible for deletion.

The plan separates three outcomes:

1. **Before Rust writes:** revert the active pointer only after verifying the prior
   generation is unchanged and compatible; still hold exclusive gates, increment
   epoch and require explicit activation of the guarded Python owner.
2. **After Rust writes, return ownership without data loss:** quiesce Rust, create
   a fresh backup of the latest generation, then build a new v1 compatibility
   generation. Preserve all current legacy table records/IDs/sequences and opaque
   JSON; remove only the v2 technical tables in the new copy and set v1 in one
   transaction. Retain the sealed v2 generation/receipts through the rollback
   window; invalidate all old revision/operation tokens. Independently verify
   Python/Rust v1 reads and the guarded Python writer before publication. This
   explicit downgrade projection needs synthetic acceptance; if v2 ever contains
   domain data not losslessly representable in v1, refuse rather than drop it.
3. **Restore an older backup:** potentially discards later writes. Requires new
   explicit consent naming the selected restore point and lost commit interval,
   plus a verified backup of current state. Restore into another new generation,
   verify before publication, then activate only its schema-compatible owner.
   For v2, rebind only technical generation/epoch bookkeeping in the new copy and
   invalidate old operation/revision tokens; preserve all selected legacy records.
   Never silently restore a pre-cutover backup as ownership rollback.

Rollout chooses a rollback deadline explicitly; default is retain until the operator
reviews successful operation and opts to close it. No automatic age-based removal
of the only verified prior copy. At closure, offer selected deletion of retired
generations/backups/manifests and receipts, after checking active/recovery references.
Private values must not survive deletion in a hidden response/history cache.
Minimal tombstones may retain identity/revision only. Backup retention is disclosed;
WAL/free pages, filesystem snapshots and OS/cloud backups can retain deleted bytes.
Neither checkpoint, VACUUM nor application deletion promises forensic erasure.

All roots, catalogs, backups, manifests and sidecars stay user-local, outside
repositories/shared/cloud-synced destinations unless a separate reviewed policy
allows them. Unix creation policy: owner-only directories 0700, files 0600, refuse
unsafe ownership/modes rather than chmod unrelated files. Windows requires a
protected DACL granting the selected user and documented required OS principals,
with no broad Users/Everyone/other-user access; inspect effective ACL/inheritance
and fail closed on unsupported filesystems. C2A/C2B1 inherited temporary ACLs do
not establish that production policy. Never log/export values, Handoff/evidence,
credentials, raw SQL, private paths or raw SQLite diagnostics. Permissions, leases,
digests and operator IPC are cooperative controls, not a sandbox against a hostile
same-user process, administrator or credential theft.

## Consequences, gates and non-goals

The chosen model adds a compatibility release, explicit maintenance downtime, v2
reader upgrades and a private control catalog before Rust can own writes. A daemon
failure makes writes unavailable rather than falling back to Python. Recoverability
requires retained verified generations; ownership rollback after writes costs a
verified compatibility projection. These costs buy a single inspectable authority
and avoid silently overwriting live SQLite files or racing two writers.

Accepting this ADR does not open a real DB or authorize rollout. Follow the
[phased briefs and no-go gates](../p1/storage-ownership-plan.md#phased-follow-up-briefs).
No N–R domain persistence, generalized review UI, parser/scaffold/export, MCP writes,
new UI, model call, telemetry, network listener, package dependency or code is added
by M0. ADR-0004 remains unchanged. The owner resolved future transitions; M1/M2
implementation, transition tests and the intentional CLI difference remain pending.
Power loss, physical disk-full, network filesystems, cross-account ACL attacks and hostile same-user
guarantees remain unverified, not inferred from synthetic or document checks.
