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
(`~/.claude/settings.json`) and Codex (`~/.codex/hooks.json`) in the daemon user's
home: `file`, `exists`, `sha256`, `malformed` (not a JSON object), `installed`,
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

### Profile export and import

`precept profile export --out FILE` writes every preference of the selected Profile
(personal scope and each served project) to a new owner-only file (never
overwritten): versioned JSON `precept-profile-export` v1 with scope, key, value,
tags, origin and source status per record. Handoffs are not carried. `precept
profile import FILE --operation-prefix P` needs writes on and only ever creates
suggestions through the ordinary reviewed write path (`<P>-<index>` operation
ids, so an uncertain write can be retried exactly). It skips other statuses, records
the Profile already holds (same scope, key, value and tags) and project records
whose project is not served; the operator confirms imported suggestions again.
This is how preferences move between Profile schemas: there is no migration.

## Read-only MCP stdio

The Rust `precept-mcp` binary uses newline-delimited UTF-8 JSON-RPC; stdout contains only
protocol output and failures are sanitized. It uses an explicit protected native
credential and endpoint, with one bound project. Tool arguments cannot change scope.

`initialize` returns `instructions` telling the agent to call
`get_effective_context` before planning or editing and again after compaction,
and that repository files and contracts outrank preferences.

`precept-hook --vendor claude|codex|cursor` (same location, project and capability
arguments as `precept-mcp`) is the read-only session-start hook. It reads the
harness's hook input (only `source`; `compact` adds a resume note), calls agent
`context.get` with the client label `<vendor> hook`, and prints a bounded (12 KiB)
Markdown briefing of the effective contracts and confirmed preferences in the
harness's shape: `hookSpecificOutput.additionalContext` for Claude Code and Codex,
`additional_context` for Cursor. With `--root`, a session whose working directory
(the input's `cwd`, else the hook's) is outside that root gets nothing at all. Any
refusal or a 4 s timeout prints nothing on stdout, exits 0 and writes one
sanitized line to stderr (fail-open).

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

The [repository contract boundary](architecture.md#repository-contracts-and-context-resolution)
specifies the optional exact `ACP.contracts.md` source. It is enforced even when
`include_documents` is false. `contract_resolution.status` distinguishes `ready`,
`partial`, `not_configured`, `invalid_source`, `source_unavailable`, `source_changed`.
Whole-source refusal returns empty `preferences` and `effective_contracts`; partial
resolution suppresses affected unresolved keys only. Absence does not mean ordinary
AGENTS/ADRs have no authority. Each projected preference carries its `tags`, so an
agent can find the ones relevant to a task; tags never remove a confirmed preference
from the context. Agent projections omit origin, review history, losing values and
raw typed explanations. No repository files are
created or modified by this read path.

### Agent activity

The daemon keeps an in-memory log of agent requests for the operator (the newest
500 events, emptied on restart, never written to disk): `precept-mcp` starting
(`daemon.health`), `context.get` and `handoff.get` with their project, arrival
(`mcp`), duration and outcome code, plus refused handshakes without a
project. A context read records only counts (preferences, contracts, documents)
and the contract-resolution status. Never values, selectors, task ids, paths,
tokens or response bodies. `precept-mcp` forwards the MCP client's self-reported
`clientInfo` name and version as an optional `client` label on agent `context.get`
and `handoff.get` (bounded, printable, never authentication). Operator-only
`activity.list` takes optional `project` and `before` (an event id) and returns
`daemon_started_ms`, `capacity`, up to 200 events newest first and `more`; a
project filter includes refused handshakes. `precept activity` prints it.

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
  `mutate` (one OperatorWrite `Mutation`), `project_write` (`project.add|remove`),
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
