# Architecture and data ownership

The [product charter](product.md) defines purpose and authority. This document
describes the current Rust-native architecture and its durable target boundaries;
[interfaces](interfaces.md) defines wire-level behavior and
[security](security.md) defines the threat model.

## Topology

```mermaid
flowchart LR
    Operator["Operator"]
    Clients["NSH desktop<br/>native precept"]
    Daemon["Rust daemon"]
    Core["precept-core"]
    Writer["precept-write"]
    Store["precept-store<br/>read-only"]
    Profile[("Explicit local<br/>Rust-native Profile v1")]
    Agent["Trusted agent"]
    MCP["precept-mcp<br/>read tools + suggest_rule"]

    Operator --> Clients
    Clients -->|"Operator IPC"| Daemon
    Daemon --> Core
    Daemon --> Writer
    Daemon --> Store
    Writer --> Profile
    Store --> Profile
    Agent -->|"MCP stdio"| MCP
    MCP -->|"Project capability"| Daemon
```

The daemon is the only production write owner. Clients exchange typed data and
requests with it; they do not open writable SQLite connections or duplicate domain
resolution.

## Components

| Component | Responsibility | Must not own |
| --- | --- | --- |
| `crates/precept-core` | Preference/Handoff resolution, typed context and conflict resolution, bounded repository document and fingerprint adapters | SQLite connections, UI, process execution, protocol transport or authorization grants |
| `crates/precept-store` | Explicit-path, strict read-only access to fresh Rust-native Profile v1 | Writes, schema migration or legacy compatibility |
| `crates/precept-ipc`, `preceptd` | Local transport, peer/capability checks, scoped reads, project state, leases and daemon-owned writes | TCP listener or caller-selected wider scope |
| `crates/precept-mcp` | Read-only stdio bridge to project-bound IPC | Profile or task mutation |
| `crates/precept-write` | Private Profile v1 creation and typed transactions with lease/epoch fencing, CAS and receipts | Direct UI/CLI access, arbitrary existing-path writers or generic SQL |
| `crates/precept` | Native operator reads and typed writes through authenticated daemon IPC | Direct database access or a second writer |
| `apps/core`, `apps/client`, `apps/ui` | Pure operator state/presentation; native IPC and Windows→WSL2 bridge; NSH desktop surface | Direct database access, duplicate resolution logic or independent authority |

The root product crates and operator app are separate Cargo workspaces with separate
lockfiles. NSH dependencies do not enter the daemon's dependency closure. The Web
UI is an offline synthetic target without native IPC.

## Windows GUI → WSL2

The Windows-native Desktop can operate a daemon inside one explicitly selected WSL2
distribution. WSL owns Linux project paths, daemon and Profile/runtime state. The GUI starts one long-lived `precept-bridge` helper per
connection through `wsl.exe --exec`, without a shell, TCP, port forwarding or
mirrored networking.

