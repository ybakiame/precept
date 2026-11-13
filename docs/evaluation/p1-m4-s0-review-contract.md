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
| `b140af5`, `942a60e`, `f552ee1` | Evidence-report documentation only |
| `015f818` | `.gitattributes` repository-wide LF policy, owner-directed (section 1.1) |
| `2a5746e`, `729cb09` | Evidence-report documentation only |
| `31b0a6b` | PR #54 blocking fixes: `rpc_id` on the wire and validated per reply, cancelable bounded write, both IO threads joined, driver emissions correlated, 18 harness tests |
| `3fa77e8` | Matrix and contract: TUI review parity restored, delivered bounded-RPC properties, owner decision D11 |
| `d39b2fd` | Windows Clippy: drop the redundant `AsRawHandle` import |
| `2d2f021`, `f200137` | Windows reply pipe polled with `PeekNamedPipe` instead of `PIPE_NOWAIT` |
| `d3a4a3d` | Harness README and matrix wording for the per-platform polling strategy |
| `f59e3ed` | Evidence-report documentation only (first review rework) |
| `8f36266` | Fail-closed bounded-IO precondition (`SpawnError::Unfit`/`Launch`), GUI owned from spawn (`OwnedProcess`), 21+4 harness tests |
| `11ffef6` | Flush trailing partial probe line at stderr EOF |
| `8d03548` | Matrix section 5 and README: record the precondition and ownership properties |
| `963c308` | Windows `ERROR_BROKEN_PIPE` type correction (`u32`→`i32`) and unused import removal |

**Final tested code SHA: `963c308fb2fc69d4dcee6325890a49624a66c296`.** Every gate and
native run in this report is executed at that tip or on a tree proved equal to it.
`git diff --name-only d0c6cfc 963c308` contains only `.gitattributes`, `docs/`,
`tools/native_window.ps1` and `tools/native-acceptance/`, so no product input changed at
any point in this PR. Earlier rows in this table record what each intermediate revision
measured rather than being re-presented as final evidence.

## 1. Changed surface

`git diff --name-only 18f2670 963c308` returns 17 paths: five under `docs/`, eleven
under `tools/native-acceptance/`, `tools/native_window.ps1` and the repository
`.gitattributes`. No product crate, DDL, writer, daemon, IPC, CLI, MCP,
NSH or shared-model file is touched. The full Linux/WSL **and** native Windows
workspace, shared-model and NSH gate suites were still run at the final tip
(sections 2 and 3, 13/13 each) rather than argued away, and all four native surfaces
were re-executed there (section 4).

The driver change replaces the unbounded
`BufReader<ChildStdout>::read_line` in `tools/native-acceptance/src/desktop.rs` with
`tools/native-acceptance/src/worker.rs`: one request settles with exactly one
correlated, validated reply inside one deadline that covers both the write and the reply
(init 300 s, capture 180 s, other actions 60 s). Each request carries a monotonic
`rpc_id` **on the wire** and every reply must name it, so a stale, duplicated,
unsolicited or wrong-nonce reply fails closed instead of settling a newer action; the
PowerShell worker now echoes `rpc_id` in all six of its emission paths, refusals included.
The stdin write is a cancelable job on its own thread, so a worker that stops consuming
stdin cannot wedge the harness on a full pipe. Teardown stops both IO threads by flag,
terminates and reaps only the owned child, and joins both threads, reporting
`reader_joined` and `writer_joined` rather than assuming them. Foreground readiness keeps
its separate finite 900 s wait. Nonce, exact PID and WSLg `msrdc` ownership, frontmost-only
input, the fixed chord vocabulary, Ctrl+1 exclusion and the already-installed-English-layout
restoration path are unchanged.

Bounded pipe IO is a **precondition**, not a preference. The mode setters in
`src/pipe.rs` return `Result<Mode, String>` whose only success values are `nonblocking`
and `peek-poll`; there is no `blocking` value to continue with. If the mandatory setup
fails, `WorkerRpc::spawn` refuses the session (`SpawnError::Unfit { pid, reaped, detail }`)
after terminating and reaping exactly the child it launched. A program that never started
is a distinct `SpawnError::Launch` cause. The GUI window process is owned from the moment
`Desktop::start` spawns it (`src/process.rs`): `OwnedProcess::adopt` establishes the
bounded stderr reader before any initialization step can fail, and a refused `init`, a
window that never appears, a bounded worker fault, a refused adoption or a panic unwinds
through ownership, which terminates and reaps that child and bounded-joins its log thread.

