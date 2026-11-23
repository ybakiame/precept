# Interfaces and current conformance boundary

## Native local operator

`precept` talks to an explicitly addressed native daemon through authenticated
IPC v2. It supports Profile create/select/info, independent write enable/disable,
exact scoped Preference/Handoff previews and typed mutations. Use its command help
for current syntax. Operator control/read is distinct from the
profile/generation/epoch-bound write credential. Only the daemon invokes the
private writer. No SQL/JSON patch, default Profile, automatic grant/startup,
generic bridge forwarding or direct database fallback.

Preview methods `profile.preferences.list`, `profile.preferences.get`,
`profile.handoff.get` and `profile.handoff.list {project}` (newest first, at most 50: task, objective's first line, branch, revision, saved time) are Operator-only, bounded and exact-scope/revision. They are
not exposed to agents, OperatorWrite peers or MCP.

`suggest_preference` carries `key`, `value`, `tags` and `origin`. `tags` is one to
ten canonical tags, sorted with no repeats: lowercase letters (any script), digits
and inner hyphens, 1–32 characters each. A non-canonical list is refused, never
rewritten. `origin` is exactly `operator` or `agent`. Both are part of the proposal
digest. `precept preference-suggest` normalizes `--tag` (repeatable, required) and
defaults `--origin operator`.

Every preference is a rule. `suggest_preference` also carries `rule`: `applies_to`
(`agents`, `languages`, `paths`, `task_tags`; each canonical — sorted, no repeats,
at most 16, labels lowercase without spaces, paths relative without `..`, `*`, `:`
or `\\`, a trailing `/` covering a folder; empty lists apply everywhere),
`priority` (−100..100, higher wins among one key's rules of one scope) and
`enforcement` (`advisory`, `verify`, `hard`; a declaration, never an achieved
level). All three are part of the proposal digest. Scopes are `global` and
`project`; facts about this machine are project memory, not rules.
`set_preference_enabled
{id, scope, enabled, channel, expected}` switches a confirmed rule off or on
without a new review; it records an `enable`/`disable` decision, and an off rule is
never delivered. The CLI exposes `--agent`, `--language`,
`--path`, `--task-tag` (repeatable), `--priority`, `--enforcement` and
`preference-enable` / `preference-disable`.

Confirm, reject and deprecate decisions take an optional operator note (`note`;
CLI `--note`; at most 500 characters), kept with the decision and returned in its
history. `revise_preference {id, scope, proposal_digest, key, value, tags, rule,
channel, expected, note?}` replaces a record with the operator's edited version in
one write, with origin `operator`. For a suggestion the new record is `suggested`
and still needs confirmation, and the old one becomes `rejected`. For a confirmed
rule that is switched on, the edit is the operator's reviewed change: the new
record is `confirmed` at once (other confirmed records of its key are demoted), and
the old one becomes `deprecated`, keeping the earlier text. Either way the old one
gets a `replace` decision naming `replaced_by` and the note. A switched-off rule is
switched on before it is edited. Agents have no edit path; they propose updates
(`suggest_rule` with `replaces`).
`decide_batch {decision: confirm|reject, items: [{id, scope, proposal_digest,
expected}], channel, note?}` decides 1 to 50 exact suggestions in one write, each
as its own decision with the note; any stale item, or two items touching one record
(two suggestions of one key in a confirm batch), refuses the whole batch.
`suggest_batch {items: [{scope, key, value, tags, origin, rule}]}` files 1 to 50
suggestions in one write (an import the operator picked); none is confirmed.

Operator-only `import.scan {project}` reads the served project's `AGENTS.md` (the
instruction file agents share; a regular file only, never a link, at most 256 KiB)
and returns `candidates`, one per `##` section or per `###` inside one, outside
Precept's own block and code fences (`source`, `title`, `key` `agents-…`, `value`,
tags `agents-md` and `imported`, default `rule`, project `scope`, and `exists`: the
Profile already holds that key and value suggested or confirmed), and `skipped`
sections (`too_long`, `empty`, `unreadable`). Nothing is written; AGENTS.md is not
changed. Precept's own `.precept/rules` is never an import source.

