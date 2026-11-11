# P1-K: cross-agent context and Handoff interoperability

## K0 plan and authorization

Issue #31 starts at merged P1-J main `8e979ba` (2026-11-11), following P1-I merge
`14798f2`. Dedicated branch: `feat/p1-k-cross-agent-interop-evidence`.
The merged charter preserves the existing two read-only MCP tools and optional
single ACP session. ADR-0003 now accepts NSH as the framework direction; this does
not upgrade historical platform evidence. ADR-0004 remains Proposed and introduces
no implemented evidence schema. This evaluation changes no product/UI/authority.

All trials use disposable synthetic repositories and SQLite state. No normal client
configuration, user profile, real project, credential history or device identity is
copied. No installs/upgrades, broad execute/shell grants, paid fallback, automatic
prompt retries, hosted Actions, remote branch deletion or automatic merge.

The owner separately authorized **one** Kimi `kimi-for-coding` subscription prompt
for this stage, allowing only the two read-only MCP tools. Tool round trips may use
multiple model HTTP requests; exact provider cost/request count is uninstrumented.
Codex starts with deterministic app-server-to-MCP calls, without inference/auth.
OpenCode modes will be inspected without a model; Code Mode remains blocked where
general execute cannot be safely excluded. Existing failed I attempts remain intact.

| Installed client | Direct MCP plan | Managed ACP plan | Identity / permissions |
| --- | --- | --- | --- |
| Codex CLI 0.167.1 | Native app-server direct tool calls, independent stdio observer, no model | No native acp subcommand in installed help; no adapter installation | Isolated HOME/CODEX_HOME, read-only thread, no auth/inference |
| Kimi Code 2.5.2 | Separate mode NOT_RUN unless safely observable without spending the ACP authorization | One opted-in synthetic Handoff resume and daemon counts, then Stop denials | `kimi-for-coding`, manual, exactly two MCP tools, bounded allow-once |
| Pi | NOT_RUN: no upstream pi executable found in PATH | NOT_RUN: same absence | Installed Oh My Pi 18.16.0 is distinct and not substituted |
| OpenCode 2.2.7 | Inspect native API/discovery without a model before claiming unsupported | Keep Code Mode execute restriction; session acceptance is not a tool pass | No user config; no provider/model call authorized for K |

The installed `opencode2` executable also reports 2.2.7 with a different hash; it is
not silently substituted for `opencode`. Exact entrypoint hashes and modes will be
reported with the final per-attempt source provenance. CLI help/catalog observations
alone establish neither working forwarding nor successful tool use.

## K1/K3 acceptance design

Use a synthetic retry-policy investigation with a recorded objective/next action and
explicit historical PASS/FAIL/SKIPPED/NOT_RUN records. Read it through installed Codex,
then edit only the owned synthetic repository to retain HEAD/dirty count while changing
content, switch branch and advance HEAD. Compare actual server-returned freshness
warnings and unchanged historical validation records. A Kimi receiver then reads the
same stale handoff on the changed checkout and reports a bounded stale/historical
classification; daemon counters establish actual reads, not model obedience.

Each claimed PASS requires trusted server-side observation: a content-free stdio
observer for static direct MCP clients, existing daemon counters for managed ACP.
Observer output must omit raw arguments, IDs, conversations, credentials and paths;
only allowlisted operation/result labels, counts and returned-data digests are kept.
Operator equality/policy checks independently verify confirmed preferences, scoped
optional docs, task/project isolation and unchanged fixture DB bytes. Managed sessions
must deny established peers and saved-secret reconnects after Stop. Historical Kimi
I evidence is linked as historical until a current authorized run exists.

## Results and exact per-attempt source provenance

The table above preserves the pre-implementation K0 plan. The following results are
current K observations on local Linux/WSL (Kimi/Codex resume at clean `757eac3`,
final no-model Codex/OpenCode probe at clean `a26825f`), not renewed claims for every platform or
provider. Installed binaries were not upgraded or substituted. No product defect was
reproduced, and no production source, schema, permission or UI was changed.

