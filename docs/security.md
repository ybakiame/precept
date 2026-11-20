# Threat model and privacy limitations — Rust-native validation runtime

The [product charter](product.md) defines repository authority, user-local review
and independent evidence distinctions. Current controls are native Profile v1,
scoped IPC v2 and a daemon-exclusive writer; validation evidence is not enterprise
enforcement or production activation.

## Threat model

A trusted operator selects a Profile and explicitly configures trusted local
clients. Repository content and agent protocol frames may be untrusted. Same-user
processes with filesystem access can bypass cooperative application controls; this
is **not an OS sandbox** or a credential isolation boundary.

## Hooks and enforcement

Session-start injection and pre-tool gates run inside the agent's harness, as
commands the harness chooses to run. They are **cooperative compliance, not a
security boundary**: an agent or harness that skips the hook, a tool call the
harness does not route through it, a same-user process editing files directly, or
an operator who disables the integration is not contained. Gates fail open by
default (an unreachable daemon allows the call); fail-closed is an explicit
per-project choice. "Enforced" therefore means only that a hard rule was checked
on the calls a hook actually mediated, and is reported per session, never assumed.
A proxy chokepoint that mediates tool calls outside the harness would be a
separate design with its own threat model.

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
- External agent access is switched per project by the operator
  (`project.agents`): an unshared project has no agent credential, and a held
  token is refused. `precept-mcp` is stdio-only (no port); the GUI shows client
  configuration to copy and never writes other tools' files.
- The agent activity log is daemon memory only (bounded, emptied on restart),
  readable by the Operator alone, and records methods, outcomes, counts and the
  client's self-reported label, never values, selectors, task ids, paths or tokens.
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
