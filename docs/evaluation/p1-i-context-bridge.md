# P1-I: revocable session Context MCP evidence

## Result and boundaries

Issue #28 adds an explicitly opted-in project/session/agent-bound MCP capability to
the existing single native ACP session. The two-tool bridge is disabled by default.
A lease is checked on every request and before admitting its computed response for
publication, including previously authenticated peers. Stop, valid prompt Cancel,
expiry, failure, direct-child exit, root drift and daemon shutdown/restart end the
authority. Prompt Cancel revokes context for the whole session; completion retains
access until expiry or termination. Re-enabling requires fresh explicit Start.

Source starts from refreshed main `3c7799a2c72dd80c1ec142fdfbbfe2347cf987d4` (PR #27
verified merged), branch `feat/p1-i-revocable-session-mcp`. Only the merged local H
branch was removed; no remote branches were deleted in this milestone. Existing
NSH layout/interaction/model architecture, Python profile-write authority, SQLite schema, fingerprint
bytes, core resolver and two MCP tool schemas are unchanged. One Cargo lock change
adds the existing DTO crate to the operator CLI; no external dependency was added.
ADR-0003 remains Proposed; hosted Actions are NOT_RUN; commits use `[skip ci]`.
One obsolete EN/ZH capability-help sentence was corrected to describe explicit context
sharing, without adding controls, pages or a visual redesign.

The [I0 threat/lease contract](../p1/session-context-bridge.md) was committed before
implementation. It cites current ACP v1 and Kimi primary documentation and separates
ACP client control from MCP server context. This is project scoping and revocation,
not an OS sandbox, descendant containment or protection against malicious same-user
filesystem access. No Windows→WSL execution bridge, login UI, listener or persistent
transcript was introduced. Windows→WSL owner queries remain read-only.

## Operator interface and protocol compatibility

The trusted daemon must receive an absolute `--context-mcp` binary outside project
roots. Each native operator Start independently requires `--confirm-start` and
`--share-context`. The existing UI never sends sharing opt-in and keeps its current
Start behavior. Headless operator command shapes:

```text
acp-daemon <explicit private location/database/allowlist/agent-config> --context-mcp <trusted absolute acp-mcp> [--context-lease-seconds 300]
acp-query <explicit native location> session-start alpha <configured-agent-label> --confirm-start --share-context
acp-query <explicit native location> session-context alpha
acp-query <explicit native location> session-stop alpha <exact-generation>
```

Without either required opt-in, no session bridge is attached and normal
`session/new.mcpServers` remains `[]`. The daemon supplies one fixed stdio entry;
caller server lists, executable/argv/env/path/TTL overrides are rejected. The MCP
process is launched by the managed ACP agent from that entry. Capability lifetime
belongs to the daemon even if a vendor-owned descendant survives process cleanup.

IPC framing/auth API v1 and optional session API v2 are preserved. Existing view DTOs
are unchanged; `share_context` and operator-only `session.context` are additive.
Startup operator/project credentials stay format 1. Only ephemeral session
credentials use explicit format 2 with project access and validated agent/generation
binding. Rights derive from the live daemon registry, not credential metadata or
caller role/project parameters. A random private, exclusively created/non-following
file is synced before its path is sent on the private ACP channel. Expiry uses a
monotonic clock, defaults to 300 seconds and is bounded to 1–3600 seconds. No secret
is stored in SQLite, UI state, argv, public diagnostics or evidence.

Response publication is admitted at the final lease check. Responses admitted before
revocation can finish writing; delivered data cannot be recalled. New requests after
revocation/expiry fail even on an established peer, and a blocked computation whose
final check sees revocation returns no data. Cleanup cannot reactivate authority.
Hard-kill restart has no session registry and rejects saved tokens; recognized stale
private files are checked and removed under the exclusive runtime lock. Malformed,
unsafe or colliding files fail closed. Full cross-account/hostile-owner race defense
is not claimed.

## Negative security and deterministic integration

IPC unit cases verify old/new credential formats, malformed bindings/roles, safe
stale-file handling, monotonic expiry without a cleanup tick, revocation of an
already authenticated peer, reconnect rejection and old-generation isolation.
The process suite adds **14** cases using real daemon/IPC/MCP executables:

- Default off, two explicit operator controls, malformed/unauthorized Start refusal.
- Established peer and reconnect denied after Stop; saved token cannot join a fresh
  beta generation; old generation cannot stop the replacement.