Effective context keeps a confirmed, enabled rule unless the request's selector
definitely mismatches its `applies_to` (a dimension the request does not name
keeps it); each projected preference carries `applies_to`, `priority` and
`enforcement` when they are not the defaults. The session-start briefing says where
a rule applies.

### AGENTS.md pointer

Operator-only `integration.pointer {project}` reports the served project's
`AGENTS.md`: `exists`, its `sha256`, `state` (`absent`, `current`, `outdated`) or
`malformed` (unbalanced markers), and the exact `block`. `integration.pointer.apply
{project, enabled, expected_sha256}` adds or updates (`enabled: true`) or removes
the block only if the file still has the previewed digest (`null` for absent),
otherwise `Conflict`. The block is fixed text between `<!-- precept:begin v1 -->`
and `<!-- precept:end -->`; everything outside the markers is kept byte for byte,
a block-only file is deleted on removal, and writes are atomic and keep the file
mode. Links, non-regular and oversized (> 1 MiB) files are refused. The daemon never
commits or writes any other file. `precept project pointer` exposes both.

### Session-start hooks in the agents' configuration

Operator-only `integration.hooks {project}` reports, for Claude Code
(`~/.claude/settings.json`) and Codex (`~/.codex/hooks.json`), the two harnesses
Precept sets up itself, in the daemon user's home: `file`, `exists`, `sha256`, `malformed` (not a JSON object), `installed`,
`current` (installed exactly as written now) and the exact `command`, or
`available: false` when no `precept-hook` ships beside `precept-mcp`.
`integration.hooks.apply {project, vendor, enabled, expected_sha256}` adds
(`enabled: true`) or removes the project's one Precept entry under
`hooks.SessionStart` (`{"type":"command","command":…,"timeout":10}`) only if the
file still has the reported digest, otherwise `Conflict`. A handler is Precept's
when its command runs `precept-hook` with `--project <name>`; every other key, hook
and their order are kept, the previous file is kept as `<file>.precept.bak`,
writes are atomic, a new file is owner-only. The command passes `--root`, so the
hook prints nothing in sessions outside the project. Codex asks the user to
approve a new hook once.

The same methods take `event: "pre_tool"` (default `session_start`) for the
pre-tool gate entry: its command adds `--event pre-tool`, and it sits under
`PreToolUse` in a group with a tool matcher (Claude Code
`Bash|Write|Edit|MultiEdit|NotebookEdit`, Codex `Bash|apply_patch`).
`event: "post_read"` (Claude Code only; other vendors are `Unsupported`) adds the
stale-document warning: `--event post-read` under `PostToolUse` with matcher
`Read`, listed under `read_hooks`. `event: "pre_compact"` (Claude Code only;
Codex is `Unsupported`) adds the work-state reminder
before the harness compacts the conversation: `--event pre-compact` under
`PreCompact`, listed under `compact_hooks`. Session-start, gate, read and
compact entries are added and removed independently. The status
lists them under `gate_hooks`, with `gate`, the project's switch. Operator-only
`project.gate {name, enabled}` turns the gate on or off (off by default; persisted
in daemon state; a project rejoins with it off).

Agent `gate.check {tool: "shell", command}` or `{tool: "write", paths}`
(repository-relative, at most 32) answers `{decision: "allow", gate: "off"}`
while the gate is off. Otherwise it reads the committed rule files and answers
`deny` with `reason` and `rule {id, key, file}` when a hard rule's guard matches
(a shell command containing its `deny` text, whitespace-normalized, or a written
path equal to, or under, one of its `paths`), else `allow`. Uncommitted rules
never deny. Each answer is a `gate` entry in the project's evidence chain with
outcome `allow`, `deny` or `off`; a denial also records the rule's key. A denial
carries `repeat`, the number of this rule's denials in the project in the last
24 hours including this one; from the second, `reason` adds that the agent should
stop retrying variations and follow the rule or ask the developer. Agent and
operator `context.get` add `gate_denials` (at most five rules denied in the
last 7 days: `rule`, `count`, `last_ms`, most frequent first), which the
session-start briefing renders as **Recently blocked**; `activity.list` events
carry `rule`, and the Sessions page shows it beside a denial.

