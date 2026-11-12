# Architecture — implemented baseline and accepted Rust-only target

Documented implementation baseline: main `10bad176381668aa2dcbe516edde2c17dc6ff3bb` (pre-M2; M1 PR #39 and M0 follow-up merged).
P1-J's original product baseline remains recorded in its charter and ADR.
The [product charter](product.md) defines purpose; [ADR-0004](adr/0004-product-domain-trust.md)
defines target domain semantics; [ADR-0006](adr/0006-rust-only-runtime.md) selects the Rust-only runtime and retires the Python prototype. Target concepts below are not new APIs or tables.

## Accepted topology after P1-M2

```text
operator ─ NSH Desktop/TUI / native acp-query ─ operator IPC ─ Rust daemon
                                                          │
                                                acp-core + acp-write
                                                          │
                                                acp-store (read-only)
                                                          │
                                              explicit local Profile v2
trusted agent ─ two-tool MCP stdio ─ acp-mcp ─ project capability ─┘
                                                │
                              optional native ACP session supervisor
                              explicit Start / permission / Cancel / Stop
                              separate opted-in P1-I context lease
```

This is the accepted target after P1-M2, not a claim that the current main tree
already has production writes. The current Python P0 path and v0/v1 reader are
disposable validation-era implementation details scheduled for removal by [Issue #40](https://github.com/ybakiame/agent-control-plane/issues/40).

| Component | Current responsibility | Must not own |
| --- | --- | --- |
| Legacy Python P0 | Historical prototype currently present before M2; scheduled for removal under ADR-0006 | Any new product feature, compatibility path, or release dependency |
| `crates/acp-core` | Legacy confirmed preference/Handoff resolver plus P1-L pure typed selector/conflict explanation; bounded document/fingerprint semantics through adapters | SQLite connections, UI, process execution, protocol transport or authorization grants |
| `crates/acp-store` | Target: explicit-path, strict read-only adapter for one selected current Profile schema; M1 v2 is a replaceable baseline | Writes, schema migration, legacy-format compatibility |
| `crates/acp-rehearsal` | Synthetic migration/recovery experiments | Real-profile migration rollout |
| `crates/acp-ipc`, `acp-daemon` | Local capability/peer checks, scoped reads, live repository adapter, lease authority | TCP listener or caller-selected wider scope |
| `crates/acp-session` | Optional configured native ACP child lifecycle and bounded observations | OS sandbox, arbitrary shell execution or credential login |
| `crates/acp-mcp` | Read-only stdio bridge to project-bound IPC | Profile/task/session mutation |
| `crates/acp-write` | M1 synthetic v2 transaction writer; M2 makes it daemon-only for explicitly created v2 profiles | Public arbitrary-path opens, direct UI/CLI writes, generic SQL |\n| `crates/acp-query` | Native operator reads/session controls; M2 adds typed write requests through daemon IPC | Direct database access or a second writer |
| `spikes/p1-e/shared`, `spikes/p1-e/nsh` | Shared typed presentation and NSH Desktop/TUI surfaces | Direct database access, duplicate resolution logic or autonomous daemon startup |

The paths remain as implemented; accepting NSH does not move crates or remove
historical comparison workspaces. Windows→WSL owner queries remain read-only.

The current main tree still contains the disposable Python P0 prototype.
Its CLI and MCP launch path share an initializing read/write `Store`; this is a
historical implementation defect, not an accepted target. ADR-0006 retires that
runtime rather than building a second Python read-only adapter. P1-M2 replaces the
active v0/v1 Rust reader with strict Profile v2 reads and routes every product
mutation through the daemon-owned `acp-write` capability. Until M2 merges, this
target topology is not yet an implemented end-user runtime.

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

ADR-0005 selected the native daemon as the single ordinary write owner and
established operator-only versioned IPC, explicit grants, typed transactions,
CAS/receipts and strict preference transitions. [ADR-0006](adr/0006-rust-only-runtime.md)
now supersedes its Python compatibility, legacy schema support, migration handoff
and v2-to-v1 rollback decisions. The accepted target is a single Rust runtime with one current profile schema
(M1's v2 schema is a replaceable working baseline): explicit creation, strict reads,
daemon-owned writes, and no Python runtime or legacy migration. Breaking changes
are acceptable during validation; old local prototype files may be deleted. The [historical storage ownership plan](p1/storage-ownership-plan.md)
retains original M0/M1 evidence but is superseded as an execution plan.

[P1-M1 / Issue #38](https://github.com/ybakiame/agent-control-plane/issues/38)
completed in [PR #39](https://github.com/ybakiame/agent-control-plane/pull/39):
the isolated v2 transaction writer, exact schema, CAS/receipts, strict transitions
and historical-field preservation were validated against synthetic fixtures.
Its v0/v1 migration tests remain historical evidence for that revision; they do
not create a product migration commitment. [P1-M2 / Issue #40](https://github.com/ybakiame/agent-control-plane/issues/40)
is re-scoped by ADR-0006 to native v2 runtime integration and Python prototype
retirement. It remains validation-only: disposable old ACP prototype state may be deleted; no production write activation is authorized.

Initial technical revisions/receipts are not the full human ReviewDecision history,
which remains P1-O work.

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

Keep current authorization, scope, revocation and explicit-unknown semantics.
During validation, IPC/session/MCP wire schemas and profile schema may be replaced
together; do not add old-version compatibility or migration layers. Python writer
retirement is accepted by ADR-0006; no network listener, automatic repository
rewrite or broad execution permission is part of this target.
