# AGENTS.md — Agent Control Plane

## Product and authority

Keep ACP a local-first developer context and policy layer, not another agent harness.
Current repository facts, project-owned contracts and accepted ADRs outrank stored
preferences or Handoffs. Never infer project truth from stale memory.

Read these before changing architecture or persisted behavior:
- [Product charter](docs/product.md)
- [Architecture](docs/architecture.md)
- [ADR-0004: domain, ownership and evidence trust](docs/adr/0004-product-domain-trust.md)
- [ADR-0005: storage ownership](docs/adr/0005-local-storage-and-write-ownership.md), only where not superseded
- [ADR-0006: Rust-only runtime and Python prototype retirement](docs/adr/0006-rust-only-runtime.md)

## Accepted runtime direction

- Rust is the only supported product runtime. The Python P0 implementation is
  legacy and scheduled for removal by P1-M2. Do not add Python features, runtime
  dependencies, compatibility shims, new Python tests, or Python release paths.
- Profile v2 is the only supported runtime schema. Do not add v0/v1 runtime
  readers, automatic migration, downgrade, or compatibility fallback.
- The native daemon is the only production write owner. Only the daemon may invoke
  the private `acp-write` implementation.
- `acp-store` remains read-only. UI, `acp-query`, MCP and agents never receive
  writable SQLite connections or raw SQL access.
- Operator writes use a separately versioned, typed, profile-scoped local IPC
  capability. Operator read/session grants do not imply write authority.
- MCP retains exactly the two project-bound read-only tools. Agents, prompts,
  Handoffs and session context cannot confirm or mutate profile state.
- Profile creation is an explicit operator action. Never scan for, adopt, migrate,
  reset, delete or overwrite an existing profile automatically.
- No Python rollback owner, Python read-only adapter, v2-to-v1 downgrade, or
  daemon-unavailable direct-write fallback.

Until P1-M2 removes the Python prototype from the tree, treat it as frozen legacy
code only. Existing baseline checks may still run when needed to establish an
unmodified comparison, but do not extend its behavior or treat it as the target.

## Engineering and validation

Use Rust workspace conventions and the repository CI as the source of truth.
For Rust changes, run formatting, Clippy with warnings denied, workspace/package
tests, and targeted real-process IPC/daemon/MCP tests appropriate to the change.
For UI changes, run the affected NSH and shared presentation checks. Run native
Windows and Linux/WSL acceptance when the changed boundary is platform-sensitive.
Do not claim hosted CI, macOS, device, model, or clean-machine results unless they
actually ran.

Add regressions for scope isolation, stale worktrees, malformed requests,
authorization/revocation, schema refusal, CAS/replay, process concurrency and
recovery where applicable. Check the final diff, links, secret handling and scope.
Report exact source revision, commands, platform, PASS/FAIL/BLOCKED/NOT_RUN and
limitations honestly.

## Security and data handling

Do not write personal profile data to tracked repository files. Persistent profile
state stays in user-local storage. Never log raw SQLite diagnostics, private paths,
preference values, Handoff bodies, credentials, raw agent conversations or device
identifiers. Do not store secrets or personal data in test fixtures.

All preference proposals start as suggested. Confirmation requires an explicit
operator action on the exact record/revision. Preserve ADR-0004 authority ordering:
repository contracts outrank personal preferences; session constraints may narrow
work but never grant permissions or weaken contracts. Historical Handoff validation
is not fresh evidence.

No agent-facing MCP writes, automatic preference approval, network listener,
automatic repository rewriting, shell execution, OS sandbox claim, or broad
execution permission is implied by the Rust-only migration.
