# P1-M1 — Profile v2 schema and synthetic writer contract

Status: frozen M1-A contract before implementation, under [Issue #38](https://github.com/ybakiame/agent-control-plane/issues/38)
and Accepted [ADR-0005](../adr/0005-local-storage-and-write-ownership.md) /
[ADR-0004](../adr/0004-product-domain-trust.md). Base main
`aef3a501288133549afbd85edfcf5c2cef50f7dd`, clean before branching.
This specifies disposable fixtures only. It grants no operator authority, real
profile access, production writer, daemon/IPC/CLI/MCP integration or owner cutover.
The [M0 plan](storage-ownership-plan.md) still gates M2–M5.

## Exact profile schema

`PRAGMA user_version=2`. The three legacy tables retain their constraints,
identities, values, review states, sources, timestamps and SQLite allocation
sequence. No generalized rule/review/evidence/snapshot/cache tables are added.
DDL below is the canonical fingerprinted declaration; migration also accepts the
explicit equivalent Handoff fingerprint column appended after `updated_at` and
before the composite PRIMARY KEY, as produced by SQLite ADD COLUMN. It does not
rebuild a legacy table merely to change column order. Whitespace, keyword case
and identifier double quotes outside string literals are normalized for comparison;
string literals, constraints, column order and all non-internal objects otherwise
must match the frozen allowlist. No additional indexes/triggers/views/tables.

```sql
CREATE TABLE projects (
    name TEXT PRIMARY KEY,
    root TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL
);
CREATE TABLE preferences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT NOT NULL CHECK (scope IN ('global','project')),
    project TEXT REFERENCES projects(name) ON DELETE CASCADE,
    pref_key TEXT NOT NULL,
    value TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'suggested'
      CHECK (status IN ('suggested','confirmed','rejected','deprecated')),
    source TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK ((scope = 'global' AND project IS NULL) OR
           (scope = 'project' AND project IS NOT NULL))
);
CREATE INDEX prefs_lookup ON preferences(scope, project, status, pref_key);
CREATE TABLE handoffs (
    project TEXT NOT NULL REFERENCES projects(name) ON DELETE CASCADE,
    task_id TEXT NOT NULL,
    objective TEXT NOT NULL,
    base_sha TEXT NOT NULL,
    branch TEXT NOT NULL,
    worktree_state TEXT NOT NULL,
    worktree_fingerprint TEXT,
    completed TEXT NOT NULL,
    next_action TEXT NOT NULL,
    validation_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (project, task_id)
);
CREATE TABLE acp_profile (
    singleton INTEGER PRIMARY KEY CHECK (singleton=1),
    profile_id TEXT NOT NULL CHECK (length(profile_id)=32),
    generation TEXT NOT NULL CHECK (length(generation)=32),
    owner_epoch INTEGER NOT NULL CHECK (owner_epoch>0),
    commit_sequence INTEGER NOT NULL CHECK (commit_sequence>=0),
    receipt_limit INTEGER NOT NULL CHECK (receipt_limit BETWEEN 1 AND 4096)
);
CREATE TABLE acp_revisions (
    kind TEXT NOT NULL CHECK (kind IN ('project','preference','handoff')),
    project TEXT NOT NULL,
    record_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision>0),
    deleted INTEGER NOT NULL CHECK (deleted IN (0,1)),
    PRIMARY KEY (kind,project,record_id),
    CHECK ((kind='project' AND project='' AND deleted=0) OR
           (kind='preference' AND project='') OR
           (kind='handoff' AND project<>'' AND deleted=0))
);
CREATE TABLE acp_receipts (
    operation_id TEXT PRIMARY KEY CHECK (length(operation_id) BETWEEN 1 AND 64),
    generation TEXT NOT NULL CHECK (length(generation)=32),
    request_digest BLOB NOT NULL CHECK (length(request_digest)=32),
    commit_sequence INTEGER NOT NULL UNIQUE CHECK (commit_sequence>0),
    result_json TEXT NOT NULL CHECK (length(result_json) BETWEEN 1 AND 65536)
);
```

`acp_profile` has exactly one row. The private workspace creates random 128-bit
lowercase-hex profile/generation identities, epoch 1, commit sequence 0, default
receipt limit 4096. Synthetic tests may choose a smaller positive limit to exercise
exhaustion; it never increases beyond 4096. Generation/epoch are fencing labels,
not credentials or evidence. Imported live records start technical revision 1.
Revision keys: project `('project','',name)`; preference `('preference','',decimal-id)`;
Handoff `('handoff',project,task-id)`. Only deleted preferences have tombstones,
containing identity/revision/deleted flag, never their value, key, source or scope.
All live rows have exactly one live revision; no unrelated revision may exist.
Preference IDs are positive signed-64-bit SQLite integers, never reused after delete;
`sqlite_sequence` is preserved and must cover live and tombstoned allocated IDs.
Technical revisions and sequence are positive signed-64-bit integers; exhaustion
refuses without wraparound. Receipt count equals commit sequence because no pruning
occurs in this generation. Every receipt binds this generation and one unique
sequence; its result is a minimal typed receipt, not arbitrary JSON data.

Legacy input is exactly one of v0 without fingerprint, v0 fingerprinted, or v1
fingerprinted. V1 without fingerprint, unknown versions, malformed columns/types/
constraints/indexes, extra objects and invalid records fail closed without repair.
The source schema is the same DDL above minus the three `acp_*` tables, with only
v0 allowed to omit fingerprint. A legacy missing fingerprint maps to NULL;
nullable fingerprints and opaque validation JSON bytes are preserved. Input
states are validated as stored data, not reconstructed transition histories.

M1 uses private strict v2 readers for verification. Existing `acp-store` remains
unchanged: strict RO v0/v1, **rejects v2**. Never run current Python Store against
v2. Runtime v2/Python RO adoption is M2, not a side effect of this crate.

## Planned control catalog (format 1, not implemented by M1)

The following is the frozen planned DDL/interface, not a usable catalog or an
additional profile table. `PRAGMA user_version=1` is independent of profile v2 and
any repository-contract format. All identifiers/digests/roles stay private.

```sql
CREATE TABLE catalog_profile (
    singleton INTEGER PRIMARY KEY CHECK (singleton=1),
    profile_id TEXT NOT NULL,
    active_generation TEXT NOT NULL,
    owner TEXT NOT NULL CHECK (owner IN ('python','rust')),
    owner_epoch INTEGER NOT NULL CHECK (owner_epoch>0),
    admission TEXT NOT NULL CHECK (admission IN ('closed','enabled')),
    manifest_sequence INTEGER NOT NULL CHECK (manifest_sequence>=0)
);
CREATE TABLE catalog_manifests (
    sequence INTEGER PRIMARY KEY CHECK (sequence>0),
    operation_id TEXT NOT NULL,
    profile_id TEXT NOT NULL,
    previous_state TEXT NOT NULL,
    desired_state TEXT NOT NULL,
    verified_state TEXT NOT NULL,
    source_generation TEXT NOT NULL,
    target_generation TEXT NOT NULL,
    source_version INTEGER NOT NULL CHECK (source_version IN (0,1,2)),
    target_version INTEGER NOT NULL CHECK (target_version IN (0,1,2)),
    owner_epoch INTEGER NOT NULL CHECK (owner_epoch>0),
    source_role TEXT NOT NULL CHECK (source_role='source'),
    backup_role TEXT NOT NULL CHECK (backup_role='backup'),
    candidate_role TEXT NOT NULL CHECK (candidate_role='candidate'),
    source_digest BLOB NOT NULL CHECK (length(source_digest)=32),
    backup_digest BLOB CHECK (backup_digest IS NULL OR length(backup_digest)=32),
    verified_commit INTEGER NOT NULL CHECK (verified_commit>=0),
    outcome TEXT NOT NULL
);
```

Planned typed maintenance interface: `RecordIntent`, `RecordVerification`,
`PublishSelection(expected_manifest, generation, owner, epoch)` and read-only
`InspectSelection`. No SQL/callback API. Valid stage/outcome enums are those in the
[M0 state/failure model](storage-ownership-plan.md#coordination-and-durable-stage-model),
not caller text; stage transitions must validate expected predecessor and verified
artifact identity. Selection plus manifest append is one catalog transaction;
profile commit is separate and must reconcile before selection. Source/backup
outside the fixed profile control root require operator reselection on restart.
M3 must implement strict DDL/enum/identity validation, locking, durable manifests
and recovery before any usable catalog. M1 creates **no catalog/manifest files**.

## Typed synthetic command and result boundary

`SyntheticWorkspace::new(FixtureShape)` creates all roots, fixtures and three
synthetic project directories internally. No path arguments, environment profile
lookup, adopted workspace, executable, SQL, connection or transaction callback.
The workspace is non-serializable/non-cloneable; it owns a fresh private TempDir.
`prepare()` creates a new backup and candidate; independent verification is needed
before `writer()` returns a private-connection writer borrowing that workspace.
Multiple writers can test independent SQLite connections; they confer no production
ownership. Reopen is possible only while the same workspace/capability remains alive.

Requests are strict serde DTOs (unknown fields/variants rejected), version 1, with
`binding { profile_id, generation, owner_epoch }`, operation ID and one command.
Operation IDs are 1–64 ASCII letters/digits/underscore/hyphen. IDs include generation
through the binding and receipt; a foreign/old generation is refused before replay.
Profile/generation are exactly 32 lowercase hex characters, epoch positive i64.
`Expected` is `Absent` or `Revision(n)`; n is 1..i64::MAX. Revisions are not timestamps.
New entity commands require Absent; existing entity operations require Revision.

| Command | Fields / identity and CAS |
| --- | --- |
| RegisterProject | name (1–64 ASCII letters/digits/`-_.`, excluding `.`/`..`), fixture root slot Alpha/Beta/Gamma, expected Absent; unique name and absolute harness-generated root; no caller path |
| SuggestPreference | exact Scope Global or Project(name), key/value/source, expected Absent; new allocated ID; always suggested |
| ConfirmPreference / RejectPreference / DeprecatePreference | positive preference ID, exact Scope, expected Revision; validate current row and scope inside transaction |
| DeletePreference | same ID/scope/revision checks; delete any existing review state as a separately authorized operation; advance revision into minimal tombstone |
| PutHandoff | registered project, task ID, objective/completed/next_action, raw validation JSON, fixed synthetic snapshot kind, expected Absent or Revision; exact project/task, no cross-project fallback |

New preference bounds match Python character limits: nonblank key ≤120, value
≤4096, source ≤512 Unicode scalar values, no NUL; empty/control-only fields refuse.
New Handoff: nonblank task ≤120, objective/next_action ≤4000, completed ≤8000;
raw validation JSON ≤16000 characters, array of objects with string `check` and
status PASS/FAIL/SKIPPED/NOT_RUN; unknown nested keys preserved verbatim. NUL
refuses. Snapshot kinds Clean/Dirty/UnknownFingerprint map to fixed bounded
synthetic HEAD/branch/worktree/fingerprint values; no caller “verified” flag.
Harness-only snapshots establish no live Git evidence. M2 must bind operator,
recapture live root/Git/fingerprint and enforce operator-only permission **before**
this layer; possession of a DTO, role string or digest cannot approve a preference.

`Request::from_json` caps input at 128 KiB and returns sanitized InvalidCommand
rather than leaking serde input. Direct typed callers undergo the same bounds.
Canonical bytes are serde serialization of the validated typed request in declared
field order, including binding/operation ID. Outer JSON whitespace/key order does
not matter; string bytes do (including whitespace/key order inside raw opaque
validation JSON). SHA-256 of these bytes is private equality bookkeeping, not a
signature, anonymization or verified actor. Sensitive command DTO Debug is redacted.

`Receipt { generation, operation_id, commit_sequence, affected }` has affected
record identity/revision/deleted only, sorted by `(kind,project,record_id)`; no
value/key/source/root/status/Handoff payload/timestamp. At most 1024 affected rows
and 65536 receipt bytes; capacity failure rolls back all mutations. Confirm may
therefore refuse an oversized peer set rather than partially deprecate it.
These receipts/revisions are **not** the full human ReviewDecision/audit history;
explicit human approval remains necessary and P1-O owns generalized decision history.

Sanitized errors: InvalidCommand, InvalidTransition, IdempotencyConflict, Conflict, ScopeMismatch,
StaleGeneration, OwnerMismatch, UnsupportedVersion, IncompatibleSchema,
InvalidRecord, Collision, UnsafeWorkspace, Busy, Capacity, StorageFull, IoFailure,
Verification, BeforeCommit, OutcomeUnknown. Display/Debug never include SQL,
request data, paths or SQLite/serde diagnostics. No unbounded retry or logging.

## State transitions, CAS and atomicity

| Current row | Confirm | Reject | Deprecate |
| --- | --- | --- | --- |
| suggested | confirmed | rejected | InvalidTransition |
| confirmed | InvalidTransition | InvalidTransition | deprecated |
| rejected | InvalidTransition | InvalidTransition | InvalidTransition |
| deprecated | InvalidTransition | InvalidTransition | InvalidTransition |

No Withdraw state. Historical deprecated rows survive unchanged; only **fresh**
commands follow this stricter-than-Python contract. Receipt replay is not a new
transition: same ID + canonical bytes/generation returns the original committed
receipt before CAS; same ID + changed bytes returns IdempotencyConflict.

After bounded validation, `BEGIN IMMEDIATE` obtains SQLite writer admission, with
100 ms busy timeout (below ADR's five-second ceiling). Recheck exact schema,
profile/generation/epoch, receipt, current-row revision/scope/state in that
transaction. Revision mismatch/missing row with Revision or occupied Absent gives
Conflict; wrong scope gives ScopeMismatch; inactive binding refuses even on replay.
Confirm deprecates only currently confirmed peers with identical key/scope/project,
advancing all their revisions atomically; other projects/global scope stay intact.

One transaction commits data, all affected revisions/tombstones, incremented commit
sequence and receipt. Denial/validation/capacity/precommit failure leaves **all
logical rows, revisions, sequence, receipts and allocation sequence unchanged**.
UTC ISO timestamps are generated for actual mutations, never rewritten on replay
or import. A tombstoned preference ID cannot be reused. No receipts are pruned;
limit exhaustion is Capacity, including after normal retention would be convenient.
Revision/commit arithmetic overflow is Capacity. Full/IO errors are sanitized;
an uncertain COMMIT/response is OutcomeUnknown. Retry the **same** operation ID,
never issue a speculative new ID. The library never retries a mutation itself.

## Migration, fixtures and failure evidence contract

The harness generates three source shapes with alpha/beta registrations, global
and project preferences in every status, duplicate historical confirmed peers,
legacy deprecated rows, allocation gaps/high sqlite_sequence, original timestamp
strings, same task IDs across projects and raw nested opaque JSON (including
unknown `id`/`updated_at` keys). It never calls Python Store or imports real data.
All registered roots are harness-generated directories, not external checkouts.

Source opens RO. Pin a read transaction, inspect exact legacy schema and records,
copy via bounded SQLite backup API, close/reopen backup independently RO and verify
integrity/FKs/schema/full logical fields/allocation sequence. Copy that verified
backup to a separately reserved candidate. Within BEGIN IMMEDIATE add nullable
fingerprint if absent, create only frozen metadata, initialize revisions, set
user_version=2 and commit. Independently reopen candidate RO and compare all original
logical fields plus raw JSON bytes against the pinned source/verified backup;
legacy absent fingerprint alone normalizes to NULL. Verify v2 metadata/revisions
before enabling its synthetic writer. Initial acceptance pins one RO snapshot and
requires commit sequence/receipt count zero, every imported live revision exactly
1 with no tombstones, and the original default receipt limit 4096. General validity
of a later writable profile is insufficient to accept a newly migrated candidate.
Failed/partial candidates are ineligible;
no existing source/backup/candidate is overwritten or automatically adopted.

Before-commit injected failure rolls back schema/version/revisions. A committed but
unobserved result is OutcomeUnknown and requires explicit independent candidate
verification (or same-operation receipt lookup for commands); never misreport it as
rollback. Live WAL backup includes committed and excludes uncommitted changes.
New destinations refuse existing WAL/SHM/journal; source/live candidate sidecars must
be regular, private and non-aliased, managed only by SQLite. Files/directories are
created 0600/0700 on Unix; Windows uses inherited temp ACLs, checks reparse points,
file identity and hardlink count, with no cross-account ACL claim. All paths remain
private to the workspace. Same-user deliberate tampering/TOCTOU is outside this
cooperative harness; a temp prefix is not a sandbox or proof of fixture origin.

Tests must distinguish independent-connection races, real child termination (if
used), injected precommit/aftercommit/lost-response/space failures, and historical
C2A/C2B1 evidence. Downgrade capability means the current **legacy** logical rows
remain representable in v1; M1 can independently compare a new synthetic v1
projection, but production downgrade/publication is M3. Never run old Python Store
against v2. Existing Python/Rust parity and fixture oracles remain unchanged.

No power-loss, physical disk-full, hardware flush, network filesystem, real owner
lock/ACL/cutover, parent-capability-loss or hostile same-user guarantees. Free
pages/WAL/backups can retain deleted bytes; tombstone minimization is not forensic
erasure. No raw data/error logs, credentials or private paths are published.
