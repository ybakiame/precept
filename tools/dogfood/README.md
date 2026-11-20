# Private dogfooding: Windows Desktop + ACP daemon in WSL2

Owner-only local setup for daily use of the primary workflow: the Windows HWND
Desktop operates an ACP daemon inside an Arch Linux WSL2 distribution through
`acp-wsl-bridge` (`wsl.exe --exec`, no shell, no TCP). This is **not** a release:
outputs stay in ignored `.tmp/artifacts/`, are never committed or uploaded, and carry no
support, signing or clean-machine claim.

| Side | Build | Install |
| --- | --- | --- |
| Windows | `tools/portable/windows-package.ps1` (portable ZIP, unchanged recipe) | `windows-deploy.ps1` into `%LOCALAPPDATA%\Programs\ACP` (portable; no installer, PATH, registry or Start menu) |
| WSL2 Arch | `arch-package.sh` (pacman package from one clean commit) | explicit `sudo pacman -U`, then the systemd **user** unit `acp-daemon.service` |

The Arch package ships `acp-daemon`, `acp-query`, `acp-mcp`, `acp-wsl-bridge` and
`acp-nsh-tui` in `/usr/bin`, the unit in `/usr/lib/systemd/user/`, and this file
in `/usr/share/doc/agent-control-plane/`. There is no
daemon configuration file: projects and the startup Profile are chosen in the
Desktop (or TUI, or `acp-query`) and kept by the daemon itself.
Runtime dependencies are checked against the built binaries' direct libraries.

## Build

```sh
# WSL2 Arch, clean committed checkout, offline with the existing Cargo cache
tools/dogfood/arch-package.sh dogfood-linux-01
sudo pacman -U .tmp/artifacts/local-dogfood/dogfood-linux-01/agent-control-plane-*.pkg.tar.zst
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
   systemctl --user enable --now acp-daemon.service
   sudo loginctl enable-linger "$USER"
   ```
2. Create a Profile once, explicitly: the Desktop shows **Set up your Profile**
   (Dashboard, Workspaces, Connection) with the daemon's suggested path and
   **Select at start** on, so every daemon start selects it. Or:
   ```sh
   R=~/.local/state/acp/runtime
   acp-query --runtime-dir $R --endpoint $R/control.sock profile create --remember ~/.local/state/acp/profile.sqlite3
   ```
   The daemon never creates or adopts a Profile by itself. **Forget** (or
   `profile forget-startup`) stops selecting it at start; the Profile stays.
3. Writes are per daemon run and never remembered. **Enable writes and add** in
   Workspaces turns them on with the add; Connection → Profile turns them on or
   off directly (or `acp-query ... profile write enable --profile-id ...
   --generation ... --confirm`).
4. Add projects in **Workspaces → Add project…**: choose a folder (a Git
   repository root in your Linux home; on Windows the dialog opens in
   `\\wsl.localhost\<distro>\home\<user>`) or type its path; the daemon checks
   it, then adds and registers it. Or `acp-query ... project check|add|remove`.
   Removing a project keeps its Profile records; adding it again rejoins them.
5. Let Claude Code, Codex or another MCP client read a project: **Agents**
   (set-up step 4) shows each project's sharing switch and the exact text to paste
   (a `claude mcp add` command, a Codex `config.toml` table, or MCP JSON; on
   Windows it runs `acp-mcp` through `wsl.exe`). There is no port: the client
   starts `acp-mcp` over stdio. Or `acp-query ... project mcp|share|unshare NAME`.

Status and logs: `systemctl --user status acp-daemon`, `journalctl --user -u acp-daemon`.
Agent activity (did a configured MCP client start `acp-mcp` and read?): Agents →
**Recent agent activity** in the Desktop, or `acp-query --runtime-dir $R --endpoint
$R/control.sock activity --project NAME`. It is kept in daemon memory since the
last start, by count only. `acp-mcp` also writes one line per failure to stderr,
which the MCP client shows in its own server log.
Stop is SIGTERM (graceful: capabilities removed, lease released).
Exit status 2 (a configuration or safety refusal, e.g. a damaged state file) is not
restarted; fix the cause. The daemon never repairs its state to make itself start.

Paths (fixed in the unit): state in `~/.local/state/acp` (0700: `projects.json`,
`startup.json`), runtime in `.../runtime`, socket `.../runtime/control.sock`.

Upgrading from a build that used `/etc/acp/daemon.env`: pacman removes it (or
leaves `daemon.env.pacsave` if you edited it, which nothing reads any more).
Re-add your projects once in **Workspaces**; a Profile that already has them
just rejoins its records, and select the Profile once with **Select at start**.

## Desktop (Windows)

The Desktop's **Connection** page lists your WSL distributions; choosing one runs
a read-only check inside it (`acp-wsl-bridge --discover`: systemd, this unit, the
daemon, the Profile) and offers **Start** when the unit is installed but not
running. **Connect** saves the choice (`%LOCALAPPDATA%\ACP-state\connection.json`,
owner-only) for the next launch. Explicit launch arguments always win.

Start `%LOCALAPPDATA%\Programs\ACP\ACP Desktop (WSL).lnk`. It runs
`bin\acp-nsh-native.exe` with `--wsl-distribution/--wsl-user/--wsl-bridge
/usr/bin/acp-wsl-bridge/--wsl-runtime-dir/--wsl-endpoint` pointing at the paths above.
If the WSL distribution was stopped, the first connection starts it. If the daemon
is not ready yet, the Desktop reports it as unreachable; retry from the Desktop.

## Remove

- Windows: close the Desktop. Remove the files listed in `runtime\manifest.json`,
  the manifest, the shortcut, then the empty directories.
- WSL: `systemctl --user disable --now acp-daemon`, then `sudo pacman -R agent-control-plane`.
  `~/.local/state/acp` and `~/.config/acp` (Profile and configuration) are kept;
  delete them only deliberately.
