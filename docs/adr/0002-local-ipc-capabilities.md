# ADR-0002: local transports plus explicit read-only capabilities

Status: implemented P1-D prototype, not a same-user OS sandbox.

The foreground Rust daemon uses Unix sockets on Linux/macOS and named pipes on
Windows. It never substitutes TCP. OS identity is checked in both directions:
Unix SO_PEERCRED/getpeereid via Tokio, Windows pipe peer PID -> process token SID.
Windows pipes reject remote clients and use an explicit current-user-only DACL.
Runtime directories/credential files are also owner-only, with Windows owner/DACL
inspection rather than Unix permission bits treated as portable ACLs.

OS user identity alone does not determine operator vs agent authority. Each daemon
launch creates an operator capability and separate project-bound agent capabilities
in its explicit private runtime directory, outside project checkouts. No token is
passed in argv, environment, source files or logs. The bridge receives only the
path of one agent file and refuses an operator file or mismatched project before
serving MCP. The daemon derives connection authority from its own capability table;
no caller-controlled role/project in a handshake can widen it. Only operator
capabilities can list or select launch-allowlisted projects. All channels are read-only.

This cooperative boundary isolates malformed/untrusted requests, not compromised
same-user programs. Such a program can read accessible credentials and the SQLite
file directly. Administrator/root access, arbitrary same-user races, stolen tokens
and full process sandboxing are outside the guarantee. Agent keys are per launch
and per project, not per invocation; multiple bridges may use the same scoped key.
Restart rotates all keys, so bridges must be restarted with the current file.

The native operator binary remains separate from Python acpctl. SQLite ownership
stays with Python; no automatic migration, profile discovery or write tool is
introduced. Actual external Codex/Kimi sessions, GUI lifetime, PTY/ACP orchestration,
and production rollout are not inferred from stdio or socket readiness.

A native Git adapter returns live bounded metadata but intentionally no fingerprint.
Existing core warnings mark handoff evidence incomplete; conformance reports must
record that delta instead of treating null as a matching Python fingerprint.

See [protocol and operator quickstart](../p1/local-service.md) for concrete limits,
security checks, scope, validation evidence and platform limitations.
