# P1-M2/S5 — Rust-only integrated acceptance

Scope: [Issue #50](https://github.com/ybakiame/agent-control-plane/issues/50),
[contract](../p1/rust-only-integrated-acceptance.md), parent
[#40](https://github.com/ybakiame/agent-control-plane/issues/40).

The clean tested code revision is **ddc12ac8c17215014e04cc65ed802819b1bc8376**.
Explicit diff base: S4 merge **05035e21434085faf368ddec5a06904a99ef4a35**.
The matrix was committed first in **2624bdf6945c5cdfd13fc7d70f46a7e3d58e4ab1**.
Later report/runbook edits are documentation only; the PR must preserve an exact
`git diff --name-only ddc12ac HEAD` proof.

The only product correction is an Alt+Shift+H shortcut for the existing exact
saved-Handoff read, its English/Chinese help text, and an opt-in content-free saved
revision probe. Windows Tab/Enter did not activate that read; the driver now waits
for actual revision 1 and rendered saved history instead of accepting CLI state
alone. A real-component/daemon regression checks saved NOT_RUN history and that
changing to a missing task clears the prior record. No Profile v1 DDL, Receipt cap,
ownership, permission, write method or UI layout changed.

## Hosts and reproducible gates

Both hosts use Rust/Cargo 1.99.1, locked native caches and local builds. Linux is
Arch Linux under WSL2, x86_64, kernel 6.18.54.1-microsoft-standard-WSL2, GTK 4.24.1,
Wayland client 1.26.0 and xkbcommon 1.13.2. Windows x86_64 (OS build 10.0.26300.0)
uses Visual Studio 18.8.12023.21, MSVC 14.51.36231, SDK 10.0.26100.0 and native
binaries in the owned validation checkout. PowerShell 7.6.2 runs the gates;
the native window worker uses Windows PowerShell 5.1.26100.9818. Toolchain setup
is process-local. No system configuration was changed.

Literal commands are in the [runbook](../p1/rust-only-development.md) and
[driver guide](../../tools/native-acceptance/README.md). Each full helper used its
`all`/`All` mode, new report directory and the full S4 base above. Both checked
`git diff --check BASE HEAD` before Cargo, then all specified fmt/build/Clippy
(warnings denied)/all-target tests for workspace/shared/native,tui NSH.

| Final clean-source gate | Linux/WSL | Native Windows |
| --- | --- | --- |
| Workspace | PASS: 116 tests | PASS: 117 tests |
| Shared native model | PASS: 32 tests | PASS: 32 tests |
| NSH native,tui components + process test | PASS: 7 tests | PASS: 7 tests |
| Standalone native driver fmt/build/Clippy/test | PASS: 1 VT test | PASS: 1 VT test |
| Actual helper refusal/control flow | PASS: 10 cases | PASS: 10 cases |
| Node source/YAML/links | PASS: 205 source paths, 33 archives at code SHA | NOT_RUN: Node unavailable |
| Static server | PASS: 1 test | NOT_RUN: Node unavailable |
| Offline actual Rust WASM + Playwright | PASS: Chromium 151.0.7922.34 | NOT_RUN: Node unavailable |

The one ignored workspace test on each host is the existing S1 manual performance
measurement, not a security skip. The NSH 10k test mounts 834 bounded pages, then
filters/selects; it is a component test, separate from actual native input below.
Linux Node is 26.13.1, Playwright 1.65.0. The browser fetched only four local static
assets, made zero external requests and tested five pages, exact-task validation,
filter/details, Tab/Enter, appearance, read-only operator and 1080×760 → 640×480.
No native IPC gateway or WebSocket/model service was introduced.

Both actual helper fixtures prove committed whitespace fails with Git exit 2,
fixed committed delta passes, and missing/invalid/ambiguous/nonancestor/HEAD-equal/
empty delta/dirty source/existing output refuse before Cargo. The Cargo function/
executable is intentionally stubbed **only in these control-flow fixtures**;
all full suites and actual surfaces use real Cargo/product binaries.

## Four actual native surfaces

**PASS — engineering integration acceptance at the clean code SHA above.** All
four native surfaces passed the strengthened saved-Handoff observation.
Earlier driver-only PASS with an empty saved-Handoff panel was rejected during
visual inspection and is not final evidence. No S3/S4 observation is substituted.

Every surface creates a new synthetic `acp-s5-*` root and private control-only
runtime. The actual UI explicitly reviews creation/selection and write grant.
Native CLI setup registers two pinned synthetic roots and saves exact-task history;
no default Profile, user database, authentication directory or external checkout is
read. Only the daemon links the writer in normal dependencies (`cargo tree -i
acp-write --edges normal`). Neither the maintenance driver nor UI links it.

| Surface at code SHA above | Result and independently observed evidence |
| --- | --- |
| WSLg desktop | PASS: actual GdkWaylandDisplay, GskGLRenderer, mapped window/after-paint; guarded Windows host input, owned-window PNGs, actual CLI state/receipts |
| Linux/WSL TUI | PASS: openpty, actual child VT output/keyboard, resize, alternate-screen and termios restoration |
| Windows ConPTY TUI | PASS: actual ConPTY keyboard/VT output, resize, alternate-screen restoration and clean child exit; CLI state/receipts |
| Native Windows desktop | PASS: actual nonce/PID-owned HWND, protected native keyboard input, saved revision 1 rendered, per-page/compact PNGs, CLI state/receipts and clean exit |

These are actual running Rust daemon/query/NSH processes. The test driver supplies
real keyboard sequences to PTY/ConPTY or native key events to the unique HWND.
Desktop PNGs are captured from only the nonce/PID-owned window (WSLg: nonce plus
msrdc host), not the desktop. The agent visually inspects the final PNGs and TTY
captures; this is engineering evidence, not a new product design approval.

All four surfaces showed these native-input results:

| Actual input/action | Independent result |
| --- | --- |
| Unselected → reviewed create → explicit enable | Selected synthetic v1 Profile; creation alone grants no writer |
| Competing owned daemon enables its lease | UI Busy; no write grant until explicit competitor release and new review |
| Scoped suggestion → exact revision confirm; second suggestion reject | CLI confirms statuses and revisions; only confirmed row appears in effective preferences |
| Drop a committed mutation reply, then explicit retry | UI OutcomeUnknown; actual dropped and replayed Receipts have complete JSON equality, commit 8, exactly three rows before/after retry |
| Competing CLI rejects reviewed revision 1 | Old UI apply reports Conflict; no extra commit or revived record |
| Escape and Ctrl+P project change after prepared review | Review discarded; apply after round-trip project selection adds no row |
| Inspect saved alpha/shared Handoff | Actual saved revision 1 with NOT_RUN validation/unknown snapshot; UI explicitly labels history, not fresh Git verification |
| Acknowledged disable, explicit re-enable, external revoke | Write grant off acknowledgement; later UI suggestion Unauthorized, three rows retained |
| Alt+1–5, Sessions, theme, English/zh-CN, compact resize | Separate page/window or terminal captures; existing UI retained |
| Stop exact owned daemon and refresh operator | Unavailable and controls disabled; previous context observation is not a new successful read |

The relay is **test-only fault injection** on authenticated local IPC, not a new
product transport. It drops one real successful reply after commit; it never
fabricates a response or mutates the DB directly. Replay success requires the
same immutable mutation ID/input/binding/epoch; complete Receipt equality and
unchanged logical rows are independently asserted. Wrong bytes/ID, stale epochs,
CAS and revocation are also covered by the real process suite.

Task/scope/target changes and late completion races are proved by shared-model
regressions, not all injected with OS input. Real native input covers Escape and
project invalidation, grant/review/Unknown recovery. NSH component/process tests
supply additional boundary assertions. Registration/deprecate/delete/Handoff put
remain visibly CLI-only; Windows-to-WSL helper, synthetic/offline and Web remain
read-only with agent execution disabled.

Both full suites and all driver gates were repeated at the final SHA after fixing
Windows fixture environment isolation. Each fixture restores process environment
(including PATH and temporary Git configuration), preserving caller Git/toolchain
availability; no user/system environment is changed.

Windows foreground protection remains mandatory. An exploratory run aborted when
focus changed during field input; it is not counted. The driver never calls
SetForegroundWindow or bypasses focus policy. Fixed ASCII field entry avoids
WSLg Unicode-packet delivery differences. The final HWND report records 79
successful English-layout checks. Windows per-window IME state initially
prevented complete path entry. With explicit operator authorization, the worker
checks only the owned native thread, requests an already-installed English layout
when needed, verifies acknowledgement/foreground, and restores the original layout
if the window still exists at worker exit. No layout is installed/loaded and no
persistent input/system setting changes. Chinese UI rendering is exercised; IME
composition input is not certified. ConPTY explicitly receives its own
standard handles instead of inherited redirected runner pipes; see Microsoft's
[ConPTY lifecycle](https://learn.microsoft.com/en-us/windows/console/creating-a-pseudoconsole-session).
These are driver fixes; no product security checks were removed or assertions relaxed.

## Retained security and full-chain evidence

Both full workspace suites exercise native CLI lifecycle/all seven commands;
strict fresh v1 selection and legacy refusal; Windows case/path spelling;
exclusive file identity/lease; restart epochs; write admission/drain/revocation;
exact scopes, bounded previews, CAS/state transitions and atomic peer deprecation;
receipt replay/IdempotencyConflict/capacity and whole-state equality under rejects.

Independent Rust MCP stdio/fake ACP-agent processes exercise exactly two read-only
project-bound tools, administrative role denial, malformed/auth requests, root
binding/drift/symlink escapes, default deny vs allow-once, permission expiry,
Stop/Cancel/expiry/restart revocation on open peers and new handshakes, and child
cleanup. No model/provider/network authentication is used. Full test names and
outcomes stay in the private helper logs; sources are
[ownership](../../crates/acp-daemon/tests/ownership.rs),
[sessions](../../crates/acp-daemon/tests/sessions.rs),
[MCP process security](../../crates/acp-mcp/tests/security_process.rs),
[store refusal](../../crates/acp-store/tests/sqlite.rs), and
[shared operator races](../../apps/operator/model/src/operator.rs).

## Actual binary identities

These hashes were recomputed against the on-disk executables after all native
runs and matched every per-surface report at the frozen code revision.

| Platform/program | Actual SHA-256 |
| --- | --- |
| linux/acp-native-acceptance | `556e5df1a3ca357420728d35cfe5f343db9cf3e68e14338f789c94bf06913b35` |
| linux/acp-daemon | `eb20a194f0baa711f9ae0edd4ff1fce6111efd67fb1f16005c1b2066a0923a90` |
| linux/acp-query | `46e39f07704aa3ba0c0feb43b2e2ecf4772f8b9a4ad11f92ab712aaf8c27ca64` |
| linux/acp-nsh-tui | `b2f23023e47d6acb742a08ec85be8932cf4e1313b9c5d2b7fff4afdf400c2af1` |
| linux/acp-nsh-native | `65c475bad537cf183cdaddcbb6d957706d659dab4e5d80f7eb14c4a06306fcd0` |
| windows/acp-native-acceptance.exe | `0891baebf29c3b4777948f115bd65291587a2af7fa6fa5d97c66ec9603887478` |
| windows/acp-daemon.exe | `af1433b1d560a8fdb3639a927b53f939610eb2f1b296b1c94a71f08ad89d4abd` |
| windows/acp-query.exe | `2772f1fa08b9cb1fb9b0a0b717890cc10d16c18ef42aafdd13da00ad50f3adc3` |
| windows/acp-nsh-tui.exe | `54d0298a2c3161f6a17c1b2e306f90807555851199093dffc9022c2e52528da4` |
| windows/acp-nsh-native.exe | `2bb5c8e928c7e177c0750f08ee8097249b8bcfe5aae564a2755dfb524b952349` |

## Private artifact index and limits

All paths below are relative to the corresponding Linux or native Windows
checkout, ignored and **not uploaded**. Reports record clean Git SHA and actual
SHA-256 of driver/daemon/query/NSH binaries. Per-surface `receipt-proof.json`
contains only the two synthetic Receipts, no credential or user record.

| Artifact under artifacts/local-p1-m2/ | Evidence |
| --- | --- |
| s5-linux-final-r6/; s5-windows-final-r6/ | Full helper logs, command/status, exact source and baseline |
| s5-linux-driver-final-r6/; s5-windows-driver-final-r6/ | Standalone gates/helper refusal regressions; Linux source/server/writer-dependency check |
| s5-wsl-pty-final-r6/; s5-conpty-final-r6/ | Actual terminal captures/raw VT, receipt equality, binary hashes and final report |
| s5-wslg-final-r6/; s5-windows-desktop-final-r6/ | Actual owned PNGs, content-free native probes, receipt equality/binary hashes/report |
| s5-web-final-r6/ | Actual offline WASM package/browser screenshots and web-result.json |

The final Windows full suite ran before its ConPTY and HWND scenarios. An earlier
parallel build refused to replace an executable held by an owned test daemon; that
failed attempt remains separate and is not a passing gate. Final paths above use
only the sequential completed run.

Only exact run-owned children/temp roots are cleaned. Exploratory reports remain
separate with DEV_PASS or failure, never final acceptance. No indiscriminate
cleanup, real data upload, model call or production Profile activation occurred.

Hosted GitHub Actions, macOS, mobile, niri, clean installation/package/release,
real model/provider/auth and production write activation: **NOT_RUN**.
The Draft PR does not automatically merge or close parent #40. Only after owner
review and S5 merge may #40's engineering-validation completion be assessed.
