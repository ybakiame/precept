# Feature ledger — agent-control (AC) into ACP

This is the single living record of how capabilities explored in the sibling
[agent-control](https://github.com/ybakiame/agent-control) repository map into ACP.
It supersedes the one-time [adoption assessment](agent-control-assessment.md),
which remains frozen at its original reviewed commit (`f54be80`). Initial entries
below were refreshed against AC `82a810ff0e90ac73fb52ecea536ce7622866eacf`.

AC is an application-side exploration vehicle (Windows-only). ACP is the product
line. Nothing enters ACP because AC shipped it: every entry requires an explicit
decision here, a target brief, and — where the trust surface changes — an ADR
before implementation. "Adopt"/"Adapt"/"Defer"/"Reject" follow the assessment's
vocabulary; "Unevaluated" means no owner decision exists yet.

## Maintenance rules

- One row per capability. Update the decision column when the owner decides; never
  delete a rejected row — the rejection rationale is the record.
- Any row whose decision changes persisted behavior, agent-facing authority or the
  trust model links its ADR before a brief is opened.
- AC receives no new feature work except feasibility spikes for Unevaluated rows
  and minimum maintenance, until the sunset criteria below are met.

## Ledger

| Capability (AC source) | AC state | ACP decision | Target | Notes |
| --- | --- | --- | --- | --- |
| Personal preferences vs project contracts, distinct ownership | Shipped | Adopt | Resolver (L) + M4/O | Already in typed resolver and ADR-0004 ordering |
| Environment scope (machine/project-local, git-ignored, advisory-only) | Shipped | Unevaluated | — | Overlaps profile scoping; decide whether local non-competing context belongs in Profile v1 |
| Project registry and guided onboarding | Shipped | Adapt | N | Explicit checkout/preview only; no auto-scaffold, no repository rewrite |
| Markdown/YAML contract files + local SQLite hybrid | Shipped | Adapt | N | Contracts as Git-reviewed files; profiles stay local. N should accept AC rule frontmatter on read so user repositories keep working |
| Scope/priority/applies_to resolution with provenance | Shipped | Adapt | Resolver (L, merged) | Explainable matching kept; no session-rank/priority authority over contracts/security |
| `code_checks` rule-vs-repo verification (`verify_constraints`) | Shipped | Unevaluated | Needs new brief after N | Independent repo reads fit ADR-0004 honesty; execution/parsing not copied. Pairs with assurance wording below |
| Assurance tiers (Advisory/Verified/Enforced, evidence-computed) | Shipped | Unevaluated | With code_checks brief | Honest-tier vocabulary matches ACP discipline; decide schema and where tiers are recorded |
| `suggest_rule` proposals with rationale, human review queue | Shipped | Adapt | O + M4 (S0 contract frozen) | Operator-only review binds exact record/revision; no agent-facing mutation |
| Session lifecycle and resumable work state (goal/done/doing/todo) | Shipped | Adapt | P | Historical Handoff observations, not fresh verification |
| Frozen constraint snapshots and handoff bundles | Shipped | Adapt | P | Recheck current sources/permissions on resume; checksum wording only |
| SHA-256 evidence chain | Shipped | Defer | Q | Needs separate ADR: canonicalization, anchors, privacy, independent verification. Not event truth or tamper proof |
| PreToolUse hook enforcement across nine harnesses (`check_tool_call`) | Shipped | Unevaluated | Evaluation issue first | Largest open gap; touches trust surface (deny semantics, fail-open vs fail-closed, writes into harness configs). Likely ADR before any brief |
| `export_native` pointer blocks, `check_export_drift`, `revoke_native` | Shipped | Unevaluated, conflicts with "never rewrite AGENTS.md" | Decide at N | Either Reject, or Adapt to operator-presented suggestion text only |
| Broad 25-tool MCP surface (agent writes, sessions, evidence mutation) | Shipped | Reject | — | MCP retains exactly two project-bound read-only tools; any proposal-only API needs its own ADR |
| Streamable HTTP daemon listener | Shipped | Reject | — | Local IPC only; no network listener |
| GPUI desktop stack and portable packaging | Shipped | Reject | — | NSH Desktop/TUI retained (ADR-0003) |
| Bilingual UI (English/简体中文) | Shipped | Adapt | R | NSH locales exist; release-gate acceptance belongs to R |
| Documentation-integrity direction (doc claims vs repo state, drift detection, freshness evidence) | Not started | Unevaluated | AC spike first; ACP brief only after N and Q | Claim model validated in AC; ACP brief drafts no code before contract-file and evidence foundations land |

## AC sunset criteria

AC enters maintenance-only now and archive when all of the following hold:

1. Briefs N, O and P are merged with their acceptance evidence.
2. Native Windows and Linux/WSL acceptance passes; macOS status is recorded
   honestly (verified or explicitly unverified).
3. ACP reads AC contract files (`.agent-control/rules/*.md` frontmatter) so
   existing user repositories keep functioning. SQLite session/evidence state is
   intentionally not migrated, consistent with ADR-0006's no-migration stance.
4. Both READMEs cross-link their roles (added with this ledger's PR or earlier).
