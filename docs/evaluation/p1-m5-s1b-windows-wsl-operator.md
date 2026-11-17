# P1-M5/S1-B — Windows native Operator GUI → WSL2 full backend bridge

2026-11-17. Scope: [Issue #73](https://github.com/ybakiame/agent-control-plane/issues/73), parent
[M5 #67](https://github.com/ybakiame/agent-control-plane/issues/67), after merged
[PR #71](https://github.com/ybakiame/agent-control-plane/pull/71) (`e4be2657c9417ce00ffc186d2920db0837aa09ae`).
**Windows HWND GUI → WSL2 daemon end-to-end acceptance PASS (15/15 cells) on one
recorded host and one disposable WSL2 user distribution.** This is development
acceptance of the primary workflow, not a release, packaging, OS-support or
real-Profile claim. The secondary WSLg + Weston path is
[S1-C #72](https://github.com/ybakiame/agent-control-plane/issues/72) and was not run here.

## What changed

- **`acp-wsl-bridge`** (new binary in `apps/operator/model`): one long-lived helper per
  WSL connection, started as `wsl.exe --distribution D --user U --exec /abs/acp-wsl-bridge
  --runtime-dir R --endpoint E`. It speaks a versioned, length-framed JSON stdio
  protocol (hello, request IDs, 256 KiB / 5 MiB frame limits, ≤ 8 in flight,
  cancel) and admits only `snapshot`, the existing Operator control allowlist,
  OperatorWrite `mutate` and the six session methods the Sessions UI uses. It
  loads `operator.cap` / `operator-write.cap` inside WSL and calls the same
  `acp-ipc` functions as a local client; it never validates or approves a
  mutation, opens SQLite or links the Writer. Protocol:
  [interfaces](../interfaces.md#windows-gui--wsl2-bridge).
- **Windows client** (`bridge::Client`): reply matching by ID; an unsolicited,
  ambiguous or unknown-field reply ends the bridge; helper errors map to a fixed
  vocabulary (anything else → `Unavailable`); a mutation lost or timed out after
  delivery is `OutcomeUnknown`; no automatic retry. The one-shot `wsl.exe`
  snapshot launch is replaced by the same bridge.
- **Start rules:** only explicit user actions (connect, Reconnect, navigation,
  Operator actions) may start a helper. Session polling (750 ms) never does, so a
  stopped distribution or lost helper stays an explicit unavailable state.
- **Model/UI:** the Operator guard and the Operator/Sessions UI gates admit a valid
  WSL target with the same epoch/connection/source fences as local. A retry of the
  pending uncertain mutation that gets no daemon decision (`Unavailable`) now keeps
  the uncertainty and the exact request (local and WSL). UI text no longer calls
  Windows→WSL read-only.
- **Docs:** architecture, interfaces, README, readiness plan (C5/C6, G19, D1) and
  roadmap distinguish the primary Windows→WSL2 bridge from the secondary WSLg path.

## Source, host and target

| Item | Exact identity |
| --- | --- |
| Clean source (all binaries and the acceptance run) | `87ef0c122c26dfa7a26c2ac4fdd38f56f1cdcf66`, tree `cbaba6ad68ec2aacd20129f3a3b2a8a28043336f` |
| Windows host | Windows 11 Pro for Workstations build **10.0.26300**; MSVC 14.51.36231 / SDK 10.0.26100.0 |
| WSL | WSL 3.0.5.0, kernel 6.18.58.1-1 (`6.18.58.1-microsoft-standard-WSL2`), WSLg 1.0.79 (not used) |
| User distribution | Disposable **`acp-s1b-ubuntu`**: Ubuntu **24.04.5 LTS** (official image via `wsl --install Ubuntu-24.04 --name acp-s1b-ubuntu --no-launch`), glibc 2.39; not the WSLg system distro; the daily `archlinux` distro was not used for evidence |
| Linux user | `acp` (uid 1000), non-root; binaries in private `/home/acp/acp-s1b/bin` (0700) |
| Toolchain | Stock Rust **1.99.1** on both sides (Windows: rustup toolchain path; Ubuntu: rustup minimal profile); debug builds |

| Binary | SHA-256 |
| --- | --- |
| `acp-nsh-native.exe` (Windows GUI) | `165b5659e5d99117f63fc423c07d2033c2f60c761283e53afee1db36fc169516` |
| `acp-native-acceptance.exe` (harness) | `c76ca93bfd5f2e1303239823d48775b316aa0bd2b352fd123428d3a92e065990` |
| `acp-wsl-bridge` (Linux) | `c49e98a25c5496746fa89c6abc0b4ed7f7d4d3943c5b44abf3443b19fe273d0e` |
| `acp-daemon` (Linux) | `ede76ce0d632d1812a17e228a140d3211e1123ccd38bcdfa1eeaa232dfcd940c` |
| `acp-query` (Linux) | `28d6d7c4b7e90585d213a1c224bb75d0fcb0a27d48adf3b4f146b5eeef64676a` |
| `acp-fake-agent` (Linux, deterministic test fixture) | `d0225d3632ef4f8b2d9953253fb891cc1acc1e7aedcae0096be7e8eecb255308` |

Test setup inside the disposable distro only: `useradd acp`, `apt-get install
build-essential pkg-config curl ca-certificates git`, rustup 1.99.1, and a
`safe.directory` entry for the Windows-mounted checkout. Nothing was installed on
Windows other than the distro itself; `archlinux` and WSLg were not modified.

Actor: automated. The harness drives the real HWND window with OS keyboard input
through the existing owned-window worker, reads the GUI's own probe lines, and
observes WSL independently with `acp-query`/`pgrep`/`ss`/`ps` via `wsl.exe --exec`
(no shell). Screenshots were inspected; this is not independent human, IME or
assistive-technology acceptance. Private run: ignored
`artifacts/local-p1-m5/s1b-attempt-01/` (`report.json` `01d7d045…d197b`,
`ledger.json` `f38d3b6d…f7cc`). Synthetic Profile and project only.

## Operation coverage (Windows HWND GUI → WSL2 daemon)

`acp-native-acceptance wsl-desktop`, non-development mode, clean tree before and after:
**15 PASS / 0 FAIL / 0 NOT_RUN**.

| Issue goal | Cell(s) | Observed |
| --- | --- | --- |
| 1 Dashboard, project context, Handoff preview | `wsl-dashboard-context-handoff` | Ready with `alpha`; exact saved Handoff revision 1 previewed |
| 2 Profile create/select, write grant | `wsl-connect-no-implicit-state`, `wsl-profile-create-without-write`, `wsl-write-grant-enable`, `wsl-write-grant-disable`, `wsl-daemon-absent-and-explicit-recovery` | No implicit Profile/grant; create ≠ write; explicit enable/disable; after daemon restart the Profile is **not** adopted and is re-selected explicitly |
| 3 Review: suggest, queue/inspect/history, confirm, reject, deprecate, delete | `wsl-suggest-inspect-confirm`, `wsl-reject`, `wsl-deprecate`, `wsl-delete-exact-retype` | Each applied with an exact receipt; WSL CLI confirms status; a wrong retype (`9`) writes nothing; only the exact ID deletes |
| 4 Receipts, OutcomeUnknown, identical retry | `wsl-bridge-loss-outcome-unknown-identical-retry` | Helper frozen (SIGSTOP) then killed after submit → GUI `Outcome unknown`; 2 s later no new row and **no helper restarted**; explicit Retry restarts the bridge and settles with exactly **one** new row |
| 5 Sessions | `wsl-session-lifecycle-agent-in-wsl` | Configured program discovered; confirmed Start; agent process observed **inside WSL**; prompt → permission → allow once; second prompt → deny; stop; agent gone |
| 6 CLI-only workflows | (setup) | `project-register` and `handoff-put` run via WSL `acp-query`; no UI added |
| Context sharing | — | **Not exposed by the existing native GUI** (CLI `session-start --share-context` only); no UI invented. The bridge forwards session fields unchanged and the daemon remains authoritative |

## Transport, capability and lifecycle checks

| Check | Result | Evidence |
| --- | --- | --- |
| Persistent bridge, no per-request WSL process | PASS | One helper for the whole connection; Reconnect reuses it; ~10 session polls on the same helper PID; Windows `wsl.exe` count 6 → 6 |
| No TCP/UDP listener | PASS | `ss -l -t -u -p` shows none owned by `acp-wsl-bridge`, `acp-daemon` or (while alive) `acp-fake-agent` |
| No shell | PASS | Helper parent is WSL's `Relay(513)`; helper argv exactly `exe --runtime-dir R --endpoint E` |
| No capability token across stdio | PASS | Linux real-process test scans every raw helper frame for each capability token (JSON-array and hex forms): none |
| Malformed / unknown / versioned / oversized frames, id 0, allowlist | PASS | Raw-frame tests: refused with fixed codes; oversized frame ends the helper (exit 2); stream continues after recoverable refusals |
| Wrong hello, unsolicited/mismatched IDs, ambiguous replies | PASS | Client refuses connect or ends the bridge; tested with a misbehaving fake helper whose stdout stays open |
| Timeout and cancellation | PASS | 1 µs timeout → `Timeout`; late reply ignored; bridge stays healthy; identical retry settles |
| Child exit / bridge loss after submit | PASS | Linux: SIGKILL during a mutation → `Lost` (never a fabricated failure); reconnect + identical operation ID settles; changed payload → `IdempotencyConflict`. Windows GUI: as in goal 4 |
| Daemon absent, distro/user/runtime wrong, symlinked runtime | PASS | GUI `phase=Unavailable` for wrong distribution, wrong Linux user, wrong runtime; daemon stop → Unavailable; symlinked runtime → `Permission denied` (Linux test); relative paths refused before the protocol |
| Authorization negatives | PASS | Missing/revoked OperatorWrite → `Unauthorized`; stale generation → `Unauthorized`; wrong project/scope refused; unconfigured or unconfirmed session executable refused; double start refused; permission replay refused; cancel without a turn refused |
| GUI close / reconnect | PASS | Closing the GUI ends the helper (stdin EOF); the WSL daemon PID, health and Profile digest are unchanged |
| No direct SQLite/Writer from GUI/helper | PASS | `cargo tree --features native --edges normal` for the model crate: only `acp-core`, `acp-ipc`, `acp-session`; no `acp-store`, `acp-write` or `rusqlite` |

Timing on this host with the distribution already running: GUI launch → first
Operator state including bridge start **585 ms**; Reconnect → Ready **110 ms**;
suggest prepare/apply/receipt **224 ms**; retry including bridge restart **140 ms**;
session start **613 ms**. Linux in-distro bridge round trip for `profile.info`
≈ 2.9 ms (local IPC connect included). Cold distribution boot latency was **NOT_RUN**.

## Validation commands

| Check | Platform | Result |
| --- | --- | --- |
| Model fmt / Clippy `-D warnings` / tests (`--features native`) | Linux (archlinux), Ubuntu 24.04 target distro, Windows | PASS; 57 unit + 6 bridge real-process tests (process tests are Unix-only; Windows covered by the GUI run) |
| Operator UI fmt / Clippy / tests (`--features native,tui`, CI configuration) | Linux | PASS, 27 unit + 2 real-daemon integration |
| Root workspace fmt / Clippy / tests | Linux | PASS, 153 passed, 2 ignored (unchanged by this PR) |
| Harness fmt / Clippy / tests incl. env isolation | Linux | PASS, 36 |
| Windows GUI + harness build | Windows | PASS |
| **Windows HWND → WSL2 acceptance** `s1b-attempt-01` | Windows + `acp-s1b-ubuntu` | **PASS 15/15** |

Correction to earlier reports: the four `operator::tests` UI failures recorded in
S1-A occur only when the UI crate is tested **without** its `native,tui` features (a
plain `cargo test --workspace`). In the CI configuration they pass (27/27).

Development runs `s1b-dev-01..04` are retained locally and not counted. They found
and fixed: the ledger catalog for the new mode; the window worker's fixed key
vocabulary lacking the session chords; a 17-character relaunch nonce; and — before
the GUI run — a model gap where a lost bridge could leave an `OutcomeUnknown`
mutation without any identical-retry path.

## Not run / limitations

- One Windows host and one disposable distribution; no distro or Windows matrix.
  Debug builds; shipping the Linux helper and a package is S1-C.
- The 148-cell native UI matrix was intentionally not rerun (issue scope).
- Windows ConPTY TUI → WSL target uses the same model paths but was not exercised.
- Human IME/AT, second Windows user, cold-boot latency: NOT_RUN.
- Follow-up wording: for a WSL target the unavailable message still says "check the
  local daemon". `acp-ui-snapshot` remains in the model crate but the GUI no longer
  uses it for WSL.
- WSLg + Weston Linux GUI/TUI is S1-C #72. Parent M5 stays open; no release,
  public distribution or real Profile use is authorized.
