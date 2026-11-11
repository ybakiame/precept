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

## Results

Evaluation in progress. This K0 plan does not mark any trial or acceptance item PASS.