- In-flight SQLite read discarded after revocation, using an exclusive fixture lock.
- Operator CLI confirmation and content-free inspection; old clients still work.
- Canonical runtime paths that cannot be represented in JSON fail with UnsafeRuntime,
  without a panic, child launch, credential publication or DB change.
- Expired open peer and new handshake denied; credential file cleaned.
- Prompt Cancel ends the entire lease while preserving managed turn cancellation.
- Project/role/generation overrides, project listing, session/profile mutation denied.
- Concurrent held/reconnecting peers cannot resurrect revoked authority.
- Crash and unsupported-agent rejection revoke and clean up without raw error text.
- Protocol failure diagnostics retain only fixed rejection-stage labels, never the
  untrusted method/body/credential marker.
- Daemon hard kill/restart rejects saved capability and closes old peer.
- Registration-root drift revokes instead of returning another checkout's context;
  restoring the fixture registration does not reactivate the lease.
- A deterministic ACP agent discovers and calls the actual two-tool MCP process;
  daemon counters independently observe one context and one Handoff read. A separate
  bridge read exactly matches operator context including requested allowed docs;
  mutation tools are absent and post-Stop calls fail. Fixture DB remains unchanged.

An additional local fault injection removed the final publication check. The blocked
read test failed as intended (returned data instead of Unauthorized). Source was
restored and rebuilt before further validation. This is evidence for the new guard,
separate from the three pre-existing compiled core/fingerprint/IPC guard mutations.

