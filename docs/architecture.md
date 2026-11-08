# Architecture — P0

```text
trusted local operator ─ CLI (acpctl) ─┐
                                      ├─ context/store API ─ SQLite user-state DB
trusted harness ─ MCP stdio (read only) ┘          │
                          │                         └─ explicit preference / handoff records
                          └── allowlisted live files and git metadata from one registered project
```

## Ownership

- **Project contract:** user repository's AGENTS, docs, ADRs, code, tests, config. The Context Hub never replaces these or treats all text as equally authoritative.
- **Developer profile:** global or project-scoped, source-labelled, reviewable preference records. Unconfirmed ideas are not injected.
- **Task continuity:** short-lived, explicit handoff record with objective, revision, validation statuses and next action; never an automatic chat transcript.
- **Runtime:** P0 has an on-demand CLI and child-process stdio server. P1 may introduce a Rust daemon that survives GUI closure.

## Effective context resolution

1. Require a pre-registered project name; resolve only its local root.
2. Read current Git HEAD/branch/worktree flags (best effort). No automatic checkout/reset.
3. Load **confirmed** preferences: project scope masks global scope for matching keys; rejected/deprecated/suggested records never enter effective context.
4. Provide live file metadata for a short allowlist. Content is included only when explicitly requested; links to more docs are for the agent to follow itself.
5. Return provenance/status, and warn about authority and the inability of handoffs to authorize actions.

If current code conflicts with an accepted architecture decision, surface the conflict and request the smallest necessary clarification; do not automatically equate one with the other.

## Separation for P1

An eventual Rust `context-core` should remain independent of GUI, terminal and harness implementations. A local `control-daemon` will own sessions, audit events, explicit policies, and durable task state. ACP adapters and PTY-based adapters have different observable features and should not fake equivalent event streams. GPUI or NSH interfaces should connect to these same APIs. External memory engines are optional read-through candidates, not sources of truth.

## Compatibility policy

P0 storage tables and MCP tool names are experimental: before migrating to Rust, add explicit versioned migrations and a contract test suite. Do not silently reinterpret existing profile scopes or change memory status semantics in-place.
