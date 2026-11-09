# P1-E3 local validation

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

Implementation SHA, clean-worktree final totals and measured samples will be
recorded after the final local gate. Raw artifacts remain in ignored private
`artifacts/local-p1-e3/`, with the Windows-owned test copy retaining its corresponding
host report. No absolute private path, actual project metadata, raw handoff,
profile values, token, screenshot or machine identifier belongs in this document.

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
