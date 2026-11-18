# Private dogfooding: Windows Desktop + ACP daemon in WSL2

Owner-only local setup for daily use of the primary workflow: the Windows HWND
Desktop operates an ACP daemon inside an Arch Linux WSL2 distribution through
`acp-wsl-bridge` (`wsl.exe --exec`, no shell, no TCP). This is **not** a release:
outputs stay in ignored `artifacts/`, are never committed or uploaded, and carry no
support, signing or clean-machine claim.

| Side | Build | Install |
| --- | --- | --- |
| Windows | `tools/portable/windows-package.ps1` (portable ZIP, unchanged recipe) | `windows-deploy.ps1` into `%LOCALAPPDATA%\Programs\ACP` (portable; no installer, PATH, registry or Start menu) |
| WSL2 Arch | `arch-package.sh` (pacman package from one clean commit) | explicit `sudo pacman -U`, then the systemd **user** unit `acp-daemon.service` |

The Arch package ships `acp-daemon`, `acp-query`, `acp-mcp`, `acp-wsl-bridge` and
`acp-nsh-tui` in `/usr/bin`, the unit in `/usr/lib/systemd/user/`, the default
configuration `/etc/acp/daemon.env` (a pacman backup file: upgrades keep your
edits) and this file in `/usr/share/doc/agent-control-plane/`.
Runtime dependencies are checked against the built binaries' direct libraries.

## Build

```sh
# WSL2 Arch, clean committed checkout, offline with the existing Cargo cache
tools/dogfood/arch-package.sh dogfood-linux-01
sudo pacman -U artifacts/local-dogfood/dogfood-linux-01/agent-control-plane-*.pkg.tar.zst
```

```powershell
# Windows checkout at the same commit (PowerShell 7)
./tools/portable/windows-package.ps1 -OutputParent artifacts/local-p1-m5 -Name dogfood-windows-01
./tools/dogfood/windows-deploy.ps1 -AttemptDir artifacts/local-p1-m5/dogfood-windows-01 -Distribution <distro> -User <linux-user>
```

To update the Desktop later, close it, build a new attempt and rerun
`windows-deploy.ps1 ... -Replace`. That removes only the files listed in the
installed manifest, and keeps unknown files.

## Daemon service (WSL2)

1. Start it as installed (the default `/etc/acp/daemon.env` runs it with no
   projects and no Profile), and keep the user manager running without an open
   WSL shell, so the daemon is up when the Desktop connects:
   ```sh
   systemctl --user daemon-reload
   systemctl --user enable --now acp-daemon.service
   sudo loginctl enable-linger "$USER"
   ```
2. Add your projects. Either edit `/etc/acp/daemon.env` (`sudoedit`), or keep a
   private per-user copy, which wins when it exists:
   ```sh
   install -d -m 700 ~/.config/acp
   install -m 600 /etc/acp/daemon.env ~/.config/acp/daemon.env
   $EDITOR ~/.config/acp/daemon.env
   systemctl --user restart acp-daemon
   ```
3. Create a Profile once, explicitly (the daemon never does it by itself):
   ```sh
   R=~/.local/state/acp/runtime
   acp-query --runtime-dir $R --endpoint $R/control.sock profile create ~/.local/state/acp/profile.sqlite3
   ```
   Then add `--database /home/<user>/.local/state/acp/profile.sqlite3` to
   `ACP_DAEMON_ARGS` and run `systemctl --user restart acp-daemon`, so later starts
   select it. A restart never adopts a Profile on its own.
4. Write access stays an explicit, per-run operator action, from the Desktop or
   `acp-query ... profile write enable --profile-id ... --generation ... --confirm`.

Status and logs: `systemctl --user status acp-daemon`, `journalctl --user -u acp-daemon`.
Stop is SIGTERM (graceful: sessions stopped, capabilities removed, lease released).
Exit status 2 (a configuration or safety refusal) is not restarted; fix the cause.

Paths: state in `~/.local/state/acp` (0700), runtime in `.../runtime`, socket
`.../runtime/control.sock`. Do not place state inside a Git checkout; the IPC
runtime refuses that.

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
