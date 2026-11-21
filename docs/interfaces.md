# Interfaces and current conformance boundary

## Native local operator

`precept` talks to an explicitly addressed native daemon through authenticated
IPC v2. It supports Profile create/select/info, independent write enable/disable,
exact scoped Preference/Handoff previews and typed mutations. Use its command help
for current syntax. Operator control/read is distinct from the
profile/generation/epoch-bound write credential. Only the daemon invokes the
private writer. No SQL/JSON patch, default Profile, automatic grant/startup,
generic bridge forwarding or direct database fallback.

Preview methods `profile.preferences.list`, `profile.preferences.get` and
`profile.handoff.get` are Operator-only, bounded and exact-scope/revision. They are
not exposed to agents, OperatorWrite peers or MCP.

`suggest_preference` carries `key`, `value`, `tags` and `origin`. `tags` is one to
five canonical tags, sorted with no repeats: lowercase letters (any script), digits
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
level). All three are part of the proposal digest. Scope `environment` (with a
project) holds this machine's notes for that project: advisory only, never
competing with preferences or contracts, listed after them. `set_preference_enabled
{id, scope, enabled, channel, expected}` switches a confirmed rule off or on
without a new review; it records an `enable`/`disable` decision, and an off rule is
never delivered. The CLI exposes `--scope environment`, `--agent`, `--language`,
`--path`, `--task-tag` (repeatable), `--priority`, `--enforcement` and
`preference-enable` / `preference-disable`.

Confirm, reject and deprecate decisions take an optional operator note (`note`;
CLI `--note`; at most 500 characters), kept with the decision and returned in its
history. `revise_suggestion {id, scope, proposal_digest, key, value, tags, rule,
channel, expected, note?}` replaces a suggestion with the operator's edited version
in one write: the new record is `suggested` with origin `operator` and still needs
confirmation; the old one becomes `rejected` through a `replace` decision naming
`replaced_by`. Only a suggestion can be revised. Agents have no edit path.
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
a rule applies and lists environment notes under "Work environment".

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
(`~/.claude/settings.json`), Codex (`~/.codex/hooks.json`) and Antigravity
(`~/.gemini/config/hooks.json`) in the daemon user's home: `file`, `exists`, `sha256`, `malformed` (not a JSON object), `installed`,
`current` (installed exactly as written now) and the exact `command`, or
`available: false` when no `precept-hook` ships beside `precept-mcp`.
`integration.hooks.apply {project, vendor, enabled, expected_sha256}` adds
(`enabled: true`) or removes the project's one Precept entry under
`hooks.SessionStart` (`{"type":"command","command":…,"timeout":10}`) only if the
file still has the reported digest, otherwise `Conflict`. Antigravity's file maps
hook names to events, so its entry is the named hook `precept-<project>` holding
`SessionStart: [{"type":"command","command":…,"timeout":10}]`. A handler is Precept's
when its command runs `precept-hook` with `--project <name>`; every other key, hook
and their order are kept, the previous file is kept as `<file>.precept.bak`,
writes are atomic, a new file is owner-only. The command passes `--root`, so the
hook prints nothing in sessions outside the project. Codex asks the user to
approve a new hook once.

The same methods take `event: "pre_tool"` (default `session_start`) for the
pre-tool gate entry: its command adds `--event pre-tool`, and it sits under
`PreToolUse` in a group with a tool matcher (Claude Code
`Bash|Write|Edit|MultiEdit|NotebookEdit`, Codex `Bash|apply_patch`, Antigravity
`run_command|write_to_file|replace_file_content|multi_replace_file_content`).
Session-start and gate entries are added and removed independently. The status
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
outcome `allow`, `deny` or `off`.

