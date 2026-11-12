# P1-M2 — Rust-native Profile v1 and runtime contract

- Status: contract-first draft for owner review; no implementation is authorized by this document alone.
- Authority: accepted [ADR-0006](../adr/0006-rust-only-runtime.md), [ADR-0004](../adr/0004-product-domain-trust.md), and the non-superseded parts of [ADR-0005](../adr/0005-local-storage-and-write-ownership.md).
- Implementation issue: [#40](https://github.com/ybakiame/agent-control-plane/issues/40).
- First implementation slice: [#43 — Profile v1 storage foundation](https://github.com/ybakiame/agent-control-plane/issues/43). Its schema review gate remains pending; this draft is not evidence of approval.
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

The following DDL is the proposed normative v1 contract. The implementation test must compare the frozen DDL to the implementation constants and independently inspect the opened database. Any later DDL change requires a schema decision; do not silently broaden the allowlist.

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
    name TEXT PRIMARY KEY CHECK (length(name) BETWEEN 1 AND 64),
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
    operation_id TEXT PRIMARY KEY
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
| Exact v1 DDL, including the 4096-receipt validation-stage ceiling and no automatic pruning | Required before Issue #43 changes storage implementation DDL; still proposed, not approved by this draft |
| Explicit fresh creation, no-replace publication, file identity and private permission/ACL primitives on Windows and Linux | Required before Issue #43 implements the corresponding filesystem boundary; inherited temporary ACLs from M1 are not sufficient evidence |
| Control-only daemon bootstrap state for IPC profile create/select | Later daemon/IPC slice, outside Issue #43. If unsafe or disproportionate, propose a narrower explicit bootstrap before changing the client/daemon contract |
| Profile-scoped OS writer-lock primitives and dependencies | Later ownership slice, outside Issue #43; no lease or epoch-advancement implementation is authorized by the storage slice |

The current workspace already uses `windows-sys` 0.61 for current-user security
and filesystem APIs and `libc` on Unix. Reusing those existing dependency families
is a review proposal, not a new dependency or proof that the profile-file ACL and
publication requirements are implemented. No platform primitive is accepted solely
because the historical fixture helper or IPC transport used it.

- Keep any newly discovered product fields or workflows out of v1 until their source, owner and acceptance need are demonstrated.
