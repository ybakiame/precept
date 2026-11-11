# ADR-0002: local transports plus explicit read-only capabilities

Status: implemented P1-D prototype, not a same-user OS sandbox.

P1-J clarification: this ADR's read-only transport decision remains the baseline.
Later operator-only session controls and separate ephemeral format-2 leases are
specified in the [session runtime](../p1/agent-session-runtime.md) and
[P1-I bridge contract](../p1/session-context-bridge.md). They do not extend MCP
write authority. References below to per-launch keys describe ordinary format-1
credentials, not session leases.

The foreground Rust daemon uses Unix sockets on Linux/macOS and named pipes on
Windows. It never substitutes TCP. OS identity is checked in both directions:
Unix SO_PEERCRED/getpeereid via Tokio, Windows pipe peer PID -> process token SID.
Windows pipes reject remote clients and use an explicit current-user-only DACL.
Runtime directories/credential files are also owner-only, with Windows owner/DACL
inspection rather than Unix permission bits treated as portable ACLs.

OS user identity alone does not determine operator vs agent authority. Each daemon
launch creates an operator capability and separate project-bound agent capabilities
in its explicit private runtime directory, outside project checkouts. Startup checks
canonical registered roots against the runtime before creating any control files,
including AGENTS.md-only projects and symlink/junction aliases; equality and nesting
in either direction fail closed. No token is
passed in argv, environment, source files or logs. The bridge receives only the
path of one agent file and refuses an operator file or mismatched project before
serving MCP. The daemon derives connection authority from its own capability table;
no caller-controlled role/project in a handshake can widen it. Only operator
capabilities can list or select launch-allowlisted projects. Baseline context channels
are read-only; the later native session extension is operator-only.

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

P1-D initially returned live bounded Git metadata without a fingerprint. The later
native adapter now implements the bounded v1 fingerprint; unavailable values still
produce explicit incomplete-evidence warnings. See the
[algorithm contract](../p1/worktree-fingerprint-v1.md) and
[current architecture](../architecture.md); a null value never means a match.

See [protocol and operator quickstart](../p1/local-service.md) for concrete limits,
security checks, scope, validation evidence and platform limitations.
