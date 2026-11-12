# Product roadmap and acceptance

The [charter](product.md) and [domain/trust ADR](adr/0004-product-domain-trust.md)
define the target. Checkmarks below refer to scoped historical evidence, not
production readiness. Current completed engineering baseline: PR #37 merged at
`d6168e0ca5bff150942711b8f398516e03769703` (P1-M0); ADR-0004 and ADR-0005
are Accepted. [P1-M1 / Issue #38](https://github.com/ybakiame/agent-control-plane/issues/38)
is active for synthetic-only Rust writer work. No real-profile migration or
production write ownership is authorized.
P1-K interoperability evidence is scoped separately below.

## Stage 1 — established engineering baseline (P0 through P1-I)

| Capability | Implemented / evidenced | Outstanding |
| --- | --- | --- |
| Context and personal preferences | Python local SQLite registry/review/writes, legacy Rust resolver, bounded live sources; P1-L pure typed selectors/conflicts | Live structured-contract adapters, persisted generalized model and complete review UI |
| Continuity | Exact project/task Handoff, historical checks, revision/worktree/fingerprint warnings | Frozen constraint snapshots, portable bundle format and independent cross-client acceptance |
| Rust ownership and storage | Read-only core/store, parity and synthetic backup/migration/process recovery | Production Rust writes, real-data migration/recovery, backup/export/import and retention design |
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

## Stage 4 — P1-L / M0 complete; P1-M1 synthetic-only implementation active

ADR-0004 was accepted by the owner on 2026-11-11 without changing its domain/trust
decisions. [P1-L / Issue #34](https://github.com/ybakiame/agent-control-plane/issues/34)
is complete at its stated L0–L2 scope and merged in
[PR #35](https://github.com/ybakiame/agent-control-plane/pull/35) at
`2a5949120aacd0523cd7fa5e417548e1e648bc81`; Issue #34 is closed.
[P1-M0 / Issue #36](https://github.com/ybakiame/agent-control-plane/issues/36)
is complete; [PR #37](https://github.com/ybakiame/agent-control-plane/pull/37)
merged at `d6168e0ca5bff150942711b8f398516e03769703` and the owner separately
accepted ADR-0005 on 2026-11-12. [P1-M1 / Issue #38](https://github.com/ybakiame/agent-control-plane/issues/38)
is the only active Stage 4 implementation task, restricted to disposable synthetic
databases. M2–M5 and N–R remain gated briefs; no real-profile access or production
write activation is authorized. Dependencies avoid premature storage migration,
onboarding, review UI or snapshots.

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

M0 delivered [Accepted ADR-0005](adr/0005-local-storage-and-write-ownership.md) and a
[storage ownership plan](p1/storage-ownership-plan.md): single opted-in daemon
writer, separate writer library, Python compatibility/fencing, explicit schema
versions and verified-generation cutover/rollback. It changes documentation only.
The owner resolved the transition decision in PR #37: reject new
suggested→deprecated under the future writer, preserve historical records and
follow ADR-0004. Current Python behavior is unchanged; M1/M2 implementation and
parity/transition tests remain pending, including the intentional CLI compatibility
difference and pre-cutover fencing. New Rust writes, v2 migration and real-profile
rollout remain NOT_IMPLEMENTED / NOT_RUN at this milestone. Completed C2A
synthetic migration and C2B1 genuine process recovery, including the corrected
historical Linux/macOS/Windows process matrix, remain prerequisites already proven
at their recorded scope. M0 neither repeats them nor claims production recovery.

ADR-0005 acceptance was a separate explicit owner decision after PR merge.
Issue #38 now authorizes only synthetic M1 writer/schema/transaction work. M2
compatibility, M3 synthetic cutover/recovery/privacy, M4 independently consented
per-profile rollout and M5 rollback closure/direct-writer retirement are not yet
opened. No real profile access, production Rust writes, or daemon/IPC/MCP/UI
mutations are authorized by M1. Exact gates and NOT_RUN limitations stay in the plan.

| Brief | Depends on | Deliverable / acceptance gate |
| --- | --- | --- |
| L — typed context explanation ([Issue #34](https://github.com/ybakiame/agent-control-plane/issues/34), merged PR #35) | J merged; ADR-0004 accepted | L0 contract, L1 pure resolver and L2 scoped local compatibility/review evidence delivered; legacy responses preserved |
| M0 — local storage design ([Issue #36](https://github.com/ybakiame/agent-control-plane/issues/36), merged PR #37) | J + merged L; Accepted ADR-0004 | ADR-0005 and ownership/compatibility/recovery matrices delivered and explicitly accepted; no code or real-data authorization |
| M1 — synthetic Rust writer and exact v2 schema ([Issue #38](https://github.com/ybakiame/agent-control-plane/issues/38), active) | Accepted ADR-0005; resolved transition semantics | Frozen exact v2/catalog contract; isolated synthetic-only Rust write crate, transaction/CAS/receipt and strict transition/legacy-preservation regressions, no production entrypoint |
| M2–M5 — compatibility through explicit rollout (unopened) | M1 accepted; sequential gates from storage plan | Separately reviewed Python read-only/daemon owner fencing, synthetic cutover/recovery/privacy, per-profile consent and writer retirement; no implicit adoption or dual writers |
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
