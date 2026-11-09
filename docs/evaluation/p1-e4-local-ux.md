# P1-E4 local NSH UX and Rust migration evidence

Status: implementation and development checks complete; final clean-source host
matrix and branch cleanup results are recorded below after execution. All logs,
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

Final exact code SHA, measured counts/timings/hashes, clean checkout outcome,
unsigned package locations and remote cleanup outcomes: **pending final run**.
