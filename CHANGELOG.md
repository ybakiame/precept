# Changelog

Precept follows [semantic versioning](https://semver.org/). While the version is
0.x, a minor release may change the Profile schema or the local interfaces; move
data between schemas with the versioned Profile export and import.

## Unreleased

- **Manual personal sync:** confirmed global preferences can move between paired
  devices through age-encrypted files with per-device keys or a passphrase.
  Sender signatures, owner-fingerprint pairing, vector clocks, explicit delete
  tombstones, selective import and conflict resolutions prevent silent overwrite.
  The daemon keeps private per-Profile sync state and applies content through its
  typed writer with pending/receipt recovery; no listener or automatic sync runs.
- **Review evidence:** the queue marks possible duplicates in loaded records
  without reordering or auto-selecting them. Detail shows source, affected files,
  bounded same-scope duplicate/same-key/same-value pointers, and retained project
  gate-denial count and last hit; exact-record and exact-revision decisions are
  unchanged.
- **Index-first briefings:** session-start briefings now list every applicable
  repository rule with its key, title, enforcement, check state and an explicit
  full-text pointer instead of spending the 12 KiB budget on rule bodies. Full
  text remains available on demand through `get_effective_context`, and shortened
  index metadata is marked explicitly.
- **Rule authoring diagnostics:** Operator-only `precept lint-rules PROJECT` and
  the Desktop Rules page report bounded, redacted syntax and semantic diagnostics
  with stable codes and line/column positions. Duplicate keys, identical canonical
  applicability plus priority, and currently missing path selectors are
  conservative warnings; reports never expose source snippets, values or absolute
  paths. Rust, Git, security and testing templates are previewed and copied for
  repository review, never written automatically.

## 0.4.3

- **Structured work sessions:** one goal now coordinates flat milestone/task
  items with explicit state, session-wide acyclic dependencies, owner and blocked
  reason. Agents add, start, complete, defer, block, reopen, update or remove one
  item at a time with required CAS, read full state with `get_work_session`, and
  resume an open session from another conversation. Briefings, the Sessions page
  and Handoffs preserve item metadata; validation errors name the exact field.
- **Source modularization:** daemon IPC dispatch, MCP protocol/binding/session
  handling, daemon session model/store/API, CLI diagnostics/transfer, UI domains
  and native acceptance stages now live in cohesive modules instead of oversized
  entrypoint files; every maintained Rust source is below 800 lines.
- **Native acceptance:** the harness no longer pulls the unused `unicode-width`
  dependency.
- **Multi-source rule import:** Review can preview known AGENTS.md, CLAUDE.md,
  Cursor, Continue and Copilot rule files from one registered project. The
  daemon uses fixed non-recursive paths, refuses links, bounds reads and output,
  preserves source provenance, and reports malformed metadata, unsupported
  glob/manual activation, duplicates and budget exhaustion explicitly. Selected
  items remain suggestions until exact operator confirmation; source files are
  never changed.

## 0.4.2

- **Guided activation and diagnostics:** `precept setup --plan` previews the
  first-activation steps, and `precept setup` asks before each change.
  `precept doctor --json` and the Windows package verifier report redacted,
  read-only installation diagnostics with stable reasons and remediation.
- **Safer Profile transfer:** `precept profile import FILE --preflight` reports
  duplicates, skips, omissions and destination state without writing. Apply
  rechecks the destination and still creates only suggestions for exact review.
- **Bounded repository checks:** one context resolution shares checked-file and
  parsed TOML/JSON reads, with 256 KiB per file, 64 files and 1 MiB in aggregate.
  Exhausted checks are unavailable, never verified, and `check_io` reports usage.
- **Separated Windows data directories:** configuration uses
  `%LOCALAPPDATA%\precept\config`, daemon state uses `precept\state`, and runtime
  uses `precept\state\runtime`. Unix keeps XDG config/state defaults. Old data is
  never moved automatically; see the README for the manual upgrade steps.
- **Desktop and tooling fixes:** Doctor accepts equivalent Windows path separator
  spellings without widening scope; Personal records no longer remain stuck
  loading when a workspace snapshot completes during the first Profile read;
  MCP metadata matches the package version; pull requests run both workspace
  quality gates on Windows and Linux.
- Doctor distinguishes credentials that a stopped daemon has not issued from
  unsafe existing credentials, so a cleanly stopped runtime reports incomplete
  rather than a credential problem.

## 0.4.1

- **Two harnesses, done well.** Precept sets up and supports Claude Code and
  Codex. The integrations for Antigravity, Cursor, Qoder, Qoder CN, CodeBuddy
  and WorkBuddy are removed; remove their Precept entries from those tools'
  settings by hand if they were added.
- **Other agents connect themselves** with the `precept-connect` skill:
  `precept skill [--out DIR]` prints it with this machine's MCP command and each
  shared project's hook commands; `precept-hook --client NAME` names such a
  harness in the activity log.
- Memory recall gains an index mode and per-id fetch; notes idle a week or more
  carry a verify-before-relying notice. Session todo items gain ids,
  dependencies and a revision guard. Claude Code gets a reminder before
  compaction to keep its work session current.
- Windows: integration reads and hook paths use the Windows home; hook
  commands quote for cmd; MCP roots parse Windows drive URIs.
- Operator: recognize and explicitly start a local daemon on Windows or Linux;
  allow up to ten tags per rule and remove the agent-suggestion queue caps.
  Review pending suggestions separately from reviewed Rules, keep Workspaces
  independent, align list content, and consolidate daemon details in Diagnostics.
  Deprecated records are hidden until enabled, and destructive preference deletion
  displays and copies the exact ID.

## 0.4.0

The first published release. Everything below is new relative to earlier private
builds; their Profiles move here only through `precept profile export` and
`precept profile import` (environment records are skipped).

### Agents get the rules without asking
- **Session-start briefings** for Claude Code, Codex, Antigravity and Cursor:
  confirmed rules, repository rules, open work, recently blocked rules and
  project memory, again after compaction.
- **One `precept` MCP server** for every registered project, bound to the
  session's folder; nothing is served outside a registered project.
- **Why a rule applies, or not**: `explain_rule`, `precept explain` and
  `context.explain` list every rule of a key with its outcome and reason.

### Agents give back, you decide
- Where you allow it, agents **suggest rules** and updates of existing ones;
  suggestions wait in Review, where an update shows its change as a diff.
- **Edit confirmed rules**: the new version applies at once and the earlier text
  stays in the record's history.
- **Import from AGENTS.md** as suggestions, and the AGENTS.md pointer section.

### Memory and continuity
- **Project memory** that agents keep with `remember`, `recall`, `reaffirm` and
  `forget`: this machine's environment, cautions, lessons and attempts. It is
  unreviewed and below rules, weighted (environment first, pins, confirmations,
  fading) and yours to pin, edit, restore, delete or propose as a rule on the
  Memory page. A note that keeps holding is filed once as a suggested rule.
  Search matches words in any order and parts of Chinese phrases.
- **Work sessions and evidence**: what agents did per session, kept as a
  tamper-evident chain; a session can be saved as a Handoff, and the Handoff
  page lists the saved ones.
- `precept profile export --with-memory [--with-environment]` carries project
  memory (export format v3).

### Rules in the repository, and enforcement
- **Repository rules** in `.precept/rules/*.md`: committed files apply to every
  agent, carry checks against the repository (verified or failing), and can be
  written from a confirmed convention and updated from it later.
- **Opt-in pre-tool gate**: hard rules with guards deny matching shell commands
  and file writes (Cursor: shell commands only); every decision is recorded, and
  a repeated denial tells the agent to stop retrying variations.
- **Stale-document warning** in Claude Code: after reading a file whose committed
  claims fail, the agent is told which no longer hold.

### Desktop and tools
- **Doctor** checks every link an agent needs, including how each agent is
  connected and the state of repository rules and the gate.
- The sidebar can be hidden (Ctrl+B), the window has a least size, and the
  version shows in the top bar.
- The operator interfaces are the desktop app and the `precept` CLI; there is no
  terminal UI or web preview.