| Client / mode | Tool discovery | Independently observed calls | Result and scope |
| --- | --- | --- | --- |
| Codex 0.167.1 / direct MCP | Exact two tools | 1 context + 5 successful Handoff reads; 7 negative calls independently observed as tool/RPC errors | **PASS**, installed local app-server MCP invocation, no model turn |
| Codex / native managed ACP | No native `acp` subcommand in installed CLI help | None | **UNSUPPORTED** selected native interface; external adapters not installed or evaluated |
| Kimi 2.5.2 / direct CLI MCP | Not separately exercised in K | None in this mode | **NOT_RUN**; the one authorized prompt was reserved for managed ACP |
| Kimi 2.5.2 / managed ACP v1 | Agent connected to configured two-tool bridge | Daemon counters: context 1 / Handoff 1 | **PASS**, one `kimi-for-coding` subscription prompt, stale resume and post-Stop denial |
| Pi / either mode | No upstream `pi` executable in PATH/inspected bin locations | None | **NOT_RUN**, absent selected client; Oh My Pi is not substituted |
| OpenCode 2.2.7 / configured direct MCP | CLI accepted isolated config but reported no configured servers; observer saw no frames | None | **NOT_VERIFIED** discovery; actual calls **NOT_RUN**, no model opt-in and no native direct tool-call API found |
| OpenCode 2.2.7 / managed ACP v1 | Native session accepted; discovery alone not promoted to a tool pass | Context 0 / Handoff 0, no prompt submitted | **BLOCKED** for model tool use under the current Code Mode/execute policy; session/Stop subchecks pass |

Codex calls were explicitly submitted by the evaluator through installed
`app-server --stdio`, `mcpServerStatus/list` and `mcpServer/tool/call`. There was no
`turn/start`, model credential or model request. Its provider was the deliberately
non-functional `local_validation` endpoint `http://127.0.0.1:9/v1`; this is a client/
transport PASS, not evidence that a Codex model autonomously chose the tools.
The [official MCP configuration reference](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)
describes stdio configuration; the experimental invocation API was checked by the
actual installed app-server calls, rather than inferred from that documentation.
The isolated thread used read-only sandbox and `approvalPolicy=never`. No normal
Codex configuration, login state or history was copied.

Kimi identity was verified as `Kimi Code CLI 2.5.2`. The operator-selected private
config used `kimi-for-coding`, manual mode and only the two MCP tools. The evaluator
selected allow-once for two exact vendor tool-name/content requests, with no
persistent grant. As in P1-I, Kimi omits arguments from ACP permission requests;
parameter-level approval is not claimed. Daemon/MCP independently enforce project,
role and lease scope. One application prompt completed `end_turn`; provider HTTP
request count and exact subscription usage were not instrumented. No automatic
application prompt retry occurred and no other provider/model was invoked in K.

### Exact executable/source provenance

Hashes are SHA-256 of the selected executable/entrypoint bytes, not a signature,
attestation or hash of all transitive dependencies. The real-run report records them
before launching clients; the initial installed inventory agrees.

| Selected component | Version | SHA-256 |
| --- | --- | --- |
| Codex CLI | 0.167.1 | `dd5bfdf19d794dcc6fb482d2d5a2d9fb31d11f6caa44b303bb72890d461e8a02` |
| Kimi Code | 2.5.2 | `28b0ff2e18ce35ba6e508ea50a5726095919ab6fd448682ea57070e1e3879875` |
| OpenCode | 2.2.7 | `589253d36810f21217edd51545cd435b328cdc3cf4f451fbe0cce6f88afcf5e1` |
| Native daemon | 0.1.0 | `599dbf02ac5a3644aed15f18f3c0251ff68c7580b21c2a7be44731e233fab61c` |
| Native MCP bridge | 0.1.0 | `d761861f928f599c1c6acd64be09964e75dbe8428fa0d4eb6664c92f42c330db` |

Private attempts are distinct and never overwritten:

| Attempt / ignored artifact | Source SHA | Outcome |
| --- | --- | --- |
| `codex-1` | `65348a389d6764bccf860813754a5dc695d4fdd8` | Evaluation FAIL in fixture setup: Python CLI module path omitted `.cli`; no client/model turn |
| `codex-2` | `e5ae4e80b2e72ff8667eae3139380c2bd3d7a5d7` | Codex direct MCP PASS; Kimi NOT_RUN |
| `cross-agent-resume-1` | `757eac3f44614405c1dbf50e550377b604eccf84` | Codex direct PASS and Kimi managed resume PASS in the same synthetic fixture |
| `opencode-current.json` | `757eac3f44614405c1dbf50e550377b604eccf84` | Direct discovery NOT_VERIFIED, no model calls; ACP startup/Stop subchecks pass |
| `opencode-current-2.json` | same clean SHA | Direct explicit-connect attempt failed before bridge launch; ACP subchecks again pass; neither result is a tool-call PASS |
| `final-no-model` | `a26825f7df51f081d543949f8598515525d79e43` | Public runner confirms Codex direct PASS, OpenCode config declared but no discovery; ACP/Stop subchecks pass with zero reads |

