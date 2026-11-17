# Interfaces and current conformance boundary

## Native local operator

`acp-query` talks to an explicitly addressed native daemon through authenticated
IPC v2. Profile create/select/info, independent write enable/disable, exact scoped
Preference/Handoff previews and all seven typed mutations are described in the
[operator contract and syntax](p1/native-operator-workflows.md).
Operator control/read is distinct from the profile/generation/epoch-bound write
credential. Only the daemon invokes the private writer. No SQL/JSON patch, default
Profile, automatic grant/startup, remote WSL writes or direct database fallback.

Preview methods `profile.preferences.list`, `profile.preferences.get` and
`profile.handoff.get` are Operator-only, bounded and exact-scope/revision. They are
not exposed to agents, sessions, OperatorWrite peers or MCP.

## Read-only MCP stdio

The Rust `acp-mcp` binary uses newline-delimited UTF-8 JSON-RPC; stdout contains only
protocol output and failures are sanitized. It uses an explicit protected native
credential and endpoint, with one bound project. Tool arguments cannot change scope.

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

The [accepted ProjectContract boundary](p1/project-contract-integration.md) specifies
the optional exact `ACP.contracts.md` source. It is enforced even when
`include_documents` is false. `contract_resolution.status` distinguishes `ready`,
`partial`, `not_configured`, `invalid_source`, `source_unavailable`, `source_changed`.
Whole-source refusal returns empty `preferences` and `effective_contracts`; partial
resolution suppresses affected unresolved keys only. Absence does not mean ordinary
AGENTS/ADRs have no authority. Agent projections omit private preference source,
review history, losing values and raw typed explanations. No repository files are
created or modified by this read path.

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

The [frozen Fingerprint v1](p1/worktree-fingerprint-v1.md) detects bounded content
drift alongside separate HEAD/branch/dirty warnings. It stores a digest rather than
raw diffs, filenames or content. Missing data or exceeded bounds remain unverified.
Ignored files, submodule internals and external/device state are not attested.

## Optional native sessions

The operator-only `session.*` extension uses session API v2 inside IPC v2. It needs
explicit trusted executable configuration and confirmed Start. Reads, prompt,
permission-once/deny, cancel, stop and opt-in context controls remain separate.
A Windows GUI reaches a WSL daemon's sessions only through the typed bridge below;
the daemon's executable configuration, policy and leases stay authoritative. See the
[session runtime](p1/agent-session-runtime.md) and [auth contract](p1/agent-authentication.md).
Authentication is observed readiness; no login RPC, credential UI or automatic retry.

Optional context uses a distinct expiring session credential for one
project/agent/generation. Admission and response publication recheck authority;
Stop, Cancel, failure, root validation failure, expiry and restart revoke it.
Established peers retain no authority after revoke, though already delivered bytes
cannot be recalled. Ordinary operator/project credentials have their own per-launch
semantics and never gain write authority. See the [lease contract](p1/session-context-bridge.md).

The retired prototype interfaces are available only in marked historical reports
and Git history. Current build/setup/verification is [Rust-only](p1/rust-only-development.md).

## Windows GUI → WSL2 bridge

P1-M5/S1-B. One long-lived `acp-wsl-bridge` process per active WSL connection,
started only by an explicit user action (connect, Reconnect, navigation, or an
Operator action) as `wsl.exe --distribution D --user U --exec /abs/acp-wsl-bridge
--runtime-dir R --endpoint E`. The distribution, user and three paths are validated
explicit inputs; project, task and request data never appear in argv.

- **Framing:** u32 big-endian length + JSON. Client→helper frames ≤ 256 KiB;
  helper→client ≤ 5 MiB. Empty or oversized frames end the stream.
- **Hello:** the helper's first frame is `{"version":1,"id":0,"ok":{"bridge":"acp-wsl-bridge","version":1}}`;
  any other identity or version refuses the connection.
- **Requests:** `{"version":1,"id":N,"op":{...}}`, strict fields, `id` ≥ 1, at most
  8 in flight. `op.type` is exactly one of `snapshot`, `control` (the Operator
  read/control allowlist: `profile.info|create|select|write.enable|write.disable`,
  `profile.preferences.list|get`, `profile.review.queue|inspect`, `profile.handoff.get`),
  `mutate` (one OperatorWrite `Mutation`), `session` (`session.refresh|start|prompt|permission|cancel|stop`)
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
- **Lifecycle:** session polling and Operator calls reuse the live bridge; polling
  never starts one, so a stopped distribution or lost helper is an explicit
  unavailable state. Closing the GUI or leaving the WSL target closes the helper
  (stdin EOF; in-flight work finishes briefly) without stopping the WSL daemon or
  touching Profile, auth or runtime files.
- **Non-goals:** no daemon start, Profile selection, write enable, agent launch or
  authentication by the bridge; no MCP writes. Context sharing is a per-start
  operator opt-in (off by default) carried as `share_context` on `session.start`;
  a refresh reports the daemon's `session.context` lease. The daemon's lease
  contract is unchanged.
