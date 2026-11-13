# P1-M2/S3 — native operator workflow evidence

> **Historical record — not a current runbook.** Original dates, source revisions
> and outcomes below are preserved. S4 retired the executable prototype and probes;
> use [Rust-only development](../p1/rust-only-development.md) for current commands.

Authority: [Issue #46](https://github.com/ybakiame/agent-control-plane/issues/46),
parent [#40](https://github.com/ybakiame/agent-control-plane/issues/40), accepted
ADR-0004/0006, the unchanged [Profile v1 contract](../p1/profile-v1-runtime-contract.md)
and [S2 ownership contract](../p1/daemon-profile-ownership.md).
Delivery: `feat/p1-m2-s3-native-operator-workflows`, Draft for human review.
Date: 2026-11-13. Only fresh owned synthetic Profile v1 files and projects were used.
This does not activate a real Profile, retire Python or complete parent M2.

## Exact revisions

Base main is the merged S2 PR #45 commit
`a0bf8492f59160e7d30aea645aa22626a9e08c9c`.

| Commit | Scope |
| --- | --- |
| `dec76cc8c9bbf08eaef341e170833088322e379d` | Operator contract, preview semantics, UI gates and acceptance matrix, committed before product changes |
| `af6d417255416f4a163b750a217c0bfb720b66a4` | Shared data-only DTOs, native CLI, exact scoped read-only previews and boundary regressions |
| `fab338f0014616f9d4b414f39c23234165525fd3` | Shared operator state/review/retry model, NSH components and integration tests; fixes missing await in the new IPC test |
| **`a99b2189609d22f030378d2476e740f3115d4058`** | Final tested code: distinct operator/context status, current receipt identity and visible uncertain-result recovery |

Both full native validation runs used a **clean tracked worktree at a99b218**.
Later usage/status/evidence edits are documentation only; they are not relabeled
as tested product revisions. All branch commits use `[skip ci]`.

## Implemented scope and trust boundary

- `acp-query` supports explicit Profile create/select/info, independently confirmed
  write enable/disable, all seven typed mutations and exact operator previews.
  Complex Handoff input is a strict bounded typed file. Mutations require explicit
  profile/generation/epoch/operation ID; review/delete also require exact scope,
  record revision and confirmation. No profile discovery, startup grant or default DB.
- `acp-core::operator` contains data-only commands, bindings, receipts and previews.
  Existing store/writer imports reexport those types. Validation, transaction
  ownership and Writer request construction remain in `acp-write`; normal dependency
  inspection (`cargo tree -i acp-write -e normal --locked`) shows only `acp-daemon`.
  Clients do not obtain SQLite connections or link the writer.
- Operator-only preference pages (12 rows), exact preference and exact-task Handoff
  previews return binding/revision/content from one completely verified read-only
  snapshot. Global and individual project scopes are separate. Project previews
  retain independent allowlist and pinned-root checks. No confirmed-context-derived
  revision guessing, synthetic review queue or fabricated historical validation.
- The daemon denies all three previews to agent/session and OperatorWrite peers.
  MCP still has exactly the two project-bound read tools. No DDL, 4096-receipt cap,
  schema verification, writer admission, OS lease or revocation weakening was made.
- One shared NSH model drives Desktop/TUI Profile/grant inspection, explicit create/
  select/enable/disable, preference suggestion and exact-record confirm/reject.
  Prepared reviews and completions are guarded by target/project/task/epoch/scope/
  record identity. Disable completes after acknowledgement. Existing pages, themes,
  locales and Sessions shortcuts remain; operator commands use Alt+Shift modifiers.
- A submitted mutation with a lost reply retains its immutable request and operation
  ID. Retry is explicit and same-epoch only; changing scope never rebinds it. A new
  mutation clears the old receipt display. There is no automatic retry, durable
  client journal or cross-epoch replay promise.
- Registration, deprecate/delete and Handoff put are explicitly **CLI-only in S3**.
  The UI can inspect saved exact-task Handoff data, separately from current Git.
  Windows-to-WSL helper, offline synthetic and Web targets remain non-writing.

See [contract, exact syntax, canonical-root requirement and retry usage](../p1/native-operator-workflows.md).
Registration preserves the original canonical native root spelling. On Windows
the tested spelling is Rust's extended-length canonical root; plain/extended aliases
are not silently equated against mutation admission. The CLI never changes policy.

## Full local validation

| Host | Environment |
| --- | --- |
| Linux/WSL | Arch Linux, x86_64, WSL2 kernel 6.18.54.1-microsoft-standard-WSL2 |
| Native Windows | Windows 11 10.0.26300, AMD64, isolated owned checkout |
| Both | Rust 1.99.1 (58cf8f9c3), Cargo 1.99.1 (5f94df478), existing locked local caches |

Windows uses process-local MSVC/SDK/PATH and the existing owned validation tools.
No persistent system configuration changes or tool installations were made.

At clean **a99b2189609d22f030378d2476e740f3115d4058**:

| Command | Linux/WSL | Native Windows |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | PASS |
| `cargo build --workspace --bins --locked` | PASS | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | PASS |
| `cargo test --workspace --all-targets --locked` | **PASS, 107** | **PASS, 108** |
| `cargo fmt --manifest-path spikes/p1-e/shared/Cargo.toml -- --check` | PASS | PASS |
| `cargo clippy --manifest-path spikes/p1-e/shared/Cargo.toml --all-targets --features native --locked -- -D warnings` | PASS | PASS |
| `cargo test --manifest-path spikes/p1-e/shared/Cargo.toml --features native --locked` | **PASS, 32** | **PASS, 32** |
| `cargo fmt --manifest-path spikes/p1-e/nsh/Cargo.toml -- --check` | PASS | PASS |
| `cargo clippy --manifest-path spikes/p1-e/nsh/Cargo.toml --all-targets --features native,tui --locked -- -D warnings` | PASS | PASS |
| `cargo build --manifest-path spikes/p1-e/nsh/Cargo.toml --features native,tui --bins --locked` | PASS | PASS |
| `cargo test --manifest-path spikes/p1-e/nsh/Cargo.toml --all-targets --features native,tui --locked` | **PASS, 7** | **PASS, 7** |
| `git diff origin/main --check` | PASS | PASS |

There is one explicitly ignored **S1 manual performance measurement**, not an
ignored security/acceptance test. The untouched legacy comparison command
`PYTHONPATH=src python -m unittest discover -s tests -v` also passed **29** tests on
Linux/WSL. It is not a Rust runtime dependency or new Python functionality.

The workspace totals include **15 independent-process ownership groups** (13
retained S2 groups plus two CLI/preview groups), native IPC, store/writer and
MCP/session security. Relevant assertions cover:

- Fresh/no-overwrite creation and single-assignment selection; missing/legacy/unsafe
  refusal, lease contention, restart/epoch, strict file/root identity and revocation.
- Actual CLI receipts for all seven commands, strict transitions, atomic peer
  deprecation, exact Handoff put/read, expected revisions and changed-payload
  idempotency conflict. Refused operations preserve the complete logical state
  digest over all five Profile tables, including metadata and receipts.
- Scoped/paged preview bounds and exact revisions; malformed global/project scope
  fields (including null extras) refused; established agent/session/write peers and
  MCP denied administrative previews. Effective context remains confirmed-only.
- Explicit server errors preserved; a submitted mutation followed by a lost reply
  becomes OutcomeUnknown, rather than a fake definitive failure.

Shared-model tests cover stale success **and** failure, changed target/project/task/
scope/record, invalidated reviews/retries, immutable same-request retry, malformed
previews, disable acknowledgement and unavailable/non-native target refusal before
IPC. The NSH suite includes the retained **10,000-row / 834-page bounded mounting**
check and real component interaction over a native daemon. That component test is
headless; the native observations below are separate evidence.

## Actual Desktop and terminal observations

All processes below use actual compiled Rust daemons/CLI/NSH with fresh synthetic
Profile v1 files, not a mock backend. The recorded binary source is a99b218.

| Surface | Result and actual evidence |
| --- | --- |
| Windows Desktop | **PASS**: owned native window; explicit grant, suggestion, exact inspection/confirm/reject, acknowledged disable; lost-reply recovery, stale CAS Conflict, disconnect, English/zh-CN and compact resize |
| WSLg Desktop | **PASS**: actual `GdkWaylandDisplay`, `GskGLRenderer`, mapped window and GTK after-paint; same supported mutation/review, fault, keyboard, locale and resize sequence |
| Windows TUI | **PASS**: actual ConPTY; supported grant/suggest/confirm/reject/disable, keyboard, locale, compact resize and terminal restoration; lost reply, competing-owner Busy, revoked-grant Unauthorized and disconnect |
| Linux/WSL TUI | **PASS**: actual PTY; same supported workflow and extended fault sequence as Windows, including terminal restoration |

WSLg used the existing Wayland socket resolving into WSLg's runtime, not X11,
XWayland or Xvfb. No niri session was started/tested. Windows foreground protection
initially paused keyboard automation; it continued only after the user focused the
owned synthetic window and replied ready. No foreground checks were bypassed.
Screenshots of the owned windows were inspected, including exact review, uncertain
retry controls above the fold, and the 640×480 Chinese scrolling layout. These are
interaction/rendering checks, not a complete accessibility or IME certification.

Lost-reply checks use an **ignored test-only native local relay** inside the owned
private runtime. It forwards authenticated frames to the actual daemon, consumes
one actual committed receipt, then deliberately drops that reply. Explicit UI retry
returns that same receipt without an extra preference row or new operation ID.
The relay is not part of the product/workspace or a remote transport. Synthetic state
is created using the native CLI; private orchestration does not write SQLite.

The two Desktop runs exercise stale-record **Conflict**. The two extended terminal
runs exercise **Busy** with a second real daemon holding the same Profile lease,
then explicit release/re-enable; external grant revocation produces **Unauthorized**
without another row or a fallback. These fault cases are not claimed to have each
run on all four renderers. Stale async completions and target/project/task switches
are asserted in the shared-model tests, not claimed as OS-input races.

Private report provenance (all ignored; no raw logs/screenshots or profiles uploaded):

| Reports under `artifacts/local-p1-m2/` | Source/worktree |
| --- | --- |
| `s3-wsl-final/report.json`, `s3-windows-final/report.json` | Full suites, clean a99b218 |
| `s3-ui-wsl-2/report.json`, `s3-ui-windows-6/report.json` | Actual Desktop, clean a99b218 |
| `s3-ui-wsl-tui-3/report.json` | Actual PTY normal/lost-reply sequence, clean a99b218 |
| `s3-ui-wsl-tui-4/report.json` | Extended PTY fault sequence, a99b218 binaries, tracked documentation edits present; no product/test code changes |
| `s3-ui-windows-tui-3/report.json` | Extended ConPTY fault sequence, clean a99b218 |

## Resolved findings and remaining limits

Development runs exposed a permissive Serde unit-variant scope decoder, Sessions
shortcut collisions, a compact sidebar overlap, stale localized summary and unclear
operator/context connectivity and uncertain-result presentation. The final code
fixes these and retains the regression assertions. Initial Windows private fixture
failures were corrected by using protected native directory creation and the exact
canonical registration root; product path/ACL assertions were not relaxed. A private
terminal script now explicitly re-inspects binding after uncertain retry before
requesting another grant action, matching the model's invalidation behavior.
Earlier failed/incomplete runs are not counted as acceptance passes.

- **NOT_RUN:** hosted GitHub Actions, macOS, mobile, niri, model/vendor calls,
  clean-machine installation/release, real-profile activation and full accessibility.
- Higher-risk CLI-only UI gates, in-memory-only retry retention and read-only
  Windows-to-WSL transport remain deliberate S3 limits.
- Python runtime/package/tests/CI retirement, final M2 integrated acceptance and full
  M4 ReviewDecision/provenance remain separate tasks. Historical S1/S2 reports retain
  their original revisions/results; they were not rewritten as S3 evidence.
- No real ACP database, retained authentication directory, external development
  repository, model service or user device configuration was accessed or modified.

Delivery remains **Draft**, awaiting human review. Do not merge, mark Ready or close
parent #40 as part of S3.
