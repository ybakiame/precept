# P1-E2: local read-only NSH console

Issue #16, based on main `d1cf45ca6d5a56fae92b8f79d011db87ef35dadc`.
Windows native desktop/ConPTY is the primary local acceptance platform. Windows
owners can explicitly connect to their WSL daemon without a shell or TCP bridge.
[ADR-0003](../adr/0003-ui-framework-selection.md) remains **Proposed**. Mobile is
deferred; GPUI Kit/hybrid stay in their pinned optional benchmark workspace.

## Local verification, with no Actions dependency

Use the repository's pinned Rust **1.99.1**, Python >=3.11 and Git. Windows needs
MSVC C++ Build Tools/Windows SDK; `windows_local_verify.ps1` discovers the installed
VS DevShell and adjusts only its child process environment. Install/cache locked
Cargo dependencies deliberately before offline validation:

```powershell
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy
cargo fetch --locked
cargo fetch --manifest-path spikes/p1-e/shared/Cargo.toml --locked
cargo fetch --manifest-path spikes/p1-e/nsh/Cargo.toml --locked
python -m pip install pyte==0.8.2 psutil==7.2.2 mss==10.2.0 pywinpty==3.1.2
.\tools\windows_local_verify.ps1 -All -Python python
```

A virtual environment or explicit `pip --target` directory can isolate these
**test-only** Python packages. Add that directory to `PYTHONPATH` for the child;
the runner preserves it. Production Python remains stdlib-only. The ConPTY probe
uses the low-level `winpty.PTY` pipe interface; it does not use `PtyProcess`'s
loopback listener. No test downloads browser/mobile SDK dependencies implicitly.

On Unix, install native framework prerequisites first (GTK4 >=4.14, GStreamer,
Fontconfig, a C/C++ toolchain; the existing spike README lists the full set).

```sh
cargo fetch --locked
cargo fetch --manifest-path spikes/p1-e/shared/Cargo.toml --locked
cargo fetch --manifest-path spikes/p1-e/nsh/Cargo.toml --locked
PYTHONPATH=src python tools/local_verify.py --all
```

`--core`, `--service`, `--ui`, `--all`, `--local-project` and `--report-dir DIR`
select local gates. `--all` does **not** opt into a real project. Cargo builds/tests
use `--offline --locked`; the runner never installs packages. A genuine compiler,
assertion or test regression exits nonzero. Missing cache/SDK/display/dependencies
are `NOT_RUN`, never a fabricated PASS. Dependent gates are explicitly skipped.
`report.json` records argv, exit code, duration, observed test/comparison counts,
versions, host OS/architecture, tested Git SHA and dirty state. Use a clean task
commit when collecting final evidence; a report from a dirty worktree says so.

Raw logs, generated unsigned packages and **synthetic-only** screenshots belong in
ignored `artifacts/local-p1-e2/` or another operator-controlled private directory
outside every selected project. Do not upload that directory. Review and sanitize
a summary by hand; reports/logs are not automatically safe to publish. The runner
has no GitHub operations. Push all task commits with **`[skip ci]`**, including PR
HEAD. Existing required GitHub checks can remain **Pending**. A future change to
manual `workflow_dispatch` and branch policy needs separate owner review; this
issue does not change or delete workflows and local results are not cloud CI.

## Foreground service and independent clients

Use a **new** private temporary directory outside ACP and every registered project.
Never substitute the existing default `AGENT_CP_HOME`. The Python CLI explicitly
owns registration/suggestions/confirmation; the UI and Rust service never migrate,
open a default profile, create fixtures as a fallback, or start each other.

Unix operator terminal (replace the absolute checkout placeholder deliberately):

```sh
export ACP_LOCAL_PROJECT_ROOT=/absolute/operator-selected/checkout
export ACP_LOCAL_PROJECT_LABEL=selected-local
DEMO=$(mktemp -d /tmp/acp-e2-XXXXXXXX)
chmod 700 "$DEMO"
PYTHONPATH=src python -m agent_control_plane.cli --data-dir "$DEMO/state" \
  project add "$ACP_LOCAL_PROJECT_LABEL" "$ACP_LOCAL_PROJECT_ROOT"
target/debug/acp-daemon --database "$DEMO/state/context.sqlite3" \
  --runtime-dir "$DEMO/run" --endpoint "$DEMO/run/control.sock" \
  --allow-project "$ACP_LOCAL_PROJECT_LABEL"
```