### 1.1 Repository-wide line endings

The Windows validation copy first reached the tested source by file copy and then by a
`ddc12ac` checkout. Native Git called that tree clean while Linux Git reported 15
modified files, because `core.autocrlf=true` with no attribute file writes a Windows
working tree as CRLF: "same source" then held under Git's normalization rather than
byte-for-byte. At the owner's direction `015f818` added `.gitattributes`
(`* text=auto eol=lf`, explicit LF for `*.sh` and `*.ps1`, `*.png binary`). After
re-extracting the formal Windows checkout at `C:\Users\ybakiame\code-workspaces\agent-control-plane`,
`git status --porcelain` is empty when read by **either** host's Git, no text file
contains a carriage return, and spot-checked SHA-256 values equal the Linux tree. Only
repository content and that validation copy's working tree changed; no global or system
Git setting was touched. This supersedes the per-file manifest-hash workaround that
sections 3 needed before the attribute file existed.

## 2. Linux/WSL verification

Host: Linux 6.18.54.1-microsoft-standard-WSL2, x86_64, WSLg `wayland-0`.
Toolchain `rustc`/`cargo` 1.99.1 from `rust-toolchain.toml`.

`tools/local_verify.sh all <repo>/artifacts/local-p1-m2/s0-linux-963c308 18f2670d…`
ran in the working copy at the final tip `963c308`: this CLI's untracked session
directory is covered by `.git/info/exclude`, so the script's own clean-tree guard
applies without weakening it. The script refuses a dirty tree, demands a 40-character
ancestor baseline and executes `git diff --check 18f2670… HEAD` as its first and last
gates.

| Gate | Result |
| --- | --- |
| branch-diff `git diff --check 18f2670… 963c308` | PASS |
| workspace-fmt / workspace-build / workspace-clippy `-D warnings` | PASS |
| workspace-tests | PASS: 116 tests, 0 failures |
| shared-fmt / shared-clippy `-D warnings` | PASS |
| shared-tests | PASS: 32 tests, 0 failures |
| nsh-fmt / nsh-build / nsh-clippy `-D warnings` | PASS |
| nsh-tests | PASS: 7 tests, 0 failures |
| final `git diff --check` | PASS |

All 13 recorded gates PASS (`artifacts/local-p1-m2/s0-linux-963c308/result.tsv`, whose
`source.txt` records `963c308fb2fc69d4dcee6325890a49624a66c296`, the diff base,
`Linux 6.18.54.1-microsoft-standard-WSL2 x86_64`, rustc and cargo 1.99.1). The
product-side counts equal merged S5's Linux counts, which is the expected result for a
byte-identical product tree.

Earlier runs are retained as intermediates rather than presented as final evidence:
13/13 PASS at `8d03548` (`artifacts/local-p1-m2/s0-linux-final/`), 13/13 PASS at
`8f36266` (`artifacts/local-p1-m2/s0-linux-review2/`), 13/13 PASS at `d3a4a3d`
(`artifacts/local-p1-m2/s0-linux-reviewfix/`), and earlier tips in their respective
directories.

Harness manifest gates at the final SHA in the working copy:

| Command | Result |
| --- | --- |
| `cargo fmt --manifest-path tools/native-acceptance/Cargo.toml -- --check` | PASS |
| `cargo clippy --manifest-path tools/native-acceptance/Cargo.toml --all-targets --locked -- -D warnings` | PASS |
| `cargo test --manifest-path tools/native-acceptance/Cargo.toml --locked` | PASS: 21 `desktop_worker` + 4 `owned_process` + 1 `screen` test, 0 failures |
| `node tools/check_source.mjs` | PASS: 217 active/historical tracked paths, zero tracked prototype executables or packaging, 33 marked archives, all active refs, local Markdown links and workflow targets valid |

## 3. Native Windows verification

Host: Windows 10.0.26300.0, x86_64; cargo and rustc 1.99.1; global Git
2.56.0.windows.4. The formal checkout is
`C:\Users\ybakiame\code-workspaces\agent-control-plane`, synced by offline incremental
`git bundle` (no network, no credentials, no interactive auth) and fast-forward merges
only; both hosts' `git status --porcelain` are empty on the same bytes at every measured
tip.

Only a process-local environment is used: `C:\Program Files\Git\cmd` plus
`%USERPROFILE%\.cargo\bin` on `PATH`, and `vswhere`/`Enter-VsDevShell` for MSVC.
Nothing is installed and no persistent system or global Git setting is modified. Every
helper script, bundle, manifest and log lives under the ignored
`artifacts/local-p1-m2/s0-windows-scratch/` directory, not beside the checkout.

