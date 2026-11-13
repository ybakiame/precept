# P1-H: native agent authentication and capability contract

P1-I supersedes the H2 forwarding deferment with a separately opted-in
[revocable session bridge](session-context-bridge.md). Default turns remain unshared;
the private-authentication and explicit operator-permission boundaries below remain.

## Baseline and owner decision

Start: clean main `ff8db143fcbd6f2dc1614094a20a3ac697317f26`, merge #25,
no submodules; branch `feat/p1-h-real-agent-e2e`. The former P1-G feature branch was
verified merged and removed locally/remotely at the owner's request.

The [recorded owner decision](https://github.com/ybakiame/agent-control-plane/pull/25#issuecomment-6772650402)
authorized merging the existing Sessions UI and explicitly deferred its dedicated
visual review. This is **DEFERRED**, not completed visual acceptance. The owner's
subsequent instruction to execute #26 authorizes this next phase with that recorded
deferment; no visual approval is invented. ADR-0003 stays Proposed.

## H0 protocol and actual target inventory

Reviewed 2026-11-11 against primary sources:
[ACP v1 initialization](https://agentclientprotocol.com/protocol/v1/initialization),
[authentication](https://agentclientprotocol.com/protocol/v1/authentication),
[prompt updates](https://agentclientprotocol.com/protocol/v1/prompt-turn),
[Kimi ACP](https://www.kimi.com/code/docs/en/kimi-code-cli/reference/kimi-acp),
[IDE prerequisites](https://www.kimi.com/code/docs/en/kimi-code-cli/guides/ides),
[private configuration](https://www.kimi.com/code/docs/en/kimi-code-cli/configuration/config-files),
[environment behavior](https://www.kimi.com/code/docs/en/kimi-code-cli/configuration/env-vars).
ACP v1 remains Latest; v2 is Draft. Authentication is negotiated in initialize;
protocol authenticate is distinct from interactive terminal login. This client
advertises neither terminal authentication nor filesystem/terminal tools. It never
runs login arguments/environment supplied by an agent or calls logout.

Installed Kimi Code CLI **2.5.2**, documented `kimi acp`, was initialized with a new
empty HOME and cleared environment, without credentials or model invocation. It
negotiated ACP v1 and advertised `login` with type `terminal` and legacy terminal-auth
metadata, despite client capabilities being empty. The client therefore treats that
flow as unsupported; it must not pass a terminal-method ID to authenticate. Observed
capabilities: loadSession, image/embeddedContext, HTTP/SSE MCP, session list/resume/
close/delete/fork and logout; audio false. Advertised is not client-implemented.
Raw login commands, env, URLs/descriptions and arbitrary metadata are discarded.

## Supported flow and explicit refusals

Only **operator-prepared authentication outside the app** is supported. An operator
may supply a trusted private agent HOME through the existing explicit config env
allowlist. No credential value crosses IPC/UI, no credential input widget, no token
variables or inherited shell environment, no reading/copying the normal user's login
files. The trusted child may read its operator-selected HOME itself and retains local
user privileges; this is not an OS sandbox.

The operator can prepare an isolated Kimi config locally. Kimi documents provider
API keys in its private TOML config, not automatic inheritance of arbitrary API-key
environment variables. The one-shot smoke uses a separate disposable HOME, manual
permission mode, telemetry off and no automatic skill merge. No login flow is launched
by this product. No permission is inferred from valid authentication.

On a matching AUTH_REQUIRED error, observations become `auth_required`, pending
permissions are revoked, and the direct child is killed/reaped. This is action needed,
not a generic crash. There is **no automatic retry**. After external preparation,
the operator selects Review retry (Alt+U) and confirms Start, producing a fresh random
generation of the same observed configured agent. A removed executable cancels pending
review; passive polling retains an intentional valid selection.
Prompt/permission decisions from the old generation remain invalid. Failed cleanup
still reports failed/alive and blocks another launch. Missing or invalid auth leaves
the model-turn acceptance NOT_RUN; rejection errors are sanitized.

Initialize records only bounded known capability bits and method types; it validates
bounded unique method IDs without retaining them. Method names, descriptions, args, env and extension metadata are not
retained. Unknown auth types are explicitly unsupported. Malformed auth arrays/IDs,
duplicate IDs or malformed known capability fields fail closed without echoing values.
An accepted session means only session creation succeeded, not that repository writes
are authorized or a model turn has completed.

The optional local **session extension API becomes v2** because it adds auth state
and typed capability observations. Old session API v1 requests fail Version explicitly;
external ACP v1, IPC framing/auth API v1, existing context/health/handoff and two-tool
MCP remain unchanged. Config version 1 stays valid. No SQLite migration.

Known session metadata updates may arrive outside a turn. While session/new is in
flight, retain at most eight content-free metadata observations, then validate their
session ID against the returned session before publishing them. Once bound, accept
only known metadata outside a turn. Message/tool content requires an active prompt;
wrong IDs, arbitrary notifications, overflow or unsupported variants fail closed.
No update infers completion; only the matching prompt response with valid stopReason.

## H1 real-turn authorization and evidence

The owner first authorized one model call using a separately prepared private Kimi
HOME, selected Kimi Code API-key configuration, and reported local setup ready. The
owner then explicitly authorized necessary model calls during this debugging session
using subscription quota. Runs remain deliberate, bounded and separately recorded;
there is no automatic prompt retry. The key is never
sent in chat or written to tracked files. A credential-free private template was
created outside the repo, mode 0600; the owner filled it locally. After the authorized
completion/cancel checks and direct-child reaping, the owned private HOME, credential
copy and vendor-generated state were deleted.
The runner supplies only the selected HOME path, not credential contents, to the
configured child and never prints or archives config/session/log content.

One disposable project, one fixed synthetic no-tool prompt, no real repository or
user database. The runner requires explicit credential-use and model-call opt-in;
it never retries prompts, authenticates, installs adapters or launches login. On tool
activity/permission it cancels rather than authorizing tool use or another turn.
Evidence contains known version/capability bits, bounded counters, stopReason and
PASS/FAIL/AUTH_REQUIRED/NOT_RUN classifications, not response text or transcripts.
Real cancellation/error compatibility is recorded separately from deterministic tests;
the initial one-call authorization was not silently expanded; the owner explicitly
authorized debugging calls before the separately recorded follow-up attempts.

## H2 optional project-context bridge decision

ACP session/new supports stdio mcpServers; Kimi documents MCP forwarding. This does
not justify copying an operator credential or the daemon's existing long-lived
project credential into a child. The current immutable startup credential registry
has no session-lease revocation for already-connected peers. Therefore **no bridge
is enabled in P1-H**: session/new still sends `mcpServers: []`, and arbitrary caller
MCP/config overrides remain rejected. Real context tool-call evidence is NOT_RUN.

A future explicit per-launch opt-in must mint a separate random, short-lived,
project-bound agent lease, enforce it on every request including existing peers,
launch only the trusted two-tool MCP executable with argv, and revoke/close/clean up
on session stop, failure and daemon restart. It must never expose project enumeration,
operator IPC, profile mutation or permission approval. Existing two-tool/process
isolation tests remain mandatory; advertised HTTP/SSE support is not actual usage.
This design records the seam without adding a new listener/auth redesign to this slice.

## H3 validation and UX

Preserve existing six surfaces, themes, English/Chinese and focus/scope/lifetime
behavior. Add only static auth guidance and observed capability/state summaries.
Keep secrets out of diagnostics/copy/probes. Deterministic fixtures cover auth-required
→ external synthetic readiness → explicit new generation, repeated rejection, malformed
or sensitive auth metadata, unsupported reverse methods, metadata ordering/bounds,
expiry/cancel/stop and old-scope denial. They are not real coding-agent success.

Run stable local Windows/ConPTY and native WSLg Wayland matrices once per host after
implementation stabilizes; repeat only affected gates. Keep pinned/offline Rust,
Historical Python/parity guards are retired; current Rust fingerprint/IPC/MCP/10k gates replace them. Retain ignored `artifacts/local-p1-h/`, no
Actions/system config/real data changes, no niri/mobile. Draft PR only, no auto merge.
