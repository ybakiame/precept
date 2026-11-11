# P1-I: revocable session Context MCP contract

## I0 baseline and protocol review (before implementation)

Start from clean main `3c7799a2c72dd80c1ec142fdfbbfe2347cf987d4` after PR #27;
branch `feat/p1-i-revocable-session-mcp`. Existing NSH surfaces, layout and model architecture
remain unchanged; one obsolete bilingual capability-help sentence is corrected. ADR-0003 remains Proposed. No hosted Actions or production DB use.

Reviewed official [ACP v1 session setup](https://agentclientprotocol.com/protocol/v1/session-setup),
[Kimi ACP reference](https://www.kimi.com/code/docs/en/kimi-code-cli/reference/kimi-acp)
and [Kimi MCP documentation](https://www.kimi.com/code/docs/en/kimi-code-cli/customization/mcp).
The control plane is the ACP **client**; `acp-mcp` is the MCP **server**. ACP v1
supports stdio server entries with absolute command, fixed args and explicit env.
Kimi 2.5.2 was installed in P1-H; its actual version/forwarding behavior must be
verified for I3, not inferred from documentation or its advertised HTTP/SSE bits.
Stdio support is the baseline ACP transport, distinct from optional HTTP/SSE.
The existing bridge implements two read-only tools, legacy MCP 2025-11-25 and a
2026-07-28 stateless subset; this is not full MCP certification or all-vendor support.

## Explicit opt-in and channel ownership

The daemon must be explicitly configured with a trusted absolute `--context-mcp`
executable outside allowlisted roots. It supplies only fixed `acp-mcp` argv. Each
operator `session.start` additionally requires `share_context: true`, the selected
allowlisted project, configured agent label and `confirmed: true`. Missing/false
sharing keeps `mcpServers: []`, including every existing UI Start operation. No
arbitrary caller server list, command, env, capability path or TTL is accepted.
The native operator CLI supports `session-start <project> <agent> --confirm-start
--share-context`, `session-context <project>` and `session-stop <project> <generation>`;
all still authenticate with the existing private operator capability. Windows→WSL
owner helpers remain read-only.

After the direct child is spawned successfully, the daemon creates one distinct
random credential with format **2**, access `agent` bound to project, and a binding
containing the selected configured agent and random session generation. It is not
the format-1 operator/startup project credential. Format-1 users and IPC framing/auth
API v1 remain compatible. Optional session API v2 gains additive opt-in/inspection
methods without altering existing view DTOs. Malformed format/binding combinations
fail closed. The daemon's live registry, not caller credential metadata, determines
rights. No SQLite, fingerprint, profile authority or core MCP tool schema changes.

A private runtime file is created exclusively/non-following, written and synced
before publication to the agent's ACP channel. Random filenames contain no project
or user paths. Existing secure Unix mode/owner/link and Windows owner/DACL/reparse
checks apply. Collisions, broad permissions and unsafe paths fail closed. Paths that cannot be
represented as JSON strings are explicitly rejected; no path-serialization panic
is allowed to poison the session mutex. The
agent launches only the configured stdio server; the daemon owns the capability
lifetime, not every vendor-created descendant. Minimal explicit server environment
is empty; a vendor may inherit its own cleared, explicitly allowlisted agent env.
No secret value appears in argv, UI, diagnostics or audit counters. The capability
**path** is required on the private child channel and is never copied into public
evidence. An agent may independently log its launch metadata inside its selected
private HOME; real tests remove vendor-generated state after child cleanup. The owner explicitly
requested retaining the private authentication config for later verification.

## Lease state and revocation

`absent → active → revoked/expired`, with no reactivation. Usable only for the
associated live direct child. Monotonic expiry defaults to 300 seconds; the daemon's
explicit operator TTL option is bounded to 1–3600 seconds. Clock rollback cannot
extend authority. Only one managed session/lease is active. A replacement always
gets a new secret and random generation, even for the same project/agent.

Every context/handoff request checks the bound live lease at admission and again
before admitting computed data for publication, including an already-authenticated peer. Revocation
also removes its secret from new-handshake lookup. Requests after revoke/expiry are
Unauthorized; data already returned cannot be recalled, and responses admitted by the final check
before revocation may finish writing. A long computation that observes
revocation before publication is discarded. Existing framing/admission/deadline/
32-request-per-connection bounds remain; a held peer grants no permanent authority.

Stop, valid prompt Cancel, child death/failure/auth-required, replacement, expiry and
daemon shutdown revoke before confirming the transition. **Prompt Cancel revokes
context access for the entire session**, even if the child later accepts another
prompt; re-enabling requires explicit Stop and fresh opted-in Start. Completion of
a normal turn retains access until expiry or session termination. A failed kill/reap
never retains context rights or allows a replacement while the child is alive.

Revocation removes only its own matching private credential file. Hard-kill restart
has an empty session registry and rejects old secrets/peers; it validates and removes
recognized stale format-2 session files while holding the exclusive runtime lock.
Malformed/colliding files fail closed rather than being overwritten. No session
resume or persisted lease database. Cleanup failures do not restore authorization.

## Scope, evidence and threat model

A session capability permits only `context.get` and `handoff.get` (plus bounded
health for bridge startup), with project derived server-side. It cannot enumerate
projects, control sessions, mint credentials, approve permissions or mutate profiles.
Extra project/role/session overrides are rejected. Output uses the unchanged resolver:
confirmed preferences only, live allowlisted documents on explicit request, project
rules ahead of memory, existing exact Handoff/fingerprint semantics.

An operator-only content-free `session.context` observation records enabled/active
status and successful context/handoff call counts. These counters originate in the
daemon after authorized reads, independent of an agent's text/tool advertisements.
Only the two existing MCP tools are exposed. Tool discovery/launch/session acceptance
alone is not actual context-call evidence.

A stolen capability path/secret has only its current project's lease rights until
revocation, never operator authority. This is not protection against a malicious
same-user process that can read the operator credential, SQLite or private files
directly. Root/admin, filesystem replacement races by a hostile owner, containment
of independent vendor descendants and OS sandboxing are outside this slice. No
network listener, implicit HOME, shell, login script, preference learning or permanent
transcript is introduced. Real auth/network/model use needs separate owner opt-in;
without it I3 is honestly NOT_RUN while deterministic security work continues.

## Validation sequence

First prove revocation/expiry on established peers, reconnect/concurrency, stale
binding/replay, forged/malformed credentials, scopes and mutations; then deterministic
ACP discovery/calls and stop/crash/restart cleanup. Only after these pass, exercise
an opted-in installed real agent on disposable synthetic context and independently
inspect daemon call counters. Run stable local Windows/ConPTY and native WSLg
matrices once, then only affected gates. Evidence belongs in
`docs/evaluation/p1-i-context-bridge.md`; private artifacts under ignored
`artifacts/local-p1-i/`. No raw credentials, conversations or identifying paths in PRs.


I3 owner decision: the owner authorized private Kimi authentication, network/model
calls during debugging and key retention, then additionally authorized an OpenCode
free-model comparison. Each turn is a separate explicit local smoke invocation on a
synthetic project; there is no automatic application prompt retry or paid fallback.

Real installed Kimi 2.5.2 now completed both read-only MCP calls, independently counted
by the daemon, followed by established-peer/reconnect denial after Stop. Its manual
ACP approval requests omit arguments. A separately opted-in test selector approves
at most two exact read-only tool-name/content requests with allow-once; daemon/MCP
still enforce project/lease/role scope. This does not add product auto-approval or
change the UI. Original failed/incomplete attempts are preserved. OpenCode 2.2.7
accepted the stdio session, but GLM/DeepSeek free-model turns were blocked by provider
quota responses in keyed and anonymous isolated runs. The model ID is verified and
the owner demonstrated regular WSL TUI success; the difference remains unexplained.
Real OpenCode MCP tool use remains unverified. Replaced-lease file cleanup now follows
an atomic Starting reservation and occurs outside the supervisor mutex. See the
[per-attempt and per-host evidence](../evaluation/p1-i-context-bridge.md).
