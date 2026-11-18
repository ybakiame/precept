# Daemon-owned state: projects and the startup Profile, managed from any GUI

Status: **accepted plan (2026-11-18), owner-requested.** Not implemented yet. One
design replaces the startup allowlist and `daemon.env`; see Decisions.

## Principles

1. **The daemon owns its host's directories.** A project root is always a path on
   the daemon's own system. The Windows GUI driving a WSL daemon and the WSLg GUI
   driving the same daemon choose the same Linux directories. A GUI never reads
   the filesystem itself: the user picks one folder in the operating system's own
   folder dialog (or types a path), and the daemon alone decides whether that one
   path is acceptable. The daemon never lists or walks directories for a client.
   Windows paths never reach a Linux daemon.
2. **GUIs are thin clients of a trusted daemon.** HWND (through `acp-wsl-bridge`),
   WSLg/Linux (local IPC) and the TUI call the same daemon methods and get the
   same answers and refusals. The bridge forwards typed requests; it decides
   nothing.
3. **Authority stays layered.** Checking one chosen path is an Operator read.
   Adding or removing a project, and remembering a Profile for start, are
   operator writes or explicit operator choices; write access itself is never
   remembered. Agents and MCP never check, add, remove or remember anything; an
   agent credential is only ever minted for a project after it exists.
4. **One configuration model.** Everything the operator decides lives in
   daemon-owned state, changed through the daemon. There is no argument file to
   edit: the packaged unit's command line is fixed.

## Today (what this replaces)

- The allowlist (`--allow-project` / `--allow-root`) is fixed at daemon start and
  feeds five places: Profile root checks (`profiles.rs`), the operator project
  list (`project.list`), one agent credential per project (`acp-ipc`
  `RuntimeLease::credentials`), the session supervisor's refusal of agent
  executables/config inside project roots, and the context bridge's roots.
- `--database` selects a Profile at start; otherwise a restarted daemon has none
  until the operator selects one again.
- The dogfood unit reads `/etc/acp/daemon.env` and `~/.config/acp/daemon.env` for
  all of these plus `--agent-config` / `--context-mcp`.
- `RegisterProject` is an existing typed OperatorWrite mutation, CLI-only by
  decision D8 ([review contract](operator-review-contract.md)); it requires the
  name to be allowlisted already.
- The bridge forwards a fixed list of Operator methods (`CONTROL_METHODS`).

## Design

### Daemon state directory

New required argument `--state-dir <abs path>` (the unit passes `%S/acp`, i.e.
`~/.local/state/acp`): private (0700, owned by the daemon user, not inside a Git
checkout), checked with acp-ipc's private-directory rules. The runtime directory
stays separate (`--runtime-dir`, the unit passes `%S/acp/runtime`). Files in it
are daemon-owned, private (0600), written atomically and bounded:

| File | Content | Changed by |
| --- | --- | --- |
| `projects.json` | `{format: 1, projects: [{name, root}]}`, at most 64 | Add / remove project |
| `startup.json` | `{format: 1, profile: "/abs/profile.sqlite3"}` or absent | Select / create Profile with "Select at start", or Forget |

A malformed, non-private or out-of-bounds file refuses to start (exit 2, no
restart loop) with a fixed reason; the daemon never repairs or rewrites state to
make itself start.

### Retired arguments