Initial process-test failures were fixture errors (stopping a beta generation with
the alpha default; then trying to assign another registered project's unique root).
Both original logs remain private. The corrected root-drift fixture uses a new owned
synthetic directory and explicitly distinguishes its deliberate registration writes
from the service's read-only behavior. No failed run is relabeled as passing.

## Real installed agent evidence

The owner authorized Kimi authentication/network/model use for this debugging session,
retention of its private key for later validation, and an OpenCode free-model comparison.
All real turns used disposable synthetic projects and private operator-selected HOME;
the additionally offered public project was unnecessary and was not accessed. No normal
login state, real ACP database or external repository was copied or modified.

Installed binaries were independently identified:

| Agent | Version | Executable SHA-256 |
| --- | --- | --- |
| Kimi Code CLI | 2.5.2 | `28b0ff2e18ce35ba6e508ea50a5726095919ab6fd448682ea57070e1e3879875` |
| OpenCode | 2.2.7 | `589253d36810f21217edd51545cd435b328cdc3cf4f451fbe0cce6f88afcf5e1` |

The final real Kimi run, `real-context-6`, is **PASS** at clean
`df3d8da741506544f0f8175991d95bee8e90f741`. One fixed application prompt completed with
`end_turn`, three streamed response chunks, and independent daemon counters of
**one `get_effective_context` and one `get_handoff` call**. Stop then denied both an
already-authenticated IPC peer and a new handshake with the saved ephemeral secret.
The direct child was reaped, lease file removed, and synthetic DB bytes unchanged.
Daemon SHA-256 was `164b2cfaa5b56e79c74c3203127077194aeeed0bfcc904175606a6aadd996a66`;
MCP SHA-256 was `d761861f928f599c1c6acd64be09964e75dbe8428fa0d4eb6664c92f42c330db`.
Security Clippy, IPC unit and 14-case process gates passed at that same clean SHA
before model use. Provider HTTP requests were not instrumented; one application
prompt can require multiple model/tool round trips and is not claimed as one HTTP call.

Kimi remained in manual permission mode with its tool catalog restricted to the two
MCP tools. Auto title, telemetry and automatic skill merging were disabled. Its ACP
adapter still requested approval despite the private tool-name allow rules. The
separately opted-in test runner selected **allow once** for each of the two exact
vendor tool-name/content shapes, at most twice; it never selected persistent approval.
The vendor adapter omits arguments from this permission request, so parameter approval
is **not** claimed. Tool names/content are vendor claims, while project/role/lease
restrictions are independently enforced by the daemon/MCP. Unknown requests, extra
fields, different content, absent opt-in and non-Kimi requests stop the smoke. A focused
negative Python regression covers this selector. This is test-only behavior; product
permission handling and the reviewed UI retain their existing operator controls.
See the [Kimi adapter source](https://github.com/MoonshotAI/kimi-code/blob/main/packages/acp-server/src/approval.ts)
and [configuration reference](https://www.kimi.com/code/docs/en/kimi-code-cli/configuration/config-files).

Original failures and incomplete runs remain recorded rather than overwritten:

| Private attempt | Clean source | Observation |
| --- | --- | --- |
| `real-context-1` | `3dbe865` | FAIL / incomplete reporting after a cleanup race; one prompt, counters not captured |
| `real-initialize-2` | `a7a7e1a` | PASS initialize/new/stop only; zero prompts, not tool-call proof |
| `real-context-3` | `c85e8b5` | FAIL model acceptance, `auth_required`, zero tool reads; retained template still contained a placeholder |
| `real-context-4` | `c85e8b5` | Correct owner-supplied key installed privately; model reached first MCP approval; stopped without approving |
| `real-context-5` | `5508e33` | Stopped at approval: strict initial selector expected raw arguments that the vendor does not expose |
| `real-context-6` | `df3d8da` | PASS real two-tool calls and post-Stop denial/cleanup |

The earlier pending-authorization claim was too narrow: the owner had already allowed
debugging calls. The subsequent runs used that standing authorization, each as a
separate deliberate invocation with no automatic prompt retry loop. The runner saves
content-free progress before submission and always attempts bounded child reaping.

OpenCode 2.2.7 accepted native ACP v1 `initialize` and the same stdio MCP session.
Its isolated configuration explicitly selected free models and pinned build/title/
summary/compaction to the same selected free model, with all tools denied except the
two named MCP reads. The installed catalog listed both selected models, and the
[official model page](https://opencode.ai/docs/console/models/) listed them as free
at validation time. The [ACP reference](https://opencode.ai/docs/cli/acp/) documents
a private stdio process that does not use the shared background service or ACP port.

| OpenCode model / private attempt | Clean source | Real result |
| --- | --- | --- |
| `opencode/glm-5.3-flash-free` / `real-opencode-glm-1` | `5508e33` | BLOCKED by provider free-usage limit; original runner status FAIL; zero MCP calls |
| `opencode/deepseek-v4.1-flash-free` / `real-opencode-deepseek-1` | `df3d8da` | BLOCKED by provider free-usage limit; original runner status FAIL; zero MCP calls |

Each original keyed run submitted one fixed prompt, returned an agent error before
streamed/tool output, and reaped its direct child, removed the lease and preserved DB
bytes. A fixed-category scan identified `free usage` / `subscribe` / `limit`; original
FAIL reports are retained. This establishes the service response for those isolated
runs, **not** that the owner's account or interactive terminal has exhausted its quota.
No paid-model fallback or application retry loop was attempted.

### Review follow-up: anonymous OpenCode and actual model ID

The owner demonstrated a successful WSL TUI response using **DeepSeek V4.1 Flash Free**.
The screenshot remains private and is not uploaded. The installed command
`opencode models --standalone`, in an owned HOME without credentials or inherited
environment, returned exactly `opencode/deepseek-v4.1-flash-free` for DeepSeek. This
matches the model ID used in the original comparisons. The vendor's
[v2.2.7 provider source](https://github.com/anomalyco/opencode/blob/v2.2.7/packages/core/src/plugin/provider/opencode.ts)
also enables free models with a built-in public credential when no user key is present;
a user-supplied API key is not a prerequisite for trying these free models.

Follow-up tests reused one separate anonymous HOME, without copying normal login,
user/device identity, database or project state. The test configuration pins build,
title, summary and compaction to the free model and denies non-MCP tools. The helper
now accepts explicit `--anonymous --agent opencode --private-home …`; anonymous and
credential-use flags are mutually exclusive, Kimi cannot select anonymous mode, and
an existing authentication file or provider/extension-bearing config is refused.
The new negative Python case covers the credential-mode combinations.

| Anonymous follow-up | Source | Observation |
| --- | --- | --- |
| Catalog query | installed OpenCode 2.2.7 | DeepSeek ID verified; no auth file |
| Fixed no-tool `run --standalone --model opencode/deepseek-v4.1-flash-free` | installed OpenCode 2.2.7 | No marker response; free-usage error, not model-not-found |
| Fixed two-tool ACP prompt, `real-opencode-deepseek-anonymous-1` | clean `01b3a307f93d7a22d612d22ecfd6593ce7cb8f9f` | Session accepted; zero MCP calls; original runner FAIL; BLOCKED by free-usage response; reaping/revocation/DB preservation pass |
| Fixed no-tool TUI prompt on a private PTY | same installed binary and anonymous HOME | Owned synthetic vendor DB confirms actual provider `opencode`, model `deepseek-v4.1-flash-free`, HTTP 429/free-usage assistant errors; no response marker |

Terminal redraws of the synthetic prompt are not counted as model responses. Only
allowlisted model metadata, numeric status, error-category flags and marker booleans
were extracted from the owned synthetic vendor DB; no raw conversations or identifiers
are published. Provider HTTP request counts remain uninstrumented. This independently
rules out a mistyped ID in these runs, while **not explaining** the difference from
the owner's successful regular WSL terminal. No normal user database/config/session
was opened or changed to reproduce that success, and no quota reset or bypass is
claimed. OpenCode MCP tool use and model quality remain NOT_VERIFIED; the observations
do not support a quality/performance ranking against Kimi.

Private reports remain under ignored `artifacts/local-p1-i/`. Credentials stay outside
the repository in owner-only local files (HOME 0700, auth config 0600); supplied key
files and directories were also restricted to these permissions. Key values are used
only to prepare private vendor configuration and are never printed or published.
Generated vendor sessions/logs are removed after reaping; authorized credential configs
are retained. Retention does not reactivate a lease or implicitly initiate future calls.

## Local regression matrix and source provenance

Full matrices ran once per host at clean
`77822fb7edf02cee62139046bf4e8c668c398d96`, with Windows GUI/ConPTY input completed
before native WSLg input. Originals were **Windows 32 PASS / 1 FAIL / 4 NOT_RUN** and
**WSL 33 PASS / 2 FAIL / 2 NOT_RUN**. Both session UI failures were an obsolete fixture
count (five configured programs, still expecting four). WSL custom chrome additionally
stopped before a click because the owned target was occluded; no other window was
clicked. Original reports/failures remain intact.

Test-only `cc0165259ff5f74c28d62fd81a72ac1100440c14` corrected the count and added
source/script/binary hashes and new-generation TUI observation. Actual session UI
passed on both hosts; WSL chrome passed its isolated retry. Visual review then found
an H-era claim that MCP forwarding was disabled even in the opted-in context session.
`ef77994f4e5d065171550e0de8910f6d0c4458f9` changes only that EN/ZH help literal.
Only affected fmt, shared native-feature tests, snapshot, NSH Clippy/release and actual
session UI gates were repeated on both hosts and passed; no 834-page/full-suite repeat.
UI capture review confirmed the corrected guidance and unchanged layout.

Final path review also guards unrepresentable UTF-8 canonical runtime/project paths
before ACP JSON serialization and releases the session mutex before lease-file I/O.
An added native process case proves sanitized refusal on both platforms. Only affected
root build/fmt/Clippy, IPC unit and context-process gates were supplemented; existing
UI binaries/surfaces do not depend on the changed runtime-only modules. These six
backend gates passed on both hosts at clean
`2d4e1c476a531405a3030d3b52625f29dc470dde`; private
`path-supp-{host}/report.json` records exact commands and counts. The 14-case process
suite includes the native Windows unpaired-UTF-16 runtime and Unix non-UTF-8 canonical
alias cases, with no skipped case.

Private provenance: original `final-windows/report.json` (owned Windows clone) and
`final-wsl/report.json`; `ui-supp-{host}/report.json` at cc01652; final
`guidance-{host}/report.json` at ef77994. `composed-{host}.json` records each gate's
source separately, rather than claiming one full green run at the newest SHA.
Baseline archives were built at 77822fb and predate the guidance-only correction;
current release executables were rebuilt and their hashes are in the latest actual
UI reports. No binary archive or private screenshot is uploaded with the PR.

| Check | Native WSLg / Linux | Native Windows |
| --- | --- | --- |
| Full local host gates | 35 PASS, 0 FAIL/BLOCKED, 2 NOT_RUN (composed) | 33 PASS, 0 FAIL/BLOCKED, 4 NOT_RUN (composed) |
| Rust / Python | 50 / 18 | 49 / 18 |
| JSON / SQLite parity | 120 / 78 | 120 / 78 |
| Backup / process recovery / existing service | PASS / 11 / 11 | PASS / 11 / 11 |
| Shared native-feature / NSH component / terminal-cell tests | 25 / 6 / 3 | 25 / 6 / 3 |
| Existing session / new scoped-context process cases | 18 / 14 | 18 / 14 |
| IPC lease unit cases (includes three new cases) | 5 | 5 |
| Existing real-runner stream guard | 1 | 1 |
| 10k workload, themes, EN/ZH, chrome, five-page baseline | PASS | PASS |
| Actual Sessions clients incl. scoped context turn and Stop | PASS | PASS |
| Existing local Codex two-tool MCP | PASS; two tools, seven denials, no model turn | Native command unavailable: NOT_RUN |
| Windows GUI/ConPTY → WSL read-only owner helper | PASS; explicit driver, no execution controls | Separate WSL-side driver |

Rust/Cargo 1.99.1 on both hosts; Python 3.14.9 WSL / 3.14.5 Windows. Windows 11
x64 build 26300, Arch WSL2 kernel `6.18.54.1-microsoft-standard-WSL2`, WSLg 1.0.79 /
Weston, GTK 4.24.1. Installed Codex CLI 0.167.1 exercised direct two-tool stdio with
unchanged normal user configuration and no remote inference. Actual Windows owner
GUI/ConPTY target switching, auth refusal/restoration, task clearing and daemon
reconnect passed through the separate WSL-side driver.

The native desktop actually used `GdkWaylandDisplay` / `GskGLRenderer`, with nonblank
owned-window captures, keyboard, resize and process independence. A GL renderer is
not proof of hardware acceleration. No XWayland/Xvfb substitute was accepted.
All 834 pages of the 10k component workload mounted with fewer than 100 text nodes
on both hosts; actual native/PTY or ConPTY filtering also passed.

After the real-smoke helper changes, the Python suite (now 18 checks, including the
permission selector and anonymous-mode negative cases) passed on both Linux/WSL and native Windows.
Only the affected Python/helper gates were supplemented; the previously passing
GUI/ConPTY/Wayland/10k evidence remains tied to its original product source revisions.

All pinned Cargo build/test/run gates use `--offline --locked`; local fmt/Clippy,
Python/parity, fingerprint and IPC/MCP guards preserve the existing core contract.
Only owned synthetic windows receive input/capture. UI source/layout and six
surfaces remain unchanged. P1-F base acceptance stays separate from the dedicated
Sessions visual review deferred by the owner at PR #25; no new acceptance is inferred.

## Review lock-hygiene fix

The source review correctly identified that `Supervisor::start` still retired its old
lease while holding the session mutex. At clean
`01b3a307f93d7a22d612d22ecfd6593ce7cb8f9f`, Start now checks fallible generation setup
before taking the old lease, atomically reserves the new Starting view and control
channel, releases the mutex, then revokes/drops the old lease before spawning the new
child. Credential metadata/read/unlink work therefore runs outside the supervisor
mutex, while concurrent Starts still observe the reserved busy state. No public DTO,
permission, UI or lease lifetime contract changed.

Only seven affected backend/helper gates were rerun on each host: root fmt, native
binary build, workspace Clippy, five IPC unit cases, 14 context process cases, 18
existing session process cases, and 18 Python checks. All passed with no skipped test
in the process suites. Private `review-supp-{host}/report.json` records the exact clean
source and commands. Prior native GUI/ConPTY/Wayland/10k evidence is preserved with its
original source provenance; no full matrix or Kimi model turn was repeated for this
lock-only runtime change. The successful Kimi tool-call evidence remains tied to df3d8da.

## Remaining limitations

Real installed Kimi tool-call acceptance is verified on synthetic Linux/WSL state.
OpenCode isolated keyed and anonymous free-model tests returned quota errors, while
the owner demonstrated interactive WSL success; that environment difference remains
unexplained. No OpenCode MCP success or quality comparison is claimed. No extra public-project test was needed. All earlier
failed/incomplete observations remain distinct from the final Kimi success.

No native Windows real-agent model turn, real-model GUI prompt, macOS/mobile,
IME/screen-reader, clipboard or hardware acceleration proof. Optional web smoke
not selected; niri excluded by scope. No hosted Actions, automatic merge, real user
profile/database access, production schema change, external project writes or
operator credential forwarding. A Draft PR is source review, not production
isolation, all-vendor ACP/MCP certification or approval of a new framework/brand.
