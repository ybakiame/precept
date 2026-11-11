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

## Real installed Kimi evidence

Installed Kimi Code CLI 2.5.2 was independently checked with empty/disposable HOME;
binary SHA-256 `28b0ff2e18ce35ba6e508ea50a5726095919ab6fd448682ea57070e1e3879875`.
The owner prepared Kimi Code API-key configuration locally and explicitly authorized
one fixed synthetic context turn, including necessary model/tool round trips. Two
exact MCP tool names were preauthorized in that private config; no wildcard/yolo,
client permission auto-approval or inherited login configuration. Auto session title,
telemetry and automatic skill merging were disabled. The owner separately requested
retention of the key for future verification; this does not authorize implicit model
calls. The runner never reads config/key contents or publishes raw vendor output.

1. `real-context-1`, clean `3dbe86571a92d570eb6d1ab67714482ce24d40f8`: **FAIL / incomplete
   evidence**. One application prompt and one LLM-request event were counted from
   fixed event tags in the owned synthetic wire metadata; no message text was
   published. Cleanup incorrectly tried Stop while the failed child receiver was
   closing and raised Unavailable. Independent daemon tool counters and final child
   cleanup were not retained. No real MCP success is claimed from this attempt.
2. `real-initialize-2`, clean `a7a7e1a0922aae5a8d2523ea53c18ca8c6d6f03b`: **PASS for
   initialize/new/stop only**, zero application prompts. Installed identity verified,
   ACP v1 accepted the stdio MCP session, counts remained zero, direct child reaped,
   lease file removed and synthetic DB unchanged. This proves neither a model turn
   nor a context tool call.
3. Additional real model turn: **NOT_RUN / pending fresh opt-in**. The initial fixed
   prompt authorization was consumed; no prompt was automatically resubmitted.
   Consequently a successful real Agent MCP context call is **NOT_VERIFIED** in this
   Draft PR. The actual first-turn failure is not yet classified, rather than being
   misreported as vendor MCP incompatibility or success.

The runner now saves content-free progress before prompt submission and preserves
an original report for each attempt. Failed/auth-required children are allowed to
finish their existing reaping path instead of issuing Stop to a closed receiver.
Security gates must pass at the same clean SHA before any real prompt. Private
reports stay under ignored `artifacts/local-p1-i/`; actual credential paths, raw
conversations and private-home vendor files do not enter this document or PR.

The owner-authorized private authentication config was retained in a separate user-local
validation directory (HOME 0700, config 0600) outside the checkout. Cross-filesystem
retention used a private local kernel file copy, not credential inspection or output;
the original temporary copy and generated vendor sessions/logs were removed. Only the
key-containing config and ownership marker remain. Normal login state and the existing
ACP user database were not read or copied. Retaining this config does not reactivate
any lease or implicitly initiate a future model call.

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
| Rust / Python | 50 / 16 | 49 / 16 |
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

All pinned Cargo build/test/run gates use `--offline --locked`; local fmt/Clippy,
Python/parity, fingerprint and IPC/MCP guards preserve the existing core contract.
Only owned synthetic windows receive input/capture. UI source/layout and six
surfaces remain unchanged. P1-F base acceptance stays separate from the dedicated
Sessions visual review deferred by the owner at PR #25; no new acceptance is inferred.

## Remaining limitations

Real Agent tool-call acceptance remains open: original fixed prompt attempt FAIL /
incomplete evidence, initialize-only PASS, no verified real context/Handoff call.
The source remains a Draft for review; no issue completion or real compatibility is
inferred from deterministic success, discovery or session acceptance. A separately
approved follow-up can use the retained private config and saved-progress runner.

No native Windows real-agent model turn, real-model GUI prompt, macOS/mobile,
IME/screen-reader, clipboard or hardware acceleration proof. Optional web smoke
not selected; niri excluded by scope. No hosted Actions, automatic merge, real user
profile/database access, production schema change, external project writes or
operator credential forwarding. A Draft PR is source review, not production
isolation, all-vendor ACP/MCP certification or approval of a new framework/brand.
