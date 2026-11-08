# P0 threat model and privacy limitations

## Threat model

A trusted user invokes `acpctl` and configures a trusted local coding-agent process as MCP client. The project folder can contain untrusted repository content. A compromised same-user process or a hostile MCP client outside this trust model can read accessible files and is **not** isolated by the P0 server.

## Current controls

- Local SQLite database under a user-specific state directory; best-effort POSIX `0700` directory and `0600` database file permissions.
- No network listener, credential access, prompt injection execution or agent-facing write tools.
- Each MCP process requires a registered project name at startup, with no tool parameter that changes scope.
- Document content is not returned unless requested; a short allowlist, bounded file size, and symlink containment check apply.
- Shell-free, bounded Git commands inspect only the registered checkout. No `git reset`, `git clean`, `git checkout`, or service restarts.
- Preferences are opt-in through explicit confirmation. Suggested/rejected/deprecated values are invisible to agent-facing effective context. Provenance remains visible to the operator.
- No automatic history scraping, remote synchronization, telemetry, memory plugin or third-party network call.

## Known limitations / do not deploy as enterprise policy enforcement

- The harness retains same-user filesystem access. Process/project scoping in MCP is not an OS sandbox or authorization firewall.
- The CLI has no interactive authentication. Any process with local database access could bypass intended review states.
- SQLite WAL sidecar files and OS backups may retain deleted content; app-level deletion is not forensic secure deletion.
- Explicitly included project docs may contain sensitive information; user is responsible for selecting trusted projects and clients.
- Git may execute configured external helpers in some repository environments; the subprocess is bounded but should not be used on hostile repositories without additional isolation.
- Storage encryption, per-client permissions, approval UI, data export/retention workflows, and robust migrations are not yet implemented.

Future profile-learning must be opt-in, explainable and separately approved; do not automatically classify behavior as an instruction. Do not store secrets or raw transcripts, even in private Git repositories.