The helper loads operator capabilities inside WSL and forwards only typed requests
to the same daemon authority used by local clients. Tokens never cross the stdio
boundary. It cannot validate or approve mutations, open SQLite or start agents. The framing, allowlists and uncertainty behavior are defined
in [interfaces](interfaces.md#windows-gui--wsl2-bridge).

The secondary WSLg Linux GUI path connects directly to the same WSL daemon over
Unix IPC and does not use the bridge. WSLg evidence is not standalone Linux support
evidence.

A Windows Desktop may instead connect to a Windows daemon over its current-user
named pipe. Its conventional local check uses
`%LOCALAPPDATA%/precept/state/runtime` and endpoint `precept`; an explicit Start
action launches the `preceptd.exe` beside the Desktop with the matching private
daemon state directory. This never selects a Profile or enables writes. Linux
local discovery and explicit start continue to use the packaged systemd user
unit.

On Windows, Desktop configuration (`connection.json`, `settings.toml`) lives in
`%LOCALAPPDATA%/precept/config` and daemon state (project/startup records and the
suggested Profile) in `%LOCALAPPDATA%/precept/state`; runtime coordinates and
credentials live in `state/runtime`. On Unix, configuration uses
`$XDG_CONFIG_HOME/precept` (default `~/.config/precept`) and state uses
`$XDG_STATE_HOME/precept` (default `~/.local/state/precept`), with runtime in
`state/runtime`. Environment roots must be absolute; durable state never uses a
cache directory. The app does not fall back to the retired `Precept-state` name
or move existing data automatically.

## Data ownership

| Data | Authority / source of truth | State and change path |
| --- | --- | --- |
| Personal preferences, including project-scoped choices | Human-reviewed user-local SQLite | Explicit operator action; never tracked Profile files |
| Personal sync identity, trust, clocks, tombstones and apply recovery | Daemon-private per-Profile sidecar (`personal-sync-<profile_id>.json`, format 1) | Explicit OperatorWrite pairing/trust/export; preference changes use the current OperatorWrite grant and daemon-owned writer |
| Project contracts | Repository-owned AGENTS/docs and reviewed engineering configuration | Bounded live reads; no automatic prose interpretation or file creation |
| Work sessions | Daemon-private state (`sessions-<project>.json`, format 2) | Agent `session.*` with flat structured items, single-item operations and required CAS; operator `sessions.delete`; format 1 coordination lists normalize on read |
| Project memory (environment, cautions, lessons, attempts) | Daemon-private state (`memory-<project>.json`); agents' notes, unreviewed, below rules and code | Agent `memory.*` where the operator allows agent writes; operator `memories.*` edits, restores, pins, deletes and clears; confirmed environment notes move in by one operator-write |
| Project registration | User-local Profile plus daemon-private project state | Operator-write `project.add` / `project.remove` after checking one chosen path; `project.register` records a listed project in a newly selected Profile at its pinned root |
| Suggestions and reviews | User-local SQLite proposal/review records | Approval binds exact content, revision and scope; repository changes still need repository review |
| Effective context | Derived from current sources and review state | Any cache requires source versions and invalidation; never outranks sources |
| Session constraints and Handoff | Historical local records | Frozen references/content versions describe history, not live policy or execution authority |
| Provenance/evidence | Minimal local records with reporter/observer/verifier attribution | No raw transcript or credential store; export requires explicit selection |

The daemon's private state contains the registered project set and optional startup
Profile choice with its explicit write-restoration choice. Each project entry stores its canonical root and persistent filesystem
identity (Unix device/inode or Windows volume/file index). Registration is not
permission to discover or share every project. The daemon checks only the path chosen
by the operator and refuses system, control and otherwise ineligible roots; Unix
also enforces the daemon uid and home filesystem. Entries without identity, or whose
current root does not match the stored
identity, stay listed but unavailable; the daemon never adopts or repairs them
automatically. The operator can explicitly remove and re-add such a project. GUI,
bridge and agent clients never scan directories.

## Profile and write ownership

Only fresh Rust-native Profile v1 (`PRAGMA user_version=1`) is accepted. It is not
compatible with the retired Python prototype's v1 or the historical synthetic v2.
There are no legacy readers, in-place migrations, downgrades, repair paths or
runtime fallbacks. Moving data between schemas uses only the operator's explicit,
versioned Profile export and import; imported rules still require exact review.

Profile creation is explicit and never overwrites an existing file. Startup selects
a Profile only when the operator requested that exact path. Write enablement is
independent and bounded to the Profile/generation/owner epoch. The startup record
remembers only the operator's explicit choice to restore writes for that Profile;
each run issues a fresh credential, and disabling writes or forgetting startup
clears restoration.

Only the daemon links and invokes `precept-write`. Operator read/control and write
capabilities are separate. Transactions use exact typed commands, expected
revisions or absence, immutable operation IDs, request digests, same-epoch receipts
and strict preference transitions. Disabling writes drains admitted work and
prevents new admission. An uncertain transport result may already have committed;
only an explicit identical retry can settle it.

During validation, coordinated wire-schema and Profile-schema breaking changes are
acceptable. Do not introduce dual-version support or migration layers.

## Repository contracts and context resolution

Repository rules are current project sources, not personal Profile values. They
live in `.precept/rules/<id>.md`: one rule per file, TOML front matter between `+++`
lines (`key`, optional `title`, `allowed`, `tags`, `priority`, `enforcement`,
`[applies_to]`, `[[check]]`, `[[guard]]`; unknown fields refuse the file) and the
rule's Markdown text below. At most 64 files of 16 KiB each are read through a pinned
root handle and descriptor-relative no-follow traversal; links and Windows reparse
points are never followed. A file Git tracks with no uncommitted change is reviewed through
the repository and served; any other file is listed as pending (id and key only)
and masks nothing. Any invalid file refuses the whole source, naming the file.

A served rule replaces Profile preferences of its key where it applies; one with
`allowed` values keeps preferences whose value it allows and reports unresolved
conflicts. Checks (a file exists, contains or lacks a substring, or a TOML/JSON
value equals an expected one) run read-only against the repository on every read.
One resolution shares a pinned-root checked-file cache, including parsed TOML/JSON,
across all applicable rules. Each file is at most 256 KiB; a resolution
reads at most 64 distinct files and 1 MiB of checked-file content in aggregate.
Exhaustion marks the affected checks `unavailable` with `budget_exhausted`, never
`verified`, and `contract_resolution.check_io` reports reads, bytes, cache size
and exhaustion. Each served rule carries `verified`, `failing` (with the first failure),
`unchecked` or `unavailable`. Existing allowlisted documents remain valid document
sources. Ordinary repository prose is not automatically parsed into rules. The
only prose-import path is an explicit Operator `import.scan` over the fixed,
bounded agent-rule inventory in [interfaces](interfaces.md); it remains a read,
and selected text enters the Profile only as suggestions through the normal
reviewed write path. Onboarding never scans, scaffolds or rewrites AGENTS.md or
rule files.

The resolver binds one registered project, rechecks registration and reads live
Git/allowlisted metadata. A compatible project preference masks a global preference
for the same key; within one scope/key, the newest confirmed ID wins. Suggested,
rejected and deprecated records are excluded. Document bodies are explicit opt-in.

Typed resolution represents applicability, masking, conflicts and unknown inputs
explicitly. Caller-supplied selector labels affect applicability only; they never
grant authorization. Repository claims outrank personal confirmation. Missing or
malformed source data refuses or narrows affected output rather than widening it.
Private resolution explanations are not exposed wholesale to agents.

Handoff lookup is exact project/task and compares current HEAD, branch, worktree and
bounded content fingerprint with historical state. Missing observations remain
unknown. Historical validation JSON and source labels do not constitute fresh
verification or a complete provenance engine.

## Trust

MCP is project-bound: one server binds to the registered project of the session's
folder and serves nothing elsewhere. Local IPC separates operator, operator-write
and project-bound agent capabilities; OS user identity alone does not grant an
operator role. Unix sockets and Windows named pipes remain local, without a browser
gateway.

Agent-reported actions, daemon observations and independent verification are
separate claims. Checksums detect specified content differences, not reporter
identity or event truth. Same-user filesystem access bypasses cooperative protocol
boundaries; see [security](security.md).
