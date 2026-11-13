# P1-H real-agent evidence

> **Historical record — not a current runbook.** Original dates, source revisions
> and outcomes below are preserved. S4 retired the executable prototype and probes;
> use [Rust-only development](../p1/rust-only-development.md) for current commands.

## Result and authority

Issue #26 implements a bounded native ACP authentication/turn slice, not a login
platform or Windows→WSL execution bridge. The installed **Kimi Code CLI 2.5.2**
completed an explicitly authorized authenticated model turn through the real Rust
daemon/operator IPC. Deterministic Desktop/TUI tests independently cover recovery,
permissions and lifecycle. Context MCP attachment is **NOT ENABLED** (`mcpServers: []`).
See the [reviewed contract and official sources](../p1/agent-authentication.md).
ADR-0003 remains Proposed; this work is for a Draft PR, without automatic merge.

Started from refreshed main `ff8db143fcbd6f2dc1614094a20a3ac697317f26` (PR #25
merged), on `feat/p1-h-real-agent-e2e`. The merged P1-G feature branch was deleted
locally and remotely only after merge/ancestor checks. The
[owner's PR #25 decision](https://github.com/ybakiame/agent-control-plane/pull/25#issuecomment-6772650402)
authorized its merge and **deferred dedicated Sessions visual review**. The later
instruction to execute #26 authorized proceeding with that recorded deferment;
this is not a claim of completed Sessions visual acceptance. Prior P1-F base UI
acceptance remains separate. No further owner action was required for these runs.

The owner selected a Kimi Code API-key configuration, filled an isolated private
HOME locally, and first authorized one model call, then explicitly authorized
necessary debugging calls using subscription quota. Three application prompt
submissions occurred: two completion attempts and one immediate-cancel attempt.
The runner did not retry a prompt automatically. Provider HTTP request/billing counts
were not instrumented. No key was supplied in chat or committed.

## Auth and protocol behavior

External ACP v1 and core IPC framing/auth v1 remain unchanged. The optional session
extension is **v2**; v1 session requests receive an explicit Version refusal. Existing
agent configuration v1 still works. No Cargo dependency/lock, production schema,
Python profile-write authority, fingerprint bytes or existing MCP tool was changed.

Initialize retains only bounded auth method types and known capability booleans.
Agent method IDs, descriptions, login argv/env, raw errors and arbitrary metadata
are discarded. Unsupported login/reverse methods never run an auth script.
`AUTH_REQUIRED` becomes action needed, revokes pending authority and reaps the direct
child. No automatic retry. The operator prepares auth outside the app, uses
**Review retry / Alt+U**, then explicitly confirms a new generation. Retry binds to
the observed configured agent; config removal cancels an armed review. Authentication
never approves tools or adds filesystem/terminal client capabilities.

Known content-free metadata can precede `session/new` (at most eight observations),
but must bind to its returned session ID. Response/tool chunks require the active
prompt and correct session. Only its matching response and valid stopReason complete
a turn. Prompt drafts, permission review and stale target/generation callbacks do
not survive authority changes. The trusted child still has local user privileges;
ACP mediation is not an OS sandbox or descendant-containment guarantee.

## Real installed agent: exact evidence

Host: native Linux daemon under Arch WSL2, x86_64, kernel
`6.18.54.1-microsoft-standard-WSL2`. Invocation: installed absolute executable with
fixed argv `["acp"]`, cleared environment and explicit disposable HOME. Only synthetic
project/SQLite state and one fixed no-tool prompt were used. Credentials were read
only by the selected agent; the runner checked private-path shape/ownership/modes,
never config contents. All evidence retained here is content-free.

Observed ACP v1: client capabilities `{}`; terminal auth method; loadSession,
image/embeddedContext, HTTP/SSE MCP and logout advertised; audio false. Advertised
capabilities do not mean those optional operations were implemented or exercised.
No authenticate/logout/resume/load/fork, permission approval or MCP call was sent.

| Attempt | Clean source | Observed outcome |
| --- | --- | --- |
| First completion | `319e9a5a4c6a07ca8147fba34faa2a9634c4a44d` | Original report **FAIL**: `end_turn`, 5 response chunks/12 characters, but the checker incorrectly required a whole synthetic marker in one chunk. Original failure retained. |
| Explicit follow-up completion | `ea5e33e36cf3f06255e3615ffb1889d30231696d` | **PASS**: initialize → new → prompt → 5 agent response chunks/12 characters → `end_turn`; synthetic marker verified across chunks; 0 tool events. |
| Separate cancellation | `ea5e33e36cf3f06255e3615ffb1889d30231696d` | **PASS**: prompt accepted, immediate cancel acknowledged with `cancelled`; 0 response chunks/characters/tools. This does not demonstrate cancellation during streamed inference. |
| Empty-HOME negative | `a6cc337a2be9ff6ea58c9e13c2e169e82363aa04` | initialize PASS, `session/new: AUTH_REQUIRED`; prompt/model **NOT_RUN**, overall PARTIAL as expected. |

All four attempts observed direct-child reaping and byte-identical synthetic DBs.
The stream checker regression excludes user/tool echoes. Its corrected runner only
formatted between `ea5e33e` and `a6cc337`; runtime sources stayed identical through
the subsequent UI-only fix. No extra model call was needed to validate that fix.
Actual model prompts used daemon/operator IPC; the GUI/TUI tests used deterministic
agents. A real-model prompt originating in the GUI is **NOT_RUN**.

Binary SHA-256:

- Installed Kimi: `28b0ff2e18ce35ba6e508ea50a5726095919ab6fd448682ea57070e1e3879875`.
- Tested daemon: `76be7b23a89da70dfb24576c1140de02af6ff5acdf50ca2a4f362882f6fa02b3`.

Private reports remain under ignored `artifacts/local-p1-h/real-turn-1/`,
`real-turn-2/`, `real-cancel/` and `real-auth-required/`; only the sanitized facts
above are published. After all child-reap checks, the owned private HOME, config,
vendor session/log files and supplied key copy were deleted. The ordinary user's
HOME/login configuration, existing ACP database and external development repositories
were neither copied nor modified. No raw response/transcript, credential file,
private-app screenshot or machine-specific path is included in this PR.

## Local host matrices and provenance

Full local matrices ran **once per host**, clean
`a6cc337a2be9ff6ea58c9e13c2e169e82363aa04`; no Actions were dispatched. After final
review found that a reopened auth UI could default to a different configured agent,
`3629d23e3e86fa79469413e8f9170ba2968b0fda` corrected selection/retry binding and added
an explicit same-executable assertion. The original GUI test did not prove that
property; its report is retained, not relabeled. Only affected gates were repeated:
Python, shared/NSH fmt, Clippy, shared native-feature tests, snapshot/release builds,
UI contract, the single session-control component test, and actual session Desktop/
PTY or ConPTY tests. Unchanged baseline checks were reused explicitly.

The WSL supplement first failed before input because its launcher omitted explicit
`ACP_WINDOWS_PYTHON`. After supplying the test-only environment, the UI test reached
auth-required but the owned WSLg input probe rejected the newly added Alt+U key.
Test-helper-only `6920c7e994f7be21d325edb0561c7d02d655f697` adds that one key to its
closed vocabulary; only that actual UI gate and the required Python suite were
repeated. Product sources/binaries are identical to `3629d23`. Both failed attempts
remain separate private reports; neither is claimed as passing.

Private originals: `final-wsl/report.json`, owned Windows clone
`final-windows/report.json`. Supplements: `retry-wsl/report.json` and owned Windows
clone `retry-windows/report.json`, followed by WSL `retry-wsl-ui-2/report.json`
(failed key probe) and `retry-wsl-ui-3/report.json` (corrected probe). The composed
matrix retains per-gate source SHA; it is not another full run at the newer SHA. The checked-in evidence-only follow-up
does not change tested product/test sources. Development archives belong to the
original full baseline; they do not contain the later retry-binding fix.

| Check | WSL / native WSLg | Native Windows |
| --- | --- | --- |
| Full baseline host gates | 33 PASS, 0 FAIL/BLOCKED, 2 NOT_RUN | 31 PASS, 0 FAIL/BLOCKED, 4 NOT_RUN |
| Final composed named gates (adds one targeted component gate) | 34 PASS, 0 FAIL/BLOCKED, 2 NOT_RUN | 32 PASS, 0 FAIL/BLOCKED, 4 NOT_RUN |
| Final affected-gate set | 10 PASS, composed as described; original failures retained | 10 PASS |
| Root Rust / Python | 47 / 16 PASS | 46 / 16 PASS; Unix-only case excluded |
| JSON / SQLite parity | 120 / 78 PASS | 120 / 78 PASS |
| Backup / process recovery / service E2E | PASS / 11 / 11 PASS | PASS / 11 / 11 PASS |
| Compiled core/fingerprint/IPC guard mutations | 3 rejected, PASS | 3 rejected, PASS |
| Shared native-feature / NSH components | 25 after fix / baseline 6 PASS | 25 after fix / baseline 6 PASS |
| Session real-process deterministic cases | 18 PASS | 18 PASS |
| Real-runner stream guard | 1 PASS | 1 PASS |
| Actual session clients | Wayland + PTY, same-agent auth recovery PASS | Windows + ConPTY, same-agent auth recovery PASS |
| 10k workload | All 834 component pages, <100 text nodes; native/PTY filtering PASS | All 834 pages, <100 text nodes; native/ConPTY filtering PASS |
| Themes, EN/ZH, custom chrome, five-page previews | PASS | PASS |
| Local Codex two-tool MCP | PASS; seven denials, no model turn | Native command unavailable: NOT_RUN |
| Windows GUI/ConPTY → WSL read-only connection | PASS via WSL driver and same-source Windows baseline binaries | Separate from native Windows daemon session gate |

Both hosts: Rust/Cargo 1.99.1; Python 3.14.9 WSL / 3.14.5 Windows. Windows 11 x64
build 26300. WSLg 1.0.79 / Weston, GTK 4.24.1; NSH pinned
`87c7e8da85d2f15a56107baa6f0fabb419b508b3`. Baseline and corrected session captures
use real **GdkWaylandDisplay**, not XWayland/Xvfb. GL renderer observed; hardware
acceleration is **NOT_VERIFIED**. Actual keyboard/resize, nonblank target-window
captures, reconnect, process independence, explicit permissions and unchanged DB
were checked. Only owned synthetic windows received automated input/capture.

Installed Codex CLI 0.167.0 direct MCP used temporary configuration and exact
operator-query equality; normal user configuration remained unchanged. The separate
Windows→WSL read-only test exercised target switching, stale-task clearing, auth
refusal/restoration and daemon loss/reconnect. It never enabled session execution
across hosts.

Reproducible local command shapes (private placeholders are not repository values):

```text
PYTHONPATH=src:tools:<private-test-deps> python tools/local_verify.py --ui-full --owner-reviewed --windows-report <private-windows-report> --report-dir artifacts/local-p1-h/final-wsl
<Windows Python> tools/local_verify.py --ui-full --owner-reviewed --report-dir artifacts/local-p1-h/final-windows
python tools/test_agent_session.py -v
python tools/test_agent_session_ui.py --desktop <host-release-desktop> --tui <host-release-tui> --auth --out artifacts/local-p1-h/<private-run>/sessions
python tools/test_real_acp_smoke.py --opt-in --executable <installed-kimi> --version 2.5.2 --authenticated-home <owned-private-home> --allow-credential-use --allow-model-call --out artifacts/local-p1-h/<separate-run>
# Separate cancel attempt adds --cancel-after-send; empty-HOME negative omits all credential/model flags.
```

All Cargo builds/tests used pinned dependencies with `--offline --locked`;
format/Clippy, Python standard-library tests and core/security checks passed.
`--owner-reviewed` records the accepted **P1-F base**, while the session review field
explicitly remains `DEFERRED_BY_OWNER_AT_PR25`.

## Remaining gaps

H2 bridge/tool-call evidence is **NOT_RUN**. Existing startup credentials lack safe
per-session revocation for already-connected peers; forwarding them would broaden
rights. The contract specifies a future explicit short-lived project lease, checked
on every request, with two tools only and stop/restart cleanup. Current original
two-tool MCP negative regressions remain intact.

No native Windows Kimi/model turn, real-model GUI turn, mid-stream real cancellation,
new owner Sessions visual acceptance, macOS/mobile, IME/screen-reader, clipboard or
hardware acceleration proof. Optional web smoke was not selected. niri was excluded
by scope. Hosted CI is NOT_RUN by instruction; every commit carries `[skip ci]`.
No system configuration, production database/schema or real project/user data was changed.