Use a second terminal for explicit native read clients:

```sh
target/debug/acp-query --runtime-dir "$DEMO/run" --endpoint "$DEMO/run/control.sock" health
spikes/p1-e/nsh/target/release/acp-nsh-tui \
  --runtime-dir "$DEMO/run" --endpoint "$DEMO/run/control.sock" \
  --project "$ACP_LOCAL_PROJECT_LABEL" --task-id example-task
```

Windows PowerShell operator terminal (its chosen project must be a Windows-local
checkout for this native Named Pipe variant):

```powershell
$env:ACP_LOCAL_PROJECT_ROOT = 'D:\absolute\operator-selected\checkout'
$env:ACP_LOCAL_PROJECT_LABEL = 'selected-local'
$demo = Join-Path ([IO.Path]::GetTempPath()) ('acp-e2-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $demo | Out-Null
$env:PYTHONPATH = 'src'
python -m agent_control_plane.cli --data-dir "$demo\state" project add $env:ACP_LOCAL_PROJECT_LABEL $env:ACP_LOCAL_PROJECT_ROOT
.\target\debug\acp-daemon.exe --database "$demo\state\context.sqlite3" `
  --runtime-dir "$demo\run" --endpoint e2-control --allow-project $env:ACP_LOCAL_PROJECT_LABEL
```

In another PowerShell terminal, use the same explicit `$demo` value:

```powershell
.\spikes\p1-e\nsh\target\release\acp-nsh-native.exe `
  --runtime-dir "$demo\run" --endpoint e2-control `
  --project selected-local --task-id example-task
```

Windows endpoint is a bare validated pipe label, Unix endpoint an absolute socket
under its private runtime. See [the service security/lifecycle contract](local-service.md).
The UI labels results `LIVE_READ_ONLY` or `OFFLINE_SYNTHETIC`. A live snapshot says
**Last read succeeded**, not continuous health monitoring; Reconnect refreshes it.
A refused read clears details. No flags means only a fixed offline synthetic
snapshot. `--synthetic-rows 10000` is an explicit offline renderer workload and
cannot combine with live flags.

## Windows owner connecting to WSL

Build the Linux `acp-ui-snapshot` helper in the explicitly selected WSL checkout:

```sh
cargo build --manifest-path spikes/p1-e/shared/Cargo.toml --features native \
  --bin acp-ui-snapshot --offline --locked
```

Start the WSL daemon explicitly as above. Then run **the Windows executable**:

```powershell
.\spikes\p1-e\nsh\target\release\acp-nsh-native.exe `
  --wsl-distribution '<explicit-distribution>' --wsl-user '<explicit-linux-owner>' `
  --wsl-snapshot '/absolute/acp-checkout/spikes/p1-e/shared/target/debug/acp-ui-snapshot' `
  --wsl-runtime-dir '/absolute/private-temp/run' `
  --wsl-endpoint '/absolute/private-temp/run/control.sock' `
  --project selected-local --task-id example-task
```

All five WSL location/owner flags are mandatory together and conflict with native
location or fixture flags. This invokes `wsl.exe --distribution … --user … --exec
/absolute/acp-ui-snapshot` as an argv array. It never uses `sh`, `bash`, a command
string, shell expansion, a listener or a copied token. The already-built Linux
helper reads the private operator capability and uses the unchanged UID-checked
Unix IPC. The daemon still enforces the exact project allowlist. Response parsing
is bounded to 4 MiB, a 20-second helper deadline and the shared typed schema.
Errors are sanitized; no helper stderr or token enters UI logs. The chosen WSL
user must own that daemon/runtime. Windows ownership of WSL is not a new security
boundary against the Windows account owner; choosing a different user does not
bypass the daemon's Linux peer/credential checks. No default distro, login shell,
profile discovery, implicit daemon start or authenticated browser gateway exists.

## Operator workflow and keyboard

Select an allowlisted project in the left pane or with Ctrl/Cmd+P. Switching clears
the task selection. Enter an exact task ID (1–120 characters, no control bytes) and
press Enter/Lookup. A missing task is Empty; IDs are never filesystem paths and
never shell input. There is no task-list enumeration or new agent-facing tool.
Only the selected project context/handoff is fetched. Empty confirmed preference
state is preserved, with scope/source on every returned row, stale/fingerprint
warnings and recorded evidence explicitly distinguished from current verification.

