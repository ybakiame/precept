# Architecture — implemented baseline and accepted Rust-only target

Implementation baseline: merged S3 main `58750f346bf2288d79f9ff29754ca110ebca52d0`, plus S4 retirement changes.
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
                                              explicit local Rust-native Profile v1
trusted agent ─ two-tool MCP stdio ─ acp-mcp ─ project capability ─┘
                                                │
                              optional native ACP session supervisor
                              explicit Start / permission / Cancel / Stop
                              separate opted-in P1-I context lease
```

This is the implemented validation topology, not production activation or final
M2 integrated acceptance.

| Component | Implemented responsibility | Must not own |
| --- | --- | --- |
| `crates/acp-core` | Confirmed preference/Handoff resolver, bounded repository contract adapter and P1-L pure typed selector/conflict resolution; document/fingerprint adapters | SQLite connections, UI, process execution, protocol transport or authorization grants |
| `crates/acp-store` | Explicit-path, strict read-only adapter for the fresh Rust-native Profile v1 | Writes, schema migration, legacy-format compatibility |
| `crates/acp-ipc`, `acp-daemon` | Local capability/peer checks, scoped reads, live repository adapter, lease authority | TCP listener or caller-selected wider scope |
| `crates/acp-session` | Optional configured native ACP child lifecycle and bounded observations | OS sandbox, arbitrary shell execution or credential login |
| `crates/acp-mcp` | Read-only stdio bridge to project-bound IPC | Profile/task/session mutation |
| `crates/acp-write` | S1 fresh-v1 storage; merged S2 adds guarded existing-v1 attachment through daemon-owned lease/epoch and explicit grant | Arbitrary existing-path Writer, direct UI/CLI writes, generic SQL |
| `crates/acp-query` | Native operator reads/session controls and S3 typed write requests through authenticated daemon IPC | Direct database access or a second writer |
| `apps/core`, `apps/client`, `apps/ui` | Pure operator state/presentation (`core`); native IPC and the `acp-wsl-bridge` stdio helper for the Windows→WSL2 target (`client`); NSH Desktop/TUI surfaces (`ui`) | Direct database access, duplicate resolution logic, autonomous daemon startup or a second authority in the bridge |

Product crate paths remain as implemented; the operator app moved from `spikes/p1-e`
to `apps/operator` and then to the `apps/` workspace (`core`/`client`/`ui`), and the historical comparison workspace is
archived under `docs/archive/gpui-kit-spike`, not removed.

**Windows → WSL2 (P1-M5/S1-B, primary workflow).** The Windows-native HWND GUI is
the normal control surface for an ACP daemon that runs inside an explicitly selected
WSL2 user distribution. WSL owns Linux project paths, the daemon, Profile/runtime
state and any managed agent processes. The GUI starts one long-lived
`acp-wsl-bridge` helper per connection with `wsl.exe --distribution D --user U
--exec /abs/acp-wsl-bridge --runtime-dir R --endpoint E` (no shell, TCP, port
forwarding or mirrored networking). The helper loads `operator.cap` and
`operator-write.cap` inside WSL and calls the same `acp-ipc` Operator, OperatorWrite
and session paths as a local client, so the daemon stays the only authority; tokens
never cross the stdio boundary. The protocol is in [interfaces](interfaces.md#windows-gui--wsl2-bridge).
The **secondary** WSLg + Weston Wayland Linux GUI/TUI path ([S1-C #72](https://github.com/ybakiame/agent-control-plane/issues/72))
connects to the same WSL daemon directly over Unix IPC and does not use the bridge.

The [merged S2 implementation and native evidence](evaluation/p1-m2-s2-ownership.md) add
single-assignment daemon selection, native file binding, a distinct write role,
serialized grant admission/drain and typed IPC v2. Only the daemon normally links
the writer. [S3](p1/native-operator-workflows.md) adds typed native CLI workflows,
operator-only exact record previews and a shared NSH operator panel. Clients share
data-only DTOs through `acp-core::operator`; validation and transactions stay in
the daemon-owned writer. S4 removes the legacy package/tests/probes and replaces active CI; these
synthetic results do not activate a real Profile or complete the cutover.

The active tree has one Rust product runtime. Current operator writes traverse
the daemon; the retired prototype is available only in Git history and historical
reports. No compatibility frontend, migration/import or direct-write fallback is
retained. See [S4 inventory](p1/python-retirement.md) and [native development](p1/rust-only-development.md).

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
| Project contracts | Repository-owned AGENTS/docs/accepted ADRs and reviewed engineering configuration | Live bounded reads; optional exact Markdown source under the [accepted P1-N boundary](p1/project-contract-integration.md); no required directory or automatic prose interpretation |
| Project registration | User-local SQLite plus the daemon-owned project set (`--state-dir/projects.json`), changed only by the operator-write `project.add` / `project.remove` after a daemon-side scope check (Git root in the daemon user's home, no control or system path) | Registration is not permission to discover or share every project; the daemon checks only the one path the operator chose |
| Suggestions and reviews | User-local SQLite proposal/review records (generalized model planned) | Approval binds exact content/version/scope; project proposals still need repository review |
| Effective context | Derived view of current sources and review state | Optional cache must carry source versions and invalidation; never outranks source |
| Session constraints and Handoff | Historical local snapshots (constraint snapshots planned) | Frozen references/content versions, not live policy or execution authority |
| Session capability/lease | Daemon memory and owner-only runtime files | Ephemeral, rotated/revoked; no SQLite token persistence |
| Provenance/evidence | Local minimal records with reporter/observer/verifier attribution (unified model planned) | No raw transcript or credential store; exported data requires explicit selection |

Repository Markdown/YAML is an **adopted ownership pattern**. P1-N freezes one
optional `ACP.contracts.md` source with versioned JSON metadata in Markdown;
YAML parsing, export and onboarding are not included. Existing
allowlisted documents stay valid. Onboarding only registers and previews; it must
not scaffold, migrate, rewrite AGENTS.md or activate examples automatically.
SQLite may index contract metadata/source digests; it must not become a hidden
authoritative copy. Dirty source versions need a content digest as well as HEAD.

ADR-0005 selected the native daemon as the single ordinary write owner and
established operator-only versioned IPC, explicit grants, typed transactions,
CAS/receipts and strict preference transitions. [ADR-0006](adr/0006-rust-only-runtime.md)
now supersedes its Python compatibility, legacy schema support, migration handoff
and cross-schema downgrade decisions. The accepted target is a single Rust runtime with one current profile schema
(fresh Rust-native Profile v1): explicit creation, strict reads,
daemon-owned writes, and no Python runtime or legacy migration. Breaking changes
are acceptable during validation; old local prototype files may be deleted. The [historical storage ownership plan](p1/storage-ownership-plan.md)
retains original M0/M1 evidence but is superseded as an execution plan.

[P1-M1 / Issue #38](https://github.com/ybakiame/agent-control-plane/issues/38)
completed in [PR #39](https://github.com/ybakiame/agent-control-plane/pull/39):
the isolated synthetic v2 transaction writer, exact schema, CAS/receipts and strict
transitions were validated against disposable fixtures. This is historical evidence,
not the product data schema. [P1-M2 / Issue #40](https://github.com/ybakiame/agent-control-plane/issues/40)
is re-scoped by ADR-0006 to a fresh Rust-native Profile v1 runtime and Python
prototype retirement. The new v1 is not compatible with either legacy Python v1
or M1 synthetic v2; no conversion is required. It remains validation-only: disposable old ACP prototype state may be deleted; no production write activation is authorized.

Initial technical revisions/receipts are not the full human ReviewDecision history,
which remains P1-O work.

Issue #43 implements only the [frozen Profile v1 storage contract](p1/profile-v1-runtime-contract.md):
fresh creation, strict read-only access, seven private commands and current-schema
tests. `acp-rehearsal` and M1 migration APIs are removed from the workspace; their
evidence remains historical. No daemon write entrypoint, ownership lease, grant,
epoch advancement, IPC/UI change or Python retirement is added by Stage 1.
The [local evidence](evaluation/p1-m2-s1-storage.md) distinguishes this slice from
completion of the full Rust-only runtime.

## Context resolution

The resolver binds one registered project, rechecks its registration, reads
live Git/allowlisted metadata and returns only compatible confirmed preferences. A matching
project preference masks global; within the same scope/key the newest confirmed
ID wins. Suggested/rejected/deprecated records are excluded. Document bodies are
explicit opt-in. Handoff lookup is exact project/task and compares current
HEAD/branch/worktree/fingerprint with historical state. Current source labels and
validation JSON are not a complete structured provenance/conflict engine.

P1-N integrates the [approved repository source](p1/project-contract-integration.md)
through the production daemon context route and two-tool MCP bridge. The source is
bounded and reread before returning context; malformed/unavailable/changed input
refuses all effective preference/contract output. Accepted claims feed typed v1
for affected keys; pending or definitely nonmatching claims preserve exact legacy
fallback, including valid Profile text outside typed-v1 bounds. Optional selector
labels never grant authorization. The agent receives only safe effective output
and sanitized status, not the private typed explanation. Ordinary repository prose
is still not parsed into contracts. This implementation remains under Draft review;
local test evidence and milestone acceptance are separate decisions.

Merged P1-L additionally implements an isolated, pure Rust typed resolver for
selectors, source versions, masked rules, conflicts and unknown applicability using
[ADR-0004](adr/0004-product-domain-trust.md) and its
[v1 contract](p1/typed-context-v1.md). Callers supply already-bound values; this
API remains separate from persisted domain state. P1-N supplies fresh authorized
contract inputs and projects a safe subset through existing MCP/IPC context; it
does not serialize the whole operator explanation to agents.
Authorization/security gates apply first. Current repository facts and accepted
contracts outrank stored preferences or Handoff; facts describe what exists while
contracts describe what is required. Code/ADR disagreement is surfaced for human
resolution, not silently resolved in favor of either. Session additions may narrow
work, but never override contracts or grant permissions. Stale snapshots cannot
freeze obsolete permissions or hide new repository rules.

## Trust and compatibility

MCP is project-bound and read-only (`get_effective_context` and `get_handoff` are the current tools). Local IPC separates
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
