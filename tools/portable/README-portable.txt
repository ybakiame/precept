ACP Windows x86_64 portable prototype - local, unpublished, synthetic use only

Layout
  bin\acp-daemon.exe        local daemon; the only Profile write owner
  bin\acp-query.exe         operator CLI
  bin\acp-mcp.exe           MCP stdio server with two project-bound read-only tools
  bin\acp-nsh-native.exe    native Desktop (HWND)
  bin\acp-nsh-tui.exe       terminal UI (ConPTY)
  bin\VCRUNTIME140*.dll     unmodified Microsoft Visual C++ runtime, app-local copies
  runtime\manifest.json     source/build provenance and SHA-256 of every other file
  notices\                  dependency notice ledger/texts and VC++ runtime notice
  LICENSE.txt               first-party license status

No installer, Windows service, PATH edit, elevation, registry change, scheduled task,
auto-update, Profile discovery/creation or model invocation is performed by extraction.
No signing, supported OS minimum, clean-machine or completed license claim is made.
The VC++ runtime DLLs are not updated by Windows Update; replace the whole archive.

Before first use
Extract into a NEW private local directory; never extract over an existing tree.
Verify the externally recorded ZIP hash, then every file (hashes do not authenticate
the author). From the extracted ACP directory in PowerShell 7:
  $m = Get-Content .\runtime\manifest.json -Raw | ConvertFrom-Json
  foreach ($f in $m.files) { if ((Get-FileHash -LiteralPath $f.file -Algorithm SHA256).Hash.ToLower() -ne $f.sha256) { throw "Mismatch: $($f.file)" } }
Only after verification, remove a downloaded-file zone mark if Windows added one:
  Get-ChildItem -LiteralPath .\bin -File | Unblock-File
Executable ACL and runtime/Profile ACL are separate: require current-user ownership and
no other-user write access to bin\. Do not weaken security if startup refuses.

State lives OUTSIDE this directory
Use a separate private local runtime directory outside Git, network, interop/shared and
synchronized roots. Never use this ACP\runtime metadata directory as --runtime-dir,
never copy capabilities into ACP\, and do not add state directories to PATH.

Start (replace placeholders; use synthetic data for this prototype)
 .\bin\acp-daemon.exe --runtime-dir <private-local-runtime> --endpoint <unique-name> --allow-project synthetic --allow-root synthetic=<synthetic-project>
Start the daemon in its own console. It creates only its owned runtime and credentials;
without --database it selects no Profile. It never creates a Profile automatically.
In a second console:
 .\bin\acp-query.exe --runtime-dir <same-runtime> --endpoint <same-name> health
 .\bin\acp-query.exe --runtime-dir <same-runtime> --endpoint <same-name> profile info
 .\bin\acp-nsh-native.exe --runtime-dir <same-runtime> --endpoint <same-name> --operator
 .\bin\acp-nsh-tui.exe --runtime-dir <same-runtime> --endpoint <same-name> --operator
GUI/TUI use native operator read credentials. Write-enable is a separate explicit
Profile-scoped operator action; read authority does not grant write access.

For a separately selected synthetic Profile and registered allowlisted project only:
 .\bin\acp-mcp.exe --runtime-dir <same-runtime> --endpoint <same-name> --project synthetic --capability-file <same-runtime>\agent-synthetic.cap
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
and ACP non-recursively. Leave unknown files and user Profile/auth state untouched;
retain the separate state directory. Never recursively delete an arbitrary extraction,
Profile or runtime root. No data-erasure or forensic deletion guarantee is made.
