# P1-E3 local validation

> **Historical record — not a current runbook.** Original dates, source revisions
> and outcomes below are preserved. S4 retired the executable prototype and probes;
> use [Rust-only development](../p1/rust-only-development.md) for current commands.

Issue #18, based on latest main `6c331d2c5632fce6140425eb53ed96ce924ec711`.
Date: 2026-11-09. All execution is local WSL/Windows; no Actions dispatch/rerun,
release or upload. Commits use `[skip ci]`; the deliverable is a Draft PR with no
automatic merge. ADR-0003 remains **Proposed**, mobile remains deferred.

## Observed baseline and diagnosis

The clean Windows main baseline completed **22 PASS / 0 FAIL / 2 NOT_RUN**.
WSL main root fmt/Clippy/tests, 24 Python tests, 120 JSON / 78 SQLite comparisons,
backup/recovery/service, model checks and both NSH component tests passed locally.
The legacy Linux X11 desktop acceptance was deliberately not used to certify
Wayland. An early development runner report records a transient probe compile
failure, and later experiments record real input/resize assertion failures; those
private reports are retained rather than relabeled as passing baseline results.
Final implementation evidence is recorded separately below.

The current Codex subprocess and explicit Windows `wsl.exe --exec` connect to the
same WSLg runtime socket classification. The peer PID is hidden by the system-
distro namespace; socket reachability and actual GDK/protocol observations provide
additional evidence. Installed niri is not running. After seeing the direct
Wayland window render normally, the owner explicitly chose not to start niri;
**niri remains NOT_RUN**, not a WSLg/niri combined pass. Interactive-terminal and
niri-launched-terminal environment captures remain NOT_RUN.

Observed software: WSL2 kernel **6.18.54.1**, WSLg **1.0.79**, GTK **4.24.1**,
client Mesa **26.2.6**, EGL **1.5**, Rust/Cargo **1.99.1**, local Codex **0.167.0**;
NSH pinned `87c7e8da85d2f15a56107baa6f0fabb419b508b3`. The installed niri version
is **26.04**; it is not counted as an executed compositor test. Exact WSLg/Weston
build revisions and host version observations are in the private doctor/reports.

The old probe forced X11. Removing that assumption and using actual GTK backend
and compositor-appropriate OS window detection fixed the acceptance path. NSH and
minimal GTK controls both use **GdkWaylandDisplay**, map and paint under automatic
and explicit per-child Wayland selection. A bounded private trace observed surface
commits/presentation feedback and real resize configure/ack. The host's GTK theme
warnings also occur in the minimal control and disappear with a temporary child
configuration directory; user configuration was left untouched. A fresh synthetic
X11 capture succeeded and did not reproduce the historical E2 BadMatch. No universal
X11 repair, upstream widget crash fix or GPU acceleration is claimed.

## Development observations

Three direct WSLg release samples reached real windows, model/layout, GTK after-
paint and accepted project/task/filter/Tab/Escape/resize/reconnect input. Synthetic
10k filtering produced one row in the real Wayland content capture. Real WSL PTY,
Windows native/ConPTY, Windows-owner helper and local Codex direct MCP probes were
also exercised. Deterministic model tests cover out-of-order scope/task results,
late success after authorization loss, invalid task invalidation, unexpected task/
scope and non-ready snapshots, bounded evidence and sanitized display. The NSH
renderer checks all 10,000 rows across all 834 pages with bounded text-node count.

## Final reproducible evidence

All final executable checks used clean implementation commit
`da40bbd62bcf84fc7ed8ca0818ddd12a22d87992` on both hosts. The following evidence
commit changes documentation only. Latest main was rechecked and remained the
base above. Private reports preserve argv, versions, exact SHA and dirty=false.

