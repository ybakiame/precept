ACP Linux x86_64 portable prototype - local, unpublished, synthetic use only

Layout
  bin/acp-daemon        local daemon; the only Profile write owner
  bin/acp-query         operator CLI
  bin/acp-mcp           MCP stdio server (two project-bound read-only tools); also the
                        trusted context bridge for opted-in sessions
  bin/acp-nsh-native    native Desktop (GTK 4, Wayland)
  bin/acp-nsh-tui       terminal UI (PTY)
  bin/acp-wsl-bridge    helper started by the Windows GUI through wsl.exe (Windows -> WSL2)
  runtime/manifest.json provenance, ELF facts, declared runtime packages, file hashes
  notices/              NOTICE.txt, dependency ledger and license texts
  LICENSE.txt           Apache License 2.0 (Copyright 2026 ybakiame)

Validated target: Ubuntu 24.04 LTS x86_64 in WSL2 with WSLg (Weston, Wayland).
No standalone Linux, other distribution or compositor claim is made. No installer,
service, PATH or profile change, root requirement, auto-update or network bootstrap.

Runtime dependencies (not included; OS-owned libraries are never bundled)
  glibc providing at least {{GLIBC}}, plus these Ubuntu 24.04 packages:
    sudo apt-get install {{RUNTIME_PACKAGES}}
  The Desktop needs a Wayland session (WSLg provides one); the TUI needs only a
  terminal. Git is optional and external (live repository evidence only).

GPU rendering on WSLg (optional)
  By default Mesa renders the Desktop in software on WSLg, because WSLg's Weston
  exposes no GPU device to Wayland clients. To use the Windows GPU through WSL's
  d3d12 driver, set this for the Desktop process only (nothing in this archive sets it):
    GALLIUM_DRIVER=d3d12 ./bin/acp-nsh-native --runtime-dir ... --endpoint ... --operator
  It uses WSL's own /usr/lib/wsl libraries; no extra package is needed. If the Desktop
  misbehaves, start it without the variable to return to software rendering.

Before first use
Extract into a NEW private directory as your normal user; never over an existing tree:
  mkdir -m 700 ~/acp-portable && tar --keep-old-files -xzf acp-linux-x86_64-prototype-*.tar.gz -C ~/acp-portable
(--keep-old-files makes tar fail instead of overwriting an existing file.)
Verify the externally recorded archive SHA-256, then every file (from ~/acp-portable/ACP):
  python3 -c 'import json,hashlib;m=json.load(open("runtime/manifest.json"));[exit("mismatch "+f["file"]) for f in m["files"] if hashlib.sha256(open(f["file"],"rb").read()).hexdigest()!=f["sha256"]]'
Keep the product directory writable only by you.

State lives OUTSIDE this directory, in a private directory (mode 0700):
  install -d -m 700 ~/.local/state/acp
Use e.g. ~/.local/state/acp/runtime as --runtime-dir (the daemon creates it privately)
and keep the Profile there. Stay outside Git checkouts, network and shared roots.

Start (replace placeholders; synthetic data for this prototype)
  ./bin/acp-daemon --runtime-dir <state>/runtime --endpoint <state>/runtime/control.sock --allow-project synthetic --allow-root synthetic=<synthetic-project>
In another terminal:
  ./bin/acp-query --runtime-dir <state>/runtime --endpoint <state>/runtime/control.sock health
  ./bin/acp-nsh-native --runtime-dir <state>/runtime --endpoint <state>/runtime/control.sock --operator
  ./bin/acp-nsh-tui --runtime-dir <state>/runtime --endpoint <state>/runtime/control.sock --operator
The daemon never creates or selects a Profile and never enables writes by itself.

Stop
Close the Desktop normally; TUI Ctrl+Q exits and restores the terminal. UI close does
NOT stop the daemon. In the daemon's terminal use Ctrl+C and wait for it to exit.

Remove
After all ACP processes exit (Linux lets a running executable be deleted, so check
first), remove ONLY the files listed in runtime/manifest.json, then
runtime/manifest.json, then the empty bin, runtime and notices directories and ACP.
Never recursively delete a Profile, runtime or unknown directory.
