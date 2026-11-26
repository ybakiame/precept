Precept Linux x86_64 portable - {{PACKAGE_POLICY}}

Layout
  bin/preceptd        local daemon; the only Profile write owner
  bin/precept         operator CLI
  bin/precept-mcp     project-bound MCP stdio server
  bin/precept-hook    session-start and pre-tool hook helper
  bin/precept-bridge  helper started by the Windows GUI through wsl.exe
{{GUI_LAYOUT}}  lib/systemd/user/preceptd.service
  runtime/manifest.json provenance, ELF facts, declared runtime packages, file hashes
  notices/           NOTICE.txt, dependency ledger and license texts
  LICENSE.txt        Apache License 2.0 (Copyright 2026 ybakiame)

No installer, PATH or profile change, auto-update or network bootstrap is performed.
{{RUNTIME_REQUIREMENTS}}

Before first use
Extract into a NEW private directory as your normal user; never over an existing tree:
  mkdir -m 700 ~/precept-portable && tar --keep-old-files -xzf precept-*-linux-x86_64-*.tar.gz -C ~/precept-portable
(--keep-old-files makes tar fail instead of overwriting an existing file.)
Verify the externally recorded archive SHA-256, then every file (from ~/precept-portable/Precept):
  python3 -c 'import json,hashlib;m=json.load(open("runtime/manifest.json"));[exit("mismatch "+f["file"]) for f in m["files"] if hashlib.sha256(open(f["file"],"rb").read()).hexdigest()!=f["sha256"]]'
Keep the product directory writable only by you.

State lives OUTSIDE this directory, in a private directory (mode 0700):
  install -d -m 700 ~/.local/state/precept
Use ~/.local/state/precept as --state-dir and ~/.local/state/precept/runtime as
--runtime-dir, and keep the Profile there. Stay outside Git checkouts and shared roots.

Start
  ./bin/preceptd --state-dir <state> --runtime-dir <state>/runtime --endpoint <state>/runtime/control.sock
In another terminal:
  ./bin/precept --runtime-dir <state>/runtime --endpoint <state>/runtime/control.sock health
{{GUI_DETAILS}}The daemon never creates a Profile or enables writes by itself; it selects one at
start only when you asked ("Select at start", or profile create/select --remember).
Add projects after enabling writes with precept project check|add|remove.

Stop
In the daemon's terminal use Ctrl+C and wait for it to exit.

Remove
After all Precept processes exit, remove ONLY the files listed in runtime/manifest.json,
then runtime/manifest.json, then the empty bin, lib, runtime, notices and Precept directories.
Never recursively delete a Profile, runtime or unknown directory.
