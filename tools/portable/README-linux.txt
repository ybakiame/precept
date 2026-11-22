Precept Linux x86_64 portable prototype - local, unpublished, synthetic use only

This archive is a local acceptance output, not a release asset. Keep it and its
sidecars in ignored local artifacts; do not commit or upload it. Formal distribution
and signing require a later, explicit release decision.

Layout
  bin/preceptd        local daemon; the only Profile write owner
  bin/precept         operator CLI
  bin/precept-mcp           MCP stdio server (two project-bound read-only tools); also the
                        trusted context bridge for opted-in sessions
  bin/precept-desktop    native Desktop (GTK 4, Wayland)
  bin/precept-bridge    helper started by the Windows GUI through wsl.exe (Windows -> WSL2)
  runtime/manifest.json provenance, ELF facts, declared runtime packages, file hashes
  notices/              NOTICE.txt, dependency ledger and license texts
  LICENSE.txt           Apache License 2.0 (Copyright 2026 ybakiame)

Validated target: Ubuntu 24.04 LTS x86_64 in WSL2 with WSLg (Weston, Wayland).
No standalone Linux, other distribution or compositor claim is made. No installer,
service, PATH or profile change, root requirement, auto-update or network bootstrap.

Runtime dependencies (not included; OS-owned libraries are never bundled)
  glibc providing at least {{GLIBC}}, plus these Ubuntu 24.04 packages:
    sudo apt-get install {{RUNTIME_PACKAGES}}
  The Desktop needs a Wayland session (WSLg provides one). Git is optional and external (live repository evidence only).

GPU rendering on WSLg (optional)
  By default Mesa renders the Desktop in software on WSLg, because WSLg's Weston
  exposes no GPU device to Wayland clients. To use the Windows GPU through WSL's
  d3d12 driver, set this for the Desktop process only (nothing in this archive sets it):
    GALLIUM_DRIVER=d3d12 ./bin/precept-desktop --runtime-dir ... --endpoint ... --operator
  It uses WSL's own /usr/lib/wsl libraries; no extra package is needed. If the Desktop
  misbehaves, start it without the variable to return to software rendering.

Before first use
Extract into a NEW private directory as your normal user; never over an existing tree:
  mkdir -m 700 ~/precept-portable && tar --keep-old-files -xzf precept-linux-x86_64-prototype-*.tar.gz -C ~/precept-portable
(--keep-old-files makes tar fail instead of overwriting an existing file.)
Verify the externally recorded archive SHA-256, then every file (from ~/precept-portable/Precept):
  python3 -c 'import json,hashlib;m=json.load(open("runtime/manifest.json"));[exit("mismatch "+f["file"]) for f in m["files"] if hashlib.sha256(open(f["file"],"rb").read()).hexdigest()!=f["sha256"]]'
Keep the product directory writable only by you.

State lives OUTSIDE this directory, in a private directory (mode 0700):
  install -d -m 700 ~/.local/state/precept
Use ~/.local/state/precept as --state-dir (the daemon's projects and remembered startup
Profile) and ~/.local/state/precept/runtime as --runtime-dir (created privately), and keep
the Profile there. Stay outside Git checkouts, network and shared roots.

Start (replace placeholders; synthetic data for this prototype)
  ./bin/preceptd --state-dir <state> --runtime-dir <state>/runtime --endpoint <state>/runtime/control.sock
In another terminal:
  ./bin/precept --runtime-dir <state>/runtime --endpoint <state>/runtime/control.sock health
  ./bin/precept-desktop --runtime-dir <state>/runtime --endpoint <state>/runtime/control.sock --operator
The daemon never creates a Profile or enables writes by itself; it selects one at
start only when you asked ("Select at start", or profile create/select --remember).
Add projects (Git repository roots in your home) in the Desktop's Workspaces page or
with precept project check|add|remove, after enabling writes.

Stop
Close the Desktop normally. UI close does NOT stop the daemon. In the daemon's terminal use Ctrl+C and wait for it to exit.

Remove
After all Precept processes exit (Linux lets a running executable be deleted, so check
first), remove ONLY the files listed in runtime/manifest.json, then
runtime/manifest.json, then the empty bin, runtime and notices directories and Precept.
Never recursively delete a Profile, runtime or unknown directory.
