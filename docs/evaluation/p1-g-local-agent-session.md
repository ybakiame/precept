# P1-G local agent-session evidence

## Tested source and review status

Local validation completed 2026-11-10–11 (Asia/Shanghai). Base was clean latest main
merge #23, `52c01b7b6c5b48a2884e77bcd8a8893a5a010e62`, no submodules; branch
`feat/p1-g-acp-session-runtime`. G0 contract commit
`8d8244a0657e34e282730d279865eeae9b2015cd` preceded runtime implementation.
The last product change is **`fd58e8fac41df1d069d136f8221b46b5768105c2`**.
The final test/capture correction is **`2105b8835962289ae3ab016275337d9905509706`**;
its successor evidence commit changes documentation only. No hosted Actions ran;
all task commits carry `[skip ci]`. Draft PR only, no automatic merge.

**New Sessions owner review: PENDING.** The actual Windows Sessions window was
opened with a disposable synthetic project and `fixture-normal`,
`fixture-permission`, `fixture-cancel` executables. These are deterministic protocol
fixtures, not real coding agents. The owner was invited to review Start confirmation,
prompt, permission and Stop behavior; automation stopped before that review.
Prior P1-F acceptance (“没有阻塞问题，可作为本阶段基础”) covers the established
five-page/appearance/connection baseline, not approval of these new controls or merge.
ADR-0003 remains Proposed.

## Verified implementation boundary

The [G0 contract](../p1/agent-session-runtime.md) selects external ACP v1, separate
from internal IPC/session API v1. An isolated optional Rust supervisor owns one
native direct child for one allowlisted project. Default execution is disabled.
A private explicit configuration supplies a trusted absolute executable, fixed argv
and narrowly allowed environment; environment inheritance is cleared. No shell,
PATH executable search, download, project-derived command or automatic startup.

The daemon owns child lifetime and ephemeral observations independently of UI
clients. Actual matching prompt responses and valid stop reasons determine turn
completion. Child-alive is separate from session state; cleanup is reported only
when observed. Frame/queue/event/request bounds and deadlines fail closed.
Permissions default to deny after expiry, support only a specific explicit once
choice, and are revoked on cancel/stop. Rendered pointer callbacks and keyboard
review are bound to project, target, epoch and random session generation; an old
callback cannot act on a replacement session reusing permission IDs.

Every new IPC method requires authenticated operator authority, explicit session API
version and project scope. Agent credentials are rejected before parsing/disclosing
session/config state. Existing MCP exposes exactly `get_effective_context` and
`get_handoff`. Windows→WSL remains a read-only owner helper and rejects these new
execution controls. Python profile writes, production SQLite schema, core/MCP source
and the Rust fingerprint byte contract are unchanged.

No OS sandbox, independent descendant containment, hard-kill cleanup guarantee or
persistent vendor resume is claimed. The trusted child has local user privileges
and may access files/network independently; ACP permission mediation is not an OS
policy firewall. Reopen attaches to the same live daemon's memory only. No transcript,
credential or personal profile is persisted by this slice.

## Local matrix and precise reuse

One full baseline ran per host; only affected gates were repeated. This table is
composed evidence, **not a claim that the original failed runs passed**. Original
reports/logs remain intact beside supplements with per-gate source provenance.

- Windows full matrix: clean `9e17478b5ca032d9791a63cf603823ac7c0affdb`.
  After permission-callback and TUI resize fixes, fmt/Clippy, shared tests, snapshot,
  release, UI contract, mutations, PTY/ConPTY, session process/UI and package gates
  were supplemented at clean `fd58e8f`. The unchanged core/parity/recovery/service
  and five-page visual gates were reused explicitly.
- The corrected directory-drift fixture alone was retested on Windows at clean
  `50ae4d35836a097b98a6b6165d2abd0de127578d`. Eleven other methods were AST-compared
  and unchanged from the prior process run, where they passed. This is 11 passing
  unchanged cases plus one passing corrected case, not a second claimed full pass.
- WSL full matrix began at clean `c7575edb6b9d977712c6051f9722a049a27d7847` and
  completed at clean `50ae4d3`. Core/shared/component sources stayed unchanged;
  the terminal-only change received supplementary Clippy/release before GUI tests.
  The complete new 12-case process suite passed at `50ae4d3`.
