# P1-M4/S0 — review/provenance contract freeze and bounded desktop RPC

Scope: [Issue #53](https://github.com/ybakiame/agent-control-plane/issues/53),
parent [Issue #52](https://github.com/ybakiame/agent-control-plane/issues/52).
Deliverables: [operator review and provenance contract](../p1/operator-review-contract.md)
and [gap plus four-surface acceptance matrix](../p1/operator-review-acceptance-matrix.md).
This is a contract and acceptance-harness preflight, not M4 production implementation.

Baseline: merged M2/S5 main
`18f2670d294deebbc17f530c36bebecdaf8beb90` (PR #51). Issue
[#40](https://github.com/ybakiame/agent-control-plane/issues/40) was verified closed
at 2026-11-13T11:25:19Z before this branch was created from that exact revision, and
PR #51 had no unresolved review feedback.

| Commit | Content |
| --- | --- |
| `f0b7db3` | Deliverable A only: contract, gap/acceptance matrix, current-status roadmap lines. No source file changed |
| `ba51c4c` | Deliverable B: bounded desktop worker RPC, owned fake worker, 12 harness tests |
| `d0c6cfc` | Native Windows Clippy `large_enum_variant` correction: box the driver's desktop variant |
| `b140af5` | Evidence report plus its roadmap link |

**Final tested code SHA: `d0c6cfcf163757ccbb266d763b5da1f1f95073a8`.** Every gate and
native run below was executed on that revision or on a byte-identical tree. Commits
after `d0c6cfc` are documentation only — `git diff --name-only d0c6cfc HEAD` lists
nothing outside `docs/` — so no result here is claimed to have been re-executed at a
report commit.

## 1. Changed surface

`git diff --name-only 18f2670 d0c6cfc` returns three `docs/` paths and eight paths under
`tools/native-acceptance/`. No product crate, DDL, writer, daemon, IPC, CLI, MCP,
NSH or shared-model file is touched, which is the precise rationale for not
re-executing the product-only regression suites as new acceptance: their inputs are
byte-identical to merged S5. The full Linux/WSL workspace, shared-model and NSH gates
were still run at the final SHA (section 2) rather than argued away.

The driver change replaces the unbounded
`BufReader<ChildStdout>::read_line` in `tools/native-acceptance/src/desktop.rs` with
`tools/native-acceptance/src/worker.rs`: one request settles with exactly one
validated reply inside its own deadline (init 300 s, capture 180 s, other actions
60 s) under a monotonic sequence number; replies carrying a nonce must name the owned
window; hung, malformed, oversized, truncated, silent, early-exiting, unsolicited or
wrong-nonce worker behaviour faults the session closed, terminates and reaps only the
owned worker, and joins the reader thread through its own stdout EOF instead of
detaching it. Foreground readiness keeps its separate finite 900 s wait. Nonce, exact
PID and WSLg `msrdc` ownership, frontmost-only input, the fixed chord vocabulary,
Ctrl+1 exclusion and the already-installed-English-layout restoration path are
unchanged.

## 2. Linux/WSL verification

Host: Linux 6.18.54.1-microsoft-standard-WSL2, x86_64, WSLg `wayland-0`.
Toolchain `rustc`/`cargo` from `rust-toolchain.toml`.

`tools/local_verify.sh all /tmp/acp-s0-final-report 18f2670d294deebbc17f530c36bebecdaf8beb90`
ran on a separate clean clone checked out at `d0c6cfc` (the working copy holds an
untracked CLI session directory, so gates were executed on a pristine checkout instead
of loosening the script's clean-tree guard). Output is retained under the ignored
`artifacts/local-p1-m2/s0-linux-final/` directory. The script refuses a dirty tree,
demands a 40-character ancestor baseline and executes `git diff --check 18f2670… HEAD`
as its first and last gates.

| Gate | Result |
| --- | --- |
| branch-diff `git diff --check 18f2670… d0c6cfc` | PASS |
| workspace-fmt / workspace-build / workspace-clippy `-D warnings` | PASS |
| workspace-tests | PASS: 116 tests, 0 failures |
| shared-fmt / shared-clippy `-D warnings` | PASS |
| shared-tests | PASS: 32 tests, 0 failures |
| nsh-fmt / nsh-build / nsh-clippy `-D warnings` | PASS |
| nsh-tests | PASS: 7 tests, 0 failures |
| final `git diff --check` | PASS |

All 13 recorded gates PASS (`artifacts/local-p1-m2/s0-linux-final/result.tsv`, whose
`source.txt` records the SHA, diff base,
`Linux 6.18.54.1-microsoft-standard-WSL2 x86_64`, rustc and cargo 1.99.1). The
product-side counts are identical to merged S5's Linux counts, which is the expected
result for a byte-identical product tree.

An earlier identical run at `ba51c4c` recorded 13/13 PASS too
(`artifacts/local-p1-m2/s0-linux-ba51c4c/result.tsv`); the final-SHA table above is the
one this slice relies on.

Harness manifest gates at the final SHA in the working copy:

| Command | Result |
| --- | --- |
| `cargo fmt --manifest-path tools/native-acceptance/Cargo.toml -- --check` | PASS |
| `cargo clippy --manifest-path tools/native-acceptance/Cargo.toml --all-targets --locked -- -D warnings` | PASS |
| `cargo test --manifest-path tools/native-acceptance/Cargo.toml --locked` | PASS: 12 `desktop_worker` + 1 `screen` test, 0 failures |
| `node tools/check_source.mjs` | PASS: 213 active/historical tracked paths, zero tracked prototype executables or packaging, 33 marked archives, all active refs, local Markdown links and workflow targets valid |

## 3. Native Windows verification

The Windows host has no `git.exe` on a non-interactive `PATH`, so the source was not
checked out there. Instead the tracked file set was copied from this working copy into
the owned Windows validation copy
`C:\Users\ybakiame\code-workspaces\acp-m1-validation-m44gv_kj`, and identity was
proved by hashing all 212 tracked paths on both hosts: both manifests normalize to
SHA-256 `a657895e5b8e0c3a4eb313ea28679243ca8b14c74fd3fae8c94bf9c5c01d4ad2`, so the
Windows tree is the same source as `d0c6cfc`.

`C:\Users\ybakiame\code-workspaces\s0-windows-harness-gates.ps1` (process-local
`%USERPROFILE%\.cargo\bin` PATH plus `vswhere`/`Enter-VsDevShell` only; nothing is
installed and no persistent setting changes) produced
`s0-windows-harness-gates/report.json` on Windows 10.0.26300.0 with cargo 1.99.1 and
rustc 1.99.1. It is retained, with each gate log, under the ignored
`artifacts/local-p1-m2/s0-windows-harness-gates/` directory:

| Gate | Result |
| --- | --- |
| `harness-fmt` | PASS |
| `harness-clippy --all-targets --locked -- -D warnings` | PASS after the `d0c6cfc` boxing correction |
| `harness-tests` | PASS: 12 `desktop_worker` + 1 `screen` test, 0 failures, 0.35 s |
| `tools/windows_local_verify.ps1 -Suite All` | NOT_RUN: `git diff --check BASE HEAD` cannot execute without a Windows `git`, so the whitespace/diff guard evidence is Linux-side only |
| `git diff --check 18f2670… d0c6cfc` | PASS on Linux/WSL |

An earlier Windows Clippy run at `ba51c4c` **FAILED** with `large_enum_variant`
(360-byte desktop variant against a 72-byte TTY variant, `src/main.rs:191`); this is
the finding that produced `d0c6cfc`. It is recorded rather than smoothed over, and it
shows the value of running the lint on the host that measures the layout.

## 4. Native desktop and PTY observations

Owned synthetic root, private runtime, dedicated IPC endpoint and a freshly created
disposable Profile only. No real ACP database, external development repository, model,
provider, credential or authentication directory was touched.

| Surface | SHA | Result | Evidence |
| --- | --- | --- | --- |
| WSLg native Wayland desktop (real OS input through the refactored bounded driver, real daemon/CLI/NSH) | `d0c6cfc` | **PASS** | `artifacts/local-p1-m2/s0-wslg-desktop-20261113-213012/report.json` |
| Same surface at the intermediate commit | `ba51c4c` | PASS | `artifacts/local-p1-m2/s0-wslg-desktop-20261113-212806/report.json`; superseded by the final-SHA row |
| Linux/WSL PTY (`tui`) | `d0c6cfc` | **PASS** | `artifacts/local-p1-m2/s0-linux-pty-20261113-213058/report.json`, backend `PTY`, `acp-nsh-tui` `178764dc0260…` |
| Native Windows HWND desktop | — | NOT_RUN | requires product binaries built on Windows; not authorized for this slice |
| Windows ConPTY | — | NOT_RUN | as above |
| niri, mobile, X11/XWayland/Xvfb, headless, Web | — | out of scope / not substitutes | unchanged S5 boundary |

The WSLg run printed the new bounded-lifecycle markers and completed the whole S5
scenario (unselected Profile, explicit UI create, Busy, grant enable, exact
suggestion/confirm/reject, lost-reply same-request replay, Conflict, Escape/project
invalidation, saved exact-task Handoff `NOT_RUN` history, disable acknowledgement,
revoked-grant `Unauthorized`, five pages plus Sessions, theme/locale/resize,
disconnect, clean child exit):

```text
OWNED_WINDOW_READY nonce=a43a2dcfcee8204d pid=3590265 foreground protection enabled
DESKTOP_WORKER_CLEANUP reaped=true reader_joined=true
OWNED_DRIVER_CLOSED gui_pid=3590265 worker_reaped=true reader_joined=true
status=PASS worktree_dirty=false rows=3 historical_source_substitution=false
```

Measured binaries for that run: `acp-native-acceptance`
`d1603f9379717195107aab39db3260117403c58100dcce67d1d84ea8c5871ff3`, `acp-daemon`
`f8b25ad9660fae38af4fa12eda45a03742c905ec9d303a36ab0f9018ed7aadde`, `acp-query`
`46e39f07704aa3ba0c0feb43b2e2ecf4772f8b9a4ad11f92ab712aaf8c27ca64`, `acp-nsh-native`
`71ba7a93d6523b81fffd16c06313706ab6508ba4f9acd99b12c9fdd5eddc4914`. The Windows-built
fake worker measured `2c72950034c3c9480513e483e1329e3210245bfebdda1598d3f1c4cf95156bf5`.
Raw logs, screenshots and per-run window captures stay in the ignored artifact tree.

No timeout actually fired during the real-window run: the bounded path is exercised by
the injected fake workers below, and the real run demonstrates that the bounding does
not regress legitimate slow work (PowerShell cold start, `Add-Type` compilation,
layout switching and `PrintWindow`).

## 5. Negative and positive worker injections

`tools/native-acceptance/tests/desktop_worker.rs` drives the owned
`acp-fake-desktop-worker` binary with no window, no PowerShell and no product process,
so it runs identically on either host. Every case asserts a bounded outcome and
`Cleanup { reaped: true, reader_joined: true }`.

| Injected worker behaviour | Required outcome | Proven |
| --- | --- | --- |
| healthy single reply | each action settles once | yes |
| never replies | deadline refusal, session unusable afterwards, worker reaped, reader joined | yes |
| exits without a reply | `worker eof` refusal | yes |
| non-JSON line | `malformed json response bytes=N` refusal, and the assertion proves the raw bytes are not echoed into evidence | yes |
| 200 kB unterminated line | `oversized response line` refusal instead of buffering | yes |
| two replies for one request | the leftover reply is refused as `unsolicited worker response while idle`; it can never acknowledge the next action | yes |
| reply after the deadline | first request times out, the late reply is never adopted | yes |
| `ok` without the action's required field | `init acknowledgement lacks images` | yes |
| reply naming another nonce | `mismatched response nonce` | yes |
| explicit `refused` status | returned as a normal result, session stays usable | yes |
| oversized request line / actionless request | refused locally before touching the pipe, session unaffected | yes |

These prove driver boundedness only. They are not native GUI, OS-input, model or
real-window evidence, and no real window PASS is inferred from them.

## 6. Limitations and honesty

- M4 review/provenance behavior is **not implemented**; the four-surface matrix in
  section 2 of the acceptance matrix is definitionally NOT_RUN at this revision.
- Ten contract choices (D1–D10) need owner acceptance before S1, including the fresh
  v1 DDL replacement and whether `proposal_digest` gains a new refusal name. Nothing
  was self-approved.
- `typed_context` provenance/review types currently have no consumer outside
  `acp-core`; this is reported as a finding, not silently rewired or deleted.
- Native Windows HWND and ConPTY surfaces were not run, so this PR does not recertify
  S5's four-surface PASS. S5 evidence remains scoped to
  `ddc12ac8c17215014e04cc65ed802819b1bc8376`.
- Hosted GitHub Actions are NOT_RUN because every commit carries `[skip ci]`; the
  repository source/link checker was run locally instead. No macOS, device, model,
  clean-machine or hosted-CI result is claimed.
- One local-only change was needed to keep the clean-tree guards honest: `/.qoder/`
  (this CLI's untracked session directory) was added to `.git/info/exclude`. No
  tracked file, ignore list in the repository, or local data directory was modified
  or deleted.
- `local_verify.sh`/`windows_local_verify.ps1` never overwrite an existing report
  directory; each table above names the directory it produced.
