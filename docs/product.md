# Precept — product charter

**Precept gives every coding agent you use reviewed, scoped engineering context —
your preferences and your project's contracts — through each agent's own
channels, and keeps an honest record of what was delivered, verified or enforced.**

Tagline: *reviewed context for coding agents, kept true.*

Its purpose is to make the right rules reach the right agent, explain why they
apply, keep them true against the real repository, preserve continuity when work
moves between agents, and never confuse delivery with verification or
enforcement. "Control" means scoped context, explicit operator decisions and
cooperative enforcement where a harness offers it; it does not mean a local
application can contain a same-user agent.

This charter defines durable product direction and boundaries. Implemented protocol
and platform details live in [architecture](architecture.md),
[interfaces](interfaces.md) and [security](security.md). A capability this charter
permits is not implemented until those documents describe it.

## User and problem

The primary user is a developer working in local Git checkouts with one or more
coding agents (Claude Code, Codex, Cursor, Antigravity and others). Rules live in
per-tool files and memories that drift apart; agents do not reliably ask for
context; documentation rots; switching agents loses the goal, the to-do list and
the constraints; an old test claim turns into a false current PASS. The developer
needs one reviewed account of what applies, delivered to every agent, with an
honest statement of what was actually checked.

The product is useful with one agent. It does not replace the user's editor or
harness, schedule a fleet or store conversations.

## The loop

Precept closes one loop. Each stage states what it achieved; none implies the next.

| Stage | What happens | Achieved claim |
| --- | --- | --- |
| Resolve | Select the rules that apply to this project, agent, path, language and task; record why each applied, lost, was filtered or disabled; surface conflicts | Explained |
| Reach | Deliver them through the agent's channels: MCP reads, session-start injection, a managed pointer in the repository's agent files | Delivered |
| Verify | Check rules and documentation claims against the real repository | Verified (only what a check covered) |
| Enforce | Where a harness has a pre-tool hook, deny calls that break a hard rule | Enforced (only calls the hook mediated) |
| Continue | Carry goal, work state, constraints and achieved levels to the next agent, which re-verifies | Historical until re-verified |

## Rules and scopes

Everything Precept delivers is a **rule** with an id, a body, a kind, tags,
applicability (agents, paths, languages, task tags), priority, an enabled state,
a declared enforcement intent (advisory, verify, hard), optional checks, an
origin and a review history.

