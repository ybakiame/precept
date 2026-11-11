# Product roadmap and acceptance

The [charter](product.md) and [domain/trust ADR](adr/0004-product-domain-trust.md)
define the target. Checkmarks below refer to scoped historical evidence, not
production readiness. Latest reviewed baseline: main `14798f2` (PR #29).

## Stage 1 — established engineering baseline (P0 through P1-I)

| Capability | Implemented / evidenced | Outstanding |
| --- | --- | --- |
| Context and personal preferences | Python local SQLite registry/review/writes, Rust confirmed-only project/global resolver, bounded live sources | Structured contracts/selectors/conflicts and complete review UI |
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

## Stage 2 — P1-J product formalization / Issue #30 (this work)

Deliver the charter, implemented/target architecture, domain/trust ADR, accepted NSH
ADR, UI IA, sibling adoption assessment and consistent README. This stage changes
no code, schema, tools, permissions or UI. Owner review/merge remains the decision
gate; documentation alone does not complete implementation acceptance.

## Stage 3 — P1-K interoperability / Issue #31 (separate)

[Issue #31](https://github.com/ybakiame/agent-control-plane/issues/31) depends on J
review/merge. Do not fold it into this stage or call it complete from J documents.

- [ ] Independently captured Codex/Kimi traces plus installed Pi/OpenCode matrix,
  distinguishing direct MCP, ACP forwarding, client and provider/model.
- [ ] Synthetic cross-harness Handoff resume with source/version and stale-state checks.
- [ ] Actual server-side call evidence for each PASS, project/permission isolation,
  unchanged fixture DB and post-Stop established-peer/reconnect denial.

P0 Codex/Kimi [user-reported continuity](evaluation/2026-11-08-codex-kimi-handoff.md)
is historical. P1-I Kimi 2.5.2 completed both tools with independent daemon counters;
OpenCode's official DeepSeek model turn had **zero MCP reads**, so it is not an MCP
pass. Original quota/permission failures remain preserved. K owns further evaluation,
provider opt-in and compatibility conclusions; J runs no agent/model experiments.

## Stage 4 — follow-up implementation briefs after decisions are reviewed

These are **draft issue briefs**, not opened implementation issues or authorization
to begin code. Re-read merged J/main, refine the relevant ADR/spec, then create
bounded GitHub issues. Dependencies are explicit to avoid premature migration/UI work.

| Brief | Depends on | Deliverable / acceptance gate |
| --- | --- | --- |
| L — typed context explanation | J merged | Versioned rule/source/selector/conflict DTO design, then pure Rust resolver changes; deterministic unresolved conflicts, scope isolation, stale/unknown and malformed-selector regressions; preserve legacy responses until versioned |
| M — local storage and Rust write ownership | J + L semantics | DB/contract format ADR and backup/recovery/privacy design first; synthetic rollback/crash/lock/disk-failure/old-reader acceptance before any production write rollout; no implicit real-profile migration |
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
