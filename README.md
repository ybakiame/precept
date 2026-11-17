# Agent Control Plane

ACP is a local-first context and policy layer for coding agents: current repository
rules, confirmed personal preferences and honest task Handoffs. It is not an Agent
harness or an OS sandbox. Repository contracts and accepted ADRs outrank preferences
and historical Handoffs.

The active product runtime is **Rust**. M2 S1–S3 are merged through PR #47 at
`58750f346bf2288d79f9ff29754ca110ebca52d0`: fresh Profile v1, daemon-exclusive
writer/lease/grants, native CLI and shared NSH Desktop/TUI operator workflows.
S4 removes the disposable prototype package/tests/probes and replaces active CI and
local validation. [Issue #40](https://github.com/ybakiame/agent-control-plane/issues/40)
still requires separate final integrated M2 acceptance. This is `0.0.x-rc` validation,
not production activation or a release.

## Development

Use the [Rust-only developer runbook](docs/p1/rust-only-development.md) for exact
Cargo, Windows/WSL and optional offline Web gates. Local helpers invoke Cargo
directly; verification creates only owned synthetic test state, never a default
user Profile or a real agent. Do not dispatch hosted Actions while quota is exhausted;
commits use `[skip ci]`.

```text
cargo build --workspace --bins --locked
cargo test --workspace --all-targets --locked
```

Profile setup and all seven typed commands are documented in the
[native operator guide](docs/p1/native-operator-workflows.md). Creation, selection
and write enablement are separate explicit operator actions. NSH shares one
Desktop/TUI model; its Profile panel supports exact preference inspection,
suggestion and confirm/reject. Registration, deprecate/delete and Handoff put are
CLI-only at S3. The Windows-to-WSL helper, offline and Web targets remain read-only.

## Data and trust

- Only explicitly created fresh Rust-native **Profile v1** is supported. The retired
  prototype's v1 and M1 synthetic v2 are incompatible. No discovery, adoption,
  migration/import, dual runtime, repair or unavailable-daemon write fallback.
- Only the native daemon links the production writer. CLI/UI use typed local IPC;
  operator read/control authority never implies the separate epoch-bound write grant.
- Preferences begin suggested and require exact-record human confirmation. Project
  choices mask global ones but never override repository contracts.
- MCP exposes exactly `get_effective_context` and `get_handoff`, bound to one project.
  Agents/session grants cannot inspect administrative previews or mutate Profiles.
- Live document contents require explicit opt-in, narrow allowlists and bounded,
  contained reads. Handoff checks and fingerprints are historical observations,
  not fresh verification, execution authorization or complete worktree attestation.
- No secrets, credentials, device identifiers, raw conversations or real profile
  data belong in tracked files. Source retirement does not delete local user data.

## Source layout

```text
crates/
  acp-core/       # domain, typed resolver and bounded pure Fingerprint v1
  acp-store/      # strict read-only Profile v1 access
  acp-write/      # private transactions, daemon-only owner
  acp-ipc/        # local transport, scoped credentials, grants and leases
  acp-daemon/     # service and sole write owner
  acp-query/      # explicit native operator CLI
  acp-mcp/        # project-bound read-only stdio bridge
  acp-session/    # optional native ACP session supervisor
apps/operator/    # accepted NSH Desktop/TUI app workspace: model/, ui/, tests/,
                  # plus the optional offline Web fixture (formerly spikes/p1-e)
 tools/          # native validation helpers and Node maintenance checks
 docs/           # contracts, ADRs, current runbooks and marked historical evidence
```

## Design and evidence

- [Product](docs/product.md), [architecture](docs/architecture.md), [interfaces](docs/interfaces.md), [security](docs/security.md), [roadmap](docs/roadmap.md)
- [ADR-0004 trust](docs/adr/0004-product-domain-trust.md), [ADR-0006 Rust-only runtime](docs/adr/0006-rust-only-runtime.md), [accepted NSH direction](docs/adr/0003-ui-framework-selection.md)
- [Profile v1 contract](docs/p1/profile-v1-runtime-contract.md), [S4 inventory/replacement map](docs/p1/python-retirement.md)
- [S3 native evidence](docs/evaluation/p1-m2-s3-native-operator.md), [S4 evidence](docs/evaluation/p1-m2-s4-retire-python.md)

Historical evidence retains its original date, SHA and result. It is not runnable
current setup or a fresh platform/model claim. S3 recorded Windows/ConPTY and WSLg
native Wayland/PTY; S4 distinguishes native process/headless coverage from actual
GUI/TTY observations. macOS, mobile, niri, full IME/accessibility, installation and
release remain unverified at the current source unless explicitly recorded.

Optional native sessions require a trusted explicit configuration and reviewed
Start. Authentication, a single permission decision and opt-in revocable context
are independent. Closing the UI does not stop the child. No shell, automatic agent
startup, persistent resume or remote WSL execution. See [session](docs/p1/agent-session-runtime.md),
[auth](docs/p1/agent-authentication.md) and [context lease](docs/p1/session-context-bridge.md)
contracts; older vendor results remain in their original reports.

The separate [agent-development-protocol](https://github.com/ybakiame/agent-development-protocol)
remains authoritative for neutral engineering procedures. ACP consumes those rules;
it never rewrites a project's AGENTS.md, dependencies or CI automatically.

## License

Licensed under the [Apache License, Version 2.0](LICENSE). Copyright 2026 ybakiame; see
[NOTICE](NOTICE). Third-party dependencies keep their own licenses.