| Scope | Lives in | Authority |
| --- | --- | --- |
| Personal | The user-local Profile | Operator review |
| Project preference | The Profile, keyed by project | Operator review |
| Project contract | The repository (reviewed rule files) | Repository review |
| Project memory (this machine's environment, cautions, lessons, attempts) | The daemon's private state, per project; never committed | Agents keep it; the operator edits or deletes it; unreviewed |

Current repository facts and project contracts outrank personal preferences,
project memory and Handoffs. A code/contract disagreement is surfaced for human
resolution, never silently resolved. Session constraints may narrow work but never
override contracts or grant permissions. Project memory is descriptive context:
it never supersedes a contract or preference.

Repository-owned contract files and reviewed engineering configuration remain the
source of truth. The Profile never holds a hidden authoritative copy of repository
contracts. Document contents are shared only through explicit, bounded reads.

## Agents: read and propose, never decide

Per project, the operator grants agents up to three capabilities: **read**
(context, explanations, conflicts, search, Handoffs), **propose** (create
suggestions; record their own session work state, evidence and Handoff drafts)
and **gate** (answer pre-tool checks). No agent capability confirms, rejects,
edits, deletes, enables or disables a reviewed record, registers a project or
changes an integration. Agent suggestions are durable only after the operator
confirms the exact record; editing a reviewed value is a new suggestion.

Automatic learning (for example, a repeated gate denial or test failure becoming a
suggested rule) may only create suggestions, never bind a rule.

## Connecting agents

Two channels carry rules to an agent; use both where the harness allows it.

| | Hooks only | MCP only | Hooks + MCP (recommended) |
| --- | --- | --- | --- |
| How rules arrive | A briefing at session start and after compaction, without the agent asking | When the agent calls `get_effective_context`; an agent that does not call gets nothing | The briefing always; details on demand |
| What the agent reads | Confirmed preferences, repository rules with their check results, open work sessions (at most 12 KiB, then cut), and project memory in its own 3 KiB | Everything, narrowed by path, language or task; documents on request; Handoffs | Both |
| Can the agent give back | No (read-only) | Yes: rule suggestions and updates, its work session, project memory | Yes |
| Continuity across agents and compaction | Sees work others left, cannot record its own | Records its work, may not read others' | The full loop |
| Harnesses | Those with a session-start hook: Claude Code, Codex, Antigravity, Cursor, Qoder, Qoder CN, CodeBuddy, WorkBuddy | Any MCP client | Claude Code, Codex, Antigravity, Cursor, Qoder, Qoder CN, CodeBuddy, WorkBuddy |

Add-ons, for the cases that need them:

| Add-on | What it adds | When to use it | Cost |
| --- | --- | --- | --- |
| Pre-tool gate | Denies shell commands and file writes that break a hard rule's guard (Cursor: shell commands only) | Things that must not happen (force pushes, editing a lock file), with those rules committed | One local question per mediated tool call; only calls routed through the hook; not a sandbox |
| Stale-document warning | After a file read, tells the agent when committed rule checks on that file fail | Documents that drift from the code, with checks committed | One local question per read; Claude Code only; never blocks |
| AGENTS.md pointer | Tells agents that read AGENTS.md to call the `precept` MCP server | Agents without a session-start hook, or MCP-only setups | Committed: teammates without Precept see a pointer to a server they do not have; best for solo repositories or teams that all use Precept |
| Rules in Git (`.precept/rules`) | Conventions that travel with the repository, reviewed like code, for every person and machine; checks and guards | Shared conventions; checks ("verified"); the gate, whose guards come only from committed rule files | Kept and committed in the repository; uncommitted files do not apply |

The gate depends on committed rules: Profile conventions never deny a call. The
pointer matters most without hooks; with hooks the briefing already tells the agent
where its context is.

A path that grows with need: start with hooks + MCP and rules in the Profile; commit
stable project conventions with checks when a team shares them; add guards and the
gate for what must never happen; use MCP with the AGENTS.md pointer for agents that
have no hooks.

## Integrations Precept writes

Precept may write, only on an explicit operator action with a preview, and always
revocably:

- a Precept-marked pointer block inside a repository's `AGENTS.md`, touching
  nothing outside its markers;
- Precept-owned hook groups and the one `precept` MCP server entry inside a
  harness's own configuration, touching nothing else;
- repository rule files under `.precept/rules/`, as a change the user commits
  through their own review.

It never commits, pushes, rewrites other content, scans for repositories or
installs integrations on its own. Project memory never enters a committed file.

## Assurance and honesty

A delivered rule is not a followed rule; a passing check covers only what it
checked; an enforced rule covers only the calls a hook mediated; a semantic or
model-judged assessment, if ever offered, is labelled as such and never counted
as Verified or Enforced. Hooks are cooperative compliance, not a security
boundary: an agent that does not run the hook, or a same-user process, is not
contained (see [security](security.md#hooks-and-enforcement)). Checksums and hash
chains detect alteration; they are not signatures and do not prove a reporter told
the truth.

## Primary workflows

1. **Set up a project.** Register a checkout explicitly, share it with agents,
   apply the integrations you choose from a preview, import existing rule files
   as suggestions.
2. **Review.** Inspect suggestions from you or agents with their source, scope,
   tags and conflicts; confirm or reject the exact record, singly or in bulk.
3. **Work with any agent.** Agents receive the applicable rules at session start
   and on request, and propose what they learn.
4. **Keep it true.** Run contract checks and documentation claims against the
   repository; stale documentation is reported to the agent and the operator.
5. **Hand over.** Close a session with its goal, work state and achieved levels;
   the next agent compares the current repository and re-verifies.
6. **Assess.** See who reported, observed, checked and enforced what, on which
   revision, and what remains unknown.

## Ownership and platform

Personal Profiles, review state, project memory and session records belong in
user-local storage owned by the native daemon, the only writer. Project contracts
belong to the repository. Rust implements domain logic, the daemon and local IPC;
NSH Rust provides the desktop operator experience. Precept does not
launch or supervise agents; closing a UI does not stop the daemon.

## Non-goals and claims

- No replacement agent harness, agent launcher, terminal multiplexer, fleet
  scheduler, automatic worktree orchestration or autonomous repository rewriting.
- No rule auto-confirmation, chat-history mining, raw transcript ingestion, hidden
  telemetry, credential scraping or background model calls.
- No cloud account or remote service requirement. Any future team sharing keeps
  runtime state out of repositories and private data local unless the user
  explicitly exports it.
- No OS sandbox, trusted execution attestation, digital-signature claim, universal
  MCP certification or guaranteed vendor behavior.
- No platform, accessibility, device, model or clean-machine support claim inferred
  from compilation, headless tests or historical evidence.

Precept remains at validation stage. Packaging, installation, release and real-profile
activation require their own explicit decisions and current evidence.
