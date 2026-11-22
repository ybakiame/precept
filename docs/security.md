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
an operator who disables the integration is not contained. The gate fails open
(an unreachable daemon allows the call); there is no fail-closed mode. A denial
means only that a hard rule's guard matched a call a hook actually mediated; each
decision is recorded as evidence, and enforcement is never assumed from a switch.
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
- Agent activity is recorded per project as an evidence chain in the daemon's
  private state (`evidence-<project>.jsonl`, 0600; at most 1500 entries, the
  oldest dropped with the last dropped hash kept as the base). Each entry holds
  the method kind, outcome, counts, the client's self-reported label and the work
  session id `precept-mcp` attaches, and for a gate denial the key of the
  committed rule that denied, never values, selectors, task ids, paths,
  commands, file contents or tokens, plus the SHA-256 of the entry before: an edit or a
  removal in the middle is detected and reported, which is integrity, not a
  signature (whoever can write the daemon's state can rewrite the whole file).
  Refused handshakes, which carry no project, stay in daemon memory. Readable by
  the Operator alone.
- The session-start hook is read-only, uses the project's agent capability,
  stays silent outside the project root and fails open. Its entry in Claude Code's,
  Codex's or Antigravity's user configuration is added or removed by the daemon only on an
  Operator request carrying the reported file digest, touching only Precept's
  entry and keeping a backup. The same holds for the one `precept` MCP server
  entry in Claude Code's, Codex's or Antigravity's user configuration; it names
  only `precept-mcp` and the runtime paths, never a project or a token. The server
  uses only the capability of the registered project that holds the session's
  folder, so a session never sees another project's tools or context. The AGENTS.md pointer is written by the daemon only on an Operator
  request carrying the previewed file digest, only between Precept's markers, in a
  served project's root, never through links; Precept never commits.
- The pre-tool gate is off until the Operator turns it on per project
  (`project.gate`). It judges only committed hard rules' guards, answers through
  the project's agent capability, fails open (an unreachable daemon, a timeout or
  an unknown tool allows the call) and records each decision in the evidence
  chain. It mediates only the calls a harness routes through its hook.
- Project-bound MCP tools: two read-only tools and `suggest_rule`; no
  administrative previews, decisions, context-driven approval or tool-supplied
  project selector. `suggest_rule` is accepted only for a project whose agents
  the operator allowed to suggest (`project.propose`, off by default, persisted in
  daemon state) and only while the operator has writes on for the daemon run; it
  can create nothing but a `suggested` record with origin `agent` in its own
  project or the personal scope, bounded per run and per scope,
  with duplicates refused. Nothing is queued while writes are off.
- Work sessions are agents' own notes (goal and done / doing / todo), written
  only where the operator allows agent writes for the project, bounded, kept in
  the daemon's private state directory (`sessions-<project>.json`, 0600, never
  in the Profile or the repository) and deleted by the operator at will. They
  are read back by the project's agents as context, never as rules, and grant
  nothing.
- Project memory follows the same boundary: agents' notes written only where
  the operator allows agent writes, bounded, kept in `memory-<project>.json`
  (0600) in the daemon's private state, never in the Profile or the repository,
  labelled unreviewed wherever it is delivered, and edited, deleted or cleared
  by the operator at will. Agents may add, correct, reaffirm and forget notes;
  nothing in memory confirms, enables or outranks a rule.
- Preference confirmation binds the exact record/revision/scope. Only confirmed
  applicable values reach effective context; repository contracts remain authoritative.
- Document bodies are opt-in, narrowly allowlisted, bounded and contained. Native
  Git commands are shell-free and bounded; configured external filters/submodules
  are refused before status/fingerprint. No repository rewriting or credential scraping.
- Handoff validation is historical. Bounded Fingerprint v1 stores only a digest,
  not raw diffs/files; overflow or missing observations remain unverified.
- No product network listener, telemetry, automatic model call, history ingestion,
  credential import or remote synchronization.

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