Operator-only `integration.mcp {}` reports, for Claude Code
(`~/.claude.json`, `mcpServers`), Codex (`~/.codex/config.toml`,
`[mcp_servers."precept"]`) in the daemon user's home:
`file`, `server`, `exists`, `sha256`, `malformed`, `installed` and `current` (the
entry starts this daemon's `precept-mcp` exactly) and `legacy` (older per-project
`precept-<name>` entries launching `precept-mcp`), or `available: false` without
`precept-mcp`. `integration.mcp.apply {client, enabled, expected_sha256}` adds,
updates in place or removes the one `precept` entry, and removes the legacy
entries, if the file still has the reported digest, otherwise `Conflict`; an exact
entry with no legacy entries is left untouched. JSON files keep every other
key and their order. Codex's TOML is edited as text: only Precept's own table
goes or comes, other tables and their comments stay, and the result must parse
with exactly the intended entry, else nothing is written (an inline
`mcp_servers = {…}` is refused). Backups and atomic writes as for hooks.

### Profile export and import

`precept profile export --out FILE` writes every preference of the selected Profile
(personal scope and each served project) to a new owner-only file (never
overwritten): versioned JSON `precept-profile-export` v3 with scope, key, value,
rule options, enabled state,
tags, origin and source status per record. With `--with-memory` it also carries
the served projects' memory (project, kind, title, body, pinned), cautions,
lessons and attempts only; `--with-environment` adds this machine's environment
notes. Handoffs are not carried. `precept
profile import FILE --operation-prefix P` needs writes on and only ever creates
suggestions through the ordinary reviewed write path (`<P>-<index>` operation
ids, so an uncertain write can be retried exactly). It skips other statuses, records
the Profile already holds (same scope, key, value and tags) and project records
whose project is not served; the operator confirms imported suggestions again.
Records of the retired `environment` scope are skipped (`file_index`, reason
`environment`). Memory notes go to the projects the daemon serves through
`memories.import` (reported per project; a project it does not serve is
`not_served`), never through review. Version 1 files (before rule options) still
import, with the defaults.
This is how preferences move between Profile schemas: there is no migration.

## MCP stdio