| Gate | Result |
| --- | --- |
| `tools/windows_local_verify.ps1 -Suite All -DiffBase 18f2670…` at `8d03548` | **PASS 13/13**: branch-diff, workspace fmt/build/clippy/tests, shared fmt/clippy/tests, nsh fmt/build/clippy/tests, final `git diff --check` (`…\s0-windows-reviewfix-20261113-234230\report.json`, `worktree_dirty=false`) |
| Windows test counts | PASS: workspace 117, shared-model 32, NSH 7 tests, 0 failures |
| harness `cargo fmt --check` at `963c308` | PASS |
| harness `cargo clippy --all-targets --locked -- -D warnings` at `963c308` | PASS |
| harness `cargo build` / `cargo test --locked` at `963c308` | PASS: 21 `desktop_worker` + 4 `owned_process` + 1 `screen` test, 0 failures (`…\s0-windows-harness-final-20261113-234615\`) |

The full product suite ran at `8d03548`; `git diff --name-only 8d03548 963c308` is
exactly `tools/native-acceptance/src/pipe.rs` (a Windows-only `#[cfg(windows)]` type
correction), so the product tree is byte-identical at both tips. The harness gates and
native surfaces ran at `963c308` itself.

An earlier full-suite run at `d3a4a3d` under **Windows PowerShell 5.1** aborted inside
`workspace-build`: with `$ErrorActionPreference='Stop'`, 5.1 turns cargo's ordinary stderr
progress into a terminating `NativeCommandError`, which is a host-shell behavior of the
invocation and not a gate failure. All recorded PASS results above are under the installed
PowerShell 7, which does not do that. This is noted rather than silently worked around,
because the runner shell is part of reproducibility.

Windows findings retained from earlier rounds: Clippy at `ba51c4c` **FAILED** with
`large_enum_variant` (producing `d0c6cfc`); the reworked harness at `3fa77e8` **FAILED
1 of 18 tests on Windows only** because `PIPE_NOWAIT` is not honored for the read end of
an anonymous pipe (producing `2d2f021`/`f200137`); Clippy denied one dead `AsRawHandle`
import at `d39b2fd`; and at `8d03548` Clippy denied `ERROR_BROKEN_PIPE: u32` against
`raw_os_error() -> Option<i32>` plus a redundant import (producing `963c308`). Each was
a Windows-only lint or type error that Linux could not surface.

## 4. Native desktop and PTY observations

Owned synthetic root, private runtime, dedicated IPC endpoint and a freshly created
disposable Profile only. No real ACP database, external development repository, model,
provider, credential or authentication directory was touched.

| Surface | SHA | Result | Evidence |
| --- | --- | --- | --- |
| Native Windows HWND desktop (real OS input, real daemon/CLI/NSH) | `963c308` | **PASS** | `…\agent-control-plane\artifacts\local-p1-m2\s0-windows-desktop-final-20261113-234615\report.json` |
| Windows ConPTY | `963c308` | **PASS** | `…\agent-control-plane\artifacts\local-p1-m2\s0-windows-tui-final-20261113-234615\report.json` |
| WSLg native Wayland desktop | `963c308` | **PASS** | `artifacts/local-p1-m2/s0-final2-wayland/report.json` |
| Linux/WSL PTY | `963c308` | **PASS** | `artifacts/local-p1-m2/s0-final2-pty/report.json` |
| niri, mobile, X11/XWayland/Xvfb, headless, Web | — | out of scope, and never a substitute for native acceptance | unchanged S5 boundary |

Both desktop surfaces printed the bounded-lifecycle and ownership markers and completed
the whole S5 scenario (unselected Profile, explicit UI create, Busy, grant enable, exact
suggestion/confirm/reject, lost-reply same-request replay, Conflict, Escape/project
invalidation, saved exact-task Handoff `NOT_RUN` history, disable acknowledgement,
revoked-grant `Unauthorized`, five pages plus Sessions, theme/locale/resize,
disconnect, clean child exit), each with 15 window captures. Native Windows HWND:

```text
DESKTOP_IO_MODE stdin=nonblocking stdout=peek-poll gui_stderr=peek-poll
OWNED_WINDOW_READY nonce=a43a2e1940c70f58 pid=43196 foreground protection enabled
OWNED_PROCESS_CLEANUP name=gui pid=43196 reaped=true reader_joined=true
OWNED_DRIVER_CLOSED gui_pid=43196 gui_reaped=true gui_reader_joined=true worker_reaped=true reader_joined=true writer_joined=true
```

