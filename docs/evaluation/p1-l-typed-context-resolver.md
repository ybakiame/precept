# P1-L — typed context resolver implementation and local evidence

> **Historical record — not a current runbook.** Original dates, source revisions
> and outcomes below are preserved. S4 retired the executable prototype and probes;
> use [Rust-only development](../p1/rust-only-development.md) for current commands.

Date: 2026-11-12. Authority: [Issue #34](https://github.com/ybakiame/agent-control-plane/issues/34)
and [Accepted ADR-0004](../adr/0004-product-domain-trust.md).
One PR contains L0, L1 and L2; Draft review and owner merge remain separate.

## Source and implementation

- Started from clean main `e9b56c53c26fca1e77e49c70c2aa1afb68e720e2` on
  `feat/p1-l-typed-context-resolver` after verifying the merged P1-K heads.
- L0 contract frozen at `b0fa516`: [typed-context-v1.md](../p1/typed-context-v1.md).
  It defines types/DTOs, exact selector/path grammar, three-valued matching plus
  invalidity, review/source gates, authority, finite-value compatibility/narrowing,
  explicit supersession, ordering, conflicts/actions and serialized examples.
- L1 implementation and **both hosts' clean L2 source**:
  `22b6ffc31bf224f36272c448e333c615cc2bbe52`, `worktree_dirty=false` at gate startup.
  Subsequent report/roadmap documentation does not change executable source.

The additive [`acp_core::typed_context`](../../crates/acp-core/src/typed_context/mod.rs)
API resolves explicit values into `EffectiveContextV1`. Types and resolution are
separate modules. The new module imports only its value types and ordered standard
collections; it performs no I/O, source inspection, transport or session operation.
It does not grant capabilities or authenticate source/review labels.

The finite categorical value model makes compatibility explicit: contracts must
have a nonempty intersection, session additions must be subsets of contract/policy
limits, and incompatible equal-authority rules remain unresolved. No timestamp,
path specificity or arbitrary priority selects a contract winner. A relevant
unknown/invalid input blocks the affected key while unrelated keys remain visible.
Query uncertainty and independently stale source state can both be explained.
Unmatched invalid/stale observations remain diagnostic, rather than becoming facts.

The existing `ContextResolver`, Python writer, legacy fixture, SQLite schemas,
Handoff validation, IPC/session APIs and two MCP tools are unchanged. The only edit
to the existing core implementation is an additive module export. New explanation
records contain operator review information and are **not wired into either agent
tool response**. No dependency, parser, new storage writer or UI change was added.

## Focused resolver regressions

[`typed_context.rs`](../../crates/acp-core/tests/typed_context.rs) supplies 15
table-driven/adversarial cases on each host:

| Area | Independently checked behavior |
| --- | --- |
| Selector matching | AND/OR across all five dimensions; exact path/case; absent vs missing vs known empty; false vs unknown; invalid lists/paths never widen a match |
| Scope and registration | Unknown project/root and ambiguous registration fail; foreign project/root/session rule, fact and policy contents/source references are omitted |
| Review and personal fallback | Suggested/rejected/deprecated/unaccepted exclusions; project masks global; newest confirmed legacy ID within scope; incompatible colliding IDs remain unresolved |
| Authority and narrowing | Contract masks contradictory preference with both references retained; compatible intersections; total-intersection conflicts even when pairs overlap; session-vs-contract/policy refusal |
| Source and repository state | Drift, stale/unavailable/unknown/malformed/unsupported source states; unverified observations never become repository authority; unrelated context survives |
| Revision and malformed records | Identical deduplication, divergent duplicate identity, explicit same-domain supersession, ineligible/new invalid revisions, invalid schema/shape/provenance, unknown serialized fields/enums/types |
| Determinism/provenance | Equivalent permutations produce identical JSON across rule, selector, fact, policy and multi-conflict collections; exact source versions/digests and agent-reported historical PASS remain attributed and historical |

Runtime validation uses explicit results/branches. Assertions are test expectations,
not executable authorization gates. Serde syntax/type errors are separate from
per-record resolution diagnostics; future transport adoption must sanitize parse
errors and separately review/version any wire projection.

## Local hosts and invocation

| Host | OS / architecture | Toolchain |
| --- | --- | --- |
| Linux/WSL | Arch Linux, WSL2 kernel `6.18.54.1-microsoft-standard-WSL2`, x86_64 | Rust `1.99.1 (58cf8f9c3 2026-10-13)`, Cargo `1.99.1 (5f94df478 2026-08-27)`, Python 3.14.9 |
| Native Windows | Windows 11, AMD64; owned disposable test checkout | Same Rust/Cargo versions, Python 3.14.5 |

All commands ran locally against synthetic state. Cargo used `CARGO_NET_OFFLINE=true`
and `--locked`; cached dependencies were sufficient. The private orchestration
environment set `PYTHONPATH` to the checkout's `src` and `tools`, and
`ACP_TEST_PYTHON` to the selected local interpreter. The command table spells that
interpreter `python` to avoid publishing its user-specific absolute path; the exact
argv/environment labels, timing, exit status and full logs are retained privately.
Windows SDK/MSVC variables were initialized only in the test PowerShell process.
No system configuration, regular project or normal user database was changed.

## L2 command/results matrix

Each final gate below ran once per host on clean `22b6ffc`; there was no full UI or
model/platform matrix repetition. PASS counts are from actual logs, not inferred
from compilation or another host.

| Command | Linux/WSL | Native Windows |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | PASS |
| `cargo test --workspace --all-targets --locked` | PASS, 65 tests | PASS, 64 tests |
| `cargo build --locked --example contract_reader` | PASS | PASS |
| `cargo build --locked -p acp-store --example sqlite_reader` | PASS | PASS |
| `python -m unittest discover -s tests -v` | PASS, 29 tests | PASS, 29 tests |
| `python tools/check_p1_parity.py` | PASS, 120 comparisons | PASS, 120 comparisons; Windows reader below |
| `python tools/check_sqlite_parity.py` | PASS, 78 comparisons / 13 scenarios | PASS, 78 comparisons / 13 scenarios; Windows reader below |
| `cargo run --locked -p acp-rehearsal --example synthetic_rehearsal -- --synthetic-only` | PASS | PASS |
| `cargo build --workspace --bins --examples --locked` | PASS | PASS |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | PASS | PASS |
| `python tools/test_local_service.py -v` | PASS, 11 tests / 132 MCP response comparisons | PASS, 11 tests / 132 MCP response comparisons |
| `python tools/test_agent_session.py -v` | PASS, 18 tests | PASS, 18 tests |
| `python tools/test_session_context.py -v` | PASS, 14 tests | PASS, 14 tests |
| `cargo test --locked -p acp-rehearsal --features recovery-tests --test process_recovery -- --nocapture` | PASS, 11 tests | PASS, 11 tests |
| `git diff --check` | PASS | PASS |

Windows parity argv explicitly appended
`--reader target/debug/examples/contract_reader.exe` and
`--reader target/debug/examples/sqlite_reader.exe`, respectively. All other command
arguments are as shown. The one-count workspace difference is the existing
Unix-only escaping-symlink test in `read_only.rs`; it is not a Windows PASS.
The new typed-context suite runs all 15 cases on both hosts.

Legacy JSON parity covers confirmed-only preference resolution, documents,
malformed/scope-changing calls, Handoff freshness and unchanged validation metadata.
SQLite parity includes legacy/current schemas, WAL visibility and failure cases.
The local-service suite independently compares native MCP replies with Python across
three schemas and six real bridges, including exact fingerprint/Handoff warnings.
Session/IPC suites cover permission rejection, root drift, expiry, Stop/Cancel,
held peers/reconnects, generation changes and restart cleanup. Managed ACP tests use
the repository's synthetic fake agent, not an installed vendor/model service.
Synthetic rehearsal/recovery exercises only owned temporary DBs and backups; it
does not roll out a production migration.

During development, the first core Clippy pass failed on five unnecessary
clone-to-singleton-slice expressions. They were replaced with `slice::from_ref`
before `22b6ffc`; the original development failure log remains private and is not
reported as a PASS. A private runner invocation with an abbreviated expected SHA
was rejected by its clean-source preflight before any gate; the full-SHA runs above
are the retained final attempts.

Private artifacts: `artifacts/local-p1-l/wsl-1/`, the companion `windows-1/`,
`development/` and `run_validation.py`. They are ignored, not uploaded. The logs
contain local diagnostics/paths; this report publishes only synthetic results,
source identifiers and sanitized host/toolchain metadata.

## Acceptance and limits

The initial local L2 results above belong to `22b6ffc`. The source review found an
explanation defect not covered by those 15 original resolver cases; the corrected
behavior and follow-up checks are recorded below. The final diff inspection verifies no
Python/store/MCP/IPC/session implementation, legacy fixture, Cargo dependency/lock
or UI source changed. Review remains a Draft PR; no automatic merge or issue close
has occurred. M–R remain gated briefs in the [roadmap](../roadmap.md).

Hosted GitHub Actions, macOS/mobile, native Wayland/GUI/TUI/10k interaction tests and
real-agent/model turns are **NOT_RUN for P1-L**. Existing platform and P1-K vendor
evidence retains its original source/provenance; it is not renewed by these tests.
No API key, authentication HOME or external development repository was needed.
This additive library is not full MCP certification, a generic policy language,
OS enforcement, a durable snapshot or a cryptographic evidence implementation.

## PR #35 review follow-up — unresolved authorities cannot resolve a mask

[Review 5866647760](https://github.com/ybakiame/agent-control-plane/pull/35#pullrequestreview-5866647760)
at `4516bad` identified three instances of the same explanation defect. The old
resolver withheld effective values but could still mask a preference as resolved
using an empty contract/constraint intersection, a drifted observation or one of
several disagreeing observations. The old test results are retained at their
original source; they did not establish correctness for these missed scenarios.

Fix `495e197c8ffbd43fcaa87dd274d1b75f551ec540` defers all authority-based preference
masking until the key's full contract/constraint/policy/observation analysis has
completed. Any relevant unresolved conflict, drift or unknown source keeps the
remaining preference candidate conflicted, with no resolved authority mask.
Nonempty compatible authority sets still mask legitimate lower-authority choices.
Consistent observations retain every contributing source in the resolved conflict.
Independent legacy fallback/supersession and unrelated keys remain unchanged. The
[v1 contract](../p1/typed-context-v1.md) clarifies this invariant without changing
DTO fields, enums, wire/schema versions, dependencies or authorization.

Three new table-driven review regressions first **failed against the pre-fix
resolver** (`Masked` instead of `Conflicted`), then passed after the fix. Together
they cover 13 negative scenarios: empty/competing/stale/unknown contract sets,
empty/competing constraints, contract/policy conflicts and unknown policy, repository
drift against an agreeing contract/preference or constraint/preference, disagreeing
facts and an unknown second observation. They check stable input permutations,
absence of resolved authority claims, retained affected-key uncertainty and an
unaffected selected key. The new positive case uses two consistent observations
and verifies all source references; the original positive single-observation case
also remains. The typed resolver suite now has **18 tests**.

The following narrower checks ran on **clean `495e197`, dirty=false, on both local
hosts**. Host/toolchain versions are unchanged from the table above. Cargo remained
offline through `CARGO_NET_OFFLINE=true`; Python children used `PYTHONPATH=src`.
`python` below denotes the same selected local interpreter as above.

| Exact command | Linux/WSL | Native Windows |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | PASS |
| `cargo clippy -p acp-core --all-targets --locked -- -D warnings` | PASS | PASS |
| `cargo test -p acp-core --all-targets --locked` | PASS, 30 tests | PASS, 29 tests |
| `cargo build --locked --example contract_reader` | PASS | PASS |
| `python -m unittest discover -s tests -v` | PASS, 29 tests | PASS, 29 tests |
| `python tools/check_p1_parity.py` | PASS, 120 comparisons | PASS, 120 comparisons; appended Windows reader argv as above |
| `git diff --check` | PASS | PASS |

Both core totals include all 18 typed resolver cases; the Unix-only legacy symlink
case accounts for the one-test host difference. Package tests also cover the
unchanged legacy core/fingerprint behavior. The correction is confined to the
additive pure resolver and its tests; legacy implementations are unchanged. Full
workspace/SQLite/IPC/session/recovery and GUI/vendor/platform matrices were **not
repeated** for this follow-up. Their earlier evidence remains pinned to `22b6ffc`,
not promoted to this correction's source. Hosted Actions remain NOT_RUN.

Ignored follow-up evidence lives in `artifacts/local-p1-l/review-red.log`,
`review-green.log`, `review-wsl-1/`, `review-windows-1/` and `review_validation.py`.
No raw logs or local paths were uploaded, and no auth/model call was needed.
The corrected candidate remains Draft for re-review; the fixes are not merge approval.