The Rust `precept-mcp` binary uses newline-delimited UTF-8 JSON-RPC; stdout contains only
protocol output and failures are sanitized. One `precept` server, started with only
the runtime directory and endpoint, serves every registered project: it binds to the
project whose registered root holds the session's folder. The folder is the
client's MCP roots when the client offers them in `initialize` (the server asks
`roots/list` after `notifications/initialized` and again on
`notifications/roots/list_changed`), else the working directory the client started
it in; roots spanning more than one project, or none, bind nothing. To find the
project it asks each published agent capability for its own root (agent
`project.root`, which answers only the caller's project) and then uses only the
matching capability. Unbound, every tool answers that the folder is not a
registered project. Tool arguments cannot change scope.

Tools: `get_effective_context`, `explain_rule` and `get_handoff` (read-only), and `suggest_rule
{scope: project|personal, key, value, tags?, applies_to?, enforcement?,
replaces?}`, which calls agent `preference.suggest`. With `replaces` (the id of a
confirmed rule of that scope, from `get_effective_context`; else `NotFound`) the
suggestion is an update of that rule: it takes its key and, when not given, its
tags, where it applies, priority and enforcement; `key` may be left out.
Confirming it demotes the rule it updates. The daemon accepts it only when the
operator allowed suggestions for the project (`project.propose {name, enabled}`,
Operator; `precept project suggestions NAME on|off`; off by default, listed as
`propose` in `project.list`) and writes are on for this run; otherwise
`Unauthorized` or `Unavailable`, and nothing is queued. The record is created as
`suggested` with origin `agent` (an untagged one is tagged `agent`; priority 0). The same scope, key and value already suggested
or confirmed is `Conflict`. There is no separate per-run or waiting-queue count
limit. The tool's reply says the rule is not active until the operator confirms it,
and whether to retry. Each call is in the activity log as `suggested` with its outcome.

`explain_rule {key, selector?}` calls agent `context.explain` (also operator
`context.explain {project, key, selector?}`, CLI `precept explain PROJECT KEY
[--agent --path --language --task-tag]`): every Profile rule of that key in the
project's scopes, any status, and every repository rule file of that key, each
with `outcome` (`delivered` or `not_delivered`), `reason` (`delivered`,
`constrains_values`, `not_confirmed`, `switched_off`, `does_not_apply`,
`replaced_by_repository_rule`, `value_not_allowed`, `conflict`, `outranked`,
`uncommitted`) and a short `detail`, computed from the same projection
`context.get` serves. A key on `context.get`, or documents on
`context.explain`, is `InvalidRequest`.

Work sessions: `start_session {goal, conversation_id?, done?, doing?, todo?}`,
`update_work_state {session_id, goal?, expected_rev?, done?, doing?, todo?}`
(each given list replaces the previous one) and `close_session {session_id,
summary?, expected_rev?, todo?}` call agent `session.start` / `session.update` /
`session.close`. They are accepted only where the operator allows the project's
agents to write (`project.propose`), need no write grant and never touch the
Profile. A start with a conversation id returns the agent's open session for it,
if any. Work items are objects `{id?, text, depends_on?}`: the id is a stable
lowercase label (letters, digits, hyphens, at most 24 characters; assigned when
omitted), unique within its list; a `depends_on` entry names another item of the
same list that must finish first (never the item itself). Every session carries
`rev`, counting its writes; an update or close naming an `expected_rev` that no
longer matches is `Conflict`, never a silent overwrite of another agent's write.
Bounds: goal 500 characters, 12 items of 200 characters per list, summary 1000,
20 sessions per project (closed ones are dropped first). A closed session takes
no updates (`InvalidTransition`); an unknown id is `NotFound`. Agent and operator
`context.get` add `work_sessions` (at most five: open first, then recently
closed, newest first), which the session-start briefing renders as **Work in
progress** (the items' text). Operator `sessions.list {project}` returns every
session and `sessions.delete {project, id}` removes one. Operator-write
`sessions.handoff {project, id, task_id}` saves a session as a new Handoff
through the ordinary reviewed write (`put_handoff`, expected absent, so a taken
task id is `Conflict`): the goal as objective, done items' text and summary as
completed, todo items' text as next action, no validation entries, and the
repository's revision, branch, state and worktree fingerprint read by the daemon
at that moment, never supplied by the caller. Calls are in the activity log as
`session`.

Project memory: `remember {kind, title, body}`, `recall {query?, kind?, limit?,
index?}` (or `recall {id}` alone, which fetches one note's full text through
agent `memory.get {id}` without counting a recall), `reaffirm {id, basis?}` and
`forget {id}` call agent `memory.remember` / `memory.recall` / `memory.get` /
`memory.reaffirm` / `memory.forget`. Kinds: `environment`
(facts about this machine), `caution`, `lesson`, `attempt`. Memory is agents'
own notes, unreviewed: accepted only where the operator allows the project's
agents to write (`project.propose`), needing no write grant, kept in the
daemon's private state (`memory-<project>.json`) and never in the Profile.
Bounds: title 120 characters, body 2000, 300 notes per project (past it the
lightest unpinned note goes). `remember` with a kind and title (compared without
case or repeated spaces) already kept updates that note: the same body
reaffirms it (one more confirmation), another body corrects it (the earlier
body joins its history, at most five; confirmations restart; a pin stays).
Weight: environment 100, caution 60, lesson 40, attempt 20; pinned +1000; +4
per confirmation (at most 10); +1 per recall (at most 20); +10 while the last
correction is the operator's; after 60 days without confirmation, −0.5 per day
down to nothing. Three changes in seven days mark a note `unstable` (shown, never
weighted). `recall` splits the query into terms (its words, lowercase; a run of
more than two CJK characters also gives its overlapping pairs; at most 32),
returns notes matching at least one term, ranked by matched terms (a title match
counting double) and then weight, at most 20, and counts a recall for each
returned note. With `index: true` the matches come back as one-line summaries
(`id`, `kind`, `title`, `weight`, no body) for the agent to pick from before
fetching full text by id. Every read carries `unverified_days` (days since the
last confirmation); from 7 days it also carries a `notice` telling the agent
to verify the note against the current code before relying on it. `reaffirm`
may state a `basis` (at most 300 characters: what verified the note), kept on
the note until a correction. A caution, lesson or attempt confirmed three times is filed once
as a suggested project rule through the agent-suggestion path (key from its
title, its text as the value, tags `<kind>` and `memory`; only where agents may
suggest and writes are on; never confirmed); the note records it as `proposed`.
Agent and operator `context.get` add `memory`: counts by kind, every
environment note (at most 8) and the five heaviest others that are not fading,
at most 2.5 KiB of text; the session-start briefing renders it as **Project
memory** in its own budget of at most 3 KiB after the rules. Operator
`memories.list {project}` returns every note with its weight and flags;
`memories.edit {project, id, title?, body?, pinned?}` (a body change is the
operator's correction; a taken title is `Conflict`), `memories.restore
{project, id, index}`, `memories.delete {project, id}` and `memories.clear
{project}` change it; `memories.import {project, entries:[{kind, title, body,
pinned?}]}` (at most 300) keeps each note as `remember` would, author `import`,
pins kept. Agent calls are in the activity log as `memory`.

Other harnesses connect themselves: `precept skill [--project NAME] [--out DIR]`
(operator; reads `project.list` and `project.mcp`) prints the `precept-connect`
skill (`skills/precept-connect/SKILL.md`) with this installation's MCP command
and, for each shared project (or `--project`), its hook commands in Claude
Code's shape (`--vendor claude --client <harness>`); with `--out` it writes
`DIR/SKILL.md` once (`Conflict` if it exists). The skill tells the agent to
change its own configuration only with the developer's approval, to use hooks
only where its harness documents Claude Code's hook format, and otherwise to
connect MCP only. Precept does not report or manage such connections.

`initialize` returns `instructions` telling the agent to call
`get_effective_context` before planning or editing and again after compaction,
and that repository files and contracts outrank preferences.

`precept-hook --vendor claude|codex [--client NAME]` (same location, project and
capability arguments as `precept-mcp`) is the read-only session-start hook. It
reads the harness's hook input (only `source`; `compact` adds a resume note),
calls agent `context.get` with the client label `<vendor> hook` (`<NAME> hook`
with `--client`, for a harness that reads Claude Code's hook shape), and prints a
bounded (12 KiB) Markdown briefing of the effective contracts and confirmed
preferences as `hookSpecificOutput.additionalContext`. With `--root`, a session
whose working directory (the input's `cwd`, else the hook's) is outside that root
gets nothing at all. Any
refusal or a 4 s timeout prints nothing on stdout, exits 0 and writes one
sanitized line to stderr (fail-open).

`precept-hook --event pre-tool` reads the PreToolUse input (`tool_name` /
`tool_input` with `Bash` `command`, `Write`/`Edit`/`MultiEdit` `file_path`,
`NotebookEdit` `notebook_path`, Codex `apply_patch` file headers), makes written
paths relative to the project root (dropping paths outside it) and asks
`gate.check`. A denial prints `hookSpecificOutput.permissionDecision: "deny"`
with the reason; anything else prints nothing (fail-open).

`precept-hook --event post-read` reads a PostToolUse input for `Read`
(`tool_input.file_path`), makes the path relative to the project root (dropping
paths outside it) and asks agent `doc.check {paths}` (1–32 repository-relative
paths, no `..`, not absolute; not recorded in the evidence chain): the checks of
committed, unchanged rule files on those files that fail now, each `{rule, key,
file, failure}`. When there are any, it prints
`hookSpecificOutput.additionalContext` (event `PostToolUse`): the file may be
out of date, with each failing claim. Otherwise, and on any error, it prints
nothing.

`precept-hook --event pre-compact` (Claude Code) calls agent
`context.get` and prints `hookSpecificOutput.additionalContext` (event
`PreCompact`): the conversation is about to be compacted, so keep the work state
in the project's sessions — bring the open session current with
`update_work_state` (its id and `expected_rev` are named) or record one with
`start_session` — and call `get_effective_context` again after compaction. It
writes nothing itself: a Handoff stays the operator's write. On any error it
prints nothing (fail-open); other vendors print nothing.

| Tool | Inputs | Outputs | Side effects |
| --- | --- | --- | --- |
| `get_effective_context` | optional boolean `include_documents`; optional `selector` object | compatible confirmed preferences, effective repository contracts, explicit resolution status, bounded metadata and opt-in allowlisted documents | Read-only |
| `explain_rule` | string `key`; optional `selector` | every rule of the key with outcome and reason | Read-only |
| `get_handoff` | exact string `task_id` | saved task/revision and historical checks, current Git state and drift warnings | Read-only |
| `suggest_rule` | `scope`, `key`, `value`, optional `tags`, `applies_to`, `enforcement`, `replaces` | the suggestion's id; never active until confirmed | A `suggested` record, where allowed and writes are on |
| `start_session`, `update_work_state`, `close_session` | goal and done / doing / todo item lists (`{id?, text, depends_on?}`), optional `expected_rev`, summary | the session with its `rev` | Daemon-private session record, where allowed; a stale `expected_rev` is `Conflict` |
| `remember`, `recall`, `reaffirm`, `forget` | kind, title, body; query or note id (optional index flag); note id and optional basis | the note(s) or index lines | Daemon-private project memory, where allowed (`recall` counts recalls; a fetch by id does not) |

The optional `selector` contains only `agent_id`, `path`, `language`, `task_tags`.
These are caller-supplied applicability labels, never authentication or permission
grants. Missing labels are unknown for populated contract selectors; malformed
queries fail rather than widen applicability. Project/root/session overrides are
refused. Operator `context.get` supplies its authorized project separately; Agent
`context.get` derives it from the capability and accepts the same optional selector.

The [repository rule boundary](architecture.md#repository-contracts-and-context-resolution)
specifies `.precept/rules/`. It is enforced even when `include_documents` is false.
`repository_rules` lists the committed rules that apply (id, key, text, file,
sha256, tags, enforcement, optional title, allowed values, priority, applies_to,
guard count, and `checks`); `pending_rules` lists uncommitted files by id and key.
`contract_resolution.status` distinguishes `ready`, `partial`, `not_configured`,
`invalid_source` (with `invalid_files`), `source_unavailable`, `source_changed`.
Whole-source refusal returns empty `preferences` and `repository_rules`; partial
resolution suppresses affected unresolved keys only. Absence does not mean ordinary
AGENTS/ADRs have no authority. Each projected preference carries its `tags`, so an
agent can find the ones relevant to a task; tags never remove a confirmed preference
from the context. Agent projections omit origin, review history, losing values and
raw typed explanations. No repository files are
created or modified by this read path.

Operator `rules.export` previews writing one confirmed convention of a served
project as `.precept/rules/<id>.md`: path, exact text (key, `title` from the text's
first heading, tags, options, the text), the current text, its digest, and
`absent`, `same`, `different` (an update) or `invalid` (a file that does not parse).
`rules.export.apply` with `enabled` writes it, guarded by the previewed digest: a
new file, or an update of a file that parses, keeping the allowed values, checks
and guards only the file declares; an invalid file is never touched. Disabled, it
takes back a file still exactly as Precept would write it. Nothing is committed; agents cannot call either.

### Agent activity

The daemon keeps an in-memory log of agent requests for the operator (the newest
500 events, emptied on restart, never written to disk): `precept-mcp` starting
(`daemon.health`), `context.get` and `handoff.get` with their project, arrival
(`mcp`), duration and outcome code, plus refused handshakes without a
project. A context read records only counts (preferences, contracts, documents)
and the contract-resolution status. Never values, selectors, task ids, paths,
tokens or response bodies. `precept-mcp` forwards the MCP client's self-reported
`clientInfo` name and version as an optional `client` label on agent `context.get`
and `handoff.get` (bounded, printable, never authentication). `precept-mcp` also attaches the id of the work session it started
(`session`), which the daemon removes from the parameters and records as
evidence only. Operator-only `activity.list` takes optional `project` and `before`
(an entry number). With a project it returns that project's persisted evidence
(up to 200 entries newest first, each with `session` when known, plus recent
refused handshakes from memory), `more`, and `chain {ok, entries, dropped,
broken_at}` from verifying the whole chain; without one, the in-memory log since
the daemon started. `precept activity` prints it.

`precept-mcp` writes one sanitized line per failure to stderr (refused arguments,
calls before initialization, daemon errors by code), which MCP clients show in
their own server logs. Stdout stays protocol-only.

Implemented protocol subsets include initialize/initialized, ping, tools/list and
tools/call for 2025-11-25, plus the 2026-07-28 stateless per-request metadata mode.
This is not universal MCP certification. There are no write tools, resources,
prompts, HTTP API, subscriptions, batching or task augmentation. Independent Rust
process tests cover both supported versions, malformed calls, notifications and
project-bound negative cases. Historical client/model trials are separate evidence.

## Handoff evidence

Only the fresh Profile v1 schema is accepted. There is no legacy reader, migration
or nullable-column upgrade at startup. Operator Handoff put requires explicit
expected-absent or exact revision and a bounded typed file. Saved objective,
completed work, next action, validation JSON and snapshot remain historical.
Recognized check states are PASS/FAIL/SKIPPED/NOT_RUN; a saved PASS is not current PASS.

Fingerprint v1 detects bounded content drift alongside separate
HEAD/branch/dirty warnings. It stores a digest rather than
raw diffs, filenames or content. Missing data or exceeded bounds remain unverified.
Ignored files, submodule internals and external/device state are not attested.

## Retired interfaces

Retired prototype interfaces exist only in Git history and local historical
material. Current build and setup instructions are in the [README](../README.md).

## Windows GUI → WSL2 bridge

One long-lived `precept-bridge` process serves each active WSL connection,
started only by an explicit user action (connect, Reconnect, navigation, or an
Operator action) as `wsl.exe --distribution D --user U --exec /abs/precept-bridge
--runtime-dir R --endpoint E`. The distribution, user and three paths are validated
explicit inputs; project, task and request data never appear in argv.

- **Framing:** u32 big-endian length + JSON. Client→helper frames ≤ 256 KiB;
  helper→client ≤ 5 MiB. Empty or oversized frames end the stream.
- **Hello:** the helper's first frame is `{"version":1,"id":0,"ok":{"bridge":"precept-bridge","version":1}}`;
  any other identity or version refuses the connection.
- **Requests:** `{"version":1,"id":N,"op":{...}}`, strict fields, `id` ≥ 1, at most
  8 in flight. `op.type` is exactly one of `snapshot`, `control` (the Operator
  read/control allowlist: `profile.info|create|select|write.enable|write.disable`,
  `profile.preferences.list|get`, `profile.review.queue|count|inspect`, `profile.handoff.get|list`),
  `mutate` (one OperatorWrite `Mutation`), `project_write` (`project.add|register|remove`),
  `health` (daemon health with the Operator credential, for the GUI's live WSL
  status) or `cancel` (abandon an in-flight id; no reply of its own). Anything else is
  `Unsupported`/`InvalidRequest`. There is no generic IPC forwarding or shell.
- **Replies:** `{"version":1,"id":N,"ok":…}` or `{"version":1,"id":N,"error":"Code"}`,
  exactly one of the two. Error codes are a fixed vocabulary; the Windows side maps
  anything else to `Unavailable` and never displays helper text verbatim. An
  unsolicited, mismatched or ambiguous reply ends the bridge.
- **Authority:** capabilities load inside WSL. Operator control and the exact
  Profile/generation/owner-epoch-bound OperatorWrite capability stay separate. The
  helper never validates or approves a mutation, opens SQLite or links the Writer.
- **Uncertainty:** a mutation whose bridge times out or is lost after the request
  may have been delivered is `OutcomeUnknown`. Nothing is retried automatically.
  Only the explicit identical retry (same operation ID, binding and epoch) settles
  it; a retry that cannot be submitted keeps the uncertainty.
- **Status:** launch failures are classified from the launcher's exit status
  (`wsl.exe` -1: distribution or user, decided by `wsl.exe --list --quiet` only after
  a failure; 1: helper not executable; 2: helper argument refusal), plus incompatible
  hello, no response, daemon unreachable and runtime refused. The GUI shows the exact
  cause. A 2 s watcher reports a lost bridge or unreachable daemon without starting a
  bridge or process and without changing page data or Operator guards.
- **Lifecycle:** page reads, the status watcher and Operator calls reuse the live bridge; polling
  never starts one, so a stopped distribution or lost helper is an explicit
  unavailable state. Closing the GUI or leaving the WSL target closes the helper
  (stdin EOF; in-flight work finishes briefly) without stopping the WSL daemon or
  touching Profile, auth or runtime files.
- **Non-goals:** no implicit daemon start, Profile selection, write enable, agent
  launch or authentication by the bridge; no MCP writes.
