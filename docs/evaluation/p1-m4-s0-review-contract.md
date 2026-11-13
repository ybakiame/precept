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

**Final tested code SHA: `d3a4a3dbb5572a8af18793a9fa02c687e0e9c23c`.** Every gate and
native run in this report is executed at that tip or on a tree proved equal to it;
`015f818` is superseded as the tested tip by the review rework, not by documentation.
`git diff --name-only d0c6cfc d3a4a3d` contains only `.gitattributes`, `docs/`,
`tools/native_window.ps1` and `tools/native-acceptance/`, so no product input changed at
any point in this PR. Earlier rows in this table record what each intermediate revision
measured rather than being re-presented as final evidence.

## 1. Changed surface

`git diff --name-only 18f2670 d3a4a3d` returns 14 paths: four under `docs/`, eight
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
Toolchain `rustc`/`cargo` from `rust-toolchain.toml`.

`tools/local_verify.sh all <repo>/artifacts/local-p1-m2/s0-linux-reviewfix 18f2670d…`
ran in the working copy at the final tip `d3a4a3d`: this CLI's untracked session
directory is now covered by `.git/info/exclude`, so the script's own clean-tree guard
applies without weakening it. The script refuses a dirty tree, demands a 40-character
ancestor baseline and executes `git diff --check 18f2670… HEAD` as its first and last
gates.

| Gate | Result |
| --- | --- |
| branch-diff `git diff --check 18f2670… d3a4a3d` | PASS |
| workspace-fmt / workspace-build / workspace-clippy `-D warnings` | PASS |
| workspace-tests | PASS: 116 tests, 0 failures |
| shared-fmt / shared-clippy `-D warnings` | PASS |
| shared-tests | PASS: 32 tests, 0 failures |
| nsh-fmt / nsh-build / nsh-clippy `-D warnings` | PASS |
| nsh-tests | PASS: 7 tests, 0 failures |
| final `git diff --check` | PASS |

All 13 recorded gates PASS (`artifacts/local-p1-m2/s0-linux-reviewfix/result.tsv`, whose
`source.txt` records `d3a4a3dbb5572a8af18793a9fa02c687e0e9c23c`, the diff base,
`Linux 6.18.54.1-microsoft-standard-WSL2 x86_64`, rustc and cargo 1.99.1). The
product-side counts equal merged S5's Linux counts, which is the expected result for a
byte-identical product tree.

Earlier runs are retained as intermediates rather than presented as final evidence:
13/13 PASS at `015f818` (`artifacts/local-p1-m2/s0-linux-eol/`), 13/13 PASS at `d0c6cfc`
in a separate clean clone (`artifacts/local-p1-m2/s0-linux-final/`, measured before the
session directory was locally excluded) and 13/13 PASS at `ba51c4c`
(`artifacts/local-p1-m2/s0-linux-ba51c4c/`).

Harness manifest gates at the final SHA in the working copy:

| Command | Result |
| --- | --- |
| `cargo fmt --manifest-path tools/native-acceptance/Cargo.toml -- --check` | PASS |
| `cargo clippy --manifest-path tools/native-acceptance/Cargo.toml --all-targets --locked -- -D warnings` | PASS |
| `cargo test --manifest-path tools/native-acceptance/Cargo.toml --locked` | PASS: 18 `desktop_worker` + 1 `screen` test, 0 failures, 0.52 s |
| `node tools/check_source.mjs` | PASS: 214 active/historical tracked paths, zero tracked prototype executables or packaging, 33 marked archives, all active refs, local Markdown links and workflow targets valid |

## 3. Native Windows verification

Host: Windows 10.0.26300.0, x86_64; cargo and rustc 1.99.1; global Git
2.56.0.windows.4. Git became available on this host during this slice, so the earlier
Windows evidence had to work without it: the tracked file set was file-copied into the
owned validation copy `acp-m1-validation-m44gv_kj` and identity was proved by hashing
all 212 tracked paths on both hosts, which normalized to the same SHA-256
`a657895e5b8e0c3a4eb313ea28679243ca8b14c74fd3fae8c94bf9c5c01d4ad2`. That workaround is
now history. The copy was renamed to the formal
`C:\Users\ybakiame\code-workspaces\agent-control-plane`, synced by an offline
incremental `git bundle` (no network, no credentials, no interactive auth), and after
`015f818` **both** hosts' `git status --porcelain` are empty on the same bytes.

