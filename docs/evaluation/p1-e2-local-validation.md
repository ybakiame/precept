# P1-E2 local validation

Date: 2026-11-09. Issue #16; base main
`d1cf45ca6d5a56fae92b8f79d011db87ef35dadc`. **Local execution only**. No Actions
request, rerun, hosted check or release is part of this evidence. Task commits use
`[skip ci]`; required GitHub checks can remain Pending. ADR-0003 is Proposed,
mobile is deferred, and production Rust write ownership is unchanged.

## Scope and method

Windows native MSVC desktop/ConPTY is the primary acceptance host, following the
operator's direction. The ACP source/build/test copy is independent of other
Windows workspaces. One explicitly opted-in development checkout was read as a
WSL project-root source; no other real project was enumerated or registered.
All project names, paths, Git values, document metadata and pre/post digests remain
local. No real preference values, handoff evidence, transcripts, credentials,
user/device identifiers or screenshots enter this document or the PR.

Toolchains observed: Windows 11 **10.0.26300**, Rust/Cargo **1.99.1**, MSVC Build Tools, Python
**3.14.5**, portable Git **2.56.0.windows.4**; WSL Rust/Cargo **1.99.1**, Python
**3.14.9**, local Codex CLI **0.167.0**. NSH and GPUI source revisions and all
separate Cargo.lock files remain pinned. Test-only packages: pyte 0.8.2,
psutil 7.2.2, mss 10.2.0 and Windows pywinpty 3.1.2. Cargo gates execute
`--offline --locked` after a deliberate one-time cache/bootstrap step.

Reproduce with [the quickstart](../p1/local-codex-e2.md) and
[`tools/local_verify.py`](../../tools/local_verify.py). Private `report.json` files
retain the exact argv, exit code, duration, versions, counts, Git SHA and dirty
state. Raw diagnostic directories and local unsigned packages are ignored and
were not uploaded. The committed-source revision and gate totals are recorded below. Earlier
work-in-progress reports honestly retain their base SHA and `worktree_dirty=true`.

## Actual observations

| Check | Observed local result |
| --- | --- |
| Root fmt/Clippy/workspace tests | PASS on Windows and WSL; 31 Windows / 32 Unix Rust tests |
| Python reference/security regressions | 24 tests PASS on each host: the 21 original tests plus three local-runner regressions |
| JSON contract parity | 120 comparisons PASS on each host |
| Exact SQLite parity | 78 comparisons PASS on each host |
| Synthetic backup/rehearsal and process death/restart | PASS on each host; 11 process-recovery tests |
| Foreground daemon/operator/IPC/project-bound MCP | 11 process tests PASS on each host, including malformed requests, allowlists, permission changes, stale/unknown worktree state and lifecycle |
| Shared presentation/native-client model | Five tests PASS, including scope, unconfirmed-state refusal, input/response bounds and explicit shell-free WSL argv |
| NSH component renderer | Two tests PASS on Windows; 10,000 rows mounted across all 834 pages, every row checked, fewer than 100 text nodes at each page, filtering/selection/input/explicit copy tested |
| Windows native desktop | Actual visible windows, loaded-model layout, resize/close, three release samples, rendered project-switch/filter assertions PASS |
| Windows ConPTY | Actual NSH/ratatui process; Tab project switch, task including `q`, input focus/filter, Ctrl+Q close, no readiness-marker leakage PASS |
| 10k actual native-window and ConPTY workloads | Windows native window filtered 10,000→1 through Ctrl+F/WM_CHAR; actual ConPTY filtered 10,000→1; both PASS |
| Windows owner → WSL helper → Unix IPC | Context equality, project/hostile-input denial, removed capability, daemon stop/refusal/restart and independent lifetimes PASS |
| Explicit real project | Operator/UI/MCP/Codex same-scope context; confirmed preferences and handoff Empty; project override/foreign task refused; PASS |
| Real-project Windows views | Actual native window and ConPTY loaded the WSL-backed selected-project model; terminal observed disconnect/reconnect; UI closure left daemon healthy; PASS |
| Local Codex MCP client | Actual local app-server stdio, two schemas and direct tool calls; seven denial cases, context equality and no model turn; PASS |
| Protected checkout/database | Pre/post Git HEAD/branch/status and agreed lstat metadata equal; disposable DB unchanged during reads; no existing ACP user DB opened; PASS |
| Local packaging | Unsigned Windows x64 desktop/TUI development ZIP produced locally; no installer/autostart/distribution |

The task input performs an exact lookup, so no new `handoff.list`, agent tool,
project enumeration outside the allowlist or daemon write API was needed. The UI
has one shared typed model and authenticated native client. Windows→WSL executes
an explicit helper using `wsl.exe --exec`; it never starts a shell, transfers a
token, binds TCP, launches an agent or implicitly starts the daemon. Returned
metadata is display data, not instructions. Only Python CLI commands deliberately
write the new isolated state and synthetic fixtures.

## Release measurements on this Windows host

Three repetitions per surface; warm dependency/OS caches, ordinary local desktop,
no cache flush or isolated-machine guarantee. Stopwatch starts immediately before
process launch. Windows visible-window enumeration and the loaded-model layout
marker are separate desktop observations; ConPTY readiness requires expected text
in an emulated actual terminal screen. psutil samples process RSS after readiness
(desktop after an additional 0.3 s). Values are observations, not a framework
ranking or a clean-install support claim.

