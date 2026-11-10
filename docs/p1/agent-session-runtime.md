# P1-G: operator-controlled local Agent Client Protocol session

## G0 architecture and authority contract (before implementation)

Baseline: clean main `52c01b7b6c5b48a2884e77bcd8a8893a5a010e62` (merge #23),
no submodules; branch `feat/p1-g-acp-session-runtime`. Existing Python P0,
read-only store, fingerprint byte contract, two-tool MCP and P1-F UI remain the
reference. Required paths exist; IPC DTOs are in `acp-ipc/src/lib.rs` and the
Daemon/MCP are binaries in `src/main.rs`, not invented lib/protocol files.

Protocol reviewed 2026-11-10: official docs label **v1 Latest**, **v2 Draft**.
Implement only v1 integer `protocolVersion: 1`, newline-delimited UTF-8 JSON-RPC
2.0 over child stdio. Do not confuse this external protocol with product ACP or
internal length-prefixed IPC API v1. Primary sources:
[initialization](https://agentclientprotocol.com/protocol/v1/initialization),
[transport](https://agentclientprotocol.com/protocol/v1/transports),
[session setup](https://agentclientprotocol.com/protocol/v1/session-setup),
[prompt turn](https://agentclientprotocol.com/protocol/v1/prompt-turn),
[permission](https://agentclientprotocol.com/protocol/v1/tool-calls),
[cancellation](https://agentclientprotocol.com/protocol/v1/cancellation).

| External ACP method | This slice |
| --- | --- |
| initialize | Require v1 and object agentCapabilities; record safe agent name/version and supported loadSession flag |
| session/new | One canonical allowlisted cwd, empty mcpServers; require bounded sessionId |
| session/prompt | Text only; one turn; completion only from matching response and valid stopReason |
| session/update | Scoped observed message/tool/plan metadata; bounded ephemeral events; no transcript persistence |
| session/request_permission | Default deny; pending request expires, only explicit operator may choose allow_once; no remembered approval |
| session/cancel | Notify; pending permission cancelled; await prompt response with deadline, otherwise kill/fail |
| close/exit | v1 baseline has no session/close or shutdown; deliberate stop closes/kills/reaps child |
| authenticate, load/resume/list, config/model/mode, files, terminal, extensions | Unsupported; do not advertise client fs/terminal/auth abilities; unknown requests receive method-not-found |

A product Handoff is recorded project context, never this protocol conversation.
Reopening a UI reattaches to the daemon's live **in-memory observation**, not vendor
persistent resume. No history survives daemon exit/restart and no session SQLite
migration/transcript store is introduced.

## Ownership, launch trust and failure rules

The daemon owns one isolated Rust supervisor and at most one direct agent child.
GUI/TUI are independent authenticated clients. Closing either client does not
stop the child. Daemon graceful shutdown cancels/kills/reaps its direct child;
no OS sandbox, descendant-process containment or hard-kill survival is promised.
Agents execute with the local user's privileges and can access files themselves.
Protocol permission mediation is not an OS policy firewall.

Execution is disabled unless the operator explicitly supplies a private bounded
JSON `--agent-config` file. Config version 1 contains trusted named executables,
fixed argv arrays and an explicit environment allowlist. Absolute canonical regular
executable paths only; no shell, script wrapper on Windows, PATH executable search,
download, repository configuration discovery or repository-derived arguments.
Project cwd is resolved from the existing registered/daemon-allowlisted root.
Config and control-runtime files must stay outside project roots. Environment is
cleared before supplying explicit allowed keys. No capability token is inherited.

An existing per-client operator capability, OS peer check and IPC API v1 are required
before **every** `session.*` request. Project-bound agent capabilities are denied
before parameter parsing or config/state disclosure. New methods require explicit
`session_api_version: 1` and project scope; legacy health/context/handoff requests
are unchanged. Unknown versions/methods/fields fail closed. Windows→WSL helper
rejects session controls: it remains read-only, with no execution bridge.

Operator methods: `session.capabilities`, `session.list/get`, `session.start`,
`session.prompt`, `session.cancel`, `session.stop`, `session.permission`.
Start selects only a configured label and requires explicit UI confirmation of
non-sandboxed access plus observed dirty/unknown worktree state. Prompt/permission
are separate operator actions. No Issue, document, Handoff or agent text confers
authority. Permissions are single-request, session/generation-bound and expire;
cancel/stop revokes pending approvals. Other clients see the same pending request.

States: idle → starting → ready → running ↔ waiting_permission → completed;
malformed/oversized/duplicate/wrong-session messages, crash and timeout → failed;
stop → stopping → stopped. Failed/completed do not imply a dead child. Process
alive is reported separately. A new session replaces only a terminal inactive
session; generations and project+session IDs reject old commands/events.

Bounds: eight configured entries, 16 argv/environment entries, 64 KiB config,
64 KiB ACP frame, 64 retained events of at most 512 display characters, 4096-byte
prompt, four pending permissions, bounded IDs, command queue 16, 5s init/new and
write deadlines, 120s turn deadline, 5s cancellation deadline, 30s permission expiry.
Stderr is discarded. Unknown raw errors never reach logs or UI. Events and prompt
are ephemeral and excluded from default copy/diagnostic probes. Polling runs off
the UI thread; request epoch/target/project/generation guards prevent stale state.

## Local validation plan

Use a real Rust fake-agent executable strictly as a deterministic protocol fixture,
never a real coding agent. Process tests cover normal updates, completion, cancel,
permission deny/explicit once/expiry, malformed/huge/duplicate/out-of-order IDs,
crash/timeout, scope and credential isolation, two clients and UI lifetime.
Then run the established local Windows/ConPTY and WSLg-native Wayland/PTY matrix,
10k/state/fingerprint/parity/MCP regressions once after implementation stabilizes.
Only targeted tests are repeated for intervening failures. All evidence stays in
ignored `artifacts/local-p1-g/`; no system config, real DB or external checkout writes.

Runtime inventory: Rust/Cargo 1.99.1 and Python/Node available locally. Codex is an
implementation harness, not an assumed ACP server. Installed CLI presence is not
ACP compatibility. A real-agent smoke requires a documented compatible installed
executable and separate owner opt-in on disposable state; otherwise NOT_RUN.
ADR-0003 remains Proposed. No niri/mobile or hosted Actions; Draft PR only.
