# Agent Control Plane

Agent Control Plane (ACP) is a local-first context and policy layer for coding
agents. It combines current repository rules, confirmed personal preferences and
explicit task Handoffs while preserving the difference between current facts,
historical claims and independent verification. It is not an agent harness or an OS
sandbox.

ACP is validation-stage software, not a production release. The supported product
runtime is Rust, with a native daemon, operator CLI, read-only MCP bridge and NSH
Desktop/TUI operator app.

## What it provides

- Explicit project registration and bounded inspection of repository facts and
  contract sources.
- Personal preferences with global or project scope, provenance and exact-record
  human review.
- Effective context that explains applicable, masked, conflicting and unknown rules
  without letting personal or session state override repository contracts.
- Exactly two project-bound, read-only MCP tools: `get_effective_context` and
  `get_handoff`.
- Historical Handoffs with revision, branch, worktree and content-drift warnings.
- Optional, explicitly configured native agent sessions with separate Start,
  authentication, permission and context-sharing decisions.
- A Windows Desktop workflow that operates a daemon inside WSL2 through the typed
  `acp-wsl-bridge` helper, without TCP or shell forwarding.

## Build from source

Install the pinned Rust toolchain from `rust-toolchain.toml`, then build the product
binaries:

```text
cargo build --workspace --bins --locked
```

The operator app is a separate Cargo workspace. Build its native Desktop and TUI
with:

```text
cargo build --manifest-path apps/Cargo.toml --workspace \
  --features acp-operator-ui/native,acp-operator-ui/tui --bins --locked
```

The optional Web target is an offline synthetic preview. It has no daemon API,
native IPC, Profile data or agent credentials.

## Operator model

Create or select a Profile explicitly. Startup selection and per-run write
enablement are separate choices; the daemon never creates or adopts a Profile by
itself. Only fresh Rust-native Profile v1 is supported, with no automatic discovery,
migration, repair or direct-write fallback.

Add a project by choosing one checkout. The daemon validates that path and owns the
registered project set; the UI and agents do not scan for repositories. Sharing a
project with agents is a separate operator action.

Preferences begin as **suggested**. Inspect the exact value, scope, source and
revision before confirming or rejecting them. Project-scoped personal preferences
mask compatible global preferences, but neither can override repository contracts.
Handoff records remain historical and must be revalidated against the current
checkout.

The operator app groups navigation into **Set up**, **Project** and **Personal**,
with Diagnostics and Settings available separately. See the
[operator app guide](apps/README.md) for its review workflow.

## Data and trust

- Only the native daemon writes Profile state, through typed local IPC and an
  independent epoch-bound write grant. CLI and UI clients never open writable
  SQLite connections.
- Operator control, operator writes, agent reads and optional session context use
  distinct authority. One grant never implies another.
- Document bodies require explicit opt-in and bounded, allowlisted reads.
- Agent-reported actions, daemon observations and independent verification are
  different claims. A saved PASS is not a current PASS.
- ACP does not provide OS sandboxing, credential isolation from same-user processes,
  hidden telemetry, remote synchronization or automatic repository rewriting.
- Keep secrets, credentials, raw conversations, device identifiers and real Profile
  data out of tracked files.

## Documentation

- [Product charter](docs/product.md)
- [Architecture and data ownership](docs/architecture.md)
- [Interfaces and conformance boundary](docs/interfaces.md)
- [Security and privacy limitations](docs/security.md)
- [Operator UI standards](docs/product-ui.md)

## License

Licensed under the [Apache License, Version 2.0](LICENSE). Copyright 2026 ybakiame;
see [NOTICE](NOTICE). Third-party dependencies retain their own licenses.