`--allow-project`, `--allow-root` and `--database` are removed from the daemon,
its tests, the portable packages' instructions and the native acceptance tools.
Tests create projects through `projects.json` fixtures or the add method. The
dogfood `daemon.env` (its `/etc` copy and the unit's `EnvironmentFile` lines) is
removed. `--agent-config`, `--context-mcp` and `--context-lease-seconds` remain
daemon arguments, but the packaged unit passes fixed values (below).

### The project set

Loaded from `projects.json` at start; one set behind one lock shared by the five
consumers above, so an add or remove applies to all of them at once, without a
restart.

### Scope rules (one validator, daemon-side, used by check and add)

A root is accepted only if, after canonicalizing:

- it is inside the daemon user's home (`$HOME`), owned by that user, a directory;
- it is a Git repository root (contains `.git`), not a path inside one;
- it is not under `/mnt`, `/proc`, `/sys`, `/dev`, `/run` or any other mount of
  another filesystem type than `$HOME` (no Windows drives through drvfs);
- no component is a symlink that leaves `$HOME`; it does not contain, and is not
  inside, the runtime, state or Profile directory, or another project's root;
- no configured agent executable or session config lives inside it (the session
  supervisor's existing rule, re-checked for the new root).

### The startup Profile

- `profile.select` and `profile.create` take an optional `remember: bool`; with
  it the daemon writes `startup.json` after the selection succeeded. A separate
  `profile.startup.forget` removes it. `profile.info` reports whether the current
  Profile is the remembered one.
- At start, a remembered Profile is selected through exactly the same checks as
  an explicit `profile.select`. If it fails (missing, moved, now inside a Git
  checkout, schema refused), the daemon starts with no Profile and reports a
  fixed reason in `profile.info`; it never creates, scans for, migrates or
  replaces a Profile, and never write-enables one.
- AGENTS.md's "never adopt a profile automatically" is refined to: never scan for,
  create or adopt a Profile, except the one path the operator explicitly asked to
  select at start.

### Sessions

The unit passes `--context-mcp /usr/bin/acp-mcp --agent-config %E/acp/agents.json`.
Daemon change: an absent agent config file at start means sessions are disabled
(today it refuses to start); a present file that is invalid, non-private or
inside a project root still refuses to start. Creating that JSON file
(`~/.config/acp/agents.json`, the existing session config format) is how sessions
are turned on; each start still needs the explicit context-share opt-in.

### New daemon methods

| Method | Access | Purpose |
| --- | --- | --- |
| `project.check {root}` | Operator read | Validates one user-chosen path with the scope rules and answers `ok` plus a suggested name (the folder name), or one fixed refusal reason. It reads only that path and its ancestors (stat, `.git`, owner, mount); it never lists a directory. |
| `RegisterProject` (existing mutation) | OperatorWrite + grant | If the name is not yet in the set: validate the root, register it in the Profile, persist `projects.json`, mint the project's agent credential, extend session/bridge roots. Same receipts, CAS and replay rules as today. |
| `project.remove {name}` (new mutation) | OperatorWrite + grant | Removes the name from the set: stops its sessions, revokes its agent credential and context leases, persists. Profile records stay; re-adding the same root restores access. |
| `profile.select` / `profile.create` `{…, remember}` | Operator (as today) | As today, plus remembering the Profile for start. |
| `profile.startup.forget` | Operator | Removes `startup.json`. |

Order inside an add: validate → Profile transaction → persist `projects.json` →
apply in memory → mint the credential. A crash between steps leaves a registered
but unlisted project, which is inert and repaired by adding it again.

### Clients and GUI

- `client`: `project.check` and `profile.startup.forget` join the bridge's
  `CONTROL_METHODS`; `project.remove` joins the mutation path. Error strings stay
  in the fixed bridge vocabulary.
- `core`: the add draft (path, name, check state with epoch-guarded replies like
  the connection probe), the remove draft, the "Select at start" choice.
- **Choosing the folder.** "Choose folder…" opens the system folder dialog
  (`nsh_core::pick_paths(OpenDialog::folders())`), starting in the daemon user's
  home:
  - Windows HWND with a WSL daemon: `IFileOpenDialog` starts at
    `\\wsl.localhost\<distribution>\home\<user>`; the GUI maps a returned
    `\\wsl.localhost\<distribution>\…` or `\\wsl$\<distribution>\…` path to
    `/…` and refuses, before sending anything, any path outside the connected
    distribution (a `C:\` folder, another distribution).
  - WSLg / Linux: the GTK or portal folder chooser returns the Linux path.
  - TUI, or a Linux without a file-chooser portal: no dialog; the path field.
  The path field is always present (the dialog fills it; pasting works). The
  mapping is a convenience only: the daemon validates whatever arrives.
- `ui`, Workspace page: the project list (name, root, state) and **Add project…**,
  a dialog with the folder field and "Choose folder…", the daemon's check result
  beside it, the name prefilled from the folder and validated like
  `valid_project`, and a confirmation naming both. Without a write grant the
  button explains why and links to Profile & review. Remove is a destructive
  `AlertDialog` naming the project and stating that Profile records are kept.
- `ui`, Profile & review: create/select get a "Select at start" checkbox (a new
  `ui` Checkbox); the current Profile shows "Selected at start" with **Forget**.
- The same flows on Windows HWND, WSLg/Linux and the TUI (a keyboard list there).
  Motion inventory and finishing checklist per the Design Guides before code.

## Verification

- Daemon real-process tests: every scope refusal (outside home, `/mnt`, symlink
  escape, nested/overlapping roots, runtime/state/Profile overlap, not a Git root,
  foreign owner); no write grant; agent and MCP credentials refused; add applies
  to credentials, sessions and the context bridge without restart; persistence
  across restart; crash between steps; remove revokes; state files that are
  malformed, shared or oversized refuse start; a remembered Profile that
  disappeared or now fails its checks starts unselected and never write-enabled;
  an absent agent config disables sessions, an invalid one refuses start.
- Bridge integration test for check/add/remove/forget parity with local IPC.
- UNC-to-Linux mapping unit tests: both prefixes, case, other distribution, drive
  paths, `..`, trailing separators; the daemon refusal still applies after it.
- Headless UI tests for the dialog states; Windows HWND and WSLg captures.
- Docs: architecture and security (daemon-owned state, retired arguments),
  AGENTS.md's refined Profile rule, D8 superseded for registration only (Handoff
  put stays CLI-only), the portable READMEs, the dogfood README and the unit.

## Order of work

1. Daemon: `--state-dir`, `projects.json` and `startup.json`, the project set behind
   one lock, the validator, `project.check`, add through `RegisterProject`,
   `project.remove`, remembered Profile, optional agent config; retire
   `--allow-*` / `--database` and migrate the daemon, MCP, client and UI tests.
2. Client and GUI: bridge methods, drafts, the folder dialog and path mapping,
   Workspace page, Profile "Select at start", dialogs.
3. Packaging and docs: the fixed unit command line, removal of `daemon.env`,
   portable READMEs, native acceptance tools, AGENTS.md, architecture, security.

## Decisions (owner, 2026-11-18)

- **One design:** no `daemon.env`; `--allow-project`, `--allow-root` and
  `--database` are retired everywhere.
- **Choosing a folder:** the system folder dialog (or a typed path), then one
  daemon check; the daemon never browses or scans the home directory.
- **Remove:** in the first version; leaves the project set only, Profile records
  kept, behind a destructive confirmation.
- **Startup Profile:** remembered only when the operator explicitly asks; write
  access is never remembered.
- **Sessions:** fixed conventional paths (`/usr/bin/acp-mcp`,
  `~/.config/acp/agents.json`); no file means no sessions.
- **Windows-local daemon:** the same rules with `%USERPROFILE%` later; out of scope
  for the WSL dogfooding target.