Only a process-local environment is used: `C:\Program Files\Git\cmd` plus
`%USERPROFILE%\.cargo\bin` on `PATH`, and `vswhere`/`Enter-VsDevShell` for MSVC.
Nothing is installed and no persistent system or global Git setting is modified. Every
helper script, bundle, manifest and log lives under the ignored
`artifacts/local-p1-m2/s0-windows-scratch/` directory, not beside the checkout.

| Gate at `d3a4a3d` | Result |
| --- | --- |
| `tools/windows_local_verify.ps1 -Suite All -DiffBase 18f2670…` | **PASS 13/13**: branch-diff, workspace fmt/build/clippy/tests, shared fmt/clippy/tests, nsh fmt/build/clippy/tests, final `git diff --check` (`…\artifacts\local-p1-m2\s0-windows-reviewfix-20261113-225941\report.json`, `worktree_dirty=false`) |
| Windows test counts | PASS: workspace 117, shared-model 32, NSH 7 tests, 0 failures |
| harness `cargo fmt --check` | PASS |
| harness `cargo clippy --all-targets --locked -- -D warnings` | PASS |
| harness `cargo build` / `cargo test --locked` | PASS: 18 `desktop_worker` + 1 `screen` test, 0 failures, 0.36 s (`…\s0-windows-harness-reviewfix-20261113-225941\`) |

The Windows copy is synced to the same commit as the Linux tree by offline incremental
`git bundle` fetches (no network, no credentials, no interactive auth) and fast-forward
merges only; both hosts' `git status --porcelain` are empty on the same bytes at every
measured tip.

An intermediate full-suite run at `f552ee1` also recorded 13/13 PASS (in the Windows
checkout's own ignored tree,
`C:\Users\ybakiame\code-workspaces\agent-control-plane\artifacts\local-p1-m2\s0-windows-final\report.json`)
but measured a CRLF working tree, so it is retained as history and superseded by the
table above. A first attempt at `d3a4a3d` under **Windows PowerShell 5.1** aborted inside
`workspace-build`: with `$ErrorActionPreference='Stop'`, 5.1 turns cargo's ordinary stderr
progress into a terminating `NativeCommandError`, which is a host-shell behavior of the
invocation and not a gate failure. The recorded 13/13 above is the same script re-run
under the installed PowerShell 7, which does not do that. This is noted rather than
silently worked around, because the runner shell is part of reproducibility.

Three Windows findings are recorded rather than smoothed over. Clippy at `ba51c4c`
**FAILED** with `large_enum_variant` (360-byte desktop variant against a 72-byte TTY
variant, `src/main.rs:191`), which produced the `d0c6cfc` boxing correction: the lint
only measures that layout on the host that owns it. The reworked harness at `3fa77e8`
then **FAILED 1 of 18 tests on Windows only** —
`held_stdout_from_an_unowned_child_still_joins_the_reader` reported an unjoined reader
after the full 10 s join bound, because Windows does not honor `PIPE_NOWAIT` for the read
end of an anonymous pipe, so a grandchild inheriting the write end left the reader parked
in a blocking `read` that no stop flag could interrupt. Linux could not surface that:
`O_NONBLOCK` is honored there. `2d2f021` replaced trust in `PIPE_NOWAIT` with a
`PeekNamedPipe` availability poll on the read side only, `f200137` corrected its buffer
pointer type, and the 18/18 PASS in the table above is the result at `d3a4a3d` on both
hosts; `DESKTOP_IO_MODE stdin=nonblocking stdout=peek-poll` in section 4 is the Windows
run confirming which strategy it actually got. Clippy additionally denied one dead
`AsRawHandle` import at `d39b2fd`. The harness-gate PASS recorded
at `d0c6cfc` on the file-copied tree (retained in
`artifacts/local-p1-m2/s0-windows-scratch/s0-windows-harness-gates/`) was only
provisional Windows evidence until a real checkout was possible. It is retained in
`C:\Users\ybakiame\code-workspaces\agent-control-plane\artifacts\local-p1-m2\s0-windows-scratch\s0-windows-harness-gates\`,
and a copy is kept in this repository's ignored `artifacts/local-p1-m2/s0-windows-harness-gates/`.

## 4. Native desktop and PTY observations

Owned synthetic root, private runtime, dedicated IPC endpoint and a freshly created
disposable Profile only. No real ACP database, external development repository, model,
provider, credential or authentication directory was touched.

| Surface | SHA | Result | Evidence |
| --- | --- | --- | --- |
| Native Windows HWND desktop (real OS input, real daemon/CLI/NSH) | `d3a4a3d` | **PASS** | `…\agent-control-plane\artifacts\local-p1-m2\s0-windows-desktop-reviewfix-20261113-225941\report.json` |
| Windows ConPTY | `d3a4a3d` | **PASS** | `…\agent-control-plane\artifacts\local-p1-m2\s0-windows-tui-reviewfix-20261113-225941\report.json` |
| WSLg native Wayland desktop | `d3a4a3d` | **PASS** | `artifacts/local-p1-m2/s0-wslg-desktop-reviewfix-225614/report.json` |
| Linux/WSL PTY | `d3a4a3d` | **PASS** | `artifacts/local-p1-m2/s0-linux-pty-reviewfix-225648/report.json` |
| niri, mobile, X11/XWayland/Xvfb, headless, Web | — | out of scope, and never a substitute for native acceptance | unchanged S5 boundary |

Earlier revisions of these surfaces (`015f818` for all four, plus the `d0c6cfc`,
`ba51c4c` and `d3a4a3d`-era Linux-side runs) are retained in the
`artifacts/local-p1-m2/s0-wslg-desktop-*`, `s0-linux-pty-*` and the Windows
`s0-windows-desktop-*`/`s0-windows-tui-*` directories as intermediates, not presented as
final evidence.

Both desktop surfaces printed the bounded-lifecycle markers and completed the whole
S5 scenario (unselected Profile, explicit UI create, Busy, grant enable, exact
suggestion/confirm/reject, lost-reply same-request replay, Conflict, Escape/project
invalidation, saved exact-task Handoff `NOT_RUN` history, disable acknowledgement,
revoked-grant `Unauthorized`, five pages plus Sessions, theme/locale/resize,
disconnect, clean child exit), each with 15 window captures. Native Windows HWND, from
`…\s0-windows-harness-reviewfix-20261113-225941\native-desktop.log`:

```text
DESKTOP_IO_MODE stdin=nonblocking stdout=peek-poll
OWNED_WINDOW_READY nonce=a43a2d161ff5fc3e pid=29952 foreground protection enabled
DESKTOP_WORKER_CLEANUP reaped=true reader_joined=true writer_joined=true
OWNED_DRIVER_CLOSED gui_pid=29952 worker_reaped=true reader_joined=true writer_joined=true
```

WSLg native Wayland:

```text
DESKTOP_IO_MODE stdin=nonblocking stdout=nonblocking
OWNED_WINDOW_READY nonce=a43a2e10f16dc745 pid=3620805 foreground protection enabled
DESKTOP_WORKER_CLEANUP reaped=true reader_joined=true writer_joined=true
OWNED_DRIVER_CLOSED gui_pid=3620805 worker_reaped=true reader_joined=true writer_joined=true
```

No `DESKTOP_RPC_FATAL` and no `DESKTOP_TEARDOWN_INCOMPLETE` line appeared in either real
window run.

Measured binaries at `d3a4a3d` — Windows: `acp-native-acceptance.exe` `a4af1e791a73…`,
`acp-daemon.exe` `657e17d6f862…`, `acp-query.exe` `df0e8db4afe1…`, `acp-nsh-native.exe`
`6ef08ee3b8b9…`, `acp-nsh-tui.exe` `9963f7f05b2f…`. Linux/WSL:
`acp-native-acceptance` `d923896ede4b…`, `acp-daemon` `eb20a194f0ba…`, `acp-query`
`46e39f07704a…`, `acp-nsh-native` `65c475bad537…`, `acp-nsh-tui` `b2f23023e47d…`. The
full 64-hex digests are inside each run's `report.json`; the owned fake worker measured
`60d83a1f93fa2e820117c0a25b5d5ffd00881aaeb3344dccc792e5b1caa43e31` as built on Windows
and `c959d6133ef809ee96cd62529304b919d5048ffc42e798c8487b7b86265aaa05` as built on
Linux/WSL. Raw logs, screenshots and per-run window captures stay in the ignored artifact
tree.

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
unreaped child cannot be mistaken for success. 18 cases:

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

These prove driver boundedness only. They are not native GUI, OS-input, model or
real-window evidence, and no real window PASS is inferred from them.

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
  `d3a4a3d` and PASS, so the revised driver is validated on both hosts with real OS
  input rather than only by injected fakes. This does **not** recertify S5's product
  acceptance, whose
  PASS remains scoped to `ddc12ac8c17215014e04cc65ed802819b1bc8376`: the product inputs
  here are unchanged, and no new product behavior was tested.
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