The first setup failure was fixed before consuming the one Kimi authorization.
Content-free stage diagnostics were added so future setup failures are not mistaken
for client incompatibility. Ad-hoc local debugging made no model calls. Historical
P1-I Kimi and OpenCode successes/failures remain at their original source revisions
in [P1-I evidence](p1-i-context-bridge.md); K does not relabel or overwrite them.

### Independent call trace and policy checks

For Codex, [the test-only stdio observer](../../tools/interop_trace.py) sits between
the installed MCP client and the real native `acp-mcp`. It forwards bounded frames
and correlates actual server responses with requests. It records only sequence,
allowlisted method/tool/result labels, exact-two-tool inventory, canonical returned-
data SHA-256, known freshness labels and wire validation statuses. Raw IDs,
arguments, credentials, paths, documents, messages and unknown error text are not
persisted. Unknown labels become `unknown`, not user-controlled strings. Negative
Python regressions cover secret-marker exclusion, correlation and preservation of
historical FAIL/NOT_RUN statuses without promotion.

The final Codex trace contains one exact two-tool inventory, one successful context
read and five successful Handoff reads (four revision/worktree states plus legacy
fingerprint absence). Each successful result digest matches the independently checked
operator result; none is inferred from agent text. Seven negative requests cover
project overrides, beta-only tasks, malformed task type and preference/session/
project mutation or enumeration. All seven reached the observed server and returned
four tool errors and three unknown-tool RPC errors. Codex also probed unsupported
MCP methods (two RPC errors), consistent with the documented small MCP subset;
those are not successful tool reads or full protocol conformance.

The confirmed-only/project isolation check compares context with native operator
output and asserts exclusion of beta project markers and suggested/rejected/
deprecated preference markers. Optional document bodies were requested explicitly;
context included the synthetic project's allowlisted contract, with no second root.
Fixture DB bytes remained identical after all client reads and negative requests.

For Kimi, the existing daemon-owned `session.context` audit independently records
exactly one successful read of each tool at the live project/agent/generation. The
operator/checker establishes stale fixture state before the prompt and verifies
unchanged worktree and validation data afterward. Daemon counts establish reads,
not model comprehension or compliance with all returned rules. The bounded model
response contained `HANDOFF_STALE_CHECKS_HISTORICAL`; it is recorded only as a boolean
agent-reported classification alongside the independent state checks.

After Kimi Stop, a previously authenticated peer received Unauthorized and a new
handshake using the saved ephemeral secret was denied. The child was reaped, lease
file removed and DB unchanged. Current OpenCode no-prompt sessions also passed those
lifecycle checks with zero context/Handoff reads. These results preserve P1-I's
managed lease contract; static non-session Codex MCP credentials do not acquire
revocation semantics by implication.

### Realistic synthetic Handoff acceptance

The scenario concerns bounded retry scheduling after cancellation. An operator uses
the existing Python writer to capture objective, completed investigation and next
action: inspect the changed cancellation behavior and rerun affected checks, without
permission to edit. A client cannot write Handoff through MCP. Four **scripted,
agent-reported historical** checks retain PASS/FAIL/SKIPPED/NOT_RUN, source revision,
synthetic environment and provenance text. They are not actual current test results
or a new typed evidence schema. Verification establishes preservation and freshness
semantics, not that these scripted claims happened.

In `cross-agent-resume-1`, the saved source revision was
`e80f716cf38a939869ff283991b1e94ae2463fbb`, branch `master`. Codex first read the same
captured dirty worktree: no fingerprint-change warning, but dirty and historical
warnings remained. A deliberate owned-fixture content edit retained HEAD and dirty
status while changing the fingerprint; a subsequent branch switch and commit tested
branch/HEAD drift. A final pending edit preceded the different-client Kimi resume.
The receiver's current fixture revision was
`eb416491c4817502be02d3b988306022cefda8c5`, branch `synthetic-review`. Warning categories
were HEAD, branch, dirty, fingerprint and historical. Saved/current fingerprints were
`5bb172bf883a9106b69980a05d41d39fc3d0fc6f361066290c666bff66a8b811` and
`0bbcd2a7d2c6dd03ce001679e2a6af15558fcb54ce2868c399d8486b008e52dd`.
These synthetic source identifiers were retained from the actual returned Handoff
metadata; authority for Kimi's read is independently established by daemon counters,
not upgraded from the vendor record.

Codex and Kimi read the same operator-captured task across the change. Both actual
server behavior and the independent checker retain historical checks and identify
drift; Kimi's bounded stale classification is separate model output. No task execution,
repository edit by either model, production profile or real project was required.
Legacy null-fingerprint data is explicitly unknown, not matching. BLOCKED/UNSUPPORTED
remain evaluation outcomes and were not added to serialized Handoff status enums.