WSLg native Wayland:

```text
DESKTOP_IO_MODE stdin=nonblocking stdout=nonblocking gui_stderr=nonblocking
OWNED_WINDOW_READY nonce=a43a2d1b677c6083 pid=3639519 foreground protection enabled
OWNED_PROCESS_CLEANUP name=gui pid=3639519 reaped=true reader_joined=true
OWNED_DRIVER_CLOSED gui_pid=3639519 gui_reaped=true gui_reader_joined=true worker_reaped=true reader_joined=true writer_joined=true
```

No `DESKTOP_RPC_FATAL`, no `OWNED_PROCESS_TEARDOWN_INCOMPLETE` and no
`DESKTOP_TEARDOWN_INCOMPLETE` line appeared in either real window run.

Measured binaries at `963c308` — Windows: `acp-native-acceptance.exe`
`d15a0eeeb37b2bcb380d737c372d78c54a595b543bf607b8776059b1189c9369`,
`acp-daemon.exe` `657e17d6f86226ee242944a902f6dea36d4ce044b78f4aa622524e0c67c743b4`,
`acp-query.exe` `df0e8db4afe1d45974b91b05dff8f586363315c1dffa107ffaf9b651beb8f5f4`,
`acp-nsh-native.exe` `be9f6315a0d43f1fdb9c91d96d0aaec377e5aa424c127c9fe07b58784b7c0192`,
`acp-nsh-tui.exe` `3133df57dcba7ab921f0d6d06edbda33edceab78ae86a40e00d9966d2e6e873b`.
Linux/WSL: `acp-native-acceptance`
`874a3f1ec46bbd67253366bad067f017c32335df87c24a7a3ade1ea493966124`,
`acp-daemon` `eb20a194f0baa711f9ae0edd4ff1fce6111efd67fb1f16005c1b2066a0923a90`,
`acp-query` `46e39f07704aa3ba0c0feb43b2e2ecf4772f8b9a4ad11f92ab712aaf8c27ca64`,
`acp-nsh-native` `65c475bad537cf183cdaddcbb6d957706d659dab4e5d80f7eb14c4a06306fcd0`,
`acp-nsh-tui` `b2f23023e47d6acb742a08ec85be8932cf4e1313b9c5d2b7fff4afdf400c2af1`.
The owned fake worker measured
`d143b379748ece85d280094013bf400094cd586fecbc06d7e4bae466b74d9201` on Windows and
`3a2d3a95c07faf61c1637a6c11ce2e13934a89d3a594629b18e82c3ab66aa6f0` on Linux/WSL.
Raw logs, screenshots and per-run window captures stay in the ignored artifact tree.

No per-call deadline elapsed during either real-window run, on either host: the runs
finish the PowerShell cold start, `Add-Type` compilation, input-layout switching and
`PrintWindow` work without a single `DESKTOP_RPC_FATAL` line, so bounding each call does
not refuse legitimate slow work. The individual call durations were not instrumented;
boundedness itself is proven by section 5, not by these runs.

## 5. Negative and positive worker injections

`tools/native-acceptance/tests/desktop_worker.rs` drives the owned
`acp-fake-desktop-worker` binary with no window, no PowerShell and no product process,
so it runs identically on either host. Every case asserts a bounded outcome and
`Cleanup { reaped: true, reader_joined: true, writer_joined: true }` completed inside
10 s, and every failure assertion checks the reported cause, so an unjoined thread or an
unreaped child cannot be mistaken for success. 21 cases:

