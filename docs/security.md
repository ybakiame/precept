# Threat model and privacy limitations — P0 plus optional native sessions

The implemented controls and limitations below remain unchanged by P1-J.
[ADR-0004](adr/0004-product-domain-trust.md) proposes the product's explicit
agent-reported / daemon-observed / independently verified evidence model and
repository-contract vs user-local state ownership. These are target semantics,
not a new enforcement, storage or cryptographic implementation.

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
- New Handoff snapshots may hash tracked Git diffs and bounded untracked file contents locally. The store contains only a SHA-256 digest, not diff text or filenames. Fingerprinting fails closed if the configured limits are exceeded. This is an advisory drift detector, not an isolation boundary or complete repository attestation.

## Known limitations / do not deploy as enterprise policy enforcement

- The harness retains same-user filesystem access. Process/project scoping in MCP is not an OS sandbox or authorization firewall.
- The CLI has no interactive authentication. Any process with local database access could bypass intended review states.
- SQLite WAL sidecar files and OS backups may retain deleted content; app-level deletion is not forensic secure deletion.
- Explicitly included project docs may contain sensitive information; user is responsible for selecting trusted projects and clients.
- Git may execute configured external helpers in some repository environments; the subprocess is bounded but should not be used on hostile repositories without additional isolation.
- Storage encryption, fine-grained per-client policies, profile-review UI, data export/retention workflows, and robust migrations are not yet implemented.

Future profile-learning must be opt-in, explainable and separately approved; do not automatically classify behavior as an instruction. Do not store secrets or raw transcripts, even in private Git repositories.


## Optional P1-G operator session boundary

The native Rust daemon can explicitly opt into one ACP v1 agent through a private
trusted `--agent-config`; default remains disabled. This adds operator-only local
session controls, separate from project-bound MCP credentials. MCP still exposes
only its two read-only tools; Python profile writes and SQLite schema are unchanged.
The child has local user privileges, **not an OS sandbox**. Permission mediation
cannot contain an agent's independent filesystem/network access or descendants.
No Windows→WSL execution bridge, credential inheritance, shell or auto-launch.
See [the capability/lifetime/failure contract](p1/agent-session-runtime.md).


P1-H never retains agent-auth method IDs, descriptions, login argv/env or raw auth
errors. It records only bounded method types/known capability bits and content-free
activity counters. Private agent HOME is an explicit operator choice, not inherited
or copied credentials. The agent may access it with user privileges; authentication
and a successful session do not confer permission approval. Client login remains
disabled; P1-I below adds a separately opted-in context attachment. See [auth boundaries](p1/agent-authentication.md).


P1-I session context never reuses startup/operator credentials. A separate format-2
capability belongs to one selected native project/agent/generation, expires by a
monotonic deadline and is checked for every request and response admission. Stop,
prompt Cancel, failure, root drift and shutdown revoke it; restart rejects cached
secrets and cleans recognized private files under the runtime lock. An established
peer cannot retain session authority after revoke. Earlier-admitted responses may
finish writing, and delivered data cannot be recalled. Format-1 non-session users
retain their prior per-launch behavior; they do not gain revocation guarantees.
Same-user filesystem access remains outside this transport boundary. See
[the session bridge threat model](p1/session-context-bridge.md).
