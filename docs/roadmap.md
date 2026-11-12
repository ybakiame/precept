# Product roadmap and acceptance

The [charter](product.md) and [domain/trust ADR](adr/0004-product-domain-trust.md)
define the target. Checkmarks below refer to scoped historical evidence, not
production readiness. Current completed engineering baseline: PR #37 merged at
`d6168e0ca5bff150942711b8f398516e03769703` (P1-M0); PR #39 merged at
`3083c407d02c34bbbfef402f7ae57bb7eda248d1` (P1-M1 synthetic-only Rust writer);
ADR-0004, ADR-0005 and ADR-0006 are Accepted. [P1-M2 / Issue #40](https://github.com/ybakiame/agent-control-plane/issues/40)
is active for Rust-only runtime cutover and Python prototype retirement under
[ADR-0006](adr/0006-rust-only-runtime.md). M2 remains validation-only; local prototype state may be deleted, and no production
write activation is authorized.
P1-K interoperability evidence is scoped separately below.

## Stage 1 — established engineering baseline (P0 through P1-I)

| Capability | Implemented / evidenced | Outstanding |
| --- | --- | --- |
| Context and personal preferences | Legacy Python P0 registry/review/writes remain temporarily in the tree; Rust typed resolver and bounded live sources are implemented | Rust-native Profile v1 persistence and complete operator review workflow (P1-M2/M4) |
| Continuity | Exact project/task Handoff, historical checks, revision/worktree/fingerprint warnings | Frozen constraint snapshots, portable bundle format and independent cross-client acceptance |
| Rust ownership and storage | Read-only core/store and M1's disposable synthetic v2 writer/transaction evidence | Rust-native Profile v1 writes and live reader/owner integration; any later data-lifecycle feature must be justified by actual product use |
| Local service and MCP | Unix sockets/named pipes, operator/project capabilities, two-tool read-only stdio | Full MCP conformance and broader versioned client matrix |
| NSH Desktop/TUI | Shared model, five pages, Sessions panel, connections/settings, themes/locales and bounded 10k display | Product IA realization, clean-machine release/accessibility and current macOS verification |
| Optional ACP sessions | Explicit native lifecycle/auth observations; P1-I opt-in lease, expiry and revocation | Persistent resume, vendor parity and sandboxing are not delivered or promised |

Evidence: [P0 negative boundaries](evaluation/p0-mcp-negative-boundaries.md),
[Rust parity](p1/p1-b-validation.md), [SQLite](p1/sqlite-readonly.md),
[synthetic recovery](p1/process-recovery.md), [local service](p1/local-service.md),
[UI comparison](p1/ui-spike-evaluation.md), [E4](evaluation/p1-e4-local-ux.md),
[F](evaluation/p1-f-local-ux.md), [G](evaluation/p1-g-local-agent-session.md),
[H](evaluation/p1-h-real-agent-e2e.md), [I](evaluation/p1-i-context-bridge.md).
Older plans/evaluations record their original status; they are not current roadmap authority.

## Stage 2 — P1-J product formalization / Issue #30 (merged)

Deliver the charter, implemented/target architecture, domain/trust ADR, accepted NSH
ADR, UI IA, sibling adoption assessment and consistent README. This stage changes
no code, schema, tools, permissions or UI. P1-J was reviewed and merged in PR #32; documentation alone does not
complete implementation acceptance.

## Stage 3 — P1-K interoperability / Issue #31

[Issue #31](https://github.com/ybakiame/agent-control-plane/issues/31) follows merged J.
The [current evaluation](evaluation/p1-k-cross-agent-interop.md) distinguishes exact
client/provider identities, mode and independently observed reads from catalog claims.

- [x] Independently captured Codex direct MCP and Kimi managed ACP evidence, with an
  installed-client matrix that explicitly retains Pi absence and OpenCode limits.
- [x] Synthetic cross-client Handoff resume verifies same-HEAD dirty content drift,
  branch/HEAD changes, legacy unknown fingerprints and historical validation states.
- [x] Each claimed tool-call PASS has server-side observation, policy/equality checks
  and unchanged fixture DB; managed Stop denies established peers and reconnects.
- [ ] OpenCode actual MCP tool calls remain unverified: direct discovery did not
  launch the bridge, and managed Code Mode stays BLOCKED without broad execute.
- [ ] Upstream Pi trial remains NOT_RUN because the selected client was not found.
  Installed Oh My Pi is a distinct fork and is not substituted.
- [ ] Codex model-selected tool use and additional platform/provider modes are not
  inferred from its no-inference app-server invocation or the one Kimi turn.

Current K evidence at `757eac3`: Codex CLI 0.167.1 independently invokes both read-only
MCP tools without a model; Kimi Code 2.5.2 / `kimi-for-coding` completes one separately
opted-in stale Handoff resume, with daemon counts and post-Stop denials. No product
code/permission/UI expansion or real project data was required. P0 user-reported
[continuity](evaluation/2026-11-08-codex-kimi-handoff.md), P1-I Kimi results and OpenCode
quota/zero-read failures retain their original historical provenance. K documents
compatibility limits; it does not imply all-vendor certification or fresh platform
acceptance. Hosted Actions remain NOT_RUN. PR #33 was reviewed and merged at
`e3b1e3ad360e1ec28c8b17dc4774961463e63a12`; Issue #31 is closed. Its optimization-bypass
findings were fixed in `c285ce7`: explicit executable authorization/acceptance guards,
seven new regressions, 29 Python tests passing on both local hosts, and targeted native
IPC/MCP checks. The [follow-up evidence](evaluation/p1-k-cross-agent-interop.md#pr-33-review-optimization-safe-executable-checks)
retains original trial provenance. K is complete at its stated scope; OpenCode/Pi and
additional model/platform modes remain explicitly unverified, not silently passed.

## Stage 4 — P1-L / M0 / M1 complete; P1-M2 Rust-only runtime cutover active

ADR-0004 was accepted by the owner on 2026-11-11 without changing its domain/trust
decisions. [P1-L / Issue #34](https://github.com/ybakiame/agent-control-plane/issues/34)
is complete at its stated L0–L2 scope and merged in
[PR #35](https://github.com/ybakiame/agent-control-plane/pull/35) at
`2a5949120aacd0523cd7fa5e417548e1e648bc81`; Issue #34 is closed.
[P1-M0 / Issue #36](https://github.com/ybakiame/agent-control-plane/issues/36)
is complete; [PR #37](https://github.com/ybakiame/agent-control-plane/pull/37)
merged at `d6168e0ca5bff150942711b8f398516e03769703` and the owner separately
accepted ADR-0005 on 2026-11-12. [P1-M1 / Issue #38](https://github.com/ybakiame/agent-control-plane/issues/38)
is complete: [PR #39](https://github.com/ybakiame/agent-control-plane/pull/39)
merged at `3083c407d02c34bbbfef402f7ae57bb7eda248d1`. [P1-M2 / Issue #40](https://github.com/ybakiame/agent-control-plane/issues/40)
is now the active Stage 4 implementation task, restricted to disposable validation
profiles. Local prototype state may be deleted; production write activation is not
authorized. M3–M5 and N–R remain gated briefs. Dependencies avoid
premature migration, onboarding, review UI or snapshots.

- [x] L0: [v1 domain/selector/conflict contract](p1/typed-context-v1.md) frozen
  before implementation; additive DTO remains separate from the MCP wire contract.
- [x] L1: pure `acp-core::typed_context` model/resolver, explicit unknown/unresolved
  outcomes, source/review reasons, provenance and deterministic ordering.
- [x] L2: 15 focused resolver cases on both hosts; native Windows and Linux/WSL
  workspace, Python/Rust, SQLite, synthetic recovery and IPC/session regressions
  pass at clean `22b6ffc`. Exact commands, counts and limitations are in the
  [P1-L evidence report](evaluation/p1-l-typed-context-resolver.md).

Legacy responses, schemas, write ownership, dependencies and UI remain unchanged.
Hosted Actions and new UI/vendor-model acceptance remain NOT_RUN. Local validation
does not imply storage or broader client/platform acceptance.

PR #35 review found that an unresolved authority set could still claim a resolved
preference mask. Fix `495e197` defers those masks until complete key analysis;
18 resolver cases, core/legacy tests, 29 Python tests and 120 cross-language
comparisons pass on both local hosts. The
[review follow-up evidence](evaluation/p1-l-typed-context-resolver.md#pr-35-review-follow-up--unresolved-authorities-cannot-resolve-a-mask)
retains the original full-matrix source and records the narrower correction checks.
The review correction was included in the merged PR; its narrower checks do not
retroactively rerun the earlier full matrix at the merge commit.

M0 delivered [Accepted ADR-0005](adr/0005-local-storage-and-write-ownership.md)
and the original [storage ownership plan](p1/storage-ownership-plan.md). The owner
accepted [ADR-0006](adr/0006-rust-only-runtime.md) on 2026-11-12, superseding the
Python frontend/read-only/rollback-owner decisions, v0/v1 runtime compatibility,
and schema downgrade. The retained architecture is a Rust-only runtime, a fresh Rust-native Profile schema v1, and one daemon write owner. The legacy Python v1 and M1 synthetic v2 are historical, incompatible formats with no migration requirement.

M1 is complete: [PR #39](https://github.com/ybakiame/agent-control-plane/pull/39)
merged the isolated synthetic v2 writer and its CAS/receipt/strict-transition
evidence. Its legacy-v0/v1-to-v2 migration cases remain historical evidence only.
M1's v2 schema is not the product schema and is not migrated into Rust-native v1. See the [M1 evidence report](evaluation/p1-m1-synthetic-writer.md).

[P1-M2 / Issue #40](https://github.com/ybakiame/agent-control-plane/issues/40)
is re-scoped to Rust-only runtime cutover: explicit creation of a fresh Rust-native
Profile v1, strict v1 reader adoption, daemon-owned operator write IPC, native CLI/NSH integration,
and removal of Python runtime/tests/dependencies plus active legacy paths. M2 uses disposable validation fixtures. Existing local Python-prototype files and
old validation profiles may be deleted during a deliberate ACP development reset;
no backward-compatibility or retention guarantee is required. Production writes are
not activated.

Issue #43 Stage 1 now has a locally validated Draft candidate: exact fresh v1 DDL,
explicit no-overwrite creation, strict read-only store and seven private typed
transactions, with M1 migration/rehearsal paths removed. The
[Stage 1 evidence](evaluation/p1-m2-s1-storage.md) records exact native Windows and
Linux/WSL sources and results. This does not complete parent Issue #40: daemon
bootstrap, grants/leases, epoch advancement, IPC/CLI/NSH integration and complete
Python retirement remain later slices. No production writer is wired or activated.

P1-M3 remains conditional. Open it only if M2 reveals a concrete Profile v1 lifecycle
or recovery need; do not assume backup/restore/retention work in advance. It has no
Python owner handoff or legacy-schema migration. P1-M4 completes the
operator review/profile workflows under ADR-0004. P1-M5 handles release hardening.
Any real-profile operation or release requires a separate explicit owner decision.

| Brief | Depends on | Deliverable / acceptance gate |
| --- | --- | --- |
| L — typed context explanation ([Issue #34](https://github.com/ybakiame/agent-control-plane/issues/34), merged PR #35) | J merged; ADR-0004 accepted | L0 contract, L1 pure resolver and L2 scoped local compatibility/review evidence delivered; legacy responses preserved |
| M0 — local storage design ([Issue #36](https://github.com/ybakiame/agent-control-plane/issues/36), merged PR #37) | J + merged L; Accepted ADR-0004 | ADR-0005 and ownership/compatibility/recovery matrices delivered and explicitly accepted; no code or real-data authorization |
| M1 — synthetic Rust writer and exact v2 schema ([Issue #38](https://github.com/ybakiame/agent-control-plane/issues/38), merged PR #39) | Accepted ADR-0005; resolved transition semantics | Historical exact v2 contract, isolated `acp-write`, transaction/CAS/receipt/strict-transition evidence; merged at `3083c407`; disposable synthetic-only, not the product schema |
| M2 — Rust-only runtime cutover and Python prototype retirement ([Issue #40](https://github.com/ybakiame/agent-control-plane/issues/40), active) | M1 merged; Accepted ADR-0006 | Fresh Rust-native Profile v1 creation/reads, daemon-only `acp-write`, operator write grant/IPC, native `acp-query` and NSH integration, removal of Python runtime/tests/dependencies and legacy migration paths; disposable validation fixtures only |
| M3 — Profile v1 lifecycle and recovery (conditional/unopened) | M2 accepted plus a demonstrated product need | Only the minimum Rust-native v1 lifecycle/recovery behavior actually required; no assumed backup/restore/retention system, legacy migration, Python owner or downgrade |
| M4 — operator review and profile workflows (unopened) | M2/M3 accepted; ADR-0004 | Human review/provenance, explicit profile lifecycle UX and permission-safe end-to-end acceptance |
| M5 — release hardening (unopened) | M2–M4 accepted | Clean-machine packaging, accessibility, supported-platform evidence, security and performance gates |
| N — project onboarding and contracts | J + L; M for writes | Register/preview explicit roots, optional versioned Markdown/YAML contract format and reviewed diff workflow; no auto-scaffold, symlink escape, private profile export or stale-source activation |
| O — complete human review | J + L + M | Operator-only versioned proposal/review API and NSH workflow with view/confirm/reject/deprecate/delete/provenance; edits/review revocation/permission changes invalidate approval; no agent-facing mutation |
| P — snapshot and Handoff continuity | J + L; M for persistence | Versioned immutable constraint/source references, resume comparison and legacy unknowns; checksum wording only; stale worktree/contract/permission and evidence-origin regressions; interoperability acceptance stays K |
| Q — evidence and provenance inspection | J + L + P format | Minimal attributed records and contextual NSH details; separate claim/result/origin, independent verification method and limits; sanitization/retention/deletion tests; chain/signature work needs another ADR |
| R — NSH product IA and release gates | J, relevant L–Q APIs | Incremental shared Desktop/TUI navigation, no business-logic duplication; keyboard/scope/locale/state tests and actual host evidence; clean-machine packages and explicit IME/AT/macOS acceptance before support claims |

A proposal-only MCP API, hash chain or signatures need separate future ADRs; none
is bundled into O/P/Q by implication. Full MCP conformance remains an explicit future
integration task, not inferred from K. PTY fallbacks, remote sync, multi-user/fleet
features and external memory are deferred without a delivery commitment. No new
UI framework/runtime or second production renderer is planned.

## Invariants for every implementation task

- Suggested/rejected/deprecated preferences never reach agent effective context;
  project preferences mask global values without overruling repository contracts.
- Exact project/task scope, escaping symlink refusal, current revision/worktree
  checks and explicit unknown fingerprints remain regression requirements.
- MCP remains read-only; session Start/auth/context sharing/permission are separate.
  Preserve P1-I lease/revocation including established peers and response admission.
- Historical evidence is not fresh validation; reporter/observer/verifier stay distinct.
- Run repository-required tests for code changes and targeted security regressions;
  record exact source/platform and honest PASS/FAIL/BLOCKED/NOT_RUN outcomes.
  Never infer hosted CI, device or client results from local document checks.
- Plan malformed-input/fuzz coverage for future structured formats, resolver and
  migration boundaries; stdio legacy/stateless and real-process regression checks
  remain compatibility gates, not full MCP conformance.