| Local check | Actual result |
| --- | --- |
| WSL verified combined gates | **24 PASS / 0 FAIL / 0 BLOCKED / 2 NOT_RUN** |
| Windows native host gates | **22 PASS / 0 FAIL / 0 BLOCKED / 2 NOT_RUN** |
| Selected external project, explicit opt-in | **1 PASS**, independent fresh state; agreed pre/post Git/metadata equal |
| Windows owner → WSL snapshot regressions | PASS: context equality, foreign/hostile project denial, removed credential, stopped daemon, restart and unchanged disposable DB |
| Native Wayland direct WSLg | **RUN_PASS_WAYLAND_WSLG**, actual GdkWaylandDisplay, GskGLRenderer, three release samples |
| Existing niri-session | **NOT_RUN**, owner chose not to start it after direct Wayland rendered |
| Root tests | 32 Rust tests WSL / 31 Windows; 24 Python tests each; fmt and all-feature Clippy PASS |
| Contract and persistence | 120 JSON + 78 exact SQLite comparisons on each host; backup rehearsal and 11 process-kill recovery tests PASS |
| Native daemon / IPC / MCP | 11 process tests per host PASS; standalone/auth/scope/malformed-request and lifetime checks retained |
| Shared presentation | Nine tests per host PASS, including delayed/out-of-order project/task, revoked auth, malformed/non-ready scope, bounded evidence and input limits |
| NSH components | Two tests per host PASS; every one of 10,000 rows mounted across all 834 pages, fewer than 100 text nodes; page/focus/theme/disclosure/viewport checks |
| Real PTY / ConPTY | Three startup/RSS samples each; 10k→1 painted text, task `q`, Tab, CJK bracketed paste, 80×30 resize, Escape and Ctrl+Q restoration PASS |
| Real native desktop interaction | Windows native and WSLg: keyboard project/filter, 10k unique row in content-only captures; WSLg exact task, 640×480/1080×760 configure and restart/credential recovery PASS |
| Negative Wayland paths | Missing explicit socket fails startup; automatic actual X11 fallback rejected; no-frame/blank-content acceptance guarded by bounded probes |
| Optional app-only Cairo diagnostic | PASS, actual GdkWaylandDisplay + GskCairoRenderer; software, no hardware-acceleration claim |
| Actual local Codex | Two project-bound schemas and direct stdio tool calls, exact context parity and seven denials PASS; no model turn or user-auth copy |
| Selected-project owner views | Actual Windows Desktop/ConPTY → explicit WSL helper PASS; no screenshots, clipboard reads or raw live values retained |
| Local development packaging | Unsigned Linux x86_64 archive after exact-binary Wayland hash match; Windows x64 ZIP; no installer, signing, autostart or distribution |

The first clean WSL full run reported **21 PASS / 3 FAIL / 2 NOT_RUN**. I had shared
its target directories with a disposable main-baseline checkout: cached Rust tests
embedded that checkout's `CARGO_MANIFEST_DIR`, and after its removal the synthetic
fixture builder paths no longer existed. This affected `root-tests`,
`backup-rehearsal` and `process-recovery`, not the Wayland result. Cleaning only
ACP package build artifacts and running **all core/service gates** again produced
**11 PASS / 0 FAIL** at the same clean implementation SHA. Both original reports
remain unchanged. `final-wsl-verified.json` explicitly records the two source
reports, cache-reset reason and superseded failing gates; it combines the successful
retry with the independently passing UI gates. It is not a rewritten first-run
PASS. Do not share a target directory across removed checkouts when tests embed
compile-time fixture paths.

Private locations (relative to each owned ACP test checkout):

- WSL `artifacts/local-p1-e3/final-wsl/report.json`, `final-wsl-core-retry/report.json`
  and `final-wsl-verified.json`; Wayland captures/logs/result, GTK control/doctor,
  local package and Codex/owner integration under `final-wsl/`.
- Windows `artifacts/local-p1-e2/e3-baseline/` and `e3-final/` contain their clean
  main/implementation reports, native/ConPTY synthetic captures and unsigned ZIP.
