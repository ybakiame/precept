# Precept

Precept is a local-first context and policy layer for coding agents. It combines current repository rules, confirmed personal preferences and
explicit task Handoffs while preserving the difference between current facts,
historical claims and independent verification. It is not an agent harness or an OS
sandbox.

Precept is early (0.x) software: interfaces and the Profile schema may still change
between minor versions. It is written in Rust: a native daemon, the `precept` CLI,
an MCP server, harness hooks and an NSH desktop operator app.

## What it provides

- Explicit project registration and bounded inspection of repository facts and
  contract sources.
- Personal preferences with global or project scope, provenance and exact-record
  human review.
- Effective context that explains applicable, masked, conflicting and unknown rules
  without letting personal or session state override repository contracts.
- One `precept` MCP server for every registered project, bound to the session's
  folder: `get_effective_context`, `explain_rule` (why a rule applies or not) and
  `get_handoff`, and, where you allow it, `suggest_rule` (suggestions and updates
  for your review), work-session tools and project memory (`remember`, `recall`,
  `reaffirm`, `forget`).
- Session-start briefings, an opt-in pre-tool gate and a stale-document warning
  through harness hooks, and repository rules (`.precept/rules/`) with checks.
  Precept sets up Claude Code and Codex; other agents connect themselves with
  the [`precept-connect` skill](skills/precept-connect/SKILL.md) (`precept skill`
  fills in this machine's values).
- Project memory that agents keep themselves (this machine's environment,
  cautions, lessons, attempts), unreviewed and below rules, which survives a
  change of agent, harness or model.
- Historical Handoffs with revision, branch, worktree and content-drift warnings.
- A Windows desktop workflow that operates a daemon inside WSL2 through the typed
  `precept-bridge` helper, without TCP or shell forwarding.

Recommended setup: the session-start hook and MCP together. The pre-tool gate, the
AGENTS.md pointer and rules in Git are add-ons for specific needs; see
[Connecting agents](docs/product.md#connecting-agents).

## Build from source

Install the pinned Rust toolchain from `rust-toolchain.toml`, then build the product
binaries:

```text
cargo build --workspace --bins --locked
```

The operator app is a separate Cargo workspace. Build the desktop app with:

```text
cargo build --manifest-path apps/Cargo.toml --workspace \
  --features precept-ui/native --bins --locked
```


## Operator CLI

With the daemon already addressed by `--runtime-dir` and `--endpoint`,
`precept doctor --json` reports redacted, read-only installation diagnostics. Add
`--project NAME` to check a bound project. Exit codes are 0 (ok), 1 (attention), 2
(problem) and 3 (incomplete); it never starts the daemon or changes state. The
Windows package verifier `tools/portable/windows-doctor.ps1` additionally supports
bounded native and optional WSL checks.

## Operator model

Create or select a Profile explicitly. Startup selection and per-run write
enablement are separate choices; the daemon never creates or adopts a Profile by
itself. Each release reads exactly its own Profile schema, with no automatic
discovery, migration, repair or direct-write fallback; move data between releases
with `precept profile export` and `precept profile import`.

Add a project by choosing one checkout. The daemon validates that path and owns the
registered project set; the UI and agents do not scan for repositories. Sharing a
project with agents is a separate operator action.

Preferences begin as **suggested**. Inspect the exact value, scope, source and
revision before confirming or rejecting them. Project-scoped personal preferences
mask compatible global preferences, but neither can override repository contracts.
Handoff records remain historical and must be revalidated against the current
checkout.

The desktop app groups navigation into **Workspace** (Dashboard, Preferences &
contracts, Agents, Sessions, Memory, Handoff for the selected project), **Context**
(Review for pending suggestions and Rules for reviewed records) and **Set up**
(Runtime checks and a separate Workspaces page), with Diagnostics and Settings at the foot. See the
[operator app guide](apps/README.md) for its review workflow.

## Data and trust

- Only the native daemon writes Profile state, through typed local IPC and an
  independent epoch-bound write grant. CLI and UI clients never open writable
  SQLite connections.
- Operator control, operator writes and agent reads use distinct authority. One grant never implies another.
- Document bodies require explicit opt-in and bounded, allowlisted reads.
- Agent-reported actions, daemon observations and independent verification are
  different claims. A saved PASS is not a current PASS.
- Precept does not provide OS sandboxing, credential isolation from same-user processes,
  hidden telemetry, remote synchronization or automatic repository rewriting.
- Keep secrets, credentials, raw conversations, device identifiers and real Profile
  data out of tracked files.

## Documentation

- [Changelog](CHANGELOG.md)
- [Product charter](docs/product.md)
- [Architecture and data ownership](docs/architecture.md)
- [Interfaces and conformance boundary](docs/interfaces.md)
- [Security and privacy limitations](docs/security.md)
- [Operator UI standards](docs/product-ui.md)

## License

Licensed under the [Apache License, Version 2.0](LICENSE). Copyright 2026 ybakiame;
see [NOTICE](NOTICE). Third-party dependencies retain their own licenses.