| Sample | Desktop visible / model layout (s) | Desktop RSS bytes | ConPTY expected screen (s) | ConPTY RSS bytes |
| --- | --- | --- | --- | --- |
| 1 | 0.1184 / 0.5419 | 55,431,168 | 0.4265 | 15,757,312 |
| 2 | 0.0930 / 0.4936 | 55,488,512 | 0.4160 | 15,728,640 |
| 3 | 0.0933 / 0.4973 | 55,578,624 | 0.4325 | 15,773,696 |

Desktop executable: **41,169,408 bytes**; TUI: **37,905,920 bytes** (shared system
libraries excluded). The native 10k dataset measured **0.1626 s** from Ctrl+F and
a deterministic Win32 WM_CHAR stream to the NSH model-state acknowledgement of
one matching row. A synthetic-window capture 0.25 s later was visually inspected
and showed that exact row; this marker is not a frame-presentation timestamp. RSS was 57,675,776 bytes
before and 60,149,760 after that filter. WM_CHAR intentionally avoids dependence on
the operator's active IME; it is not a complete IME test. ConPTY measured
**0.7065 s** through real terminal input to the expected painted screen. The
component traversal took about **90.40 s** for all 834 pages; the headless test
harness settles between actions, so that duration is not per-input latency.
The screen tree remains bounded instead of mounting 10k labels simultaneously.
The separate native project-switch capture was also visually inspected: beta was
selected, the task had cleared, and the committed filter showed an honest Empty.
No exact native frame-presentation latency is claimed.

Only synthetic windows/terminal screens were captured, in ignored local files.
Real-project view tests take no screenshots and do not copy data to the clipboard.
The pre/post comparison is a bounded status/metadata check, not a whole-tree
content attestation: ignored files, unsampled metadata and concurrent external
activity are outside its claim. Git fingerprint remains unavailable in Rust and
stale/unknown warnings stay visible; recorded checks are never presented as
verification of this session.

## NOT_RUN and deferred work

- Full screen-reader/accessibility and IME coverage: NOT_RUN. Basic native focus,
  shortcuts and a composition commit were observed; they do not establish complete
  assistive-technology or IME support. Windows NSH Tab traverses text fields;
  explicit shortcuts cover project/action navigation.
- WSL/WSLg native GTK screenshot acceptance: no final PASS. An earlier capture hit
  an X11 protocol error. Further WSL desktop repair is deferred by the operator;
  WSL core/service and Windows-owner connectivity were verified separately.
- Optional offline web runtime: NOT_RUN in this task. Its smoke script follows
  the new controls; JavaScript syntax was checked. No authenticated browser IPC.
- GPUI Kit/hybrid rebuild: NOT_RUN after the optional offline check found the
  pinned GPUI Git dependency absent from cache. Sources and lockfiles were kept as
  benchmark references; no new runtime parity claim.
- macOS, other Windows architectures, clean-machine installation and universal
  packaging: NOT_RUN. The artifact is unsigned and for this development host.
- Windows-native Codex executable: NOT_RUN; the actual installed Codex client
  tested here runs locally in WSL. No remote model request or real-data upload.
- iOS/Android/mobile SDK/device tests: NOT_RUN, explicitly deferred.
- GitHub Actions: NOT_RUN due to exhausted quota. Existing workflows unchanged;
  manual workflow/branch-policy changes require a separate owner-reviewed change.

## Committed-source rerun

Tested implementation commit: **`e4b5e962028d6d32ae09e57d1a600650ebd24d01`**.
Both source copies were clean and every tracked Windows file matched that commit.
A later documentation-only commit records these observations without changing
runtime code. Actual local commands (personal absolute paths omitted):

```text
Windows: tools/windows_local_verify.ps1 -All -Python <native-python> -ReportDir <private-report>
WSL: PYTHONPATH=src python tools/local_verify.py --core --service --report-dir <private-report>
WSL: ACP_LOCAL_PROJECT_ROOT=<explicit-opt-in> [explicit Windows/WSL settings] python tools/local_verify.py --local-project --report-dir <private-report>
WSL: python tools/test_windows_wsl.py --windows-snapshot <explicit-native-exe> --distribution <explicit-distro> --user <explicit-owner> --helper <explicit-linux-helper> --out <private-report>
```

- Windows: **22 PASS, 0 FAIL, 2 NOT_RUN** (optional web and comprehensive IME/screen reader).
- WSL core/service: **11 PASS, 0 FAIL, 0 NOT_RUN**.
- Selected-project acceptance: **1 PASS, 0 FAIL, 0 NOT_RUN**; actual Windows
  desktop/ConPTY, operator/MCP/Codex, stop/restart and unchanged protected metadata.
- Windows-owner synthetic WSL regression command: **PASS**.
- Both Windows executables report Authenticode **NotSigned**. ZIP entries are
  exactly the two executables, README and manifest; SHA-256 values match the
  local release files. No data/runtime/capability/log/screenshot is bundled.
- `dumpbin /DEPENDENTS` observed `VCRUNTIME140.dll`, `VCRUNTIME140_1.dll`, Windows
  UCRT API sets and system Direct2D/Direct3D/WinRT/Media Foundation/XAudio/UIA
  libraries. They are runtime prerequisites, excluded from the archive; no
  clean-machine or other-Windows-version install claim.
- Final diff/relative links/private-path/artifact checks passed. Existing workflow
  files, external development repositories and existing ACP user state are not
  part of the change. Draft PR only; no merge or framework acceptance.
