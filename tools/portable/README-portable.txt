Precept Windows x86_64 portable prototype - local, unpublished, synthetic use only

Layout
  bin\preceptd.exe        local daemon; the only Profile write owner
  bin\precept.exe         operator CLI
  bin\precept-mcp.exe           MCP stdio server with two project-bound read-only tools
  bin\precept-desktop.exe    native Desktop (HWND)
  bin\precept-tui.exe       terminal UI (ConPTY)
  runtime\manifest.json     provenance, prerequisites and SHA-256 of every other file
  notices\                  NOTICE.txt, third-party dependency ledger and license texts
  LICENSE.txt               Apache License 2.0 (Copyright 2026 ybakiame; see notices\NOTICE.txt)

No installer, Windows service, PATH edit, elevation, registry change, scheduled task,
auto-update, Profile discovery/creation or model invocation is performed by extraction.
No signing, supported OS minimum, clean-machine or completed third-party notice claim is made.

Prerequisites (not included; install them yourself, nothing here installs them)
  Windows x64. Precept's exact minimum build is not yet established; the VC++ v14
  Redistributable itself supports Windows 10/11 and Windows Server 2016 or later.
  Microsoft Visual C++ v14 Redistributable (x64), version at least the
  "minimum_version" in runtime\manifest.json "prerequisites". The latest supported
  version always qualifies; Microsoft's permalink:
    https://aka.ms/vc14/vc_redist.x64.exe
  Installing it needs administrator rights and is serviced by Windows Update.
  All five programs need VCRUNTIME140.dll; the Desktop also needs VCRUNTIME140_1.dll.
  Without it, Windows reports that VCRUNTIME140.dll was not found and nothing starts.
  Read-only check of the installed version (no output means not installed):
    Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64' -ErrorAction SilentlyContinue | Select-Object Version, Installed
  Git is optional and external; it is needed only for live repository evidence.

Before first use
Extract into a NEW private local directory; never extract over an existing tree.
Verify the externally recorded ZIP hash, then every file (hashes do not authenticate
the author). From the extracted Precept directory in PowerShell 7:
  $m = Get-Content .\runtime\manifest.json -Raw | ConvertFrom-Json
  foreach ($f in $m.files) { if ((Get-FileHash -LiteralPath $f.file -Algorithm SHA256).Hash.ToLower() -ne $f.sha256) { throw "Mismatch: $($f.file)" } }
Only after verification, remove a downloaded-file zone mark if Windows added one:
  Get-ChildItem -LiteralPath .\bin -File | Unblock-File
Executable ACL and runtime/Profile ACL are separate: require current-user ownership and
no other-user write access to bin\. Do not weaken security if startup refuses.

State lives OUTSIDE this directory, in a PRIVATE directory
The Profile's directory must be owned by you with exactly one access rule (yours).
Folders made by Explorer or plain New-Item inherit SYSTEM/Administrators access and are
refused (UnsafeRuntime). Create one private state directory once, in PowerShell:
  $d = New-Item -ItemType Directory -Path "$env:LOCALAPPDATA\Precept-state"
  $me = [Security.Principal.WindowsIdentity]::GetCurrent().User
  $acl = New-Object Security.AccessControl.DirectorySecurity
  $acl.SetOwner($me); $acl.SetAccessRuleProtection($true, $false)
  $acl.AddAccessRule((New-Object Security.AccessControl.FileSystemAccessRule($me, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow')))
  Set-Acl -LiteralPath $d.FullName -AclObject $acl
Keep the Profile directly in it (e.g. <state>\profile.sqlite3) and use <state>\runtime as
--runtime-dir; the daemon creates that runtime directory privately itself. Stay outside
Git, network, interop/shared and synchronized roots. Never use this Precept\runtime metadata
directory as --runtime-dir, never copy capabilities into Precept\, and do not add state
directories to PATH.

Start (replace placeholders; use synthetic data for this prototype)
 .\bin\preceptd.exe --state-dir <state>\daemon --runtime-dir <state>\runtime --endpoint <unique-name>
Start the daemon in its own console. It creates only its owned state, runtime and
credentials. It never creates a Profile automatically and selects one at start only
when you asked ("Select at start", or profile create/select --remember). Add projects
(Git repository roots in your home) in the Desktop's Workspaces page or with
precept project check|add|remove, after enabling writes.
In a second console:
 .\bin\precept.exe --runtime-dir <same-runtime> --endpoint <same-name> health
 .\bin\precept.exe --runtime-dir <same-runtime> --endpoint <same-name> profile info
 .\bin\precept-desktop.exe --runtime-dir <same-runtime> --endpoint <same-name> --operator
 .\bin\precept-tui.exe --runtime-dir <same-runtime> --endpoint <same-name> --operator
GUI/TUI use native operator read credentials. Write-enable is a separate explicit
Profile-scoped operator action; read authority does not grant write access.

For a separately selected synthetic Profile and registered allowlisted project only:
 .\bin\precept-mcp.exe --runtime-dir <same-runtime> --endpoint <same-name> --project synthetic --capability-file <same-runtime>\agent-synthetic.cap
This is an MCP stdio process, not an interactive shell or network service. It exposes
only get_effective_context and get_handoff. Never substitute operator/write credentials.
Agents and authentication remain external; no agent/model needs to be launched.
Live Git evidence requires a separately available Git; it is not bundled. Missing Git
is unknown/refused evidence, not fresh verification. Font/media/WebView2 and exact OS
runtime closure remain subject to the recorded dependency and clean-target evidence.

Stop
Close GUI normally; TUI Ctrl+Q exits and restores the terminal. UI close does NOT
stop the daemon. In the daemon's own console use Ctrl+C for graceful shutdown and wait
for exit. Never stop unrelated processes or unlink a held lease/lock. A forced kill
is a crash, not a graceful stop. After a crash, retain state and follow the existing
refusal/recovery policy; do not reset or adopt a database to make it start.

Remove
After all owned processes exit, remove ONLY the files listed in runtime\manifest.json,
then runtime\manifest.json itself, then the empty bin, runtime and notices directories
and Precept non-recursively. Leave unknown files and user Profile/auth state untouched;
retain the separate state directory. Never recursively delete an arbitrary extraction,
Profile or runtime root. No data-erasure or forensic deletion guarantee is made.
