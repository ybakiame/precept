# Agent Control Plane — product charter

Product definition: P1-J charter developed under [Issue #30](https://github.com/ybakiame/agent-control-plane/issues/30).
Decision and review history: [PR #32](https://github.com/ybakiame/agent-control-plane/pull/32).
Baseline: main `14798f2809b557a16bda16729f2cdeed7f24bbb0` (merged PR #29), reviewed 2026-11-11.
This defines the product direction; it does not mark planned capabilities implemented.

**Agent Control Plane is a local-first, developer-centric control plane for engineering
context, project rules and continuity across coding agents and harnesses.**

Its promise is to make the right context available to the right agent, explain why
it applies, preserve continuity across handoffs, and show what was actually observed
or verified. “Control” means scoped context and explicit operator decisions; it
does not mean that a local application can enforce every action of a same-user agent.

## User and problem

The primary user is a developer working in local Git checkouts with one or more
coding-agent clients. Changing clients, tasks or worktrees can lose constraints,
repeat explanations, apply stale preferences, or turn an old test claim into a
false current PASS. The developer needs an inspectable account of the applicable
project contract, confirmed personal choices, remaining conflicts and next action.

The product is useful with one agent. It does not depend on running a fleet,
replacing the user's editor/harness, or storing conversations. External clients
consume scoped context; native managed sessions are an optional integration.

## Product capabilities and present maturity

| Capability | Product responsibility | Implemented baseline / remaining work |
| --- | --- | --- |
| Project context | Bind an explicit checkout; inspect current facts and contract sources | Registered/allowlisted projects, bounded live metadata, optional document content and Git inspection exist; guided UI onboarding is planned |
| Personal preferences | User-owned habits with scope, review and provenance | Native CLI covers all seven typed mutations; shared NSH supports suggestion/confirm/reject and exact previews; full review/provenance UI remains planned |
| Project contracts | Explain repository-owned rules alongside actual code/config/tests/accepted ADRs | Live allowlisted document references exist; structured contracts, selectors and conflict resolution are planned |
| Effective context | Explain inclusion, exclusion, masking, uncertainty and source versions | Confirmed project/global preference resolution exists; full typed explanation and conflict model is planned |
| Suggestions | Let a human review a proposed durable change before it applies | Preference proposals exist through CLI; generalized review queue and agent proposal capability are not implemented |
| Continuity | Carry objective, next action, revision and honest historical checks | Scoped Handoff and freshness/fingerprint warnings exist; frozen constraint snapshots and portable bundles are planned |
| Provenance and evidence | Keep source/authority separate from verification | Source labels, opaque validation JSON and daemon context-call counters exist; a unified evidence record model is planned |

Personal profiles and review state belong in user-local SQLite; project contracts
belong to the repository and its review process, with no automatic file creation.
Rust is the foundation for domain logic, daemon and local IPC. NSH Rust is the
single Desktop/TUI UI direction ([ADR-0003](adr/0003-ui-framework-selection.md)).
The native daemon is the only write owner. S4 retires the disposable Python
prototype without migration. The J charter introduced no implementation; current
S1–S4 scope and evidence are tracked in the roadmap.

## Primary workflows

1. **Select a project and inspect its context.** Explicitly register/select a
   checkout, inspect repository state and sources, and request document bodies only
   when needed. Planned onboarding previews ownership and sharing; selection does
   not create files, scan other repositories or start agents.
2. **Explain and review a rule.** Distinguish a personal preference from a project
   contract, see its source/version and scope, inspect conflicts, then deliberately
   confirm/reject a proposal. Editing a reviewed value requires a new review.
   Planned repository-contract changes use a human-reviewed diff and the project's
   review process; a local approval never silently rewrites a checkout.
3. **Give an agent scoped context.** Use the existing project-bound two-tool MCP
   interface. For an optional managed Agent Client Protocol (ACP) session, review
   Start and separately opt into session context through the current native operator
   CLI. UI Start currently
   shares no MCP context. Authentication, context sharing and permission approval
   are separate decisions.
4. **Resume through Handoff.** Capture objective/next action and exact historical
   checks. The receiver compares current project/HEAD/branch/worktree and available
   source versions, inspects conflicts, and reruns required checks. Planned frozen
   constraints describe the old session; they do not override current rules or
   authorize a new session.
5. **Assess a result.** Inspect who reported it, who observed it, what was checked,
   on which source/platform, and what remains unknown. A daemon count can establish
   a context read, not that the model followed the returned rules or passed tests.

The domain and trust contract is specified in [ADR-0004](adr/0004-product-domain-trust.md);
the planned navigation is in [UI information architecture](product-ui.md).

## Integration and extension boundaries

Core product capabilities are context resolution, explicit review, continuity and
explanation. MCP is the existing read-only delivery integration. ACP provides an
optional operator-controlled single native session. The daemon owns lifecycle and
lease authority; closing a UI does not stop it. Neither transport is the domain
model, and neither grants OS isolation.

Keep the P1-I lease admission/publication checks, expiry and revocation unchanged.
See the [lease contract](p1/session-context-bridge.md). A proposal-only MCP extension
would require a future ADR with scoped authority and human review; J adds no tools.

The [agent-control assessment](agent-control-assessment.md) adopts useful concepts,
adapts ownership and trust semantics, and rejects copying its implementation.

## Non-goals and claims

- No replacement Agent harness, general terminal multiplexer, fleet scheduler,
  automatic worktree orchestration or autonomous repository rewriting.
- No preference auto-confirmation, chat-history mining, automatic raw transcript
  ingestion, hidden telemetry, credential scraping or background model calls.
- No cloud account, remote sync, multi-user collaboration, enterprise enforcement,
  vector database or third-party memory-service dependency in this scope.
- No OS sandbox, trusted execution attestation, digital-signature claim, universal
  MCP/ACP certification or guaranteed vendor behavior.
- No macOS/mobile/full IME/screen-reader support claim inferred from compilation,
  old smoke tests, upstream platform lists or theme contrast checks.

The product remains at `0.0.x-rc` validation stage (crate versions are implementation metadata). Windows Desktop/
ConPTY and native WSLg/TUI have scoped local evidence; broader release, packaging,
accessibility and current macOS validation remain open. See [ADR-0003](adr/0003-ui-framework-selection.md)
for platform distinctions and [P1-I evidence](evaluation/p1-i-context-bridge.md)
for independent Kimi tool-call observations and incomplete OpenCode results.

## Acceptance and next steps

J is complete only as a reviewed documentation/ADR baseline with consistent
positioning, ownership, review, trust labels and UI direction. Implementation
requires the staged [roadmap and task briefs](roadmap.md), each with acceptance
criteria before code. Cross-agent interoperability belongs solely to
[Issue #31](https://github.com/ybakiame/agent-control-plane/issues/31), after J is
reviewed and merged; its outstanding checks are not completed by this charter.
