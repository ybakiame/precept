# P1-M4/S0 — Operator human review and provenance contract

Status: **S0 contract freeze — not implementation.** No DDL, IPC method, CLI flag,
writer admission, error variant or UI control in this document is authorized by it.
Parent [Issue #52](https://github.com/ybakiame/agent-control-plane/issues/52),
this slice [Issue #53](https://github.com/ybakiame/agent-control-plane/issues/53).
Baseline: merged M2/S5 main `18f2670d294deebbc17f530c36bebecdaf8beb90` (PR #51);
[#40](https://github.com/ybakiame/agent-control-plane/issues/40) is closed at its
synthetic engineering-validation scope.
Authority: accepted [ADR-0004](../adr/0004-product-domain-trust.md), accepted
[ADR-0006](../adr/0006-rust-only-runtime.md), the non-superseded ownership parts of
[ADR-0005](../adr/0005-local-storage-and-write-ownership.md), the current
[Profile v1 contract](profile-v1-runtime-contract.md),
[typed-context v1](typed-context-v1.md), the merged
[S3 operator workflow contract](native-operator-workflows.md) and the
[product UI IA](../product-ui.md) (a target IA, not implemented screens).

Companion: [gap and four-surface acceptance matrix](operator-review-acceptance-matrix.md).

Marker legend used by both documents:

| Marker | Meaning |
| --- | --- |
| **implemented** | Exists in current source at the baseline; citable file:line |
| **contract-only** | Named by an accepted ADR or a frozen document, with no current consumer |
| **proposed** | S0 design intent needing owner acceptance before S1 |
| **out of scope** | Explicitly not delivered by M4 |

## 1. Purpose and non-goals

S1/S2 must be implementable by another agent or model without inventing tables,
permissions, provenance or UX. This document fixes the vocabulary, the visibility
rules, the decision matrix and the minimal API surface; the matrix fixes what counts
as evidence.

Product maturity remains `0.0.x-rc`. Only disposable, explicitly created, owned
synthetic Profile v1 files are in scope. No production profile data, external
development repository, model/provider/auth call, installation, release or hosted
CI activation. M3 lifecycle/recovery stays unopened: no backup, import, restore,
retention job or migration is introduced here.

M4 does not change these accepted boundaries: the daemon is the only production
write owner; `acp-store` stays read-only; MCP keeps exactly
`get_effective_context` and `get_handoff`
([`crates/acp-mcp/src/main.rs:20`](../../crates/acp-mcp/src/main.rs));
only confirmed preferences reach agent-facing effective context
(`resolve_preferences` filter at `crates/acp-core/src/lib.rs:215`); profile
creation/selection stays an explicit operator action.

## 2. Current versus target inventory

The evidence-backed row-by-row inventory, each marked **implemented**,
**contract-only**, **proposed** or **out of scope**, is in
[section 1 of the matrix](operator-review-acceptance-matrix.md#1-gap-inventory).
Summary of what today exists and what does not:

Implemented at the baseline: seven typed mutations
(`acp_core::operator::Command`, `crates/acp-core/src/operator.rs:88`), the three
permitted status transitions (`crates/acp-write/src/writer.rs:291`), per-row
optimistic revision plus `Expected::Absent|Revision` CAS (`writer.rs:38`), atomic
same-key peer demotion on confirm (`writer.rs:296`), one `MutationReceipt` per
committed command with canonical-request SHA-256 idempotency and same-generation
replay (`writer.rs:96-111`), operator-only bounded 12-row preference/Handoff
previews (`crates/acp-store/src/operator.rs:32`), separate ephemeral write
capability with per-request revocation checks
(`crates/acp-ipc/src/write_grant.rs:37`), the operator CLI surface and the shared
NSH operator panel with prepare/apply and same-request retry
(`spikes/p1-e/shared/src/operator.rs:148`, `spikes/p1-e/nsh/src/operator.rs:170`).

Missing and therefore designed below: a pending-review queue with honest staleness,
a proposal content digest bound to the exact reviewed bytes, any decision history
(who/what/when, and whether a `deprecated` row was an explicit deprecation or a
silent peer demotion), a closed provenance/origin label distinct from the free-text
`source`, deprecate/delete UX with a destructive tier, and any minimal Profile
control beyond create/select/info/grant.

## 3. Identifiers and typed states

Five distinct things must never be conflated:

| Concept | Marker | Current carrier | Identity |
| --- | --- | --- | --- |
| `PersonalPreference` | **implemented** | `preferences` row; `PreferenceRecord` (`crates/acp-core/src/operator.rs:136`) | `(profile binding, scope, project, id)` plus optimistic `revision` |
| `Suggestion` | **contract-only** | the same row while `status='suggested'` | identical to the preference row; **no second suggestion table** |
| `ReviewDecision` | **proposed** | none | append-only row per operator decision or automatic demotion |
| `Provenance` | partly **implemented** (free-text `source`), **proposed** closed `origin` | `preferences.source`, immutable after insert | describes the claim; labels alone verify nothing (ADR-0004) |
| `MutationReceipt` | **implemented** | `acp_receipts` row; `Receipt` (`crates/acp-core/src/operator.rs:30`) | transaction proof only, never review identity |

Immutable Handoff validation entries (`handoffs.validation_json`, statuses
`PASS|FAIL|SKIPPED|NOT_RUN`) stay historical claims. They are **not** review
decisions and **cannot** be upgraded by being copied into one.

### 3.1 The reviewed tuple

An operator decision is only meaningful against all five of these values, taken
from one verified read and rechecked inside the write transaction:

```text
(profile_id, generation, owner_epoch) + (scope, project) + id + revision + proposal_digest
```

`proposal_digest` is **proposed**: SHA-256 over the canonical typed bytes of
`{scope, project, key, value, origin, source}` at insert time, stored as a 32-byte
column and rendered as lowercase hex. It is a comparison fingerprint between what
the operator saw and what the writer will change. It is not a signature, not an
attestation and not the same thing as `acp_receipts.request_digest`, which hashes
the whole request and exists only for idempotency (`writer.rs:96`). Preference
`key`, `value` and `source` are already immutable after `SuggestPreference`; the
digest makes that immutability checkable by the reviewer instead of inferred.

Scope rules stay exact and never unioned: `Scope::Global` forbids a project and
`Scope::Project{name}` requires a registered, allowlisted, pinned canonical root
(`writer.rs:280-283`, `crates/acp-store/src/operator.rs:32`). A `project` row is
one project, never a family of checkouts with the same name.

### 3.2 Stable ordering and limits

Proposed queue ordering is ascending `preferences.id`: IDs come from
`acp_profile.next_preference_id` inside the write transaction and are never reused
after deletion (`crates/acp-write/src/profile.rs`, contract section 2.2). Paging
reuses the existing shape: at most 12 records per page with `next_after_id`
(`crates/acp-store/src/operator.rs:32-38`).

Paging across separate requests is **not** a stable snapshot and must not be
labelled as one. Each queue/inspect response carries the `acp_profile.commit_sequence`
watermark observed in the same read snapshot, so a later page can state "N commits
occurred since the previous page". Holding a SQLite read transaction open across
IPC requests is out of scope for M4.

### 3.3 Visibility per state

| State | Operator list | Pending queue | Agent effective context | Decision history |
| --- | --- | --- | --- | --- |
| `suggested` | yes | **yes** | **never** | yes |
| `confirmed` | yes | no | **yes** | yes |
| `rejected` | yes | no | never | yes |
| `deprecated` | yes | no | never | yes |
| deleted | no | no | never | removed in the same transaction (section 9) |

Terminal states stay terminal. The permitted transitions remain exactly the three
in the current matrix (`writer.rs:291`); M4 adds no `rejected → *`, no
`deprecated → *`, no re-confirm and no in-place edit. An edited proposal is a new
`suggested` row, as already stated for S3
([native-operator-workflows.md](native-operator-workflows.md) "Exact operator inspection").
`DeletePreference` remains outside the transition matrix and remains the only
destructive preference operation.

`unreviewed` and `accepted` from `typed_context::ReviewState`
(`crates/acp-core/src/typed_context/types.rs:64`) belong to repository-owned
ProjectContracts. **M4 never persists them as profile statuses**: a local
confirmation cannot make a contract `accepted` (section 5).

## 4. Decision matrix

Every row is one operator step against one interference condition. "Refuses" names
the sanitized `acp_ipc::Error` a client observes
(`crates/acp-ipc/src/lib.rs:23`); `ReviewMismatch` is **proposed** (owner decision
D3) and is never reported as success. No cell permits a silent retry, a partial
write or a widened permission.

| Operator step | Interference | Required outcome | Logical state after |
| --- | --- | --- | --- |
| select candidate | unknown/stale origin label | renders the label verbatim as unverified; no default approval | unchanged |
| inspect | record absent for scope | `NotFound` / `ScopeMismatch` | unchanged |
| inspect | row replaced by newer revision during paging | digest/revision from the response is authoritative; stale page is disclosed by watermark | unchanged |
| prepare review | any scope/project/target/queue switch after read | UI invalidates the pending operation; nothing is submitted | unchanged |
| confirm | content differs from reviewed bytes | `ReviewMismatch` | unchanged |
| confirm | revision advanced | `Conflict` | unchanged |
| confirm | status not `suggested` | `InvalidTransition` | unchanged |
| confirm | same-key confirmed peer exists | atomic peer demotion in one transaction, disclosed by inspect, reported in the receipt | unchanged or full success |
| confirm | daemon allowlist/root changed | `UnsafeRuntime` / `Conflict` before mutation | unchanged |
| reject | same conditions as confirm | `ReviewMismatch` / `Conflict` / `InvalidTransition` | unchanged |
| deprecate | status not `confirmed` | `InvalidTransition` | unchanged |
| delete | not explicitly armed on exact `id`+`revision` | `InvalidRequest` | unchanged |
| delete | success | row removed, decision rows for that ID removed, receipt retains identity-only proof | deleted |
| any write | grant revoked between read and submit | `Unauthorized` (per-request check at `crates/acp-daemon/src/profiles.rs:229`) | unchanged |
| any write | stale profile/generation/epoch | `OwnerMismatch` / `StaleGeneration` before replay | unchanged |
| any write | busy SQLite | `Busy` after the bounded wait; never an unbounded retry | unchanged |
| any write | unavailable/incompatible profile | `Unavailable`; never repair, never fallback read | unchanged |
| any write | uncertain commit | `OutcomeUnknown` plus the retained `operation_id` | committed or rolled back, undetermined by client |
| replay | same epoch, identical bytes and `operation_id` | original receipt returned verbatim; no second write, no timestamp rewrite | unchanged |
| replay | same `operation_id`, different bytes | `IdempotencyConflict` | unchanged |
| inspect history | after deletion | decision rows already removed; `acp_receipts` identity-only proof remains | unchanged |

Two invariants across the whole table: a receipt proves that a transaction
committed, nothing about who reviewed it or why; `actor` attribution stays local
and unverified, so it may never gate a permission. A digest proves byte equality
only, never authorship.

## 5. Authority and provenance

Authority ordering is unchanged and is the reason review cannot self-authorize:

1. Current repository facts, project-owned contracts and accepted ADRs
   (`AGENTS.md`, `docs/architecture.md`, ADR-0004/0005/0006).
2. Daemon-observed live reads for the selected project/scope.
3. Confirmed personal preferences.
4. Suggested/rejected/deprecated values: never effective, operator-inspectable only.
5. Handoff and stored historical validation: historical, never fresh evidence.

A local review decision **cannot approve, modify or supersede a repository
ProjectContract**. Confirming a personal preference that contradicts a contract
remains a personal choice that the resolver masks by authority
([typed-context v1](typed-context-v1.md) conflict table); M4 adds no path by which
a profile row changes a contract file, and no auto-scaffolding.

Origins stay distinct and are never upgraded by moving them between layers. The
closed set is `agent_reported`, `daemon_observed`, `independently_verified`,
`legacy_unknown` (`crates/acp-core/src/typed_context/types.rs:66`). Finding to
resolve, not to duplicate: that vocabulary **and** `Provenance`, `ReviewState`,
`SourceState` and the whole `typed_context` module currently have no consumer
outside `acp-core` itself — nothing in `acp-store`, `acp-write`, `acp-daemon`,
`acp-mcp`, `acp-query` or the UI imports it. M4 must either adopt that frozen
vocabulary or a separately justified one; a third parallel review vocabulary is
forbidden (owner decision D4).

`daemon_observed` means the daemon itself observed an event at a bound generation;
it does not mean the underlying claim is true. `independently_verified` requires a
separately identified checker, a recorded method and the exact source/platform,
which no profile column can currently carry; a suggestion may therefore assert it
only if the operator supplies that evidence out of band, and the label is rendered
as attributed, not verified.

Sensitive review history is Operator-only. It never appears in `context.get`,
`handoff.get`, the MCP tools or any agent-facing response. The MCP surface stays at
two read-only tools with `additionalProperties:false`
(`crates/acp-mcp/src/main.rs:20-25`); a proposal-only MCP extension needs its own
future ADR and cannot confirm anything. Do not attempt to seal
`Validation.extra`: it is `#[serde(flatten)]`-backed and structurally preserves
unknown keys (`crates/acp-core/src/lib.rs:82`), which is why opaque historical
validation is displayed as historical and never treated as provenance.

## 6. Proposed writer, reader and API shape

Nothing below is implemented in S0. Every DTO is versioned, bounded and
`deny_unknown_fields`, matching the existing operator DTO discipline
(`crates/acp-core/src/operator.rs`).

### 6.1 Reads (Operator role only)

| Method | Params | Result from one verified read-only snapshot |
| --- | --- | --- |
| `profile.review.queue` | `scope`, `after_id` | `binding`, `commit_sequence` watermark, at most 12 `ReviewCandidate`, `next_after_id` or null |
| `profile.review.inspect` | `scope`, `id`, `revision` | `binding`, `commit_sequence`, exact `ReviewRecord`, at most 8 most recent `ReviewDecisionRecord`, projected peer-demotion list |

`ReviewCandidate`/`ReviewRecord`: `id`, `scope`, `key`, `value`, `source`,
`status`, `origin`, `revision`, `proposal_digest` (lowercase hex), `created_at`,
`updated_at`. `ReviewDecisionRecord`: `preference_id`, `action`
(`confirm|reject|deprecate|demote|delete`), `from_revision`, `to_revision`,
`proposal_digest`, `receipt_commit_sequence`, `channel` (`cli|desktop|tui`),
`decided_at`. No token, absolute private path, raw SQL or diagnostic is ever placed
in these DTOs, and content-bearing request/receipt `Debug` stays redacted
(`crates/acp-write/src/command.rs:16`).

These reads are denied to agent, session and write-only credentials and remain
unreachable through MCP. They extend, and do not replace, `profile.preferences.*`.

### 6.2 Writes

No new mutation kind is needed except a bounded origin and digest binding:

- `SuggestPreference { scope, key, value, source, origin, expected: Absent }` —
  `origin` is required and closed-set; nothing infers it.
- `ConfirmPreference`/`RejectPreference`/`DeprecatePreference`/`DeletePreference`
  `{ id, scope, expected: Revision(n), proposal_digest, channel }` — the digest
  must equal the stored value or the transaction refuses before any row is touched;
  `channel` is a client-asserted label with no authorization meaning.
- `RegisterProject`, `PutHandoff` are unchanged in M4.

One command remains one transaction and one receipt. Decision rows are written
inside that same transaction, so a refusal leaves queue, statuses, revisions,
counters and receipts byte-identical. The existing redaction rule holds:
`result_json` keeps identity-only `RecordVersion` entries
(`crates/acp-core/src/operator.rs:12`); the new digest lives in the preference and
decision rows, never in a receipt.

### 6.3 Versioning and idempotency

`API_VERSION` stays 2: framing, `Hello` and method dispatch are unchanged
(`crates/acp-ipc/src/lib.rs:17`). The write envelope breaks in place — bump
`Request.version` 1 → 2 and refuse 1 (`crates/acp-write/src/command.rs:36` today
requires exactly 1), with one coordinated commit updating daemon, writer, CLI, NSH
and fixtures. No v1 mutation reader, no dual-version support and no compatibility
shim. Digest/idempotency semantics are unchanged: replay requires the identical
canonical bytes, and a changed digest under the same `operation_id` is
`IdempotencyConflict`.

## 7. Fresh v1 DDL replacement candidate

The current v1 allowlist is exactly five tables and one index
(`crates/acp-store/src/profile.rs:12`, restated in
[Profile v1 contract](profile-v1-runtime-contract.md) section 2.1), and `inspect()`
refuses any other object. The approved model cannot be carried by that DDL, so the
S1 candidate is a **replacement of fresh v1**, not a migration:

```sql
-- changes to the existing table only (columns appended, all NOT NULL, no defaults)
CREATE TABLE preferences (
    ... unchanged columns ...,
    origin TEXT NOT NULL
      CHECK (origin IN ('agent_reported','daemon_observed','independently_verified','legacy_unknown')),
    proposal_digest BLOB NOT NULL CHECK (length(proposal_digest) = 32)
);
CREATE TABLE acp_review_decisions (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    scope TEXT NOT NULL CHECK (scope IN ('global', 'project')),
    project TEXT REFERENCES projects(name) ON DELETE RESTRICT,
    preference_id INTEGER NOT NULL CHECK (preference_id > 0),
    action TEXT NOT NULL
      CHECK (action IN ('confirm','reject','deprecate','demote','delete')),
    from_revision INTEGER NOT NULL CHECK (from_revision > 0),
    to_revision INTEGER NOT NULL CHECK (to_revision > 0),
    proposal_digest BLOB NOT NULL CHECK (length(proposal_digest) = 32),
    receipt_commit_sequence INTEGER NOT NULL CHECK (receipt_commit_sequence > 0),
    channel TEXT NOT NULL CHECK (channel IN ('cli','desktop','tui')),
    decided_at TEXT NOT NULL,
    CHECK ((scope = 'global' AND project IS NULL) OR (scope = 'project' AND project IS NOT NULL))
);
CREATE INDEX review_lookup
    ON acp_review_decisions(scope, project, preference_id, id);
-- acp_profile gains: next_review_id INTEGER NOT NULL CHECK (next_review_id > 0)
```

Decisions deliberately store **no** `key`, `value`, `source` or Handoff text. Their
growth is bounded by the existing per-generation 4096-commit ceiling, because every
decision accompanies exactly one committed command; `decision.id` comes from a new
`next_review_id` counter so no `AUTOINCREMENT`/`sqlite_sequence` dependency is
introduced, matching the v1 allocation style. Link to `acp_receipts` is by
`commit_sequence` value only — no new foreign key, because the receipt table's own
primary key relationship is already the transaction record and review rows must
survive nothing that receipts do not.

Consequences that S1 must carry in one breaking commit: object allowlist becomes six
tables and two indexes, `inspect()`'s expected-object set and the schema-drift
fixtures change, `profile-v1-runtime-contract.md` section 2.1 DDL block is replaced,
and every synthetic fixture is regenerated. `PRAGMA user_version` stays `1`: this is
a replacement of the same disposable v1, **not** a schema version bump, and it needs
explicit owner acceptance before any code (owner decision D1).

Reset policy: delete only the explicitly named disposable profile files inside the
run-owned synthetic root, regenerate fixtures, and never run `ALTER TABLE`, a
migration, a downgrade, a repair, an adoption of an unknown file or a broad
filesystem cleanup. Do not touch real ACP data, authentication or runtime
directories.

## 8. NSH Desktop/TUI review UX

One operator panel in the existing shared `Model`
(`spikes/p1-e/shared/src/operator.rs`), rendered by both Desktop and TUI, with the
current tokens, English/zh-CN copy, `compact()` behaviour below 900 px
(`spikes/p1-e/nsh/src/tokens.rs:38`), visible keyboard focus and the existing
staleness guards (read `epoch` in `spikes/p1-e/shared/src/lib.rs:148`, operator
`serial`/`Guard` in `spikes/p1-e/shared/src/operator.rs:46`). No new screen
framework, no second renderer and no TUI-only business rule.

Proposed tabs, navigated with the already-supported `alt+left`/`alt+right` chords:
Queue, Detail, History. Per-tab copy obligations are in
[section 3 of the matrix](operator-review-acceptance-matrix.md#3-presentation-copy-obligations).

| Control | Marker | Behaviour |
| --- | --- | --- |
| queue page + `commit_sequence` watermark | proposed | stale paging is stated, never hidden |
| detail: exact `id`, scope, project, key, value, source, origin, revision, digest | proposed | digest shown as 16-hex prefix, full value only on explicit expand |
| peer-demotion disclosure | proposed | "Deprecates N same-key confirmed records (ids …)" read from inspect; authoritative set is computed at commit |
| confirm / reject | **implemented** today as `alt+shift+y` / `alt+shift+n` prepare, `alt+shift+v` apply | must additionally bind the digest |
| deprecate | proposed `alt+shift+w` | armed only on exact `id`+`revision`+digest; same prepare/apply gate |
| delete | proposed `alt+shift+q` | distinct destructive tier: names the ID, states that content and decision history are removed, requires the exact ID retyped into the focused input, then `alt+shift+v` |
| history view | proposed | decision rows for the inspected ID; live reads and saved Handoff stay visually separate |
| disabled/read-only states | **implemented** | `available()` plus `Field::disabled` (`spikes/p1-e/nsh/src/operator.rs:22`, `:136`); unavailable actions are labelled, never rendered as working buttons |

Shortcut constraints are real, not stylistic: the native driver accepts only
`alt+shift+[umerxlsvynzih]`, `alt+[opkwft1-6abc]`, `alt+left/right`,
`ctrl+[alrpq]`, `escape`, `enter`, `tab`
([`tools/native_window.ps1:99`](../../tools/native_window.ps1)), `alt+1..alt+5` and
`alt+7` are taken, the operator panel is `alt+o`, and `ctrl+1` is the user's own
screenshot key and stays unavailable. `w` and `q` above are free in that chord set;
any other chord requires a separately authorized driver change.

Still CLI-only until separately authorized: project registration and Handoff put.
The S3 gate is unchanged ([native-operator-workflows.md](native-operator-workflows.md)),
and the UI keeps labelling it instead of offering nonfunctional controls
(`OperatorCliGate` copy at `spikes/p1-e/shared/src/i18n.rs:62`). S0 asserts no
complete review flow exists in the UI.

## 9. Privacy, retention and deletion

Decision rows carry identifiers, revisions, digests and timestamps only. On
`DeletePreference` the writer removes that preference's decision rows in the same
transaction, so a deleted personal value leaves no digest oracle behind: an
SHA-256 of a short low-entropy value such as an editor name is confirmable by
guessing, which is acceptable while the row exists under operator review and is not
acceptable as post-deletion residue.

What deletion does **not** promise: `acp_receipts` rows remain (identity-only
`RecordVersion` proof, no key/value/source text) and no receipt pruning is
authorized in M4; the SQLite file itself is not securely erased, and
[ADR-0004](../adr/0004-product-domain-trust.md) already refuses to treat SQLite
files as forensic secure deletion. Complete removal of a profile's content stays a
deliberate operator file deletion or development reset, never an automatic action.
Nothing in review history is ever written to a tracked repository file, a log or a
test fixture, and no raw diagnostic is echoed by CLI or UI.

## 10. Owner decisions required before S1

| # | Unresolved choice | Options | Recommended default | Blocks |
| --- | --- | --- | --- | --- |
| D1 | How to carry the model in storage | replace fresh v1 DDL (section 7) / keep current v1 and defer review persistence | **replace fresh v1, `user_version` stays 1, no migration** | S1 |
| D2 | Decision history | append-only `acp_review_decisions` / status columns only | **append-only table** (peer-demotion cause and digest binding are otherwise unrecoverable) | S1 |
| D3 | Digest-bound decisions | require `proposal_digest` + new `ReviewMismatch` error / rely on `Conflict` | **require digest and add `ReviewMismatch`** (a same-revision digest mismatch is a substituted-record signal, not a CAS race) | S1 |
| D4 | Origin vocabulary | reuse `typed_context::Origin` / define profile-local enum / delete the unused module | **reuse `typed_context::Origin` names; leave deletion of the unused module as a separate owner decision** | S1 |
| D5 | Actor attribution | closed `channel` label only / free-text actor | **`channel` only**; no forgeable actor name | S1 |
| D6 | Peer-demotion consent | disclose at inspect only / bind expected peer IDs | **disclose only** in S1; peer binding needs its own UX cost analysis | S2 copy |
| D7 | Decision history after deletion | remove rows / retain without digest | **remove in the same transaction** | S1 |
| D8 | Which high-risk operations gain UI | deprecate+delete in S2 / all four including register and Handoff put | **deprecate+delete in S2**; register and Handoff put stay CLI-only | S2 |
| D9 | 4096 receipts per generation with review decisions | keep ceiling and prove `Capacity` refusal / raise it | **keep**; raising it is an owner-only policy change | S3 wording |
| D10 | Queue consistency across pages | watermark disclosure / held read snapshot | **watermark disclosure** | S1 |

Roadmap status is updated separately in [docs/roadmap.md](../roadmap.md); this
document deliberately does not rewrite any prior evidence report. No ADR is amended
here. Unresolved ADR interpretation is above for the owner rather than silently
decided: D1/D2/D7 touch ADR-0004's tombstone and privacy sentence, and D4 touches
whether a contract-only module becomes real vocabulary or stays dead code.
