# Local Daemon and IPC — Design Contract

## Boundary

`acp-daemon` is a **user-local** service, independent of GUI lifetime. P1 should first serve read-only project context and Handoff retrieval. It must not become a privilege broker, remote server or autonomous tool runner. The Python CLI remains the reference write interface until the Rust migration is validated.

```text
 operator CLI   GPUI/NSH GUI   optional ACP client
      \             |              /
        [local IPC / explicit client identity]
                      |
              [acp-daemon]
         context resolver / policy
             |            |
       project files   SQLite store
             |
    project-bound MCP stdio adapter -> trusted harness
```

## Platform transports

Use a Unix-domain socket with owner-only directory permissions for Linux/macOS, and an ACL-restricted Windows named pipe. Prefer reliable OS peer-credential checks where available; document which OS features are actually implemented and tested. Do **not** bind TCP localhost as a convenient default. A local endpoint alone is not a sufficient authorization boundary for hostile same-user processes.

The endpoint is scoped to an explicit user account and exposes an API version handshake. Reject unsupported versions, unknown methods, malformed frame sizes and excess input. Cap message lengths, request timeouts, concurrency and outstanding work. Avoid tokens in argv, source trees, logs or public temp folders.

## Required messages (proposed; implementation not yet present)

```json
{"api_version":1,"id":"req-1","method":"context.get","params":{"project":"lanpath","include_documents":false}}
```

```json
{"api_version":1,"id":"req-2","method":"handoff.get","params":{"project":"lanpath","task_id":"task-1"}}
```

The daemon may accept an explicit project for the trusted local operator client **after authentication and authorization**. The agent-facing MCP adapter must pin its project at process launch and must not forward a caller-controlled project selector.

Preference confirmation, workspace deletion, device operations, agent process termination, releases and remote actions are out of the initial daemon API. Each requires its own later authorization design.

## Lifecycle and observability

- GUI exit must not automatically terminate the daemon or any separately managed agent process.
- Daemon restart must not assert that external agent sessions survived unless an actual ACP/PTY adapter verified them.
- Expose health and storage schema information separately from project content.
- Record optional redacted audit metadata (method, project, result, duration) **without** raw prompts, tokens, credentials or returned document contents.
- Report FAIL/BLOCKED/UNVERIFIED independently; never equate socket readiness with MCP conformance or GUI readiness.
- No automatic filesystem scanning, external network discovery, sync, or user-data migration.

## Acceptance scenarios

Two local clients request the same project while a third attempts a different project without authorization; denied requests cannot leak preference values or documents. Close GUI: daemon remains available. Restart daemon: registered projects and confirmed preferences persist. Simulate malformed/oversize requests, stale/unknown schema, crash during write migration, and concurrent read/write contention. The MCP adapter still passes its existing legacy and stateless fixtures.

This is a P1 **implementation contract**, not a claim that these controls already exist.