### OpenCode and Pi limits

OpenCode's installed native CLI exposes configuration/status commands and the ACP
server. The inspected v2.2.7 [native MCP API group](https://github.com/anomalyco/opencode/blob/v2.2.7/packages/protocol/src/groups/mcp.ts)
exposes add/remove/connect/status
and resources, not a direct operator tool-call endpoint. Direct discovery attempts
used only disposable HOME/config and `mcp list --standalone`, then an explicit
native `experimental.mcp.connect`; no bridge frame was observed. The config appeared
in a separate isolated `debug config` probe, but discovery did not succeed. This is
an unresolved client/configuration observation before product MCP launch, not a
reproduced product defect or proof that all direct OpenCode integrations fail.
A direct model test with supported per-server `codemode:false` may be evaluated under
separate future authorization; it was not silently folded into the Kimi allowance.

Managed ACP replaces the server's config with its standard ACP stdio entry, leaving
OpenCode's [default Code Mode registration](https://github.com/anomalyco/opencode/blob/v2.2.7/packages/core/src/tool/mcp.ts). Under K2, this route remains BLOCKED without a safe
narrow alternative to broad execute. No fixed product server entry, permission or
client login config was changed to force a pass. The earlier official DeepSeek turn
with zero MCP reads is still not a successful MCP integration. The owner's previously
demonstrated regular WSL TUI free-model response does not prove this isolated
managed MCP path; K made no quota retries or new OpenCode model calls.

Pi absence is limited to PATH and inspected installation locations. Oh My Pi's
installed entrypoint reports `omp/18.16.0`, but a fork's version/docs cannot prove
upstream Pi compatibility. No agent package was installed, upgraded or reconfigured.

## Reproduction and local checks

Build native binaries locally with `cargo build --workspace --bins --offline --locked`.
Run deterministic direct-client trials with:

```sh
PYTHONPATH=src:tools python tools/evaluate_cross_agent.py --out artifacts/local-p1-k/new-codex-attempt
```

To repeat the no-model OpenCode discovery/lifecycle probe, append
`--opencode-executable <absolute-installed-opencode>`; it uses a separate temporary
HOME and never submits a prompt. This option does not authorize a model call.

For the managed Kimi resume, first obtain the stage-specific provider/model/cost
opt-in and prepare a separate owner-selected private HOME. Run same-clean-source
negative security gates, then append these arguments to the command above:

```text
--kimi-home <private-home> --kimi-executable <absolute-installed-kimi>
--allow-kimi-subscription-turn --security-report <same-sha-local-report.json>
```

The report requires PASS for `root-clippy`, `lease-unit` and `context-process`, with
matching clean Git SHA; it submits no automatic prompt retries. Existing I tools
supply the gates. Use a new attempt directory each time. The runner now also checks private config metadata for the authorized Kimi model,
official endpoint/type, manual mode, exact two-tool allowlist and disabled telemetry,
title and skill merging. A negative case rejects provider/model-policy switches and
placeholder keys. This guard was added after the one real run. A separate post-run
config and vendor model-metadata check confirmed the original run used the authorized
`kimi-for-coding`; it is not misreported as a guard already present at `757eac3`.
Private auth is retained only as explicitly authorized; generated vendor state is
removed after reaping. Do not
reuse a normal client HOME or copy its auth/history/device identity.

Native security checks at `757eac3`: workspace Clippy PASS, five IPC unit cases PASS
and 14 context/revocation process cases PASS. Python initially passed 21 checks on
Linux/WSL and native Windows at that source. A subsequent helper-only provider guard
adds one case; all **22 Python checks pass on both hosts at clean
`4ec40ed689488e3aabda255632a54ceb55c0683f`**. No product source changed and no native
Windows client/model call is claimed. No full Rust/UI/10k/ConPTY/Wayland matrix,
macOS/mobile/IME/AT or hosted Actions was rerun for this evaluation-only change.
Prior product platform evidence remains historical at its original revisions.

## Privacy and acceptance limits

All calls use disposable synthetic data. No normal user DB/config, private repository,
real task, device identifier, credential value, raw conversation or screenshot is
published. Runtime secrets/capability paths stay private. Ignored local reports and
content-free traces live under `artifacts/local-p1-k/`; original failed reports remain.
Same-user filesystem access can alter helpers, files or traces, so observer digests
are not signatures or tamper-resistant attestations. No OS sandbox, all-vendor MCP/
ACP certification, automatic rule review, profile mutation or new product API is
claimed. Model output does not become independent evidence by passing through the
daemon. Draft PR only; no automatic merge or remote branch deletion.