- Final actual session Desktop/TUI gates ran at clean `2105b88` on both hosts after
  a test-only ConPTY capture wait correction. WSL custom chrome also passed its
  isolated retry at this SHA. Accepted product binaries remain those from `fd58e8f`;
  subsequent changes are test-only. Archive hashes match those actual binaries.

| Check | WSL/WSLg | Native Windows |
| --- | --- | --- |
| Final composed host gates | 32 PASS, 0 FAIL/BLOCKED, 2 NOT_RUN | 30 PASS, 0 FAIL/BLOCKED, 4 NOT_RUN |
| `p1g_host_checks` | PASS | PASS |
| Root Rust / Python compatibility | 47 / 16 PASS | 46 / 16 PASS; Unix-only case excluded |
| Independent JSON / SQLite parity | 120 / 78 PASS | 120 / 78 PASS |
| Backup / process recovery / existing service E2E | PASS / 11 / 11 PASS | PASS / 11 / 11 PASS |
| Compiled fingerprint/core/IPC guard mutations | 3 rejected, PASS | 3 rejected, PASS |
| Shared native-feature / NSH component tests | 23 / 6 PASS | 23 / 6 PASS |
| New real-process deterministic session cases | 12 PASS, full suite | 12 covered: 11 unchanged passes + corrected case retest |
| Format / Clippy / release / local archive | PASS | PASS |
| Native session UI and terminal | PASS, Wayland + PTY | PASS, Windows + ConPTY |
| 10k workload | All 834 component pages; actual native/PTY filter | All 834 component pages; actual native/ConPTY filter |
| Existing themes/i18n/chrome/five-page baseline | PASS | PASS, unchanged visual gates explicitly reused |
| Installed local Codex direct MCP | PASS, two tools + seven denials; no model turn | Native command unavailable in test process: NOT_RUN |
| Windows owner GUI/ConPTY → WSL | PASS via explicit WSL-side driver with accepted Windows binaries | Separate from native Windows daemon session gate |

Host versions: Rust/Cargo 1.99.1 on both; Python 3.14.9 WSL, 3.14.5 Windows.
WSL Codex CLI 0.167.0 used temporary configuration, exact operator-query equality,
unchanged user configuration and no remote inference. Actual owner connections
switched WSL/alpha → Windows/beta → WSL/alpha, cleared old task state, exercised
capability refusal/restoration and daemon stop/restart/manual reconnect. The helper
remains shell-free and read-only; no credentials were copied across hosts.

The deterministic Rust executable identifies itself as
`deterministic-fake-not-coding-agent`. Process cases cover init/version/auth errors,
updates and completion, two clients, disconnect/reopen, cancel and deadline kill,
permission deny/once/no-always/replay/expiry/malformed requests, duplicate/wrong IDs,
wrong session, huge/malformed frames, crash, unsafe/default-disabled configuration,
environment clearing, root drift and restart generation rejection. Agent credentials
cannot list/control sessions, choose permissions, cross project scope or add MCP tools.

Actual native tests use explicit Start review/confirmation, focused prompt input,
Cancel, Alt+I request selection followed by Deny/Allow once, and terminal prompt/Stop.
An unselected Alt+Y cannot approve anything. Both clients observe pending permissions;
closing/reopening clients preserves the same daemon-owned child; daemon loss clears
state. English/Chinese native captures at 1080×760 / 640×480 and terminal captures at
160×50 / 80×30 were inspected. No new session pointer-approval hit-testing, full IME
or assistive-technology coverage is claimed beyond these explicit tested paths.

WSLg observed **GdkWaylandDisplay**, WSLg Weston, mapped/after-paint rendering and
GskGLRenderer. Keyboard, pointer, resize, reconnect/denial, custom drag/maximize/
minimize/close and independent lifetimes passed. Missing Wayland and automatic X11
fallback were rejected. A per-child native Wayland Cairo diagnostic also passed.
GskGLRenderer is not proof of hardware acceleration. No XWayland/Xvfb acceptance,
niri session, compositor restart or system configuration modification.

## Real installed agent: partial interoperability

