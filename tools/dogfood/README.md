# Private dogfooding: Windows Desktop + Precept daemon in WSL2

Owner-only local setup for daily use of the primary workflow: the Windows HWND
Desktop operates an Precept daemon inside an Arch Linux WSL2 distribution through
`precept-bridge` (`wsl.exe --exec`, no shell, no TCP). This is **not** a release:
outputs stay in ignored `.tmp/artifacts/`, are never committed or uploaded, and carry no
support, signing or clean-machine claim.

| Side | Build | Install |
| --- | --- | --- |
| Windows | `tools/portable/windows-package.ps1` (portable ZIP, unchanged recipe) | `windows-deploy.ps1` into `%LOCALAPPDATA%\Programs\Precept` (portable; no installer, PATH, registry or Start menu) |
| WSL2 Arch | `arch-package.sh` (pacman package from one clean commit) | explicit `sudo pacman -U`, then the systemd **user** unit `preceptd.service` |

The Arch package ships `preceptd`, `precept`, `precept-mcp`, `precept-bridge` and
`precept-tui` in `/usr/bin`, the unit in `/usr/lib/systemd/user/`, and this file
in `/usr/share/doc/precept/`. There is no
daemon configuration file: projects and the startup Profile are chosen in the
Desktop (or TUI, or `precept`) and kept by the daemon itself.
Runtime dependencies are checked against the built binaries' direct libraries.

## Build

```sh
# WSL2 Arch, clean committed checkout, offline with the existing Cargo cache
tools/dogfood/arch-package.sh dogfood-linux-01
sudo pacman -U .tmp/artifacts/local-dogfood/dogfood-linux-01/precept-*.pkg.tar.zst
```

```powershell
# Windows checkout at the same commit (PowerShell 7)
./tools/portable/windows-package.ps1 -OutputParent .tmp/artifacts/local-p1-m5 -Name dogfood-windows-01
./tools/dogfood/windows-deploy.ps1 -AttemptDir .tmp/artifacts/local-p1-m5/dogfood-windows-01 -Distribution <distro> -User <linux-user>
```

`-Locale en|zh-CN` fixes the language in the shortcut; without it the Desktop uses
the language saved in its settings.

To update the Desktop later, close it, build a new attempt and rerun
`windows-deploy.ps1 ... -Replace`. That removes only the files listed in the
installed manifest, and keeps unknown files.

## Daemon service (WSL2)

1. Start it as installed (no projects, no Profile) and keep the user manager
   running without an open WSL shell, so the daemon is up when the Desktop
   connects:
   ```sh
   systemctl --user daemon-reload
   systemctl --user enable --now preceptd.service
   sudo loginctl enable-linger "$USER"
   ```
2. Create a Profile once, explicitly: the Desktop shows **Set up your Profile**
   (Dashboard, Workspaces, Connection) with the daemon's suggested path and
   **Select at start** on, so every daemon start selects it. Or:
   ```sh
   R=~/.local/state/precept/runtime
   precept --runtime-dir $R --endpoint $R/control.sock profile create --remember ~/.local/state/precept/profile.sqlite3
   ```
   The daemon never creates or adopts a Profile by itself. **Forget** (or
   `profile forget-startup`) stops selecting it at start; the Profile stays.
3. Writes are per daemon run and never remembered. **Enable writes and add** in
   Workspaces turns them on with the add; Connection → Profile turns them on or
   off directly (or `precept ... profile write enable --profile-id ...
   --generation ... --confirm`).
4. Add projects in **Workspaces → Add project…**: choose a folder (a Git
   repository root in your Linux home; on Windows the dialog opens in
   `\\wsl.localhost\<distro>\home\<user>`) or type its path; the daemon checks
   it, then adds and registers it. Or `precept ... project check|add|remove`.
   Removing a project keeps its Profile records; adding it again rejoins them.
5. Let Claude Code, Codex or another MCP client read a project: **Agents**
   (set-up step 4) shows each project's sharing switch and the exact text to paste
   (a `claude mcp add` command, a Codex `config.toml` table, or MCP JSON; on
   Windows it runs `precept-mcp` through `wsl.exe`). There is no port: the client
   starts `precept-mcp` over stdio. Or `precept ... project mcp|share|unshare NAME`.

Status and logs: `systemctl --user status preceptd`, `journalctl --user -u preceptd`.
Agent activity (did a configured MCP client start `precept-mcp` and read?): Agents →
**Recent agent activity** in the Desktop, or `precept --runtime-dir $R --endpoint
$R/control.sock activity --project NAME`. It is kept in daemon memory since the
last start, by count only. `precept-mcp` also writes one line per failure to stderr,
which the MCP client shows in its own server log.
Stop is SIGTERM (graceful: capabilities removed, lease released).
Exit status 2 (a configuration or safety refusal, e.g. a damaged state file) is not
restarted; fix the cause. The daemon never repairs its state to make itself start.

Paths (fixed in the unit): state in `~/.local/state/precept` (0700: `projects.json`,
`startup.json`), runtime in `.../runtime`, socket `.../runtime/control.sock`.

Upgrading from a build that used `/etc/precept/daemon.env`: pacman removes it (or
leaves `daemon.env.pacsave` if you edited it, which nothing reads any more).
Re-add your projects once in **Workspaces**; a Profile that already has them
just rejoins its records, and select the Profile once with **Select at start**.

## Desktop (Windows)

The Desktop's **Connection** page lists your WSL distributions; choosing one runs
a read-only check inside it (`precept-bridge --discover`: systemd, this unit, the
daemon, the Profile) and offers **Start** when the unit is installed but not
running. **Connect** saves the choice (`%LOCALAPPDATA%\Precept-state\connection.json`,
owner-only) for the next launch. Explicit launch arguments always win.

Start `%LOCALAPPDATA%\Programs\Precept\Precept Desktop (WSL).lnk`. It runs
`bin\precept-desktop.exe` with `--wsl-distribution/--wsl-user/--wsl-bridge
/usr/bin/precept-bridge/--wsl-runtime-dir/--wsl-endpoint` pointing at the paths above.
If the WSL distribution was stopped, the first connection starts it. If the daemon
is not ready yet, the Desktop reports it as unreachable; retry from the Desktop.

## Remove

- Windows: close the Desktop. Remove the files listed in `runtime\manifest.json`,
  the manifest, the shortcut, then the empty directories.
- WSL: `systemctl --user disable --now preceptd`, then `sudo pacman -R precept`.
  `~/.local/state/precept` and `~/.config/precept` (Profile and configuration) are kept;
  delete them only deliberately.
