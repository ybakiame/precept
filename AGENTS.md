# AGENTS.md — Agent Control Plane

## Repository records

Write all repository records in English, including documentation, code comments,
tests, fixtures, commit messages and local work records.

Keep only durable, currently maintained contracts in tracked documentation:
product behavior, architecture, operator/agent interfaces, security boundaries and
current UI standards. Put development plans, design drafts, architectural decision
records, test records, ad-hoc analyses and work logs in `.tmp/work/`. Put generated
local output in `.tmp/artifacts/`. Both locations are ignored; tracked source,
builds and tests must never require their contents.

Do not put process material in `docs/` or application directories. README,
CHANGELOG and release notes are for people using the software, not milestone
histories, design deliberations or validation ledgers. When moving or removing a
document, update every tracked reference and check the final diff and links.

## Product and authority

ACP is a local-first developer context and policy layer, not another agent harness.
Current code and tracked project contracts are authoritative. Material under
`.tmp/` is local or historical context only; it never overrides the active tree and
is not fresh validation evidence.

Read the relevant contracts before making changes:

- [Product charter](docs/product.md)
- [Architecture and data ownership](docs/architecture.md) before architecture or
  persisted-behavior changes
- [Interfaces](docs/interfaces.md) before IPC, MCP or CLI changes
- [Security and privacy](docs/security.md) before authorization, filesystem,
  credential or process-boundary changes
- [UI standards](docs/product-ui.md) before operator UI changes

Repository contracts and current repository facts outrank personal preferences and
Handoffs. Session constraints may narrow work but never grant permissions or weaken
contracts. Code/contract disagreements require explicit human resolution.

## Runtime invariants

- Rust is the only supported product runtime. Do not restore the retired Python
  runtime, validation dependency, compatibility shim or release path.
- The only product schema is fresh Rust-native Profile v1
  (`PRAGMA user_version=1`). Legacy Python v1 and synthetic v2 are incompatible
  historical formats. Do not add legacy readers, migration, downgrade or fallback.
- The native daemon is the only production write owner and the only caller of the
  private `acp-write` implementation. `acp-store` remains read-only.
- UI, `acp-query`, MCP and agents never receive writable SQLite connections or raw
  SQL access. There is no daemon-unavailable direct-write fallback.
- Operator writes require a separately versioned, typed, profile-scoped local IPC
  capability. Operator read grants do not imply write authority.
- MCP exposes exactly the two project-bound read-only tools. Agents, prompts,
  Handoffs and session context cannot confirm or mutate Profile state.
- Profile creation and selection are explicit operator actions. Never discover,
  adopt, migrate, reset, delete or overwrite a Profile automatically. Startup
  selection occurs only for the explicitly requested path; write access is never
  remembered.
- Projects live in daemon-private state and change only through operator-write
  `project.add` / `project.remove` after a daemon-side check of the chosen path.
  No GUI, bridge or agent scans directories.
- Preference proposals start as suggested. Confirmation requires an explicit
  operator action on the exact record and revision.

## Engineering and validation

Use the Rust workspaces and repository CI as the source of truth. The root product
crates and `apps/` are separate Cargo workspaces with separate lockfiles.

For root Rust changes, run:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
```

For UI changes, run the equivalent `apps/Cargo.toml` format, Clippy and test gates
with `acp-ui/native,acp-ui/tui`, plus affected shared presentation
and real-daemon integration checks. Run native Windows and Linux/WSL acceptance
when the changed boundary is platform-sensitive. Headless tests do not prove native
window, keyboard, terminal, accessibility or device behavior.

For documentation or source-layout changes, run `node tools/check_source.mjs` after
installing the locked `apps/web-smoke` Node dependencies when they are absent.
Store validation output under `.tmp/`, not in tracked documentation.

Add regressions for scope isolation, stale worktrees, malformed requests,
authorization/revocation, schema refusal, CAS/replay, process concurrency and
recovery where applicable. Coordinated IPC/MCP and Profile schema breaking
changes are acceptable during validation; do not add old/new version support or
migration layers.

Report the exact source revision, commands, platform and honest
PASS/FAIL/BLOCKED/NOT_RUN outcomes. Never claim hosted CI, macOS, device, model or
clean-machine results unless they actually ran. Historical results are not fresh
evidence.

## Security and data handling

Do not write personal Profile data to tracked files. Persistent Profile state stays
in user-local storage. Never log raw SQLite diagnostics, private paths, preference
values, Handoff bodies, credentials, raw agent conversations or device identifiers.
Do not store secrets or personal data in fixtures.

Do not delete ignored local data, authentication directories or runtime directories
as part of source cleanup. No agent-facing MCP writes, automatic preference
approval, network listener, automatic repository rewriting, shell execution, OS
sandbox claim or broad execution permission is implied by the current architecture.
