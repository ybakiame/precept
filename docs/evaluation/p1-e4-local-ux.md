# P1-E4 local NSH UX and Rust migration evidence

Status: local Windows + native WSLg acceptance **PASS**, with owner review of the
base UI. Final verification used clean source on both actual hosts. All logs,
synthetic screenshots and unsigned archives remain private local artifacts.
ADR-0003 remains **Proposed**. Mobile, production database writes and automatic merge
are deferred. GitHub Actions are not run; commits use `[skip ci]`.

## Scope and observed UX

Baseline main: `80fdbc57e4ec47557e9130808b401a22ad6b913f` (PR #19 merged).
The owner accepted the actual local E4 synthetic preview as a base interface;
detailed visual/brand direction will follow after merge. This acceptance does not
authorize this agent to merge the Draft PR.

E3's always-visible task/filter/paging controls crowded the 640×480 window and
80×30 terminal. Dashboard repeated a full preference page. E4 uses a 176px sidebar
on wide desktop, wrapping page navigation on compact desktop/TUI, page-specific
editors, a summary Dashboard, structured confirmed-only preference cards and
12-row paging. Detail is disclosed before the list so selecting the first row
produces a visible result. Exact Handoff lookup and six-entry evidence disclosure
remain scoped, with recorded evidence explicitly distinct from current validation.

Semantic slate/blue tokens cover light, dark and high contrast, borders, disabled
controls, focus and availability. Esc invalidates an in-flight epoch; delayed
results cannot restore cancelled, superseded or denied scope. Stable input nodes
survive page switching. Explicit copy is sanitized/bounded; no automatic clipboard
capture is used. Runtime diagnostics show actual observed GTK backend/renderer
classes without credentials, paths or a GPU-acceleration claim.

The first proposed Ctrl+1–5 navigation conflicted with the owner's Windows screenshot
shortcut. It was replaced by **Alt+1–5**, preserving the owner's system settings.
Ctrl+F/K open the appropriate page and focus its editor. No compositor, GTK config,
IME setting, screenshot hotkey or other system configuration was modified.

## Capture integrity and host limits

Private captures: `artifacts/local-p1-e4/before/`, `window-only/`, final host report
directories, and the owned Windows checkout's E4 artifact directories. Baseline
binaries were extracted from the prior E3 unsigned packages; the UI sources at
that tested E3 revision are identical to the baseline merge. Desktop captures cover
1080×760 and 640×480; own PTY/ConPTY screen buffers cover 160×50 and 80×30.

Early cropped-screen captures could contain another app's topmost overlay. Those
E4 after-captures were deleted and excluded from evidence. Collection now calls
**PrintWindow on the exact nonce-bound WSLg HWND or PID-owned Windows HWND**, never a
desktop DC/screen crop, with no fallback if unsupported/blank. Pointer actions check
ownership; key input checks foreground and releases modifiers. These are target
window pixels, not a compositor-presentation timestamp. Model-ready, GTK after-paint,
OS visibility, target capture and terminal-painted timings remain separate measures.

Linux host: local WSL2 x86_64, GTK 4.24.1, WSLg 1.0.79 Weston, actual
**GdkWaylandDisplay**. GskGLRenderer and a per-child Cairo diagnostic are observed;
hardware acceleration is not established. Windows uses the native NSH MSVC build
and actual ConPTY; Windows-owner WSL access remains `wsl.exe --exec` with operator
authentication and no shell/TCP/token copy. niri is **NOT_RUN** by owner choice;
XWayland/Xvfb never substitute for Wayland acceptance. Full IME, screen readers,
macOS and mobile are NOT_RUN.

## Mandatory Rust work

The daemon now requests Python-compatible `acp-worktree-v1` only for a validated
Handoff, through its injected adapter. Git/filesystem I/O stays outside pure core;
no production store/write or MCP mutation authority changes. Exact bytes, Windows
verbatim-link spelling, POSIX modes/raw names, safety profile, budgets and race
limits are documented in the [contract](../p1/worktree-fingerprint-v1.md).

The [test migration matrix](../p1/rust-test-migration.md) maps all seven removed
Python negative cases and both removed fingerprint duplicates to Rust core plus
real authenticated IPC/MCP subprocess assertions. Legacy nullable SQLite migration,
Python CLI/MCP runtime and reference parity remain. Three compiled guard mutations
must be rejected. The real service checks full Handoff JSON/warnings/validation
against Python, including same HEAD/count with changed dirty content.

## Commands and final evidence

Only local pinned/cached/offline Cargo is used. The canonical WSL command is:

```bash
PYTHONPATH=src:<private-test-dependencies> \
  python tools/local_verify.py --ui-full --wslg --local-project \
  --owner-reviewed --windows-report <private-clean-Windows-report.json> \
  --report-dir artifacts/local-p1-e4/final-wsl
```

The external checkout is used only when the operator explicitly sets
`ACP_LOCAL_PROJECT_ROOT`; it receives no writes/builds/hooks. Database/runtime state
for every trial is disposable, never the existing ACP user DB. No real project
screen, contents, personal preference, credential or raw conversation is uploaded.

Windows uses the process-local VS toolchain setup and explicit Python/test paths,
then `tools/local_verify.py --all --migration-mutations --owner-reviewed` in the
owned test checkout. `e4_acceptance` is separate from command success: it requires
a clean source SHA, Rust migration markers, all mandatory host checks, matching
Windows companion and completed owner review. Optional NOT_RUN is preserved.

## Final clean-code matrix

Tested code SHA: **`22dd5f91e67f71b432f751e047442c9d62708cd4`**, clean on both hosts.
The following final commit only updates documentation/audit records; no production
or test code changed after this verification. WSL reports
`e4_acceptance=PASS_LOCAL_WINDOWS_WSL_OWNER_REVIEWED` with a same-SHA Windows companion.
Windows reports **24 PASS / 0 FAIL / 0 BLOCKED / 2 NOT_RUN**; WSL reports
**27 PASS / 0 FAIL / 0 BLOCKED / 2 NOT_RUN**. The two optional gate-level NOT_RUNs are
web smoke and operator-assisted IME/screen reader. Individual platform/client
limits below remain explicit even when a containing gate passes.

| Gate / actual observed count | WSL/Linux | Native Windows |
| --- | --- | --- |
| Root fmt / Clippy, offline bin build | PASS | PASS |
| Root Rust tests | 47 PASS | 46 PASS |
| Remaining Python compatibility/safety tests | 16 PASS | 16 PASS |
| Independent JSON / SQLite comparisons | 120 / 78 PASS | 120 / 78 PASS |
| Synthetic backup / process recovery | PASS / 11 PASS | PASS / 11 PASS |
| Daemon/IPC/MCP E2E | 11 PASS | 11 PASS |
| Shared state / NSH component tests | 10 / 2 PASS | 10 / 2 PASS |
| Compiled fingerprint/core/IPC guard mutations | 3 rejected | 3 rejected |
| Actual desktop / terminal | GdkWaylandDisplay / PTY PASS | Native / ConPTY PASS |
| Native 10k interaction | filter, pointer/keyboard paging/detail/themes, resize PASS | filter and pointer/keyboard paging PASS |
| Local Codex 0.167.0 direct MCP client | Two tools, exact parity, 7 denials PASS | Native Windows Codex NOT_RUN; WSL installed client used |
| Windows owner → WSL helper | Actual native window + ConPTY, disconnect/reconnect PASS | Executed by the explicit WSL owner driver |
| Explicit external-root read-only integration | operator/IPC/MCP/UI/Codex + unchanged metadata PASS | Windows-owner views PASS, no real-data captures |
| Unsigned local archive | Exact Wayland-tested binary hash PASS | Exact tested host binaries PASS |

Rust totals differ because `documents_deny_escaping_file_and_parent_symlinks` is
Unix-only. It is NOT_RUN on Windows; no skip is relabeled PASS. Unix raw non-UTF8
name/target tests and Windows-specific mode/link tests have their own target guards.
Both hosts ran all seven Rust MCP process-security cases and the applicable exact
fingerprint oracles. UNC/surrogate-path combinations and cross-account attacks are
NOT_RUN. Clipboard intent/bounds are checked through the NSH test clipboard;
real system clipboard transfer is NOT_RUN to preserve the owner's clipboard.
Cancellation suppresses stale completion; it does not claim immediate termination
of an existing bounded IPC/WSL worker.

The selected external repository has a configuration for which status scans could
invoke an external program. No filter or submodule program is run. Git HEAD/branch
and the bounded lstat set are compared before/after; dirty/count remain **Unknown**
and status scanning is explicitly NOT_RUN. This still exercises the real scoped
operator/IPC/MCP/Codex and Windows-owner clients against disposable ACP state.
The pre/post metadata digests match; no full-tree content attestation is claimed.

### Actual release measurements

Three startup/RSS samples were recorded per surface, on this host only. WSL uses
Python 3.14.9; Windows Python 3.14.5. Rust/Cargo is 1.99.1 on both. Linux GTK first
paint, Windows layout acknowledgement and terminal text arrival are different
observations, not interchangeable presentation/frame measurements.

| Surface | Three startup observations (seconds) | RSS bytes at observation | 10k filter observation (seconds) |
| --- | --- | --- | --- |
| WSLg desktop | GTK after-paint: 0.797912 / 0.783168 / 0.792296 | 322084864 / 322908160 / 319004672 | model ack 1.8345; target capture completed 2.4544 |
| Windows desktop | layout ack: 0.6740 / 0.6284 / 0.6520 | 57110528 / 57212928 / 56954880 | model ack 0.1280; target image captured separately |
| WSL PTY | painted text: 0.0395 / 0.0383 / 0.0388 | 23400448 / 23552000 / 23564288 | painted matching text 0.0655 |
| Windows ConPTY | painted text: 0.6067 / 0.5789 / 0.5731 | 15699968 / 15826944 / 15708160 | painted matching text 0.6665 |

The Windows JSON's legacy `filter_to_rendered_state_seconds` field is an engine
state acknowledgement, not a GPU/compositor frame timestamp. Interop polling and
input injection overhead are included. Both native target-window captures visibly
contain `event-09999`; GTK readiness alone is not the rendering assertion. All 834
preference pages (10,000 rows) mount in the component test with fewer than 100 text
nodes per page; final observed workloads took about 90s WSL / 91s Windows. This is
a local stress observation, not a universal performance or accessibility claim.

### Private artifacts and packages

- WSL: `artifacts/local-p1-e4/final-wsl/report.json`, `wayland-result.json`,
  `integrations.json`, `local-project-result.json`, private logs and captures.
- Baseline/after: `artifacts/local-p1-e4/before/`, `after/`, `window-only/`; Windows
  owned checkout `artifacts/local-p1-e2/e4-before/` and `e4-after/` including full
  HWND exports. These contain synthetic data only and remain untracked.
- Linux unsigned archive: `artifacts/local-p1-e4/final-wsl/package/acp-nsh-dev-linux-x86_64.tar.gz`.
- Windows owned checkout unsigned archive:
  `artifacts/local-p1-e2/e4-final/package/acp-nsh-dev-windows-amd64.zip`.

| Packaged binary | Bytes | SHA-256 |
| --- | ---: | --- |
| Linux desktop | 52589792 | `c34fc6ea88bfb7689e9003e262b9a2674d5e16a1289a433034b7cbdb320570b4` |
| Linux TUI | 48391696 | `8de6219bc4aeef15836d3a95a827a660d0ec98395b9588dcdd84430ebcc52ce8` |
| Windows desktop | 41385984 | `8ab270445bf47fe0396db001efc257c2e7deeae1e809b53d30f3af0c1d22d468` |
| Windows TUI | 38123520 | `0bae444db6c819408063eef0d0320066bb94026b3066a08ad0148d91db232eca` |

The Linux desktop digest exactly matches the passing actual Wayland report and
package manifest. These are local unsigned development builds, not a signed
release, installer or portability guarantee.

## Branch disposition

The owner's final instruction superseded the initial remote-cleanup request:
clean local branches first, retain main/current work, and leave remote deletion
for GitHub after merge. Local `feat/handoff-worktree-fingerprint`,
`feat/p1-e2-local-console` and `feat/p1-e3-ui-wslg-wayland` were deleted only after
verifying exact retained upstream copies and the migrated content audit. Only
`main` and `feat/p1-e4-ux-rust-migrations` remain locally. Main fast-forwarded from
`d1cf45c` to `80fdbc5`; no checked-out worktree files were reset or removed.
All eleven old remote refs remain. No remote delete or repository-wide auto-delete
setting change was made. See the [exact-tip audit](../p1/e4-branch-audit.md).
The deliverable remains a Draft PR; merge and post-merge remote cleanup are the
owner's GitHub actions, not performed by this agent.