Ctrl/Cmd+R reconnects; +F focuses the filter; +K focuses task input; +D changes theme;
Ctrl/Cmd+Shift+C copies the visible detail page deliberately. Alt+Left/Right or the
page buttons navigate 12-row pages. Windows NSH Tab traverses text inputs; shortcuts
provide project and action navigation. TUI Tab also traverses pressable controls;
Ctrl+Q/C closes it (plain `q` is editable task/filter text). A 10k dataset mounts at
most 12 preference rows per page, with bounded evidence/text and visible truncation
labels. Copy is never automatic. Closing either UI leaves the daemon running.

## Explicit selected-project acceptance

Keep `ACP_LOCAL_PROJECT_ROOT` unset to test disposable synthetic fixtures only.
When set, it must be absolute, existing and have `.git` or `AGENTS.md`. Repository
files are untrusted data. The acceptance script never executes their instructions,
hooks, builds, resets, clean, checkout, staging or commits. It registers only that
root plus a second synthetic project in a **fresh disposable** DB. Real root
preferences/handoffs stay Empty; deliberate synthetic suggestions remain suggested
and invisible. Synthetic mutation/crash tests use disposable fixtures only.

```sh
export ACP_LOCAL_PROJECT_ROOT=/absolute/operator-selected/checkout
PYTHONPATH=src python tools/local_verify.py --local-project \
  --report-dir artifacts/local-p1-e2/selected-project
```

For Windows-owner UI acceptance driven from WSL, explicitly supply all four test
settings: `ACP_WINDOWS_TEST_ROOT` (the owned disposable Windows ACP checkout as a
WSL mount path), `ACP_WINDOWS_PYTHON` (its native Python executable as a WSL mount
path), `ACP_WSL_DISTRIBUTION`, `ACP_WSL_USER`. The Windows test checkout contains an
untracked `.local-acp-test-root` marker and installed test dependencies under ignored
artifacts. `test_windows_wsl.py --help` separately documents synthetic Windows→WSL
snapshot isolation and lifecycle checks. No Windows development project is inferred
from a parent directory; the selected external path remains exactly the opt-in.

The real-root test compares bounded Git HEAD/branch/complete porcelain status and
lstat metadata for the root/checkout marker, Git HEAD/index/config/packed-refs and
the five allowlisted documents. It does not claim a full-tree content attestation:
ignored files, unsampled metadata and external concurrent activity are outside this
comparison. Only digests/boolean observations may enter sanitized evidence.
`include_documents` is false; no raw document/handoff/preference values are uploaded.
No preexisting ACP user DB is opened, migrated, backed up or mutated.

## Local Codex MCP and cleanup

The test inspects installed `codex --version`, `codex mcp --help` and the app-server
JSON schema rather than assuming flags. `codex_mcp_smoke.py` uses the installed
local `codex app-server --stdio`, an ephemeral read-only thread and direct
`mcpServerStatus/list` / `mcpServer/tool/call`. Only the two project-bound schemas
are available; exact context parity, missing/foreign task, project override,
malformed task and operator-listing denial are asserted. There is **no model turn**.
A temporary child `CODEX_HOME` contains only test config, telemetry disabled and a
provider aimed at a closed loopback port; user configuration/auth are never copied.
This verifies the actual local Codex MCP client without sending real context to a
remote model. If that installed API is unavailable, report NOT_RUN. It is not a
claim of remote inference or every Codex harness version. See
[official Codex MCP configuration](https://learn.chatgpt.com/docs/extend/mcp?surface=cli).

Stop the foreground daemon yourself (Ctrl-C, Unix SIGTERM or supported Windows
console signal). It releases its capabilities/socket. Reconnect clients manually
after an explicit restart. Test-created states/runtimes are closed and removed by
the harness. For a manual demo, close every reader/writer first and remove **only**
the fresh directory whose creation you observed. Preserve any preexisting paths.
No cloud release, installation, autostart/login item, push/merge, mobile SDK or
external repository change is implied by this workflow. The task opens a Draft PR
for review and never merges it automatically.

The observed Windows development artifacts are unsigned x64 ZIPs. Their measured
non-OS dependencies include `VCRUNTIME140.dll` and `VCRUNTIME140_1.dll`; Windows
UCRT, Direct2D/Direct3D, WinRT, Media Foundation and XAudio system DLLs are also
required. The archive contains no redistributable runtime, installer or autostart
entry. Only the documented Windows 11 development host was run; a clean-machine
installation and other Windows releases remain NOT_RUN.