Operator-only `integration.mcp {project}` reports, for Claude Code
(`~/.claude.json`, `mcpServers`), Codex (`~/.codex/config.toml`,
`[mcp_servers."precept-<project>"]`) and Antigravity
(`~/.gemini/config/mcp_config.json`, `mcpServers`) in the daemon user's home:
`file`, `server`, `exists`, `sha256`, `malformed`, `installed` and `current` (the
entry starts this daemon's `precept-mcp` for the project exactly), or
`available: false` without `precept-mcp`. `integration.mcp.apply {project,
client, enabled, expected_sha256}` adds, updates in place or removes only the
`precept-<project>` entry if the file still has the reported digest, otherwise
`Conflict`; an entry already exact is left untouched. JSON files keep every other
key and their order. Codex's TOML is edited as text: only Precept's own table
goes or comes, other tables and their comments stay, and the result must parse
with exactly the intended entry, else nothing is written (an inline
`mcp_servers = {…}` is refused). Backups and atomic writes as for hooks.

### Profile export and import

`precept profile export --out FILE` writes every preference of the selected Profile
(personal scope and each served project) to a new owner-only file (never
overwritten): versioned JSON `precept-profile-export` v2 with scope, key, value,
rule options, enabled state,
tags, origin and source status per record. Handoffs are not carried. `precept
profile import FILE --operation-prefix P` needs writes on and only ever creates
suggestions through the ordinary reviewed write path (`<P>-<index>` operation
ids, so an uncertain write can be retried exactly). It skips other statuses, records
the Profile already holds (same scope, key, value and tags) and project records
whose project is not served; the operator confirms imported suggestions again.
Version 1 files (before rule options) still import, with the defaults.
This is how preferences move between Profile schemas: there is no migration.

## MCP stdio

The Rust `precept-mcp` binary uses newline-delimited UTF-8 JSON-RPC; stdout contains only
protocol output and failures are sanitized. It uses an explicit protected native
credential and endpoint, with one bound project. Tool arguments cannot change scope.

Tools: `get_effective_context` and `get_handoff` (read-only), and `suggest_rule
{scope: project|environment|personal, key, value, tags?, applies_to?, enforcement?}`,
which calls agent `preference.suggest`. The daemon accepts it only when the
operator allowed suggestions for the project (`project.propose {name, enabled}`,
Operator; `precept project suggestions NAME on|off`; off by default, listed as
`propose` in `project.list`) and writes are on for this run; otherwise
`Unauthorized` or `Unavailable`, and nothing is queued. The record is created as
`suggested` with origin `agent` (an untagged one is tagged `agent`; environment
notes advisory only; priority 0). The same scope, key and value already suggested
or confirmed is `Conflict`; more than 20 per project per daemon run or 30 waiting
agent suggestions in a scope is `Capacity`. The tool's reply says the rule is not
active until the operator confirms it, and whether to retry. Each call is in the
activity log as `suggested` with its outcome.

Work sessions: `start_session {goal, conversation_id?, done?, doing?, todo?}`,
`update_work_state {session_id, goal?, done?, doing?, todo?}` (each given list
replaces the previous one) and `close_session {session_id, summary?, todo?}` call
agent `session.start` / `session.update` / `session.close`. They are accepted
only where the operator allows the project's agents to write
(`project.propose`), need no write grant and never touch the Profile. A start
with a conversation id returns the agent's open session for it, if any. Bounds:
goal 500 characters, 12 items of 200 characters per list, summary 1000, 20
sessions per project (closed ones are dropped first). A closed session takes no
updates (`InvalidTransition`); an unknown id is `NotFound`. Agent and operator
`context.get` add `work_sessions` (at most five: open first, then recently
closed, newest first), which the session-start briefing renders as **Work in
progress**. Operator `sessions.list {project}` returns every session and
`sessions.delete {project, id}` removes one. Operator-write `sessions.handoff
{project, id, task_id}` saves a session as a new Handoff through the ordinary
reviewed write (`put_handoff`, expected absent, so a taken task id is `Conflict`):
the goal as objective, done items and summary as completed, todo items as next
action, no validation entries, and the repository's revision, branch, state and
worktree fingerprint read by the daemon at that moment, never supplied by the
caller. Calls are in the activity log as
`session`.

`initialize` returns `instructions` telling the agent to call
`get_effective_context` before planning or editing and again after compaction,
and that repository files and contracts outrank preferences.

`precept-hook --vendor claude|codex|cursor|antigravity` (same location, project and capability
arguments as `precept-mcp`) is the read-only session-start hook. It reads the
harness's hook input (only `source`; `compact` adds a resume note), calls agent
`context.get` with the client label `<vendor> hook`, and prints a bounded (12 KiB)
Markdown briefing of the effective contracts and confirmed preferences in the
harness's shape: `hookSpecificOutput.additionalContext` for Claude Code and Codex,
`additional_context` for Cursor, and `injectSteps: [{systemMessage: {systemMessage}}]`
for Antigravity. With `--root`, a session whose working directory (the input's
`cwd`, else any of Antigravity's `workspacePaths`, else the hook's) is outside that
root gets nothing at all. Any
refusal or a 4 s timeout prints nothing on stdout, exits 0 and writes one
sanitized line to stderr (fail-open).

`precept-hook --event pre-tool` reads the PreToolUse input (Claude Code and Codex:
`tool_name` / `tool_input` with `Bash` `command`, `Write`/`Edit`/`MultiEdit`
`file_path`, `NotebookEdit` `notebook_path`, Codex `apply_patch` file headers;
Antigravity: `toolCall.name` / `toolCall.args` with `run_command` `CommandLine` and
`write_to_file` / `replace_file_content` / `multi_replace_file_content`
`TargetFile`), makes written paths relative to the project root (dropping paths
outside it) and asks `gate.check`. A denial prints
`hookSpecificOutput.permissionDecision: "deny"` with the reason for Claude Code and
Codex, and `{decision: "deny", reason}` for Antigravity; anything else prints
nothing (fail-open).

| Tool | Inputs | Outputs | Side effects |
| --- | --- | --- | --- |
| `get_effective_context` | optional boolean `include_documents`; optional `selector` object | compatible confirmed preferences, effective repository contracts, explicit resolution status, bounded metadata and opt-in allowlisted documents | Read-only |
| `get_handoff` | exact string `task_id` | saved task/revision and historical checks, current Git state and drift warnings | Read-only |

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
project as `.precept/rules/<id>.md` (path, exact text, `absent` / `same` /
`different`, current digest); `rules.export.apply` with `enabled` writes it only
to an absent path, guarded by the previewed digest, or takes back a file still
exactly as Precept wrote it. Nothing is committed; agents cannot call either.

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
  `profile.preferences.list|get`, `profile.review.queue|count|inspect`, `profile.handoff.get`),
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
