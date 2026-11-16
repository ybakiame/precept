ACP Windows x86_64 portable prototype - local, unpublished, synthetic use only

This archive contains five Rust product executables. Node, PowerShell build helpers,
Rust/Cargo, acceptance drivers, test data and credentials are not product contents.
No installer, Windows service, PATH edit, elevation, registry change, scheduled task,
auto-update, Profile discovery/creation or model invocation is performed by extraction.
No signing, supported OS minimum, clean-machine or completed license claim is made.
See manifest.json, dependency-notices.json, LICENSE-STATUS.txt and SHA256SUMS.txt.

Choose a NEW private local executable directory; do not extract over an existing tree.
Verify SHA256SUMS.txt and the externally recorded ZIP hash before launching. Hashes do
not authenticate the author. Use a separate private local state directory outside Git,
network, interop/shared/synchronized roots. Preserve all existing Profile/auth files.
Do not add state directories to PATH or copy capabilities to the product directory.
Executable ACL and runtime/Profile ACL are separate; require current-user ownership
and no other-user replacement/access. Do not weaken security if startup refuses.

Example syntax (replace placeholders explicitly; do not use real data for prototype):
 .\acp-daemon.exe --runtime-dir <private-local-runtime> --endpoint <unique-name> --allow-project synthetic --allow-root synthetic=<synthetic-project>
Start daemon in its own console. It creates only its owned runtime and credentials;
without --database it selects no Profile. It never creates a Profile automatically.
In a second console:
 .\acp-query.exe --runtime-dir <same-runtime> --endpoint <same-name> health
 .\acp-query.exe --runtime-dir <same-runtime> --endpoint <same-name> profile info
 .\acp-nsh-native.exe --runtime-dir <same-runtime> --endpoint <same-name> --operator
 .\acp-nsh-tui.exe --runtime-dir <same-runtime> --endpoint <same-name> --operator
GUI/TUI use native operator read credentials. Write-enable is a separate explicit
Profile-scoped operator action; read authority does not grant write access.

For a separately selected synthetic Profile and registered allowlisted project only:
 .\acp-mcp.exe --runtime-dir <same-runtime> --endpoint <same-name> --project synthetic --capability-file <same-runtime>\agent-synthetic.cap
This is an MCP stdio process, not an interactive shell or network service. It exposes
only get_effective_context and get_handoff. Never substitute operator/write credentials.
Agents and authentication remain external; no agent/model needs to be launched.
Live Git evidence requires a separately available Git; it is not bundled. Missing Git
is unknown/refused evidence, not fresh verification. Font/media/WebView2 and exact OS
runtime closure remain subject to the recorded dependency and clean-target evidence.

Close GUI normally; TUI Ctrl+Q exits and restores the terminal. UI close does NOT
stop daemon. In the daemon's own console use Ctrl+C for graceful shutdown and wait
for exit. Never stop unrelated processes or unlink a held lease/lock. A forced kill
is a crash, not graceful-stop evidence. After a crash, retain state and follow the
existing refusal/recovery policy; do not reset or adopt a database to make it start.

After all owned processes exit, remove ONLY the exact product files enumerated by
SHA256SUMS.txt plus SHA256SUMS.txt itself. Leave unknown files and user Profile/auth
state untouched; retain the separate state directory. Never recursively delete an
arbitrary extraction, Profile or runtime root. Empty known product directories can
be removed non-recursively. No data-erasure or forensic deletion guarantee is made.
