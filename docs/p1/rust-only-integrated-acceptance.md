# P1-M2/S5 — final integration acceptance contract

Frozen before implementation for [Issue #50](https://github.com/ybakiame/agent-control-plane/issues/50),
parent [#40](https://github.com/ybakiame/agent-control-plane/issues/40).
Base: merged S4 main **05035e21434085faf368ddec5a06904a99ef4a35**.
Branch: `test/p1-m2-s5-rust-only-integrated-acceptance`.
Authority: AGENTS, accepted ADR-0004/0006, retained ADR-0005 ownership and fresh
Profile v1/S2/S3 contracts. No Schema, writer, UI workflow or production activation.

## Checked source and diff baseline

Both native helpers must accept one explicit commit baseline, resolve it to one
commit, reject missing/invalid/ambiguous/non-ancestor/HEAD-equal bases, and record
its full SHA. Validate `git diff --check BASE HEAD` before expensive Cargo gates,
while retaining clean-worktree and non-overwrite output guards. Keep separate fmt.
Full S5 uses the S4 merge above. Do not infer PR merge-base from shallow CI refs.

Use isolated temporary Git fixtures to run both actual helpers on committed bad
whitespace and fixed commits, plus absent/invalid/ambiguous/non-ancestor bases,
dirty source and existing output. Refused cases cannot run Cargo. Record exact
command/exit status. Fixture-only gate stubs may bound helper control-flow tests;
they are not runtime PASS. Full native suites use actual Cargo and binaries.

## Hosts, native drivers and owned state

| Surface | Planned Python-free driver and independent observation |
| --- | --- |
| Windows Desktop | Actual NSH HWND; bounded PowerShell/Win32 input/capture, unique 16-hex title nonce + exact child PID, foreground check before every input; native rendered state probes plus CLI receipts |
| WSLg Desktop | Direct existing WSLg Wayland socket, actual GdkWaylandDisplay/renderer/mapped/after-paint probes; unique nonce-matched WSLg host window, guarded native input/capture; CLI receipt equality |
| Windows TUI | Rust-created real ConPTY with child process, VT output capture and native keyboard byte sequences, resize/clean exit/restoration observation; CLI state/receipts |
| Linux/WSL TUI | Rust openpty/termios child, actual output and keyboard sequences, resize/terminal restoration; CLI state/receipts |

An isolated maintenance Cargo manifest may host only bounded fixture/PTY/relay
orchestration, outside product workspaces. No writer dependency or new product
transport. Windows window utility must never force foreground, select a broad
title, send to another process, capture the full desktop, or bypass ownership.
If focus is missing, pause and ask the operator; do independent TTY work meanwhile.
No niri, X11/XWayland/Xvfb, model, real auth, user profile or external repository.

Each run owns a fresh OS temporary `acp-s5-*` root: private `state`, `runtime`,
explicit synthetic alpha/beta checkout roots, unique socket/pipe endpoints and
bounded Handoff input. All writes use the actual native CLI or explicitly reviewed
NSH controls. No default Profile/grant/daemon on reconnect. Delete only this run's
owned temporary root after stopping/reaping its exact children. Private logs,
owned-window screenshots and reports go to a new ignored `artifacts/local-p1-m2/`
directory; never remove retained credentials, unrelated local data or old reports.

## Four-surface action and boundary matrix

Every actual surface must demonstrate unselected/selected Profile, explicit
create/select or independently observed selection, enable/acknowledged disable,
exact scoped list/record/revision, suggestion, deliberate confirm/reject review,
exact-task saved Handoff inspection, context/operator connectivity separation,
English/zh-CN, theme, keyboard/focus/Escape, five pages and compact resize.
Registration, deprecate/delete and Handoff put remain visibly CLI-only.

| Boundary | Native input/output and external assertion |
| --- | --- |
| Busy | Separate owned daemon explicitly holds the same Profile lease; UI enable reports Busy, no grant; explicit release allows later reviewed enable |
| Unauthorized | Explicit external operator grant revocation; observed old UI cannot submit another mutation; no new row/receipt or fallback |
| Conflict | Review exact suggested revision, then competing explicit CLI review; original UI confirmation reports Conflict without another commit |
| OutcomeUnknown | Local authenticated test-only relay consumes one actual committed receipt then drops its reply; UI retains ID/request; explicit identical same-epoch retry returns that receipt, no extra row/commit |
| Prepared review invalidation | Escape and changed project/task/scope/target discard review; no stale write. Real input demonstrates selection changes; deterministic shared tests cover late-success/failure races |
| Handoff vs live facts | CLI saves synthetic validation NOT_RUN with unknown snapshot; exact saved revision/history is inspectable, distinct from current Git observations; no invented PASS |
| Disconnect/read-only | Stop exact owned daemon, observe Unavailable; Windows-to-WSL helper, synthetic/Web targets cannot write or execute agents |

Per-renderer reports must state which fault was exercised with OS input versus
which race was proved at shared-model/process level. Critical grant/review/retry
requires actual input on all four surfaces. Missing essential native evidence is
BLOCKED/NOT_RUN and unresolved, never inherited from S3.

## Retained full-chain and final gates

Both native Cargo suites retain strict v1/legacy/unsafe refusal, selection/file
identity, exclusive lease/crash/restart epochs, write admission/drain/revocation,
all seven typed commands, CAS/transitions/atomic peer deprecation/receipt equality
and complete logical-state equality under rejection. Established agent/session/
OperatorWrite peers cannot inspect administrative records. Real independent MCP
stdio and deterministic fake ACP sessions prove two read-only tools, scope/root/
permission/malformed/auth/expiry/Stop/Cancel/restart boundaries without a vendor.

Run both helpers' full fmt/build/Clippy -D warnings/all-target tests at one clean
final code SHA, including shared and native,tui NSH/10k/component gates. Build,
fmt/Clippy/test the maintenance driver separately on both hosts. Re-run impacted
checks while developing, then full suites once at final source. Record literal
commands and test counts; one ignored S1 manual performance measurement is not a
security skip. Verify only the daemon normally links the writer and DDL/cap unchanged.

Run Linux Node source/YAML/links, static server and actual offline WASM/Playwright;
zero external requests/WebSocket/gateway, mutation controls disabled. Windows Node
is supplemental only if already available; do not auto-install system dependencies.

Publish a human-readable verdict under docs/evaluation with exact clean source,
OS/toolchain/backend/binary identities, commands, observed input/results/receipts,
who observed them, private artifact paths and PASS/FAIL/BLOCKED/NOT_RUN. Later
docs-only SHAs must have an explicit diff proof. No Actions/macOS/mobile/niri/model/
installer/release claim. Every commit uses [skip ci]. Submit Draft PR, Closes #50,
Refs #40; no self-merge or automatic parent closure.
