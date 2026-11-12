# P1-M2 — Rust-native Profile v1 and runtime contract

- Status: owner-confirmed Stage 1 storage contract, 2026-11-12; implementation authorized under Issue #43. PR remains Draft for implementation review; no merge or production activation is implied.
- Authority: accepted [ADR-0006](../adr/0006-rust-only-runtime.md), [ADR-0004](../adr/0004-product-domain-trust.md), and the non-superseded parts of [ADR-0005](../adr/0005-local-storage-and-write-ownership.md).
- Implementation issue: [#40](https://github.com/ybakiame/agent-control-plane/issues/40).
- First implementation slice: [#43 — Profile v1 storage foundation](https://github.com/ybakiame/agent-control-plane/issues/43). Owner confirmed the Phase 1 recommendations and authorized continuing on this branch after this contract freeze, without automatically merging the PR.
- Baseline: PR #41 merged into main at `d8d48fa582f60f1dd0e00b22276e2fbb29ab5e53`.
- Product maturity remains `0.0.x-rc`. Profile schema v1 is a data-format version, not the ACP product release version.

## 1. Purpose and non-goals

P1-M2 establishes the first Rust-native product Profile. It is a clean schema definition with `PRAGMA user_version=1`; it is not a conversion of the old Python v1 or M1's synthetic v2.

The old Python profile, M1 synthetic-v2 profile, migration fixtures and rehearsal paths are disposable engineering artifacts. They are not valid v1 inputs. The product must not inspect, adopt, import, migrate, repair, downgrade or preserve them. A deliberate development reset may delete only the explicitly selected ACP development profile; it must never perform broad filesystem cleanup.

Keep the following product boundaries:

- Rust is the only product runtime after this phase.
- The daemon is the sole runtime write owner. Only the daemon links and invokes the private `acp-write` crate.
- `acp-store` is a strict read-only adapter. UI, CLI, MCP and agents never open SQLite read-write.
- Profile creation and selection are explicit operator actions. No default profile path, auto-discovery, implicit adoption or automatic reset.
- Agent and MCP access remains project-bound and read-only. MCP keeps exactly the two existing read tools.
- A write grant is distinct from operator read/session authority and agent capabilities. No grant is active by default.
- No production write activation, release readiness, hostile same-user isolation, OS sandbox, cloud sync, telemetry or automatic preference learning is claimed.

## 2. Profile v1 schema

### 2.1 Version and exact object allowlist

A valid product Profile v1 has exactly these user objects:

1. Tables: `acp_profile`, `projects`, `preferences`, `handoffs`, `acp_receipts`.
2. Index: `prefs_lookup`.
3. No user-defined views, triggers, virtual tables, extra indexes, extra tables or unknown objects.
4. `PRAGMA user_version=1`.
5. Foreign-key enforcement is enabled on every writable connection. The read-only adapter also enables it and sets `query_only=ON` and `trusted_schema=OFF`.

The following DDL is the owner-confirmed normative v1 contract. The implementation test must compare the frozen DDL to the implementation constants and independently inspect the opened database. Any later DDL change requires a schema decision; do not silently broaden the allowlist.

```sql
CREATE TABLE acp_profile (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    profile_id TEXT NOT NULL
      CHECK (length(profile_id) = 32 AND profile_id NOT GLOB '*[^0-9a-f]*'),
    generation TEXT NOT NULL
      CHECK (length(generation) = 32 AND generation NOT GLOB '*[^0-9a-f]*'),
    owner_epoch INTEGER NOT NULL CHECK (owner_epoch >= 0),
    next_preference_id INTEGER NOT NULL CHECK (next_preference_id > 0),
    commit_sequence INTEGER NOT NULL CHECK (commit_sequence >= 0)
);

CREATE TABLE projects (
    name TEXT NOT NULL PRIMARY KEY CHECK (length(name) BETWEEN 1 AND 64),
    root TEXT NOT NULL UNIQUE,
    revision INTEGER NOT NULL CHECK (revision > 0),
    created_at TEXT NOT NULL
);

CREATE TABLE preferences (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    scope TEXT NOT NULL CHECK (scope IN ('global', 'project')),
    project TEXT REFERENCES projects(name) ON DELETE RESTRICT,
    pref_key TEXT NOT NULL CHECK (length(pref_key) BETWEEN 1 AND 120),
    value TEXT NOT NULL CHECK (length(value) BETWEEN 1 AND 4096),
    status TEXT NOT NULL DEFAULT 'suggested'
      CHECK (status IN ('suggested', 'confirmed', 'rejected', 'deprecated')),
    source TEXT NOT NULL CHECK (length(source) BETWEEN 1 AND 512),
    revision INTEGER NOT NULL CHECK (revision > 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (
      (scope = 'global' AND project IS NULL) OR
      (scope = 'project' AND project IS NOT NULL)
    )
);

CREATE INDEX prefs_lookup
    ON preferences(scope, project, status, pref_key);

CREATE TABLE handoffs (
    project TEXT NOT NULL REFERENCES projects(name) ON DELETE RESTRICT,
    task_id TEXT NOT NULL CHECK (length(task_id) BETWEEN 1 AND 120),
    objective TEXT NOT NULL CHECK (length(objective) BETWEEN 1 AND 4000),
    base_sha TEXT NOT NULL,
    branch TEXT NOT NULL,
    worktree_state TEXT NOT NULL,
    worktree_fingerprint TEXT,
    completed TEXT NOT NULL CHECK (length(completed) <= 8000),
    next_action TEXT NOT NULL CHECK (length(next_action) BETWEEN 1 AND 4000),
    validation_json TEXT NOT NULL CHECK (length(validation_json) <= 16000),
    revision INTEGER NOT NULL CHECK (revision > 0),
    updated_at TEXT NOT NULL,
    PRIMARY KEY (project, task_id)
);

CREATE TABLE acp_receipts (
    operation_id TEXT NOT NULL PRIMARY KEY
      CHECK (length(operation_id) BETWEEN 1 AND 64),
    generation TEXT NOT NULL
      CHECK (length(generation) = 32 AND generation NOT GLOB '*[^0-9a-f]*'),
    request_digest BLOB NOT NULL CHECK (length(request_digest) = 32),
    commit_sequence INTEGER NOT NULL UNIQUE CHECK (commit_sequence > 0),
    result_json TEXT NOT NULL
      CHECK (length(result_json) BETWEEN 1 AND 65536)
);

PRAGMA user_version = 1;
```

Application validation remains mandatory in addition to SQLite constraints:

- Project names are 1–64 ASCII letters/digits/underscore/hyphen/dot, excluding `.` and `..`. Roots are explicit, absolute, canonical directories and are checked against the daemon's independent project allowlist.
- Preference key/value/source are nonblank, contain no NUL, and respect the bounds above. Scope/project consistency and project existence are checked before mutation and again in the transaction.
- Handoff task/objective/completed/next-action bounds match the existing accepted domain contract. Validation is a JSON array of objects with string `check` and status `PASS`, `FAIL`, `SKIPPED` or `NOT_RUN`; unknown nested evidence fields remain opaque data. NUL is rejected.
- Timestamps are generated by Rust as UTC RFC 3339 values for actual mutations. A replay returns the original receipt and does not rewrite timestamps.
- IDs and counters use checked arithmetic. Overflow is a sanitized `Capacity` failure.

Stage 1 freezes ordinary SQLite tables, not STRICT tables. Every stored field is
checked for its actual SQLite storage class as well as domain bounds; positive
REAL counters, NULL identities, coercible BLOB/text and malformed timestamps are
invalid. UTC timestamps use `YYYY-MM-DDTHH:MM:SSZ` generated from Rust SystemTime.
The entire schema/record/receipt verification runs in one pinned read snapshot;
writer admission performs the same checks inside its immediate transaction.

### 2.2 Why v1 does not carry M1's metadata schema forward

- Revisions live on the mutable product rows (`projects.revision`, `preferences.revision`, `handoffs.revision`). There is no separate `acp_revisions` table.
- Preference IDs are allocated from `acp_profile.next_preference_id` inside the same write transaction. There is no `AUTOINCREMENT`, `sqlite_sequence` dependency or legacy allocation-history admission check. Deleting a preference never decrements or reuses the counter.
- `acp_receipts` remains because idempotent retry after a lost response is a required write semantic. It stores a canonical request digest and a sanitized result only; it does not store preference values, Handoff bodies, paths or credentials.
- `acp_profile` holds only the profile identity, generation, writer epoch and monotonic counters required by runtime ownership and transactions. M1's schema fingerprint and migration-only metadata are not carried forward.
- Initial v1 receipt capacity is 4096 committed operations per profile generation. Existing receipts are checked before capacity; no automatic pruning occurs in M2. Once full, new mutations fail closed with `Capacity`. This is an explicit validation-stage limit to revisit before release, not a claim that indefinite receipt retention is the final lifecycle policy.

### 2.3 Profile identity and validation

- `profile_id` is 16 cryptographically random bytes encoded as 32 lowercase hexadecimal characters and remains stable for the profile's lifetime.
- `generation` is independently random at creation. A fresh profile or future explicitly designed restore gets a new generation; no in-place schema migration changes it in M2.
- `owner_epoch` starts at zero. It increments with checked arithmetic only after the daemon successfully obtains the profile's exclusive writer lease. A failed lease acquisition must not change the database.
- `next_preference_id` starts at one; `commit_sequence` starts at zero. The initial profile row is the only row in `acp_profile`.
- Open-time verification checks exact `user_version`, exact object/DDL allowlist, columns/types/nullability/primary keys, foreign keys, integrity, the singleton metadata row, identity encoding, counter bounds, all domain rows and receipt consistency.
- Any mismatch returns a sanitized unsupported/incompatible/corrupt-profile error. Never attempt repair, DDL mutation, fallback reads or version inference.
- The old Python schema also used user_version 1. It is rejected by exact DDL/object validation. M1 synthetic v2 is rejected by version validation. No converter or compatibility fixture is shipped in a product binary.

Receipt consistency is structural bookkeeping, not tamper evidence. Sequences
are exactly 1 through `commit_sequence`, receipt count equals that sequence and
is at most 4096. Each sorted nonempty affected list has unique typed identities,
positive revisions and at most 1024 entries. Across receipts each record starts
at revision 1 and advances by one. A deletion advances once, is preference-only,
and is terminal. The latest receipt version matches each live row; a deleted
preference has no content row. Historical receipts remain valid after deletion.
No separate revisions/tombstone/history table is introduced. The allocation
counter exceeds every preference ID in both live rows and retained receipts;
deletion never reduces it. Missing history, gaps, resurrection, orphaned rows,
bad bindings and inconsistent receipts are refused, never repaired. Fresh creation
has no domain rows or receipts and next ID one. The 4096 ceiling counts every
successful command, including deletes/Handoff updates; it is not a 10k-write
capacity claim and must not trigger automatic pruning or generation reset.

## 3. Profile creation and selection

Profile management is an operator IPC operation, not a database API in `acp-query`.

- `acp-daemon` supports an explicit unselected/control-only state when no profile is supplied at startup. In this state only health and profile create/select management requests are available; project reads, sessions, MCP context and writes are unavailable.
- `acp-query profile create --path <absolute-path>` sends a typed `profile.create` request. The daemon creates a new v1 profile through its private `acp-write` capability and selects it only after independent reopen/verification succeeds.
- `acp-query profile select --path <absolute-path>` sends a typed `profile.select` request. The daemon opens an existing exact v1 profile read-only and selects it only after validation. It never creates a missing file.
- A daemon started with an explicit `--database <path>` is already explicitly selected at startup. It must validate the existing v1 profile and must not create it implicitly.
- One daemon process serves one selected profile. Switching profiles requires an orderly daemon restart; no hot-switch may leave readers, sessions, capabilities or transactions bound to the previous profile.
- Profile creation requires a non-existing destination and a deliberate operator request. It must not overwrite an existing file, follow a symlink/reparse point, accept a hard-linked database, or touch a different profile. Use an exclusive temporary file in the selected parent and publish without replacement only after schema initialization and verification.
- The selected path is canonicalized and bound to the opened file identity. Private storage and lock files use current-user-only permissions/ACLs. Unix database/lock files are mode 0600 in a mode-0700 directory; Windows uses an explicit current-user-only DACL and rejects reparse points. Do not claim protection from a hostile process running as the same user.
- A selected profile does not implicitly authorize writes. Selection opens the read-only store only. A write grant is a separate explicit action.

Stage 1 creates only beneath an existing private selected parent, never chmods
unrelated directories, and rejects destination DB/sidecar/control-file collisions.
On Windows, reserved basenames (`daemon.lock`, `daemon.sock`, `operator.cap`,
`operator-write.cap`), the `.acp-create-` staging prefix and the SQLite `-wal`,
`-shm`, `-journal` suffixes are all compared ASCII case-insensitively. Existing
mixed-case aliases must be refused without modifying their contents. Valid
destination spelling is preserved; Unix comparisons retain their case semantics.
It initializes with DELETE journaling and FULL synchronization, independently
reopens the temporary DB, closes all SQLite handles, and publishes on the same
filesystem without replacing a target that appeared concurrently. Linux uses
`renameat2(RENAME_NOREPLACE)`; Windows uses `MoveFileExW` without replacement or
cross-volume copying. Unsupported primitives/permissions fail closed. Windows
creates an explicit protected current-user-only DACL and checks owner/ACL/reparse
and handle identity; Unix checks current UID, 0700 parent, 0600 file, inode/device
and link count. Inherited M1 temp ACLs are not sufficient. Native local Windows
and WSL Linux filesystems are validated separately; cross-OS shared-profile and
network-filesystem support are not inferred. Before publication only owned temp
artifacts may be removed. After publication, verification/I/O failure is
`OutcomeUnknown`: preserve the published file and never delete/recreate it as a
retry. Concurrent creators have at most one winner. File/directory flushes are
best-effort durability mechanisms, not hardware/power-loss certification.

### 3.1 Later daemon selection: path identity acceptance gate

The path/file binding above applies to **both** explicit existing-profile selection
and startup `--database`, not just fresh creation. The later daemon slice must
validate an explicit absolute existing regular file, canonical parent/path and
all path components; refuse symlinks/reparse points, database hardlinks, unsafe
owner/permissions/ACLs and unknown file identity. Use the same native identity
requirements as creation (Unix device/inode, Windows volume/file identity), and
bind the opened database to that identity plus its verified profile ID/generation.
Recheck that the selected path still names that opened identity before exposing
the selection; replacement or ambiguity fails closed. A copied database with the
same metadata is not proof of the same filesystem identity. Later writer lease
admission must independently revalidate identity as required by section 4.1.

The later slice's native Windows and Linux tests must include a safe unchanged
selection, missing file without creation, symlink/reparse/hardlink and permission
refusals, and replacement by an otherwise valid copy with identical profile
metadata. No failed selection may repair/adopt the file or create a write grant.
This is an explicit restatement of the existing selection boundary, not a new
schema, catalog, lock abstraction or Stage 1 daemon implementation.
`SqliteStore::open` is a strict read-only schema/content reader; its absolute-path
check alone does **not** implement these daemon selection filesystem checks.

## 4. Single-writer lease and write grant

### 4.1 Profile-scoped OS lease

- The daemon may have multiple read-only readers, but at most one active writer owner for a canonical profile identity.
- The daemon acquires a stable profile-scoped lock file with an OS-backed exclusive file lock (the existing Rust `File::try_lock` capability is available at the workspace toolchain level). Process termination releases the lock; the lock file itself may remain.
- Canonical path aliases must resolve to the same lock identity. Refuse symlink/reparse ambiguity, database hardlinks, lock-file symlinks/hardlinks, unknown file identity and unsupported locking behavior. Do not infer ownership from PID or timestamps and never force-unlock.
- After obtaining the OS lock, the daemon reopens and revalidates the selected database identity, then increments `owner_epoch` in a short transaction. It issues no write grant if any validation or epoch update fails.
- The lock remains held while the grant is active and is released only after revocation has stopped new admissions and all already-admitted transactions have drained.

### 4.2 Separate, opt-in write capability

- The ordinary `operator.cap` remains read/control authority only. It cannot invoke mutation methods.
- A separate `operator-write.cap` credential carries an explicit `OperatorWrite` role plus `profile_id`, `generation` and `owner_epoch`. It is created only after an explicit operator command such as `acp-query profile write-enable --confirm`, successful lease acquisition and epoch increment.
- No write grant is created on daemon startup or profile selection. No implicit grant is inherited from operator read, session, agent or MCP credentials.
- Every mutation request validates the credential binding, active in-memory grant, profile identity, generation and epoch. It then validates the typed DTO and current CAS revision before mutation.
- Revocation is requested with the ordinary operator control capability. It closes the admission gate, rejects all later writes, waits for transactions admitted before revocation to finish, removes only the still-matching write credential, then releases the OS lease. A transaction admitted before revocation may commit; its result must be reported accurately.
- A daemon crash releases the OS lock. The write credential is ephemeral and must be removed/rejected on the next daemon start. A subsequent explicit grant increments the epoch, making stale credentials unusable.
- The writer opens SQLite read-write only inside the daemon's private storage boundary, after grant/lease checks. No caller-supplied connection, arbitrary SQL, SQL callback, arbitrary path writer or second writer is exposed.

## 5. Transaction and domain semantics

All accepted mutations execute in one bounded SQLite `BEGIN IMMEDIATE` transaction. Recheck profile identity/generation/epoch, active grant, target record, scope and expected revision inside the transaction. No external filesystem or network work occurs while the transaction is held.

For Stage 1 this is a private storage contract: initial epoch zero is a valid
binding in synthetic storage tests, not a write grant. Runtime grant/lease checks
and epoch advancement remain the later daemon slice. No production caller is
wired by #43. Project canonicalization/allowlist checks and Handoff capture occur
in the trusted caller before the transaction; storage rechecks their typed fields,
project registration, scope and binding without filesystem/Git/network operations.
M1's fixture-root enum and fabricated snapshot values are retired. Handoff capture
fields are bounded input from the future trusted owner, never agent evidence:
base SHA is empty or 40/64 lowercase hex, branch is at most 1024 Unicode scalars,
state is `clean`, `dirty` or `unknown`, fingerprint is NULL or 64 lowercase hex.
Unavailable HEAD/branch are empty strings, unavailable state is `unknown`, and
unavailable fingerprint is NULL. Unknown never becomes clean or verified.

| Command | Contract |
| --- | --- |
| `RegisterProject` | Explicit allowlisted name and canonical root; insert once; duplicate name/root conflicts. |
| `SuggestPreference` | Allocate the next monotonic preference ID; insert as `suggested`, revision 1. Never auto-confirm. |
| `ConfirmPreference` | Only `suggested → confirmed`; atomically deprecate currently confirmed peers with the same key and exact scope/project, incrementing their revisions. |
| `RejectPreference` | Only `suggested → rejected`. |
| `DeprecatePreference` | Only `confirmed → deprecated`. A fresh `suggested → deprecated` is `InvalidTransition`. |
| `DeletePreference` | Explicitly delete one existing preference at the expected revision. Receipt records a minimal deleted record-version; preference content is not copied into a tombstone. Its ID is never reused. |
| `PutHandoff` | Create or replace the exact project/task Handoff at `Absent` or the expected current revision. No cross-project fallback. |

Transition table:

| Current state | Confirm | Reject | Deprecate |
| --- | --- | --- | --- |
| suggested | confirmed | rejected | InvalidTransition |
| confirmed | InvalidTransition | InvalidTransition | deprecated |
| rejected | InvalidTransition | InvalidTransition | InvalidTransition |
| deprecated | InvalidTransition | InvalidTransition | InvalidTransition |

CAS and receipt behavior:

- New records require `Expected::Absent`; existing records require `Expected::Revision(n)` with positive n.
- A command carries a random operation ID (1–64 ASCII letters/digits/underscore/hyphen), profile binding and typed command. Canonical typed request bytes are SHA-256 hashed.
- Same operation ID + same generation + same digest returns the original committed receipt without mutation. Same operation ID with different canonical bytes returns `IdempotencyConflict`. Stale profile/generation/epoch is rejected before replay.
- Each successful mutation increments `commit_sequence` once and stores one receipt in the same transaction. All affected record revisions and the receipt are committed atomically.
- Receipt results contain generation, operation ID, commit sequence and sorted affected record identities/revisions/deleted flags only. No values, keys, source text, roots, Handoff content, SQL or private paths.
- Receipt capacity is checked before any mutation. Denial, malformed input, CAS conflict, capacity failure and pre-commit error leave all logical rows, revisions, counters and receipts unchanged.
- Busy wait is bounded (initial target 100 ms); return sanitized `Busy` rather than retrying indefinitely. An uncertain commit/response is `OutcomeUnknown`; the caller retries the same operation ID, never a speculative new one.
- No automatic retry, receipt pruning, hidden mutation or timestamp rewrite.

Request DTO format is 1, with unknown fields denied. Decode and typed serialization
are each bounded to 128 KiB UTF-8 bytes. Canonical hashing uses serde's fixed typed
field order over the complete request (version, binding, operation ID, command);
outer JSON key order/whitespace do not matter. `validation_json` is retained as a
raw string, so its byte/whitespace changes do change the digest. Domain text bounds
in the DDL are Unicode scalar counts; all text rejects NUL. Receipt JSON is at most
65536 UTF-8 bytes (in addition to the DDL character limit). Replay checks binding
first, then digest, before CAS/capacity and never writes timestamps. Receipt and
request Debug/errors are redacted. Same-generation replay after an epoch change
still requires an identical valid request; no cross-owner replay conversion exists.

## 6. IPC, CLI, NSH and MCP

- IPC is revised as one breaking contract (target `API_VERSION=2`); update daemon, CLI, NSH and tests together. Do not retain v1 shims or dual-version support.
- Add typed profile management and write-grant methods: `profile.create`, `profile.select`, `profile.info`, `profile.write.enable`, `profile.write.disable`.
- Add the seven typed operator commands listed above. Each method has a strict DTO with unknown fields denied, explicit bounds and sanitized errors. No generic mutation or SQL method.
- `acp-query` gains profile create/select/info/grant/revoke and the seven operator commands. It connects through IPC and never links `acp-write`, opens SQLite or stores preference/Handoff payloads in a config file.
- NSH exposes the minimum profile, preference and Handoff workflows through the same typed IPC client. UI rendering code has no database or writer dependency. A write action is unavailable unless the separate write capability is active.
- MCP remains exactly `get_effective_context` and `get_handoff`, project-bound and read-only. Agent credentials never deserialize or carry `OperatorWrite`.
- Session and MCP interfaces may be changed in lockstep only where needed for the new IPC/profile binding. Preserve project scope, expiry, revocation and read-only semantics; do not add agent mutation or execution permission.

## 7. Python and legacy retirement

Inventory tracked Python files, `pyproject.toml`, package metadata, dependencies, tests, CI jobs, build scripts, docs and examples. Remove the product Python package, CLI, Store, MCP server, runtime/test dependencies and Python-only CI gates. Port only still-required domain/security regressions to Rust. Delete Python/Rust parity and migration-only tests.

Remove production-reachable legacy schema inspection, migration, backup-copy, migration catalog and rehearsal APIs. Delete the obsolete `acp-rehearsal` migration crate or reduce it to non-product tests that cannot be linked or invoked by a product binary. Historical M1 docs/reports remain immutable evidence and must be labeled historical, not rewritten as product contracts.

A Python file may remain only for a separately justified non-product maintenance purpose documented in the implementation PR; it must not be imported, packaged, required by CI or reachable at runtime.

## 8. Implementation order

Issue #43 covers storage only: steps 2–3 below, after step 1's normative schema
review gate. It includes explicit fresh creation, strict read-only access, the
seven transactional commands and removal of production-reachable M1 migration
paths. Daemon bootstrap/IPC, OS ownership leases, epoch advancement, write grants,
CLI/NSH wiring and complete Python retirement are later slices of Issue #40.
They are not prerequisites to implement the isolated storage slice and are not
authorized by that slice. Production write activation remains out of scope.

1. Freeze this schema/ownership contract and the acceptance plan before code changes.
2. Implement Profile v1 DDL, explicit create, strict open/read and schema refusal tests.
3. Refactor `acp-write` from synthetic-workspace-only code into a private daemon-owned v1 writer. Remove migration/rehearsal code from product paths.
4. Implement profile selection, profile-scoped lease, owner epoch and distinct write grant/revocation.
5. Add typed IPC v2 methods and daemon admission/transaction wiring.
6. Add CLI profile lifecycle and operator commands; then connect minimum NSH workflows.
7. Remove Python product runtime/tests/dependencies/CI and stale active docs.
8. Run native Windows and Linux/WSL acceptance on exact clean SHAs. Submit a Draft PR linked to Issue #40. Do not merge or enable production writes/release without separate owner approval.

## 9. Explicit open review points

These points must be resolved before the corresponding implementation, not guessed in code:

| Review decision | Implementation gate |
| --- | --- |
| Exact v1 DDL, including the 4096-receipt validation-stage ceiling and no automatic pruning | Confirmed by owner for Issue #43; exact DDL is frozen by the contract-first commit |
| Explicit fresh creation, no-replace publication, file identity and private permission/ACL primitives on Windows and Linux | Confirmed Stage 1 policy and primitives are specified in section 3; native acceptance remains required |
| Control-only daemon bootstrap state for IPC profile create/select | Later daemon/IPC slice, outside Issue #43. If unsafe or disproportionate, propose a narrower explicit bootstrap before changing the client/daemon contract |
| Profile-scoped OS writer-lock primitives and dependencies | Later ownership slice, outside Issue #43; no lease or epoch-advancement implementation is authorized by the storage slice |

The current workspace already uses `windows-sys` 0.61 for current-user security
and filesystem APIs and `libc` on Unix. Reusing those existing dependency families
is the confirmed Stage 1 implementation choice; package versions are unchanged.
Their presence alone is not proof that profile-file ACL and publication checks
passed. No platform primitive is accepted solely
because the historical fixture helper or IPC transport used it.

- Keep any newly discovered product fields or workflows out of v1 until their source, owner and acceptance need are demonstrated.