- WSL `artifacts/local-p1-e3/environment-final/`, `final-windows-wsl/` and
  `final-selected-project/` contain launch-context, helper and protected-root checks.

No raw artifact was uploaded. No preexisting ACP database was opened. The selected
root comparison covers bounded Git HEAD/branch/complete porcelain and agreed lstat
metadata, not ignored-file content or a complete filesystem attestation.

### Release observations

Warm local caches; Windows verification was also running during WSL measurements.
These are this session's observations, not an isolated framework comparison.
WSLg visibility is an OS-poll upper bound; ready/layout and after-paint are Linux
process monotonic markers, not a compositor presentation timestamp. Windows layout
is the parent's observed marker arrival. Both actual native content crops were
visually inspected locally and showed the unique filtered row.

| WSLg sample | OS visible (s) | Model ready / layout (s) | GTK after-paint (s) | RSS bytes |
| --- | --- | --- | --- | --- |
| 1, automatic backend | 1.4404 | 0.171678 / 0.180631 | 0.898877 | 325,148,672 |
| 2, explicit Wayland | 2.2098 | 0.145978 / 0.152016 | 0.841413 | 325,361,664 |
| 3, explicit Wayland | 1.0295 | 0.144314 / 0.152864 | 0.889469 | 324,624,384 |

WSLg 10k: **1.8076 s** input-to-model acknowledgement including OS helper/input
injection; **2.4591 s** to later actual capture completion. RSS before/after filter
327,692,288 / 344,051,712 bytes. Linux desktop/TUI executables are **52,271,112 /
48,091,904 bytes**. Default GL reports Mesa software fallback; GPU acceleration is
NOT_VERIFIED. The per-child Cairo diagnostic is separately verified software.

| Windows sample | Visible / observed model layout (s) | Desktop RSS bytes | ConPTY painted startup (s) | ConPTY RSS bytes |
| --- | --- | --- | --- | --- |
| 1 | 0.3866 / 0.8095 | 55,701,504 | 0.7509 | 15,228,928 |
| 2 | 0.0987 / 0.5004 | 55,562,240 | 0.4431 | 15,331,328 |
| 3 | 0.0988 / 0.5014 | 55,554,048 | 0.4803 | 15,347,712 |

Windows 10k model acknowledgement: **0.1202 s**, with a content crop captured later
(not an exact frame timestamp); ConPTY painted filter: **0.6485 s**. Desktop/TUI
executables: **41,202,688 / 37,939,712 bytes**, PE x64 with no embedded signing
certificate. Windows native MSS captures exclude the client edges; they are
synthetic interior crops, not full-window exports. The measured host remains
Windows 11 **10.0.26300**; clean-machine installation remains NOT_RUN.

WSL PTY painted startup: **0.0382 / 0.0378 / 0.0386 s**, RSS **22,982,656 /
23,109,632 / 22,892,544 bytes**; 10k painted filter **0.0431 s**. These terminal
numbers use actual emulated screen text, including CJK paste/resize validation.

Use [the architecture](../p1/ui-console-architecture.md) and
[the Wayland commands/measurement definitions](../p1/wslg-wayland-troubleshooting.md).
A model/GTK callback is not a frame-presentation timestamp. OS visibility includes
helper polling overhead; captured-frame completion is measured separately. GSK GL
with Mesa software fallback is not verified hardware acceleration. Linux packages
are permitted only after a passing report matches the exact binary hash.

Unavailable macOS/mobile, full IME/screen reader, GPU acceleration, clean-machine
installation and optional GPUI cache-limited rebuild remain explicitly NOT_RUN.
The default ACP user DB is never opened. External checkout validation requires the
existing explicit root opt-in and fresh disposable ACP state; real data is never
captured or uploaded. P0 write ownership, standalone daemon, operator authentication,
project-pinned two-tool MCP and token isolation are unchanged.