The owner explicitly authorized: “允许隔离 smoke；缺少认证则记录限制”. Installed
**Kimi Code CLI 2.5.2** provides documented `kimi acp`
([vendor reference](https://www.kimi.com/code/docs/en/kimi-code-cli/reference/kimi-acp)).
Installed Codex 0.167.0 and Claude Code 2.1.310 were inventoried without assuming
that either exposes a native ACP server. No adapter was downloaded or undocumented
vendor API used. Native Windows candidate commands were unavailable in the test PATH.

The actual installed Kimi executable was run through the daemon supervisor using
**temporary HOME and a disposable synthetic project**:

| Observation | Result |
| --- | --- |
| ACP v1 initialize | PASS |
| session/new | AUTH_REQUIRED |
| Prompt / remote model turn | NOT_RUN |
| Real login/config copy or login | NOT_RUN; none supplied |
| Synthetic database bytes | Unchanged |
| Overall | **PARTIAL**, not a successful real coding-agent prompt |

This isolated smoke ran at `9e17478`; runtime/core sources and the real-smoke helper
are unchanged through final `2105b88`. Its private report records executable/daemon
hashes. Fake-agent process success is never labeled real interoperability. No
credentials were requested to complete this milestone.

## Failures found and corrected

Development/first-run failures remain failures in the original reports. Idle IPC
needed a `{session:null}` envelope; passive UI polling originally changed disabled
state and focus. A non-reactive in-flight guard and command epochs now avoid that
churn and prevent old observations overwriting newer explicit actions. Permission
controls retain stable projections and bind render-time authority.

The terminal embedding ignored resize notifications; ConPTY reflow could leave
stale cells. Resize now clears the terminal diff buffer for a full repaint. Captures
also drain the old dimensions and wait for the asynchronous ConPTY repaint; the
final compact frames are clean. Clean terminal close drains output before requiring
exit and terminal restoration. Fixed Tab-count assumptions were replaced by explicit
request-selection shortcuts, not recorded as successful pointer tests.

The old mutation script's generic operator-check anchor became non-unique; its
project-list-specific mutation now compiles and is rejected by the existing real
process guard. A new root-drift fixture initially reused another registered root,
violating SQLite uniqueness; it now uses an unclaimed synthetic directory and closes
its Python SQLite connection explicitly before Windows cleanup. One WSLg chrome
click was safely refused because the owned target was occluded; a later isolated
actual-pointer retry passed without relaxing the ownership check.

## Private evidence and reproduction

All raw synthetic PNGs, terminal frames, reports, unsigned archives and logs remain
ignored under `artifacts/local-p1-g/`; none are uploaded. Primary WSL evidence:
`final-wsl/` (original full run), `final-wsl-followup/` (mutations/Clippy),
`final-wsl-gui-followup/` (final sessions/chrome), `final-wsl-composed.json`,
`final-real-kimi/`. The marked disposable Windows checkout contains
`final-windows/` (original full run), `final-windows-followup/` (supplements and
composed report, with original report retained), `final-windows-ui-settled/`, and
`session-root-drift-corrected.log` inside the followup directory. Composed reports
include per-gate provenance; raw standalone failure totals are not silently changed.

Representative local commands (private cached Python/compiler setup omitted):

```sh
PYTHONPATH=src python -m unittest discover -s tests -v
python tools/local_verify.py --ui-full --owner-reviewed --report-dir artifacts/local-p1-g/<host>
python tools/test_agent_session.py -v
python tools/test_agent_session_ui.py --desktop <native-release> --tui <tui-release> --out artifacts/local-p1-g/<session-run>
# Only after the explicit opt-in already recorded for this owner:
python tools/test_real_acp_smoke.py --opt-in --executable <installed-kimi> --version 2.5.2 --out artifacts/local-p1-g/<real-run>
```

`--owner-reviewed` records accepted **base UI only**, with new Sessions separately
PENDING. Windows-owner/WSL integration uses explicit test-root/Python/distro/user
variables and disposable state; no external project root is selected for this run.
Do not repeat a full matrix for these documentation changes. Real external
repositories and existing ACP user databases were not opened or changed.

NOT_RUN: full IME/screen reader, real OS clipboard, native Windows Codex/real ACP
candidate, real Kimi prompt/model turn, optional web smoke and hardware acceleration
proof. Niri/mobile/macOS are outside this milestone. This remains an engineering
prototype, not protocol certification or a production policy/security boundary.
