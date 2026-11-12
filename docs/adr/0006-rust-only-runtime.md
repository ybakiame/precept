# ADR-0006: Rust-only runtime and Python prototype retirement

- Status: **Accepted** — owner approved 2026-11-12; implementation is sequenced by the revised P1-M2 task
- Date: 2026-11-12
- Authority: [P1-M2 / Issue #40](https://github.com/ybakiame/agent-control-plane/issues/40)
- Supersedes: the Python compatibility frontend, Python read-only adapter, Python rollback owner, legacy-schema runtime compatibility/migration, and schema downgrade decisions in [ADR-0005](0005-local-storage-and-write-ownership.md)
- Governing decisions retained: [ADR-0004](0004-product-domain-trust.md), [ADR-0002](0002-local-ipc-capabilities.md), and the non-superseded parts of [ADR-0005](0005-local-storage-and-write-ownership.md)

## Context

Python was used to establish the P0 product prototype quickly with the standard
library. It supplied the first SQLite profile, operator CLI, read-only MCP surface,
and local Handoff workflow. The project has since established Rust domain, storage,
writer, daemon, IPC, MCP, native CLI, and NSH Desktop/TUI components. M1 delivered
the isolated Rust v2 transaction writer and its synthetic acceptance evidence.

The product is still in validation and has no production-integrated profile
population. The early Python prototype was used only briefly and has produced no
material local data that needs to be retained. The owner explicitly accepts
breaking changes and deletion of local prototype files during this validation
phase. Maintaining Python as a frontend, read-only adapter, rollback owner, or
second test implementation would preserve a prototype boundary rather than a
product requirement. It would also keep two runtime languages, two storage paths,
and cross-language parity work in the critical path.

The owner therefore chooses to end the Python prototype and complete the product
on the Rust architecture. This is an intentional validation-phase breaking change. Do not design for
hypothetical compatibility or preserve disposable prototype data. When production
integration or a concrete compatibility requirement exists, the owner will state
that requirement explicitly and it will be handled as a new scoped decision.

## Decision

### 1. Rust is the only supported product runtime

The supported product runtime after the P1-M2 cutover is entirely Rust:

- `acp-core`: domain behavior and typed context resolution.
- `acp-store`: strict read-only access to the supported profile schema.
- `acp-write`: private transactional write implementation, reachable only through
  the authorized daemon owner.
- `acp-daemon` and `acp-ipc`: local service, capability checks, and versioned
  operator commands.
- `acp-query`: native operator CLI.
- `acp-mcp`: project-bound, read-only MCP stdio bridge.
- NSH Desktop/TUI: native operator interface over the typed local interfaces.

The Python package, Python `acpctl`, Python `Store`, Python MCP server, Python
profile/Handoff read or write paths, Python-only test/runtime dependencies, and
Python product CI gates are retired in P1-M2. No new feature, compatibility fix,
or release path may be added to the Python prototype while retirement is in
progress.

P1-M2 must inventory tracked Python files, packaging/dependency metadata, CI,
scripts, tests, and documentation. Remove product runtime/test/build paths that
exist only to support the Python implementation. A Python file may remain only
for a separately justified, non-product maintenance purpose documented in the
PR; it must not be imported, packaged, required by CI, or reachable at runtime.
Historical reports and Git history remain historical records, not supported
interfaces.

### 2. Rust Profile schema v1 is the first product schema

The first Rust-native product Profile schema is v1 (`PRAGMA user_version=1`). It is
a fresh schema definition, not a compatibility continuation of either the retired
Python prototype or M1's synthetic schema. The legacy Python prototype also used a
v1 label; the matching integer does **not** imply matching tables, constraints, or
semantics. Old Python profiles are disposable and must not be opened or adopted.

M1's Profile v2 (`PRAGMA user_version=2`) is historical engineering evidence only.
Its exact DDL, schema fingerprint, metadata tables, and migration contract are not
the product schema and need not be carried forward. M2 may reuse relevant domain
and transaction ideas, but must define the smallest Rust-native v1 schema that
supports current product requirements. Do not build readers or converters for
M1 v2 or the legacy Python schema:

- New profiles are explicitly created in the fresh Rust-native v1 format.
- Runtime readers and writers validate the complete Rust-native v1 schema and
  fail closed on malformed or unsupported profiles.
- There is no Python compatibility reader, legacy runtime reader, automatic
  migration, in-place upgrade, cross-version projection, or downgrade owner.
- Existing Python, legacy-v1, and M1-v2 prototype files are disposable validation
  artifacts. They may be deleted as part of a deliberate local development reset;
  no backup, import, migration, rollback, or preservation guarantee is required.
- Do not build an automatic legacy-file discovery/cleanup subsystem. A reset or
  replacement operation must target the known ACP development profile/path, not
  unrelated user files. A future production-data import requirement must be
  explicitly raised by the owner and designed as new scoped work; it is not part
  of P1-M2.

M1's legacy-v0/v1-to-v2 synthetic migration work and its evidence remain valid
historical engineering evidence for that merged revision only. It does not define
the new Rust-native Profile v1 or create a product migration requirement. P1-M2 must remove legacy migration
from production-reachable crates and either remove or isolate any remaining legacy
fixture/rehearsal code so it cannot be shipped or invoked as a product migration.
The M1 report must not be rewritten to imply its historical tests did not run.

Profile creation is an explicit operator action. Startup must not scan for or
adopt old databases. During validation, an explicit development reset may delete
the known ACP prototype profile and create a fresh current-schema profile; no
compatibility conversion is needed. The selected destination and resulting profile
identity must still be explicit and validated. Do not silently overwrite unrelated
files or add broad filesystem cleanup logic.

### 3. The native daemon is the single write owner

Retain the core ownership decision from ADR-0005: one native daemon owns ordinary
writes for one explicitly selected profile/generation. The daemon is the only
production component that may link and invoke `acp-write`. `acp-store` remains a
read-only interface; no UI, CLI, MCP bridge, agent process, or generic library
receives a writable SQLite connection.

Operator writes use a typed, operator-only local IPC interface. IPC, session and
MCP interfaces may be revised together during validation; no backward-compatible
old/new API support or compatibility shim is required. Keep the write capability
profile-scoped and distinct from operator read/session credentials and all agent
credentials. Operator role alone is not write authority.

Authorization and profile/generation binding are checked before opening a
write-capable profile and again at transaction admission. Revocation is serialized
with admission. A transaction admitted before revocation may commit and its result
must be reported accurately. Daemon unavailability, stale ownership, revoked
grants, lock collision, or unsupported profile fails closed; there is no fallback
to a Python process or a second direct writer.

The initial typed operator command set remains narrow:

- `RegisterProject`
- `SuggestPreference`
- `ConfirmPreference`
- `RejectPreference`
- `DeprecatePreference`
- `DeletePreference`
- `PutHandoff`

No generic SQL, arbitrary path, caller-supplied SQLite connection, transaction
callback, agent-facing mutation, MCP mutation, automatic preference approval, or
unreviewed content/scope edit is introduced. Retain CAS, idempotency receipts,
bounded transactions, strict transitions, and atomic peer deprecation where they
remain necessary for the fresh Rust-native v1 design. Re-evaluate M1-specific
metadata tables and sequence/fingerprint machinery against concrete product needs;
M1's exact v2 DDL and migration-preservation rules are not binding.

### 4. Keep domain and trust boundaries; simplify migration ownership

ADR-0004 remains unchanged. Repository contracts outrank personal preferences and
Handoffs; suggestions require explicit human review; technical receipts/revisions
are not human ReviewDecision or independent evidence; agents and MCP cannot confirm
or mutate profile state.

There is no Python rollback owner or cross-schema downgrade path. Recovery, when
implemented, restores a verified generation of the selected current schema only, with explicit
operator selection and disclosure of any lost commit interval. It must never
silently discard newer state or treat a historical backup as current authority.

ADR-0005's proposed multi-generation cutover catalog and migration-stage machinery
must not be implemented merely to support a Python handoff or cross-schema projection.
P1-M2 implements only the minimal per-profile identity, exclusive writer lease,
epoch fencing, and process-death behavior needed for one Rust writer. Backup,
restore, and any additional durable catalog/manifest design are re-evaluated as
Rust-to-Rust current-schema lifecycle work in P1-M3, with a separate contract and synthetic
acceptance. Reuse a catalog only where that new lifecycle design demonstrates a
concrete need.

### 5. Compatibility policy

Validation-phase compatibility policy:

- No backward-compatibility promise is made for Python commands/imports, local
  profile files, database schemas, IPC versions, session APIs, MCP schemas, or
  client configuration. Interfaces may be changed together when the Rust product
  architecture requires it.
- Do not maintain old/new protocol versions in parallel, compatibility adapters,
  aliases, wrappers, migration shims, downgrade paths, or parity suites merely to
  preserve behavior from the prototype or an earlier validation commit.
- Keep the current functional and security invariants: operator-only writes,
  profile-scoped authorization, daemon-exclusive ownership, read-only agent/MCP
  access, strict domain transitions, CAS/idempotency, and sanitized failures.
  These are product correctness requirements, not backward-compatibility promises.
- When the owner later identifies a production integration or concrete compatibility
  requirement, define the exact consumer/data/version scope in a new decision;
  do not pre-build for hypothetical consumers.

## Implementation sequence

| Phase | Scope | Gate |
| --- | --- | --- |
| P1-M2 — Rust-only runtime cutover / Issue #40 | Fresh Rust-native Profile v1 create/read/write; operator-only daemon write IPC; `acp-query` and NSH integration; single writer lease; remove Python runtime, tests, dependencies, and M1/legacy migration paths | Native Windows and Linux/WSL synthetic acceptance; exact v1 schema and core authorization/domain boundaries hold; no Python runtime or migration path remains |
| P1-M3 — Profile v1 lifecycle (conditional) | Only concrete v1 lifecycle/recovery features demonstrated necessary after M2; do not pre-build backup, restore, or retention systems | Scoped Rust-only acceptance if opened; no legacy migration or schema downgrade |
| P1-M4 — product review and profile workflows | Complete operator review/provenance workflow and explicit profile lifecycle UX | ADR-0004 human-review semantics, permission boundaries, and end-to-end acceptance |
| P1-M5 — release hardening | Packaging, clean-machine installation, accessibility, platform support, security and performance | Explicit release criteria and verified platform evidence |

M2 implementation and tests use fresh disposable fixtures of the new Rust-native Profile v1 schema. Local Python prototype files and disposable validation profiles may be
deleted during a deliberate reset; no real user data is expected or protected by a
compatibility process. This does not authorize touching unrelated files, credentials,
or external checkouts, nor does it authorize production write activation or release.
The owner will explicitly identify any future production-data or compatibility need.

## Consequences

Positive:

- One implementation language and one product runtime.
- One storage authority and one write path.
- No Python fallback, rollback owner, adapter, or cross-language parity burden.
- A clean first product schema: Rust-native Profile v1, with no legacy readers or converters.
- Native CLI, UI, and MCP use the same Rust domain and permission boundaries.

Costs and accepted limitations:

- Existing Python commands and old prototype databases are disposable and have
  no retention or migration guarantee during validation.
- Production-data compatibility is deferred until the owner explicitly identifies
  an actual integration and its requirements.
- Writes are unavailable when the daemon or write authorization is unavailable.
- M2 must finish Profile v1 creation, native v1 reads, native write IPC, and removal
  of prototype dependencies before the product can claim a complete Rust runtime.
- Backup/restore and release readiness remain separate work; Rust-only does not
  itself establish production readiness, platform certification, or a sandbox
  against a hostile same-user process.

## Alternatives rejected

- **Keep Python as a thin frontend:** rejected because it preserves a second
  runtime, packaging surface, tests, and compatibility obligations without a
  deployed consumer.
- **Build Python read-only plus rollback support:** rejected because it continues
  the dual-runtime lifecycle and makes old implementation behavior a product
  constraint.
- **Preserve the old Python v1 or M1 synthetic v2 schema through compatibility readers:**
  rejected because neither contains production product data. The new Rust-native v1
  is a fresh schema; any future production import requirement must be separately
  raised and scoped by the owner.
- **Give each UI/CLI its own Rust writer:** rejected; the daemon remains the single
  write owner under ADR-0005.
