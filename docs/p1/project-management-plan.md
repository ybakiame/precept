# Daemon-owned projects: add and manage projects from any GUI

Status: **accepted plan (2026-11-18), owner-requested.** Not implemented yet. Owner
decisions: one GUI flow adds a project end to end; only Git repositories under the
daemon user's home; every GUI only talks to the trusted daemon; see Decisions.

## Principles

1. **The daemon owns its host's directories.** A project root is always a path on
   the daemon's own system. The Windows GUI driving a WSL daemon and the WSLg GUI
   driving the same daemon see and choose the same Linux directories, through the
   daemon. A GUI never reads the filesystem itself: the user picks one folder in
   the operating system's own folder dialog (or types a path), and the daemon
   alone decides whether that one path is acceptable. The daemon never lists or
   walks directories for a client. Windows paths never reach a Linux daemon.
2. **GUIs are thin clients of a trusted daemon.** HWND (through `acp-wsl-bridge`),
   WSLg/Linux (local IPC) and the TUI call the same daemon methods and get the
   same answers and refusals. The bridge forwards typed requests; it decides
   nothing.
3. **Authority stays layered.** Checking one chosen path is an Operator read. Adding or
   removing a project is an operator write: the current Profile's explicit write
   grant plus an exact confirmation. Agents and MCP never check, add or remove;
   an agent credential is only ever minted for a project after it exists.

## Today

- The allowlist (`--allow-project` / `--allow-root`) is fixed at daemon start and
  feeds five places: Profile root checks (`profiles.rs`), the operator project
  list (`project.list`), one agent credential per project (`acp-ipc`
  `RuntimeLease::credentials`), the session supervisor's refusal of agent
  executables/config inside project roots, and the context bridge's roots.
- `RegisterProject` is an existing typed OperatorWrite mutation, CLI-only by
  decision D8 ([review contract](operator-review-contract.md)). It requires the
  name to be allowlisted already.
- The bridge forwards a fixed list of Operator methods (`CONTROL_METHODS`).

## Design

### Daemon: the project set becomes daemon state

- New explicit argument `--projects-file <abs path>` (the packaged unit passes
  `%S/acp/projects.json`). Absent: adding stays refused, exactly as today.
- The file is daemon-owned: private (0600 in a 0700 directory, acp-ipc checks),
  written atomically, `{format: 1, projects: [{name, root}]}`, bounded (64
  projects, as today). At start, the effective set is the `--allow-*` seed plus the
  file; a name bound to two roots, an invalid entry or a non-private file refuses
  to start (exit 2, no restart loop).
- The set moves behind one lock shared by the five consumers above, so an add or
  remove applies to all of them at once, without a restart.

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

### New daemon methods

| Method | Access | Purpose |
| --- | --- | --- |
| `project.check {root}` | Operator read | Validates one user-chosen path with the scope rules and answers `ok` plus a suggested name (the folder name), or one fixed refusal reason. It reads only that path and its ancestors (stat, `.git`, owner, mount); it never lists a directory. |
| `RegisterProject` (existing mutation) | OperatorWrite + grant | If the name is not yet in the set and `--projects-file` is configured: validate the root, register it in the Profile, persist the set, mint the project's agent credential, extend session/bridge roots. Same receipts, CAS and replay rules as today. |
| `project.remove {name}` (new mutation) | OperatorWrite + grant | Removes the name from the set: stops its sessions, revokes its agent credential and context leases, persists. Profile records stay; re-adding the same root restores access. The seed (`daemon.env`) cannot be removed from the GUI. |

Order inside an add: validate → Profile transaction → persist the file → apply in
memory → mint the credential. A crash between steps leaves a registered but
unallowed project, which is inert and repaired by adding it again.

### Clients and GUI

- `client`: `project.check` joins the bridge's `CONTROL_METHODS`; `project.remove`
  joins the mutation path. Error strings stay in the fixed bridge vocabulary.
- `core`: the add draft (path, name, check state with epoch-guarded replies like
  the connection probe) and the remove draft.
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
- The same flow on Windows HWND, WSLg/Linux and the TUI (a keyboard list there).
  Motion inventory and finishing checklist per the Design Guides before code.

## Verification

- Daemon real-process tests: every scope refusal (outside home, `/mnt`, symlink
  escape, nested/overlapping roots, runtime/state/Profile overlap, not a Git root,
  foreign owner); no write grant; agent and MCP credentials refused; add applies
  to credentials, sessions and the context bridge without restart; persistence
  and seed conflicts across restart; crash between steps; remove revokes.
- Bridge integration test for check/add/remove parity with local IPC.
- UNC-to-Linux mapping unit tests: both prefixes, case, other distribution, drive
  paths, `..`, trailing separators; the daemon refusal still applies after it.
- Headless UI tests for the dialog states; Windows HWND and WSLg captures.
- Docs: architecture and security (the allowlist is daemon-owned state with a
  seed), D8 superseded for registration only (Handoff put stays CLI-only), the
  dogfood README and the unit.

## Order of work

1. Daemon: project set behind one lock, `--projects-file`, the validator,
   `project.check`, add through `RegisterProject`, `project.remove`, tests.
2. Client and GUI: bridge methods, the add draft, the folder dialog and path
   mapping, Workspace page and dialogs.
3. Packaging and docs: the unit argument, `daemon.env` as seed, README, acceptance.

## Decisions (owner, 2026-11-18)

- **Remove:** in the first version; allowlist-only, Profile records kept, behind a
  destructive confirmation.
- **Seed:** `--allow-*` in `daemon.env` stays a read-only seed merged at start;
  seeded projects show in the GUI but cannot be removed there.
- **Choosing a folder:** the system folder dialog (or a typed path), then one
  daemon check; the daemon never browses or scans the home directory
  (owner, 2026-11-18, replacing the earlier browse design).
- **Windows-local daemon:** the same rules with `%USERPROFILE%` later; out of scope
  for the WSL dogfooding target.
