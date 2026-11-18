# Daemon-owned projects: add and manage projects from any GUI

Status: **accepted plan (2026-11-18), owner-requested.** Not implemented yet. Owner
decisions: one GUI flow adds a project end to end; only Git repositories under the
daemon user's home; every GUI only talks to the trusted daemon; see Decisions.

## Principles

1. **The daemon owns its host's directories.** A project root is always a path on
   the daemon's own system. The Windows GUI driving a WSL daemon and the WSLg GUI
   driving the same daemon see and choose the same Linux directories, through the
   daemon. No GUI opens a file picker, reads the filesystem or sends a path it
   resolved itself; Windows paths never reach a Linux daemon.
2. **GUIs are thin clients of a trusted daemon.** HWND (through `acp-wsl-bridge`),
   WSLg/Linux (local IPC) and the TUI call the same daemon methods and get the
   same answers and refusals. The bridge forwards typed requests; it decides
   nothing.
3. **Authority stays layered.** Browsing directories is an Operator read. Adding or
   removing a project is an operator write: the current Profile's explicit write
   grant plus an exact confirmation. Agents and MCP never browse, add or remove;
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

### Scope rules (one validator, daemon-side, used by browse and add)

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
| `project.browse {path?}` | Operator read | Lists subdirectories of one directory inside `$HOME` (default `$HOME`): name, `git_root`, `registered`; at most 256 entries, hidden directories excluded, no file names or contents. The path is daemon-resolved and re-validated on every call. |
| `RegisterProject` (existing mutation) | OperatorWrite + grant | If the name is not yet in the set and `--projects-file` is configured: validate the root, register it in the Profile, persist the set, mint the project's agent credential, extend session/bridge roots. Same receipts, CAS and replay rules as today. |
| `project.remove {name}` (new mutation) | OperatorWrite + grant | Removes the name from the set: stops its sessions, revokes its agent credential and context leases, persists. Profile records stay; re-adding the same root restores access. The seed (`daemon.env`) cannot be removed from the GUI. |

Order inside an add: validate → Profile transaction → persist the file → apply in
memory → mint the credential. A crash between steps leaves a registered but
unallowed project, which is inert and repaired by adding it again.

### Clients and GUI

- `client`: `project.browse` joins the bridge's `CONTROL_METHODS`; `project.remove`
  joins the mutation path. Error strings stay in the fixed bridge vocabulary.
- `core`: a browser model (current directory, entries, selection, epoch-guarded
  replies like the connection probe) and the add/remove drafts.
- `ui`, Workspace page: the project list (name, root, state) and **Add project…**,
  a dialog with a daemon-backed folder browser (breadcrumb from home, subfolders,
  Git badge, registered badge), the name prefilled from the folder and validated
  like `valid_project`, and a confirmation naming both. Without a write grant the
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
- Bridge integration test for browse/add/remove parity with local IPC.
- Headless UI tests for the dialog states; Windows HWND and WSLg captures.
- Docs: architecture and security (the allowlist is daemon-owned state with a
  seed), D8 superseded for registration only (Handoff put stays CLI-only), the
  dogfood README and the unit.

## Order of work

1. Daemon: project set behind one lock, `--projects-file`, the validator,
   `project.browse`, add through `RegisterProject`, `project.remove`, tests.
2. Client and GUI: bridge methods, browser model, Workspace page and dialogs.
3. Packaging and docs: the unit argument, `daemon.env` as seed, README, acceptance.

## Decisions (owner, 2026-11-18)

- **Remove:** in the first version; allowlist-only, Profile records kept, behind a
  destructive confirmation.
- **Seed:** `--allow-*` in `daemon.env` stays a read-only seed merged at start;
  seeded projects show in the GUI but cannot be removed there.
- **Discovery:** browse only; no scan of the home directory.
- **Windows-local daemon:** the same rules with `%USERPROFILE%` later; out of scope
  for the WSL dogfooding target.
