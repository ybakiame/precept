# Architecture — implemented baseline and product target

Current baseline: main `2a5949120aacd0523cd7fa5e417548e1e648bc81` (P1-L, PR #35 merged).
P1-J's original product baseline remains recorded in its charter and ADR.
The [product charter](product.md) defines purpose; [ADR-0004](adr/0004-product-domain-trust.md)
defines target domain semantics. Target concepts below are not new APIs or tables.

## Implemented topology and ownership

```text
operator ─ Python acpctl ─ store/context ─ user-local SQLite (profile/Handoff writes)
                          │                    ▲
                          └─ P0 MCP stdio       │ read-only acp-store
                                               │
NSH Desktop / TUI ─ shared typed model ─ local IPC ─ Rust daemon ─ acp-core
native acp-query ─ operator capability ────────┘       │             │
                                                     │       bounded live Git/docs
trusted agent ─ two-tool MCP stdio ─ acp-mcp ─ project capability ──┘
                                                     │
                               optional native ACP session supervisor
                               explicit Start / permission / Cancel / Stop
                               separate opted-in P1-I context lease
```

| Component | Current responsibility | Must not own |
| --- | --- | --- |
| `src/agent_control_plane` | P0 schema, operator CLI writes, reference resolver/MCP | Automatic preference approval or repository rewriting |
| `crates/acp-core` | Legacy confirmed preference/Handoff resolver plus P1-L pure typed selector/conflict explanation; bounded document/fingerprint semantics through adapters | SQLite connections, UI, process execution, protocol transport or authorization grants |
| `crates/acp-store` | Explicit-path read-only SQLite adapter and known-schema validation | Production migration or profile writes |
| `crates/acp-rehearsal` | Synthetic migration/recovery experiments | Real-profile migration rollout |
| `crates/acp-ipc`, `acp-daemon` | Local capability/peer checks, scoped reads, live repository adapter, lease authority | TCP listener or caller-selected wider scope |
| `crates/acp-session` | Optional configured native ACP child lifecycle and bounded observations | OS sandbox, arbitrary shell execution or credential login |
| `crates/acp-mcp` | Read-only stdio bridge to project-bound IPC | Profile/task/session mutation |
| `crates/acp-query` | Explicit native operator reads/session controls | Replacing Python's persistent writer today |
| `spikes/p1-e/shared`, `spikes/p1-e/nsh` | Shared typed presentation and NSH Desktop/TUI surfaces | Direct database access, duplicate resolution logic or autonomous daemon startup |

The paths remain as implemented; accepting NSH does not move crates or remove
historical comparison workspaces. Windows→WSL owner queries remain read-only.

Python CLI read commands and the Python MCP launch path currently construct
`Store`, which opens read-write, enables WAL and initializes/extends schema. Their
read-only tool surface is not a guarantee of write-free initialization. Rust
`SqliteStore` remains an explicit-path read-only adapter. No M0 documentation
change fixes these entrypoints or transfers Python's write ownership.

## Target domain and data ownership

Rust domain logic will express PersonalPreference, ProjectContract,
SessionConstraintSnapshot, EffectiveContext, Suggestion/ReviewDecision,
Provenance, EvidenceRecord and Handoff as distinct concepts. The daemon orchestrates
adapters and permissions; Desktop/TUI/CLI render or request decisions through
versioned interfaces. A single generic rule rank must not turn session scope into
authority over a repository contract.

| Data | Authority / source of truth | Local state and change path |
| --- | --- | --- |
| Personal preferences, including project-scoped personal choices | Human-reviewed user-local SQLite | Explicit operator action; never tracked profile files |
| Project contracts | Repository-owned AGENTS/docs/accepted ADRs and reviewed engineering configuration | Live bounded reads; optional structured Markdown/YAML contracts are a future versioned format, not a new required directory |
| Project registration | User-local SQLite plus explicit daemon launch allowlist | Registration is not permission to discover or share every project |
| Suggestions and reviews | User-local SQLite proposal/review records (generalized model planned) | Approval binds exact content/version/scope; project proposals still need repository review |
| Effective context | Derived view of current sources and review state | Optional cache must carry source versions and invalidation; never outranks source |
| Session constraints and Handoff | Historical local snapshots (constraint snapshots planned) | Frozen references/content versions, not live policy or execution authority |
| Session capability/lease | Daemon memory and owner-only runtime files | Ephemeral, rotated/revoked; no SQLite token persistence |
| Provenance/evidence | Local minimal records with reporter/observer/verifier attribution (unified model planned) | No raw transcript or credential store; exported data requires explicit selection |

Repository Markdown/YAML is an **adopted ownership pattern**, with exact path,
parser/schema and export mechanics deferred to an implementation ADR/task. Existing
allowlisted documents stay valid. Onboarding only registers and previews; it must
not scaffold, migrate, rewrite AGENTS.md or activate examples automatically.
SQLite may index contract metadata/source digests; it must not become a hidden
authoritative copy. Dirty source versions need a content digest as well as HEAD.

No schema change occurred in J or L. Rust write ownership remains gated on versioned
migration, consistent backup, recovery and reader compatibility as documented in
[SQLite boundaries](p1/sqlite-readonly.md). The Rust reader rejects unknown versions;
current Python Store has no version fence. Future structured contract formats and
DB schemas have independent versions, with future readers/writers required to
reject unsupported versions without silent conversion. Records lacking provenance
or snapshots remain readable as historical/unknown, never retroactively verified
or confirmed.

P1-M0 / [Issue #36](https://github.com/ybakiame/agent-control-plane/issues/36) is the
active design gate. [Proposed ADR-0005](adr/0005-local-storage-and-write-ownership.md)
selects an explicitly opted-in native daemon writer, separate writer library,
genuine Python read-only compatibility, offline exclusive maintenance and verified
database generations with a private SQLite control catalog. Profile v2 and catalog
versions, operator write IPC, fencing, cutover and rollback are proposals only.
The [ownership/compatibility and recovery plan](p1/storage-ownership-plan.md) retains
completed C2A/C2B1 synthetic evidence and sequences later implementation and
separately authorized per-profile rollout. ADR-0005 remains Proposed; ADR-0004 is
unchanged. No real profile may be opened or adopted by this design task.

## Context resolution

Today the resolver binds one registered project, rechecks its registration, reads
live Git/allowlisted metadata and returns only confirmed preferences. A matching
project preference masks global; within the same scope/key the newest confirmed
ID wins. Suggested/rejected/deprecated records are excluded. Document bodies are
explicit opt-in. Handoff lookup is exact project/task and compares current
HEAD/branch/worktree/fingerprint with historical state. Current source labels and
validation JSON are not a complete structured provenance/conflict engine.

Merged P1-L additionally implements an isolated, pure Rust typed resolver for
selectors, source versions, masked rules, conflicts and unknown applicability using
[ADR-0004](adr/0004-product-domain-trust.md) and its
[v1 contract](p1/typed-context-v1.md). Callers supply already-bound values; this
additive API has not replaced legacy MCP/IPC responses or persisted domain state.
Future adapters must supply fresh authorized sources before presenting its output.
Authorization/security gates apply first. Current repository facts and accepted
contracts outrank stored preferences or Handoff; facts describe what exists while
contracts describe what is required. Code/ADR disagreement is surfaced for human
resolution, not silently resolved in favor of either. Session additions may narrow
work, but never override contracts or grant permissions. Stale snapshots cannot
freeze obsolete permissions or hide new repository rules.

## Trust and compatibility

MCP retains exactly `get_effective_context` and `get_handoff`. Local IPC separates
operator capability from project-bound agent capability; OS user identity alone
does not grant operator role. Unix sockets/Windows named pipes stay local, without
a browser gateway. Optional ACP Start, context sharing and individual permission
decisions remain separate. P1-I checks session leases on every request and before
response publication; Stop/Cancel/expiry/failure/restart revoke established peers.
Data already delivered cannot be recalled. [Lease details](p1/session-context-bridge.md)
remain authoritative; ordinary format-1 credentials do not gain session revocation.

Agent-reported actions, daemon observations and independent verification are
separate records/claims. Historical PASS is neither fresh validation nor proof of
rule compliance. Checksums detect specified content differences, not reporter
identity or event truth; there is no implemented signature/evidence-chain system.
Same-user filesystem access bypasses cooperative protocol boundaries. See
[security](security.md), [interfaces](interfaces.md) and [ADR-0002](adr/0002-local-ipc-capabilities.md).

Preserve current IPC/MCP schemas, fingerprint bytes, Python writer and explicit
unknown states until separately reviewed changes. No network listener, new runtime,
automatic repository rewrite or broad execution permission is part of this target.
