# AGENTS.md — Precept

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

Precept is a local-first developer context and policy layer, not another agent harness.
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
- Each release line has exactly one fresh Rust-native Profile schema. Do not add
  legacy readers, in-place migration, downgrade or fallback; moving data between
  schemas goes only through the operator's versioned Profile export and import.
  Legacy Python v1 and synthetic v2 remain unsupported historical formats.
- The native daemon is the only production write owner and the only caller of the
  private `precept-write` implementation. `precept-store` remains read-only.
- UI, `precept`, MCP and agents never receive writable SQLite connections or raw
  SQL access. There is no daemon-unavailable direct-write fallback.
- Operator writes require a separately versioned, typed, profile-scoped local IPC
  capability. Operator read grants do not imply write authority.
- MCP tools are project-bound; tool arguments never widen scope. Agents may read
  and, with the operator-granted `propose` capability, create suggestions and
  their own session records. No agent, prompt or Handoff can confirm, reject,
  edit, delete or enable a reviewed record, register a project or change an
  integration.
- Profile creation and selection are explicit operator actions. Never discover,
  adopt, migrate, reset, delete or overwrite a Profile automatically. Startup
  selection occurs only for the explicitly requested path. Only an operator's
  explicit write enablement for that remembered Profile is restored on later
  runs, and only while its saved profile_id and generation match exactly;
  otherwise the Profile stays read-only. Disabling writes or forgetting startup
  clears restoration, and no write token is persisted.
- Projects live in daemon-private state and change only through operator-write
  `project.add` / `project.remove` after a daemon-side check of the chosen path.
  No GUI, bridge or agent scans directories.
- Rule proposals, including automatically mined ones, start as suggested.
  Confirmation requires an explicit operator action on the exact record and
  revision.
- Precept writes repository or harness files only on an explicit operator action
  with a preview, only inside its own markers or hook groups (or as reviewed
  contract files the user commits), and always revocably. It never commits,
  pushes or installs integrations on its own.
- Hooks are cooperative compliance, not a security boundary. Report achieved
  levels (delivered, verified, enforced) exactly; never infer one from another.

## Engineering and validation

Use the Rust workspaces and the gates below as the source of truth; run them on
the maintainer's machine before pushing. The `Gates` workflow runs them on hosted
runners for every pull request and every push to `dev`. Releases are
built only by the `Release` workflow on a pushed `v<version>` tag, which creates a
draft Release; publishing it stays a manual maintainer action. The
root product crates and `apps/` are separate Cargo workspaces with separate
lockfiles.

For root Rust changes, run:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
```

For UI changes, run the equivalent `apps/Cargo.toml` format, Clippy and test gates
with `precept-ui/native`, plus affected shared presentation
and real-daemon integration checks. Run native Windows and Linux/WSL acceptance
when the changed boundary is platform-sensitive. Headless tests do not prove native
window, keyboard, accessibility or device behavior.

For documentation or source-layout changes, run `node tools/check_source.mjs` after
installing its locked Node dependencies (`npm ci --prefix tools`) when they are absent.
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
as part of source cleanup. No automatic rule approval, network listener,
unpreviewed repository or harness writes, daemon shell execution, OS sandbox claim
or broad execution permission is implied by the current architecture.