| Injected worker behaviour | Required outcome | Proven |
| --- | --- | --- |
| healthy single reply | each action settles once | yes |
| healthy, repeated calls | each reply names its own request and the ids increase monotonically (1, 2, 3) | yes |
| never replies | `reply deadline exceeded seq=… action=…` refusal, session unusable afterwards, worker reaped, both threads joined | yes |
| exits without a reply | `worker eof` refusal | yes |
| exits abnormally (status 3) | `worker eof` refusal, child reaped, both threads joined | yes |
| non-JSON line | `malformed json response bytes=N` refusal, and the assertion proves the raw bytes are not echoed into evidence | yes |
| 200 kB unterminated line | `oversized response line` refusal instead of buffering | yes |
| two replies for one request | the leftover reply is refused (`correlation mismatch` or `unsolicited worker response while idle`); it can never acknowledge the next action | yes |
| older reply delivered only after the newer request was written | `correlation mismatch observed=… expected=…`, session fails closed — deterministic, no sleep involved | yes |
| reply naming a request nobody sent | `correlation mismatch` refusal | yes |
| reply after the deadline | first request times out, the late reply is never adopted, session stays faulted | yes |
| `ok` without the action's required field | `init acknowledgement lacks images` | yes |
| reply naming another nonce | `mismatched response nonce` | yes |
| explicit `refused` status | returned as a normal result, session stays usable | yes |
| oversized request line / actionless request | refused locally before touching the pipe, session unaffected | yes |
| worker that stops consuming stdin | each write attempt returns inside its own bound, then `write deadline exceeded` or `write refused`; cleanup still joins both threads | yes |
| stdout held open by a grandchild the harness never owned | the call fails on its deadline and the reader is still joined, never detached | yes |
| established session reports only bounded modes | `io_modes()` returns only `nonblocking` or `peek-poll`, never `blocking` | yes |
| mandatory pipe setup fails (injected) | `SpawnError::Unfit { pid, reaped: true, detail }` — session refused, child terminated and reaped | yes |
| program that never started | `SpawnError::Launch` — distinct from a half-built session | yes |

`tools/native-acceptance/tests/owned_process.rs` drives an owned mock GUI (the same
fake-worker binary in `gui-hang` and `eof` modes) with no real window. 4 cases:

| Injected GUI behaviour | Required outcome | Proven |
| --- | --- | --- |
| adopted child hangs, then terminate | `OwnedCleanup { pid, reaped: true, reader_joined: true }` inside 10 s; second terminate returns the cached result | yes |
| child exits cleanly on its own | reaped without being forced down; `exited()` reports success | yes |
| panic during initialization (after adoption) | `catch_unwind` observes the panic; the cleanup sink reports `reaped && reader_joined` for the owned child | yes |
| bounded read mode unavailable (injected) | `Unfit { pid, reaped: true, detail }` — adoption refused, child terminated and reaped | yes |

These prove driver boundedness and ownership only. They are not native GUI, OS-input,
model or real-window evidence, and no real window PASS is inferred from them.

## 6. Limitations and honesty

- M4 review/provenance behavior is **not implemented**; the four-surface matrix in
  section 2 of the acceptance matrix is definitionally NOT_RUN at this revision.
- Eleven contract choices (D1–D10 before S1, D11 before any desktop-only narrowing of
  the review actions) need owner acceptance, including the fresh
  v1 DDL replacement and whether `proposal_digest` gains a new refusal name. Nothing
  was self-approved.
- `typed_context` provenance/review types currently have no consumer outside
  `acp-core`; this is reported as a finding, not silently rewired or deleted.
- All four native surfaces (Windows HWND, WSLg Wayland, ConPTY, Linux/WSL PTY) ran at
  `963c308` and PASS, so the revised driver is validated on both hosts with real OS
  input rather than only by injected fakes. This does **not** recertify S5's product
  acceptance, whose PASS remains scoped to
  `ddc12ac8c17215014e04cc65ed802819b1bc8376`: the product inputs here are unchanged,
  and no new product behavior was tested.
- The driver's boundedness is proven by the injected fake workers, not by the real runs.
  One negative case that only Windows can reach (a reply pipe held by an unowned
  grandchild) failed there first and produced the polling fix in section 3; Linux never
  could surface it because `O_NONBLOCK` is honored on both ends there.
- Hosted GitHub Actions are NOT_RUN because every commit carries `[skip ci]`; the
  repository source/link checker was run locally instead. No macOS, device, model,
  clean-machine or hosted-CI result is claimed.
- Two repository-level accommodations are stated rather than left implicit: `/.qoder/`
  (this CLI's untracked session directory) was added to the local `.git/info/exclude`
  so the gate scripts' clean-tree guards stay strict without moving or deleting local
  data, and the owner-directed `015f818` added the tracked `.gitattributes` LF policy
  (section 1.1). Nothing outside `docs/`, `tools/native-acceptance/`,
  `tools/native_window.ps1` and that attribute file changed, and no user data directory
  was modified or deleted.
- The formal Windows directory is the renamed validation copy. Paths that existed only
  during the earlier passes — `acp-m1-validation-m44gv_kj`, and helper files that
  briefly sat beside the checkout — no longer exist; every Windows script, bundle,
  manifest and log now lives inside the repository's ignored artifact tree.
- `local_verify.sh`/`windows_local_verify.ps1` never overwrite an existing report
  directory; each table above names the directory it produced.