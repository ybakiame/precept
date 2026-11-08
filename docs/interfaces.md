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

2025-11-25: supports `initialize`, `notifications/initialized`, `ping`, `tools/list`, `tools/call` for a local stdio client. 2026-07-28: supports basic per-request `_meta` protocol version and server info for `ping`, `tools/list`, `tools/call`; no initialize needed. This is a **small subset**, not a full certified implementation. No resources, prompts, tools/call mutations, cancellation, batching, HTTP, auth, subscriptions, logging notifications or task augmentation. Live integration with multiple real harnesses remains unverified.

MCP specification references:
- https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2026-07-28/basic/transports/stdio.mdx
- https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2026-07-28/changelog.mdx
- https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/specification/2025-11-25/server/tools.mdx

## Handoff evidence

A handoff stores the HEAD at capture, branch, recorded worktree state, task objective, completed work and a structured list of checks. Accepted check statuses: `PASS`, `FAIL`, `SKIPPED`, `NOT_RUN`. A later `get_handoff` inspects the current checkout and explicitly warns if HEAD, branch or worktree changed. Even when unchanged, device/host verification has its own scope and cannot be inferred by presence of an entry.

## Future external clients

Future ACP/PTY adapters require a separate capability table for session start/resume, permission prompts, tool calls, diff data and terminal rendering. Never report a feature as supported merely because a CLI wrapper can send text to a process.
