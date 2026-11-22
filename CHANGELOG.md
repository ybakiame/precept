# Changelog

Precept follows [semantic versioning](https://semver.org/). While the version is
0.x, a minor release may change the Profile schema or the local interfaces; move
data between schemas with the versioned Profile export and import.

## Unreleased

- **Project memory.** Agents keep what they learn in a project — this machine's
  environment, cautions, lessons and attempts — with the `remember`, `recall`,
  `reaffirm` and `forget` tools, so it survives a change of agent, harness or
  model. Unreviewed and below rules, weighted (environment first, pins,
  confirmations, fading), carried in the session briefing, and yours to pin,
  edit, restore or delete on the new Memory page.
- **Environment notes are project memory.** The Profile's `environment` scope,
  its Review tab and `--scope environment` are gone (the Profile schema
  changes: export with the previous version, import with this one; environment
  records in an export are skipped).
- A memory note can be proposed as a project convention from the Memory page.
- `precept profile export --with-memory [--with-environment]` carries project
  memory (export format v3); import adds it to the served projects.

## 0.4.0

- **Rules reach agents by themselves.** Session-start hooks for Claude Code, Codex
  and Antigravity brief each session (and again after compaction) with the
  confirmed rules, repository rules and open work.
- **One `precept` MCP server** for every registered project, bound to the
  session's folder; nothing is served outside a registered project.
- **Agents give back, you decide.** Where you allow it, agents suggest rules and
  updates of existing ones and record their work sessions; suggestions wait in
  Review, where an update shows its change as a diff.
- **Edit confirmed rules.** The new version applies at once and the earlier text
  stays in the record's history.
- **Repository rules** in `.precept/rules/*.md`: committed files apply to every
  agent, carry checks against the repository (verified or failing), and can be
  written from a confirmed convention and updated from it later.
- **Opt-in pre-tool gate**: hard rules with guards deny matching shell commands
  and file writes in the agent's harness; every decision is recorded.
- **Work sessions and evidence**: what agents did per session, kept as a
  tamper-evident chain, and a session saved as a Handoff.
- **Import from AGENTS.md** as suggestions, and the AGENTS.md pointer section.
- **Doctor** checks every link an agent needs, including how each agent is
  connected and the state of repository rules and the gate.
- Desktop: the sidebar can be hidden (Ctrl+B), the window has a least size, and
  the version shows in the top bar.
