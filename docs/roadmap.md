# Product roadmap and acceptance

The [charter](product.md) and [domain/trust ADR](adr/0004-product-domain-trust.md)
define the target. Checkmarks below refer to scoped historical evidence, not
production readiness. Runtime foundation: P1-M2 is complete —
[PR #51](https://github.com/ybakiame/agent-control-plane/pull/51) merged at
`18f2670d294deebbc17f530c36bebecdaf8beb90` (final S5 Rust-only native integrated
acceptance), closing
[#40](https://github.com/ybakiame/agent-control-plane/issues/40) at its synthetic
engineering-validation scope. Earlier merged stages: PR #37 at
`d6168e0ca5bff150942711b8f398516e03769703` (P1-M0) and PR #39 at
`3083c407d02c34bbbfef402f7ae57bb7eda248d1` (P1-M1 synthetic-only Rust writer).
ADR-0004, ADR-0005 and ADR-0006 are Accepted. M4 engineering scope
[#52](https://github.com/ybakiame/agent-control-plane/issues/52) and N precedence
[#65](https://github.com/ybakiame/agent-control-plane/issues/65) are closed after
merged PR #61 (`e286c2606ed94031bc85ecd46925f2f0b62b1838`) and PR #66
(`6d8000153107c67edfa5ff3bc77da15f196623d3`). The active task is
[M5/S0 #68](https://github.com/ybakiame/agent-control-plane/issues/68), under
[M5 #67](https://github.com/ybakiame/agent-control-plane/issues/67): a documentation-only
[release boundary proposal](p1/release-readiness-plan.md) and
[gap inventory](evaluation/p1-m5-s0-gap-inventory.md), awaiting revised S0 review.
Owner-confirmed initial direction: Windows x86_64 portable ZIP (HWND + ConPTY),
standalone Linux x86_64 native Wayland + PTY, and macOS deferred. Other release
defaults/floors remain recommendations to finalize with evidence at their gates.
No release, support commitment or real-profile activation follows from engineering
closure. M3 remains deferred and conditional.
P1-K interoperability evidence is scoped separately below.

## Stage 1 — established engineering baseline (P0 through P1-I)

| Capability | Implemented / evidenced | Outstanding |
| --- | --- | --- |
| Context and personal preferences | Rust typed resolver, fresh Profile v1, retired prototype, bounded operator queue/exact review/history/confirm/reject/deprecate/delete in CLI and NSH; M4 engineering closed | M5 release/human-use gates; generalized provenance/evidence remains separate |
| Repository contract precedence | Optional bounded repository-owned source feeds typed resolution on actual daemon/IPC/MCP; #65 closed | N onboarding/contract editing remains a separate brief; arbitrary prose is not parsed |
| Continuity | Exact project/task Handoff, historical checks, revision/worktree/fingerprint warnings | Frozen constraint snapshots, portable bundle format and independent cross-client acceptance |
| Rust ownership and storage | Fresh Profile v1, read-only store and daemon-exclusive lease/epoch/grant writer validated on disposable native Windows/Linux profiles | Any later data-lifecycle feature requires actual product use (M3 conditional) |
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

## Stage 4 — P1-L / M0 / M1 complete; P1-M2 Rust-only runtime cutover complete

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
was the Stage 4 implementation task, restricted to disposable validation
profiles; it is closed at that synthetic engineering-validation scope. Local
prototype state may be deleted; production write activation is not authorized.
M3 remains conditional. M5/S0 is the active planning task; N precedence is closed,
while onboarding and other N–R work remain separately gated. Dependencies avoid
premature lifecycle, onboarding, generalized review or snapshot work.

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

[P1-M2/S1 Issue #43](https://github.com/ybakiame/agent-control-plane/issues/43)
is complete: [PR #42](https://github.com/ybakiame/agent-control-plane/pull/42)
merged at `1976d6f4d373db737b3be27d2781c191253a467a`. The fresh
Profile v1 DDL, explicit no-overwrite creation, strict read-only Store and private
seven-command transactional Writer are implemented; M1 migration/rehearsal paths
are retired. The [Stage 1 evidence](evaluation/p1-m2-s1-storage.md) records full
Workspace tests at `c6c9052`: 87 Linux/WSL and 88 native Windows PASS; hosted
Actions NOT_RUN (`[skip ci]`). The merged HEAD after that test was evidence-only.

[P1-M2/S2 Issue #44](https://github.com/ybakiame/agent-control-plane/issues/44)
merged in [PR #45](https://github.com/ybakiame/agent-control-plane/pull/45),
with [native evidence](evaluation/p1-m2-s2-ownership.md): daemon profile bootstrap/select and opened-file
identity, exclusive writer lease, epoch binding, distinct operator write grants,
revocation and typed write IPC v2. Native Workspace results at `371e577` are 103
Linux/WSL and 104 Windows PASS, plus shared-model 25 on each. No real Profile is
activated. No production writer was activated in S1. Parent
[P1-M2/S3 Issue #46](https://github.com/ybakiame/agent-control-plane/issues/46)
has a [native operator contract](p1/native-operator-workflows.md) and
[local evidence](evaluation/p1-m2-s3-native-operator.md): all seven typed CLI commands,
operator-only scoped revision previews and shared NSH Profile/grant/suggestion/
confirm/reject workflows. Higher-risk registration, deprecate/delete and Handoff
put remain CLI-only in this minimum UI. S3 merged in PR #47.
[S4 / Issue #48](https://github.com/ybakiame/agent-control-plane/issues/48) retired the
prototype and active toolchain before final M2 acceptance; see the
[replacement matrix](p1/python-retirement.md) and [Rust-only runbook](p1/rust-only-development.md).
Final integrated M2 acceptance ran as
[S5 / Issue #50](https://github.com/ybakiame/agent-control-plane/issues/50) and merged
in [PR #51](https://github.com/ybakiame/agent-control-plane/pull/51) at
`18f2670d294deebbc17f530c36bebecdaf8beb90`; #40 is closed. Its
[evidence](evaluation/p1-m2-s5-rust-only-integrated-acceptance.md) is scoped to the
tested revision `ddc12ac8c17215014e04cc65ed802819b1bc8376` and is not re-executed by
later documentation or harness changes.

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
| M2 — Rust-only runtime cutover and Python prototype retirement ([Issue #40](https://github.com/ybakiame/agent-control-plane/issues/40), closed at merged PR #51 `18f2670`) | M1 merged; Accepted ADR-0006 | Fresh Rust-native Profile v1 creation/reads, daemon-only `acp-write`, operator write grant/IPC, native `acp-query` and NSH integration, removal of Python runtime/tests/dependencies and legacy migration paths; disposable validation fixtures only |
| M3 — Profile v1 lifecycle and recovery (conditional/unopened) | M2 accepted plus a demonstrated product need | Only the minimum Rust-native v1 lifecycle/recovery behavior actually required; no assumed backup/restore/retention system, legacy migration, Python owner or downgrade |
| M4 — operator review and profile workflows ([Issue #52](https://github.com/ybakiame/agent-control-plane/issues/52), engineering closed) | M2 accepted; M3 conditional; ADR-0004 | Bounded review storage/IPC/CLI/NSH delivered through S0–S3; [native evidence](evaluation/p1-m4-s3-integrated-acceptance.md) and separate [N enforcement evidence](evaluation/p1-n-project-contracts.md). Closure does not certify real data, IME/AT or release |
| M5 — release-readiness hardening ([#67](https://github.com/ybakiame/agent-control-plane/issues/67); S0 [#68](https://github.com/ybakiame/agent-control-plane/issues/68) active) | M2–M4 engineering accepted; owner accepts S0 boundary before implementation | Proposed [support/release contract](p1/release-readiness-plan.md), [source/gap inventory](evaluation/p1-m5-s0-gap-inventory.md), then narrowly scoped packaging, human-use, security/performance and clean-candidate gates; no release authorization |
| N — project onboarding and contracts | J + L; M for writes | Precedence [#65](https://github.com/ybakiame/agent-control-plane/issues/65) closed, PR #66 merged: [approved boundary](p1/project-contract-integration.md), [runtime evidence](evaluation/p1-n-project-contracts.md). Onboarding, contract editor and reviewed diff workflow remain separate; no auto-scaffold/export |
| O — generalized human review | J + L + M | M4 delivers bounded personal preference review/history in CLI/NSH. Any generalized proposal/review/provenance model remains a separate brief; no agent-facing mutation |
| P — snapshot and Handoff continuity | J + L; M for persistence | Versioned immutable constraint/source references, resume comparison and legacy unknowns; checksum wording only; stale worktree/contract/permission and evidence-origin regressions; interoperability acceptance stays K |
| Q — evidence and provenance inspection | J + L + P format | Minimal attributed records and contextual NSH details; separate claim/result/origin, independent verification method and limits; sanitization/retention/deletion tests; chain/signature work needs another ADR |
| R — NSH product IA and release gates | J, relevant L–Q APIs | Incremental shared Desktop/TUI navigation, no business-logic duplication; keyboard/scope/locale/state tests and actual host evidence; clean-machine packages and explicit IME/AT/macOS acceptance before support claims |

A proposal-only MCP API, hash chain or signatures need separate future ADRs; none
is bundled into O/P/Q by implication. Full MCP conformance remains an explicit future
integration task, not inferred from K. PTY fallbacks, remote sync, multi-user/fleet
features and external memory are deferred without a delivery commitment. No new
UI framework/runtime or second production renderer is planned.

## Stage 5 — P1-M4 operator review / Issue #52 (engineering closed)

M4 followed M2 with S0 [#53](https://github.com/ybakiame/agent-control-plane/issues/53)
/ PR #54 contract and bounded driver; S1 #55 / PR #56 review storage/IPC; the
PR #58 app migration; S2 #57 / PR #59 shared NSH review UX; and S3 #60 / PR #61
integration/security evidence. The separately scoped N #65 / PR #66 completed
production ProjectContract precedence after owner-approved design. The owner
closed M4's synthetic engineering scope after these merges.

Delivered: bounded pending queue and exact history, proposal digest/CAS/receipt
binding, explicit confirm/reject/deprecate/delete with peer effects and exact-ID
consent, stale-state invalidation, privacy, native process and scoped four-surface
acceptance. Repository claims outrank personal confirmation on actual IPC/MCP.
Root registration and Handoff put remain CLI-only; generalized evidence/provenance,
onboarding and snapshots are not silently completed by M4 closure.

The [S3 ledger](evaluation/p1-m4-s3-integrated-acceptance.md) preserves tested UI
code `19942b6e8ba58e990e60095bf6440d572cda6123`, historical failures/partial cohorts
and its original BLOCKED disposition. [N evidence](evaluation/p1-n-project-contracts.md)
records backend code `435f4bd325d569e3bd065fc6b5d096aec1d79f0a`. Merge commits and
owner issue closure do not re-execute either cohort or establish independent human,
clean-machine, standalone Linux, IME/AT or production acceptance. M3 remains
conditional; no speculative backup/import/restore/retention is bundled here.

## Stage 6 — P1-M5 release readiness / Issue #67 (S0 active)

[S0 #68](https://github.com/ybakiame/agent-control-plane/issues/68) audits merged
main `6d8000153107c67edfa5ff3bc77da15f196623d3` without behavior changes. Deliver
the [candidate support and release plan](p1/release-readiness-plan.md) and
[verifiable gap inventory](evaluation/p1-m5-s0-gap-inventory.md). Windows HWND and
ConPTY in a **portable ZIP only**, and standalone Linux x86_64 native Wayland and
PTY are owner-confirmed initial candidate directions; **macOS is deferred**.
WSLg remains separately labeled integration evidence, never standalone Linux
certification. Begin Linux with one clean ordinary distro/compositor and both UI
surfaces; no niri/XWayland/Xvfb requirement or large per-Issue compositor matrix.
Linux `.tar.gz` and a maintained Windows 11 baseline are recommendations, not exact
approved floors/formats. Choose exact versions/dependency layout in S1 from
import/clean-target evidence; scope human IME/AT, numeric budgets, notices and
audience/tag before the respective RC/release gate. These gaps do not block the S0
documentation contract and no development PASS implies a support commitment.

After revised S0 approval, propose independent S1 packaging, S2 human input/accessibility,
S3 security/performance and S4 clean-candidate Issues with real dependencies.
Do not implement or open them automatically, replace the build system, assume an
installer/OS baseline, introduce M3 work or authorize production state/distribution.
Document-only S0 uses diff/source/link checks. G01–G20 are a risk/claim inventory,
not twenty blocking checks per PR. Later tests follow change impact;
the complete four-native-surface UI matrix runs once for a material shared NSH
milestone or explicitly accepted RC UI gate, never automatically per Issue.
M5 and any release/real-Profile go-no-go remain separate owner decisions.

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
