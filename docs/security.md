# Threat model and privacy limitations — Rust-native validation runtime

Accepted ADR-0004 defines repository authority, user-local review and independent
evidence distinctions. ADR-0006 retires the disposable prototype without migration.
Current controls are native Profile v1, scoped IPC v2 and a daemon-exclusive writer;
this is validation evidence, not enterprise enforcement or production activation.

## Threat model

A trusted operator selects a Profile and explicitly configures trusted local
clients. Repository content and agent protocol frames may be untrusted. Same-user
processes with filesystem access can bypass cooperative application controls; this
is **not an OS sandbox** or a credential isolation boundary.

## Current controls

- Explicit fresh Profile creation/no overwrite or discovery; strict complete v1
  Schema/data validation and private owner/mode/DACL/path/file-identity checks.
- Only the daemon writes through typed transactions, OS lease and epoch fencing.
  Operator read/control and write grants are separate; disabling drains admitted
  writes. Same-epoch receipts/CAS prevent silently changing or duplicating a request.
- CLI/UI never receive writable SQLite or fall back to a retired runtime. Startup
  does not automatically enable writes or adopt incompatible prototype profiles;
  it selects a Profile only when the operator asked for that path, and never
  remembers write access.
- The daemon's project set and startup choice live in its private `--state-dir`
  (0700/0600, strict JSON, bounded to 256 projects, written atomically); a damaged
  file refuses start and is never repaired. Adding a project is an operator-write
  action on one chosen path, checked daemon-side (Git root inside the daemon user's
  home; system, other-filesystem, foreign-owned and control paths refused). GUIs and
  the Windows bridge never list or scan directories; agents cannot check or add.
- Exactly two project-bound read-only MCP tools; no administrative previews, profile
  mutations, context-driven approval or tool-supplied project selector.
- Preference confirmation binds the exact record/revision/scope. Only confirmed
  applicable values reach effective context; repository contracts remain authoritative.
- Document bodies are opt-in, narrowly allowlisted, bounded and contained. Native
  Git commands are shell-free and bounded; configured external filters/submodules
  are refused before status/fingerprint. No repository rewriting or credential scraping.
- Handoff validation is historical. Bounded Fingerprint v1 stores only a digest,
  not raw diffs/files; overflow or missing observations remain unverified.
- No product network listener, telemetry, automatic model call, history ingestion,
  credential import or remote synchronization. Optional Web tests serve only
  synthetic static assets on loopback, without any daemon API/gateway.

## Limitations

- Native capabilities protect cooperative local protocol admission, not arbitrary
  same-user file access or an agent's independent execution/network behavior.
- An uncertain response may already have committed. Retry must preserve the exact
  immutable request/ID/binding in the same epoch; UI retention is in memory only.
- SQLite sidecars/backups may retain deleted content; deletion is not forensic erasure.
- Selected documents can contain sensitive material. Only attach trusted clients
  and explicitly selected repositories; historical Handoff does not authorize actions.
- Fingerprints are bounded, can race concurrent edits and do not attest ignored
  files, submodule internals, builds, devices or complete repository state.
- No production activation, migration/import/repair, encryption, full review/provenance,
  retention/recovery or release guarantee follows from this validation stage.

Never persist secrets, raw transcripts or private identifiers in tracked fixtures.
Source retirement does not delete real databases, local auth or runtime directories.

## Optional P1-G operator session boundary

The native Rust daemon can explicitly opt into one ACP v1 agent through a private
trusted `--agent-config`; default remains disabled. This adds operator-only local
session controls, separate from project-bound MCP credentials. MCP still exposes
only its two read-only tools; Profile writes require the independent native grant.
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
