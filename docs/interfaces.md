# Interfaces and current conformance boundary

## CLI

`acpctl project add/list`, `preference suggest/confirm/reject/deprecate/delete/list`, `context`, `handoff put/get`, and `mcp --project <registered-project>`.

The CLI is the **only** write path in P0. `preference confirm ID` is an operator action; a tool cannot elevate its own suggestion. `deprecate` allows retracting a confirmed value. `delete` permanently removes a preference row; no archival shadow copy is created by the application.

## MCP stdio

MCP operates on newline-delimited UTF-8 JSON-RPC; stdout is reserved for valid JSON-RPC, stderr for sanitized errors. A specific pre-registered `--project` is mandatory at launch. The agent cannot change the project scope through tool arguments.

| Tool | Inputs | Outputs | Side effects |
|---|---|---|---|
| `get_effective_context` | `include_documents` boolean (default false) | confirmed effective preferences, live metadata, optional allowlisted document contents, warnings | read only |
| `get_handoff` | `task_id` string | recorded task snapshot, current repository state, stale/dirty warnings | read only |

2025-11-25: supports `initialize`, `notifications/initialized`, `ping`, `tools/list`, `tools/call` for a local stdio client. 2026-07-28: supports basic per-request `_meta` protocol version and server info for `ping`, `tools/list`, `tools/call`; no initialize needed. This is a **small subset**, not a full certified implementation. No resources, prompts, tools/call mutations, cancellation, batching, HTTP, auth, subscriptions, logging notifications or task augmentation. Two read-only harness integrations have been reported for Codex and Kimi Code; see [2026-11-08 E2E evidence](evaluation/2026-11-08-codex-kimi-handoff.md). Broader compatibility remains unverified.

MCP specification references:
- https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2026-07-28/basic/transports/stdio.mdx
- https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2026-07-28/changelog.mdx
- https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2025-11-25/server/tools.mdx

## Handoff evidence

A handoff stores the HEAD at capture, branch, recorded worktree state, task objective, completed work and a structured list of checks. Accepted check statuses: `PASS`, `FAIL`, `SKIPPED`, `NOT_RUN`. New handoffs also store a bounded SHA-256 worktree fingerprint derived from tracked Git diffs plus untracked file contents (up to 16 MiB / 500 files). No raw diff, filename list, or file contents are persisted in the handoff. A later `get_handoff` compares the fingerprint and warns on changed uncommitted content even if HEAD, branch, and dirty count remain identical. Existing databases are migrated by adding a nullable field; old handoffs remain readable but cannot be retroactively fingerprint-verified. If fingerprint computation fails or its bounds are exceeded, the result is explicitly unverified. Ignored files, submodule internals, configuration and external environment are not covered. A matching fingerprint **does not establish complete state or validation equivalence**. Even when unchanged, device/host verification has its own scope and cannot be inferred by presence of an entry.

## Future external clients

Future ACP/PTY adapters require a separate capability table for session start/resume, permission prompts, tool calls, diff data and terminal rendering. Never report a feature as supported merely because a CLI wrapper can send text to a process.


## Optional native operator sessions (P1-G)

The versioned local `session.*` extension is operator-only and requires explicit
native daemon configuration and confirmation. It is unavailable through MCP and
the Windows→WSL read-only helper. Protocol, lifecycle, methods and bounds are in
[agent session runtime](p1/agent-session-runtime.md). It does not change the Python
P0 API, the existing IPC API v1 read DTOs or any SQLite schema. External ACP v1
stdio support is a deliberately limited subset, not a certified agent integration.


P1-H updates only the optional session extension to **session API v2** for auth state,
known capability bits and content-free response/tool counters. Session v1 requests
receive Version; other IPC API v1 requests and MCP remain compatible. AUTH_REQUIRED
is an observed action-needed state, never automatic login or prompt retry. Authentication
RPC/terminal login and MCP attachment overrides remain unsupported; see the
[auth/capability contract](p1/agent-authentication.md).


P1-I adds explicit native `session.start.share_context` and operator-only
`session.context` inspection to optional session API v2. Existing view DTOs and core
IPC/auth v1 remain unchanged. New **credential format 2** is only for ephemeral,
project/agent/generation-bound session leases; format-1 operator and ordinary project
credentials keep their prior behavior. Session leases are validated on every request
and before response admission, including established peers. They never permit
project listing, session control or profile mutation. MCP still exposes the same
two read-only schemas; the default server list is empty. See the
[lease/expiry/revocation contract](p1/session-context-bridge.md).
