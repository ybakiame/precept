# P1-M2/S3 — native operator workflows

Contract frozen before implementation for [Issue #46](https://github.com/ybakiame/agent-control-plane/issues/46),
parent [#40](https://github.com/ybakiame/agent-control-plane/issues/40).
Base: merged S2 main `a0bf8492f59160e7d30aea645aa22626a9e08c9c`.
Authority: accepted ADRs [0004](../adr/0004-product-domain-trust.md),
[0006](../adr/0006-rust-only-runtime.md), retained ownership in ADR-0005,
[Profile v1](profile-v1-runtime-contract.md) and [S2](daemon-profile-ownership.md).
Only disposable owned synthetic profiles are authorized. No DDL, receipt ceiling,
writer admission, session permission or MCP tool change.

## Native CLI

`acp-query --runtime-dir ABSOLUTE --endpoint ENDPOINT` retains current read/session
commands and adds:

- `profile create PATH`, `profile select PATH`, `profile info`.
- `profile write enable --profile-id ID --generation ID --confirm` and
  `profile write disable`. Create/select never implies enable; switching a selected
  daemon remains RestartRequired. No discovery, default path or daemon startup.
- `preferences list --scope global|project [--project NAME] [--after-id ID]`,
  `preferences get ID --scope ...`, and `handoff-record PROJECT TASK`.
- `project-register`, `preference-suggest`, `preference-confirm`,
  `preference-reject`, `preference-deprecate`, `preference-delete`, `handoff-put`.
  Every mutation requires explicit `--profile-id`, `--generation`, `--owner-epoch`
  and `--operation-id`. Review/delete require `--confirm`, exact ID/scope and
  positive `--expected-revision`. Suggest/register use Expected::Absent.
  Handoff requires exactly one of `--expected-absent` / `--expected-revision` and
  a bounded typed `--input PATH` JSON document (objective, completed, next_action,
  validation_json and snapshot only), at most 128 KiB, unknown fields refused.

Clients use protected native credentials, checking Operator for control/preview
and the independent exact OperatorWrite binding for mutation. They never link the
Writer or open SQLite. Typed serialization DTOs may be shared without moving
validation/transaction ownership out of the daemon. IPC stays v2: these are additive
operator methods, not a change to existing request/response meanings.

Success outputs bounded JSON; failures output sanitized enum errors with exit 3.
Mutation failures also identify the operation ID and uncertainty. No token, DB path,
raw diagnostics or request contents are echoed. All requests are single-attempt.
Timeout/disconnect after submission is conservatively OutcomeUnknown. The operator
must retain exactly the original arguments/input bytes and same binding/operation ID
for a deliberate same-epoch retry; no ID generation or automatic retry by the CLI.
Changed input with the same ID is IdempotencyConflict; a new epoch requires fresh
inspection, never rewriting the old operation to promise replay.

## Exact operator inspection

Three new Operator-only methods under `profile.*`, denied to agent/session/write
roles and unreachable through MCP:

| Method | Strict params | Result from one fully verified RO snapshot |
| --- | --- | --- |
| `profile.preferences.list` | scope, after_id (nonnegative; default caller supplies 0) | binding, up to 12 exact records ordered by ID, next_after_id or null |
| `profile.preferences.get` | scope, positive id | binding, exact record or NotFound |
| `profile.handoff.get` | project, task_id | binding, exact saved Handoff and revision, or explicit absent |

Scope is exactly `{kind:"global"}` or `{kind:"project",name:NAME}`; no union of
global/project records. Preference records contain ID, scope, key, value, source,
status and revision. Selection, record and revision are read in the same snapshot.
Project-scoped previews require the daemon allowlist and pinned canonical root;
global inspection requires Operator but does not pretend a selected project scope.
Lists are bounded pages, not a pending-review queue or a stable multi-page snapshot.
Handoff saved values and opaque historical validation remain distinct from the
existing resolver's current Git/worktree observations. context.get stays confirmed-only.

## NSH Desktop/TUI

Use one auxiliary operator panel in the existing shared Model and both renderers,
with existing tokens, English/zh-CN, compact scrolling and native keyboard focus.
Alt+O opens it (Alt+7 remains the existing Sessions start review); Escape cancels review and restores ordinary navigation. Existing
Alt+1–5, Sessions and user Windows screenshot shortcut remain intact.

The minimal UI supports explicit Profile create/select/info, grant enable/disable,
exact scoped preference list/inspection, suggestion and confirm/reject review.
Every grant/review submission names the exact target/binding/scope/record/revision;
confirming a preference discloses that same-key confirmed peers are deprecated.
No default approval, implicit reconnect grant or background writes. Disable completes
only after daemon acknowledgement. While busy, prevent duplicate submissions.

Registration, deprecate/delete and Handoff put are **CLI-only for S3**: they require
explicit root policy, destructive lifecycle or complex historical input review;
the UI labels this gate and does not offer nonfunctional controls. It can inspect
the exact task/revision through the new preview, alongside the existing Handoff page.
This gate is not implementation of the full later ReviewDecision/provenance engine.

The shared model captures target/project/task/request epoch plus exact observed
record and binding before review. Scope/record/target/project/task changes invalidate
review and late completions. Pending mutation retains its exact immutable request
and operation ID in memory; uncertain replies require deliberate same-request retry,
never silently constructing another request. Changing scope cannot apply an old
completion or rebind an old retry; after client closure use CLI with retained inputs.

Only explicitly applied native local targets can use these controls. WSL helper,
Web and offline/synthetic remain read-only, with no fallback IPC/SQLite/Python path.
Unselected native daemon state is observable independently of context loading.

## Acceptance matrix (both native Windows and Linux/WSL)

| Boundary | Required evidence |
| --- | --- |
| CLI lifecycle | real daemon + CLI create/select/info, RO default, enable/disable, strict missing/old/unsafe/restart refusal |
| Seven commands | actual CLI receipts; scope/CAS/transitions/atomic peer deprecation/root policy/Handoff; malformed and denied full logical-state equality |
| Preview | exact scope/paging/revision coherent binding; global vs other project isolation; control-only on established peers and MCP denial |
| Retry | same-op lost reply then receipt equality and unchanged counters; changed payload conflict; stale epoch/revocation refusal |
| Shared UI | stale success/failure, rendered review invalidation, target/project/task/record switches; denied/Busy/Conflict/OutcomeUnknown; WSL/offline never write |
| Desktop/TUI | real native owned daemon/Profile, each supported action, confirmations, Escape/focus/keyboard, locales/compact; separate actual backend/TTY evidence |
| Regression | workspace fmt/build/Clippy -D warnings/all-target tests; affected shared/NSH fmt/Clippy/tests; S1/S2 IPC/MCP/session/lease/epoch/security |

Record exact clean code SHA, commands and PASS/FAIL/BLOCKED/NOT_RUN per host.
Headless interaction tests do not imply native GUI/Wayland/TTY PASS. Raw private logs
stay ignored; no real data, credentials, vendor calls, niri/mobile/macOS or Actions.
Use [skip ci] commits and one Draft PR, Closes #46 / Refs #40. Never merge or close #40.
