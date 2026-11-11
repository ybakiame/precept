# Product UI information architecture

Status: P1-J target navigation for review, **not implemented screens**. Follow the
[charter](product.md), [domain ADR](adr/0004-product-domain-trust.md) and accepted
[NSH decision](adr/0003-ui-framework-selection.md). Preserve the shared Rust model,
Instrument / Ledger / Harbor, Light / Dark / System + high contrast, and English /
Simplified Chinese [identity](brand/identity.md). No visual redesign or new framework.

## Navigation and scope

Organize around developer tasks rather than one page for each data type. A persistent
scope strip identifies the applied daemon target, selected project/worktree and,
when relevant, exact task/session generation. Personal scope is explicitly labeled;
a selected project must not turn a personal action into a repository write.

| Destination | User question / grouped concepts | Relationship to implemented UI |
| --- | --- | --- |
| Overview | What project am I viewing, what was last read, and what needs attention? Project/daemon summary, conflicts, pending reviews, resume warnings | Evolve Dashboard; only show counts backed by actual scoped data |
| Projects | Which checkout owns this work? Registration/onboarding, observed Git facts, contract-source inventory | Existing allowlisted Projects is read-only; guided registration is future operator work |
| Context & Rules | Why does this rule apply? Effective context plus Project contracts; source/review/conflict detail | Existing confirmed Preferences and document disclosures are a subset; no current structured rule engine |
| Personal Preferences | Which user choices follow me? Global/project personal scopes and full lifecycle | Separate user scope from project contracts; initially an explicit CLI workflow, future operator review UI |
| Review | What change am I approving? Suggestions, exact diff, destination, rationale, conflicts and review history | Planned; a pending badge links here only after a real queue exists |
| Sessions & Handoff | What is running, what was shared, and how do I resume? Native session control plus exact-task continuity | Existing Sessions auxiliary panel and Handoff lookup; no invented history/task enumeration |
| Connections & Settings | Which explicit target am I using? Connection drafts/applied target, diagnostics, appearance and language | Existing connection/settings panels and Diagnostics; credentials never displayed |

Provenance and Evidence are contextual detail views reachable from rules, reviews,
sessions and Handoff, with links back to the selected item and scope. They need not
be independent top-level pages. Conflicts belong beside the affected rule/context
with an Overview entry point. This keeps causal explanation near the decision.
The grouping is a target IA; it is not a requirement to add all destinations at once.

```text
Overview → select project → Projects → Context & Rules → source / conflict detail
                                          │
                                          └→ Review → exact change and destination
Personal Preferences → review decision → updated effective-context explanation
Sessions & Handoff → exact task / generation → evidence → verifier / method / limits
Connections & Settings → draft target → explicit Apply → fresh scoped read
```

## Main flows and decision surfaces

- **Project onboarding:** select an explicit existing checkout, inspect canonical
  identity and ownership, preview the bounded sources to share, then register through
  an operator action. No current project means guidance, not an arbitrary path editor
  for writing contracts. Templates/scaffolds never run automatically.
- **Rule explanation:** show kind, personal/project scope, source/version/digest,
  applicability result, review state, masking/conflict reason and current vs frozen
  source. Distinguish metadata-only references from requested document bodies.
- **Human review:** show exact proposed value/content, target scope/project and diff,
  author/reporter, rationale, affected rules and source freshness. Confirm/reject are
  deliberate actions; edit invalidates previous approval. Provide view/confirm/
  reject/deprecate/delete and provenance inspection before automatic suggestions.
  A project proposal requires repository review; local confirmation is not a commit.
- **Managed session:** keep auth-required, Start review, permission request and context
  sharing separate. Show native-only controls and exact generation. Sharing is off by
  default; today's UI Start sends no sharing opt-in. Show lease active/revoked/expired
  from daemon observations, not an agent's assurance. Cancel revokes the whole context
  lease; fresh sharing needs Stop and a new opted-in Start.
- **Handoff resume:** exact task lookup, objective/next action, current vs recorded
  HEAD/branch/worktree/source versions, constraint differences, then checks to rerun.
  Acknowledging receipt is not permission approval or independent verification.
- **Evidence inspection:** display result, origin class, subject revision/platform,
  reporter, observer/verifier and method/limits together. A transport event and a
  model-generated test claim must never share an unexplained green “verified” badge.

## Honest states and interactions

Use separate textual dimensions: review (`Suggested`, `Confirmed`, `Rejected`,
`Deprecated`), resolution (`Applies`, `Masked`, `Conflict`, `Unknown`), connection
(`Loading`, `Unavailable`, `Refused`, `Last successful read`), lease (`Off`, `Active`,
`Revoked`, `Expired`) and evidence origin (`Agent-reported`, `Daemon-observed`,
`Independently verified`). These are target UI labels, not new IPC enum values.
Historical PASS is labeled historical with its source/time; `BLOCKED`/`UNSUPPORTED`
in an evaluation report does not change the current Handoff wire status enum.

An empty result, unavailable source, permission refusal and not-yet-implemented
capability need different explanations. Do not show unavailable review actions as
working buttons, fabricate agent presence, infer a healthy connection from an old
read, or relabel unknown fingerprints as clean. Pending counts must say whether
they are personal, selected-project or an explicit aggregate; unknown is not zero.

Project/target switches clear task/session detail and old snapshots. Preserve the
existing epoch/project/task rejection of late completions; a draft target is labeled
separately from the applied connection. Permission or source changes invalidate
affected decisions before the user confirms them.

## Desktop/TUI and implementation gates

Both surfaces use the same navigation concepts, resolution results, review state,
evidence labels and authority checks. Desktop can show adjacent detail; compact/TUI
uses progressive disclosure and return navigation, with bounded visible content.
Only current-task controls are shown. Keyboard order follows the visible controls;
focus remains visible, detail close returns focus to its trigger, and meaning uses
text as well as color. Never add a pointer-only approval or TUI-specific business rule.

Reuse existing theme/i18n tokens and bounds; project data stays in its original
language. Full IME and screen-reader behavior remain unverified. Future UI tasks
must exercise scope switches, stale completion/refusal states, every decision by
keyboard, both locales/themes and native/terminal behavior on each claimed host.
No screenshots or usability scores are fabricated by this document-only task.
