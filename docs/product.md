# Precept — product charter

**Precept is a local-first, developer-centric control plane for
engineering context, project rules and continuity across coding agents and
harnesses.**

Its purpose is to make the right context available to the right agent, explain why
it applies, preserve continuity across Handoffs and distinguish observation from
verification. “Control” means scoped context and explicit operator decisions; it
does not mean that a local application can enforce every action of a same-user
agent.

This charter defines durable product direction and boundaries. Implemented protocol
and platform details live in [architecture](architecture.md),
[interfaces](interfaces.md) and [security](security.md).

## User and problem

The primary user is a developer working in local Git checkouts with one or more
coding-agent clients. Changing clients, tasks or worktrees can lose constraints,
repeat explanations, apply stale preferences or turn an old test claim into a false
current PASS. The developer needs an inspectable account of the applicable project
contract, confirmed personal choices, remaining conflicts and next action.

The product is useful with one agent. It does not depend on running a fleet,
replacing the user's editor or harness, or storing conversations. External clients
consume scoped context.

## Product responsibilities

| Capability | Product responsibility | Boundary |
| --- | --- | --- |
| Project context | Bind an explicitly selected checkout and inspect current facts and contract sources | No directory discovery, automatic scaffolding or repository rewrite |
| Personal preferences | Maintain user-owned values with scope, review state and provenance | Suggestions never apply without exact-record operator confirmation |
| Project contracts | Explain repository-owned rules alongside code, configuration and current facts | Repository review owns changes; local approval is not a commit |
| Effective context | Explain inclusion, exclusion, masking, conflicts, uncertainty and source versions | Derived context never outranks its current sources |
| Suggestions | Let a human inspect a proposed durable change before it applies | Agents have no Profile mutation capability |
| Continuity | Carry objective, next action, revision and historical checks | Receivers must compare current state and rerun required checks |
| Provenance and evidence | Keep source authority, reporter, observer and verifier distinct | A transport observation is not model compliance or test verification |

Personal Profiles and review state belong in user-local SQLite. Project contracts
belong to the repository and its review process. Rust implements domain logic, the
daemon and local IPC; NSH Rust provides the shared Desktop/TUI operator experience.
The native daemon is the only Profile write owner.

## Primary workflows

1. **Select a project and inspect its context.** Explicitly register or select a
   checkout, inspect repository state and sources, and request document bodies only
   when needed. Selection does not scan other repositories, create files or start
   agents.
2. **Explain and review a rule.** Distinguish a personal preference from a project
   contract, inspect source/version/scope and conflicts, then deliberately confirm
   or reject a proposal. Editing a reviewed value requires a new review. A project
   contract change still requires the repository's human review process.
3. **Give an agent scoped context.** Use the project-bound two-tool MCP interface.
4. **Resume through Handoff.** Capture objective, next action and exact historical
   checks. The receiver compares current project, HEAD, branch, worktree and
   available source versions, inspects conflicts and reruns required checks.
5. **Assess a result.** Inspect who reported it, who observed it, what was checked,
   on which source and platform, and what remains unknown. A daemon context-read
   count does not establish that a model followed rules or that tests passed.

## Authority and ownership

Current repository facts and project contracts outrank personal preferences and
Handoffs. Facts describe what exists; contracts describe what is required. A
code/contract disagreement is surfaced for human resolution rather than silently
resolved. Session constraints may narrow work but never override contracts or grant
permissions. Stale snapshots cannot preserve obsolete permissions or hide new
repository rules.

Repository-owned contract files and reviewed engineering configuration remain the
source of truth. SQLite may hold user-local preferences, reviews and bounded source
metadata, but never a hidden authoritative copy of repository contracts. Document
contents are shared only through explicit, bounded reads.

## Integration boundaries

MCP is the read-only context delivery integration. Precept does not launch or supervise
agents; closing a UI does not stop the daemon. MCP is not the domain model and
grants no OS isolation.

A future proposal-only agent extension, evidence hash chain or signature system
requires a new explicit design and authority decision. None follows automatically
from the existing read tools, receipts or Handoff checksums.

## Non-goals and claims

- No replacement agent harness, general terminal multiplexer, fleet scheduler,
  automatic worktree orchestration or autonomous repository rewriting.
- No preference auto-confirmation, chat-history mining, automatic raw transcript
  ingestion, hidden telemetry, credential scraping or background model calls.
- No cloud account, remote sync, multi-user collaboration, enterprise enforcement,
  vector database or third-party memory-service requirement.
- No OS sandbox, trusted execution attestation, digital-signature claim, universal
  MCP/Precept certification or guaranteed vendor behavior.
- No platform, accessibility, device, model or clean-machine support claim inferred
  from compilation, headless tests or historical evidence.

Precept remains at validation stage. Packaging, installation, release and real-profile
activation require their own explicit decisions and current evidence.
