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
  and `--operation-id`. Review/delete require `--confirm`, exact ID/scope,
  positive `--expected-revision` and `--proposal-digest` (64 lowercase hex).
  Suggest requires `--origin` (closed set: agent_reported, daemon_observed,
  independently_verified, legacy_unknown) and uses Expected::Absent.
  Register uses Expected::Absent.
  Handoff requires exactly one of `--expected-absent` / `--expected-revision` and
  a bounded typed `--input PATH` JSON document (objective, completed, next_action,
  validation_json and snapshot only), at most 128 KiB, unknown fields refused.
- `review-queue --scope ... [--after-id ID]` and
  `review-inspect ID --scope ... --revision N` (S1 additive Operator reads).

Clients use protected native credentials, checking Operator for control/preview
and the independent exact OperatorWrite binding for mutation. They never link the
Writer or open SQLite. Typed serialization DTOs may be shared without moving
validation/transaction ownership out of the daemon. IPC transport stays API_VERSION 2;
the write envelope `Request.version` is 2 (S1 breaking change, no v1 reader).

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
| `profile.review.queue` | scope, after_id (nonnegative) | binding, commit_sequence watermark, up to 12 suggested records ordered by ID, next_after_id or null |
| `profile.review.inspect` | scope, positive id, positive revision | binding, commit_sequence watermark, exact record (Conflict if revision differs), up to 8 recent decisions, projected peer demotions |

Scope is exactly `{kind:"global"}` or `{kind:"project",name:NAME}`; no union of
global/project records. Preference records contain ID, scope, key, value, source,
status, revision, origin, proposal_digest (64-hex), created_at and updated_at.
Selection, record and revision are read in the same snapshot.
Project-scoped previews require the daemon allowlist and pinned canonical root;
global inspection requires Operator but does not pretend a selected project scope.
Lists are bounded pages, not a stable multi-page snapshot. The dedicated
`profile.review.queue` filters to `status='suggested'` and discloses a
`commit_sequence` watermark; it is still not a held cross-page snapshot.
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

The M2/S3 baseline kept registration, deprecate/delete and Handoff put CLI-only.
M4/S2 ([Issue #57](https://github.com/ybakiame/agent-control-plane/issues/57))
supersedes only the deprecate/delete gate: the shared native UI adds exact
Queue/Detail/History inspection, peer disclosure, explicit deprecate/apply and
exact-ID retype/delete/apply. Registration and Handoff put remain CLI-only.
See the [current app keyboard flow](../../apps/README.md#review-keyboard-flow).

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

## Local validation usage

Build native workspace binaries first; NSH is a separate manifest:

```text
cargo build --workspace --bins --locked
cargo build --manifest-path apps/ui/Cargo.toml --features native,tui --bins --locked
```

Use a fresh owned synthetic checkout and an existing private Profile parent (0700
on Unix; protected current-user owner/DACL on Windows). Do not substitute a real
ACP DB. The CLI never creates a default parent or repairs permissions. Start the
daemon explicitly with the selected private runtime/endpoint, `--allow-project
alpha` and independently configured `--allow-root alpha=ABSOLUTE_ROOT`. Endpoint is
an absolute socket inside the runtime on Unix, a simple pipe label on Windows.

Every following command also takes the same explicit `--runtime-dir RUNTIME
--endpoint ENDPOINT` global arguments. Uppercase tokens below are placeholders:

```text
acp-query profile create ABSOLUTE_FRESH_PROFILE_PATH
acp-query profile info
acp-query profile write enable --profile-id PROFILE_ID --generation GENERATION --confirm
acp-query profile info
acp-query preferences list --scope project --project alpha --after-id 0
acp-query preferences get ID --scope project --project alpha
acp-query handoff-record alpha EXACT_TASK
acp-query profile write disable
```

Read the binding again **after** enable to obtain its new owner epoch. Each mutation
also requires `--profile-id PROFILE_ID --generation GENERATION --owner-epoch EPOCH
--operation-id UNIQUE_ID`; these flags are omitted in the next block for readability:

```text
acp-query project-register alpha CANONICAL_ROOT
acp-query preference-suggest --scope project --project alpha --key editor --value synthetic --source operator --origin agent_reported
acp-query preference-confirm ID --scope project --project alpha --expected-revision REVISION --proposal-digest FULL_PROPOSAL_DIGEST --confirm
acp-query preference-reject ID --scope project --project alpha --expected-revision REVISION --proposal-digest FULL_PROPOSAL_DIGEST --confirm
acp-query preference-deprecate ID --scope project --project alpha --expected-revision REVISION --proposal-digest FULL_PROPOSAL_DIGEST --confirm
acp-query preference-delete ID --scope project --project alpha --expected-revision REVISION --proposal-digest FULL_PROPOSAL_DIGEST --confirm
acp-query handoff-put alpha EXACT_TASK --expected-absent --input OWNED_INPUT_FILE
acp-query handoff-put alpha EXACT_TASK --expected-revision REVISION --input OWNED_INPUT_FILE
```

Registration sends the **exact native canonical root** matching the independent
launch policy, not a guessed alias. In the Windows validation fixture this uses
Rust's extended-length canonical spelling (`\\?\C:\…`); normal and extended
spellings are not silently equated in mutation admission. No client silently
changes the daemon allowlist. Preserve the original root bytes for receipt retry,
including when the registered directory later becomes unavailable.

`--scope global` forbids `--project`. Inspect the exact current suggested record
before confirm/reject; only confirmed records can be deprecated. Deletion is a
separate destructive operation bound to the exact current record, revision and
digest, not another status transition. Refresh after any intervening write.

The Handoff file is one strict bounded object, not an arbitrary mutation envelope:

```json
{
  "objective": "Synthetic objective",
  "completed": "",
  "next_action": "Inspect current repository facts",
  "validation_json": "[]",
  "snapshot": {
    "base_sha": "",
    "branch": "",
    "state": "unknown",
    "fingerprint": null
  }
}
```

Do not turn unknown historical state into clean or invent PASS. Replacement needs
the exact saved revision. Preserve `validation_json` bytes and the complete input
file for a deliberate retry. A response error includes the retained operation ID
and an `outcome_unknown` boolean. An unknown result is not a failed transaction;
retry only the unchanged command/inputs and binding/ID in the same epoch. Inspect
fresh state after revocation/restart rather than changing the old request's epoch.

Start either NSH binary with the same explicit native target and `--operator`, or
open Connections → Profile operator / Alt+O. Profile, Preference records and Saved
Handoff tabs share the same model. Keyboard commands use Alt+Shift U/M/R/E/X for
create/select/inspect/enable/disable, L/I/Y/N/S for list/select/confirm/reject/suggest,
H to inspect the exact saved Handoff in that pane, V to apply the exact reviewed
operation and Z to retry an uncertain request. Alt
P/K/W/F/T focuses the path/key/value/source/task input. Existing Sessions shortcuts
are preserved. The pane shows errors and last acknowledged receipt separately from
the pending operation; an old context read is not daemon write-health evidence.

For an uncertain UI result, retain the displayed operation ID and use the explicit
identical-request retry while that target/scope/epoch is current. Closing the client
loses its in-memory request: there is no durable client journal or cross-epoch replay
promise. A CLI retry requires having independently retained the exact original
inputs and operation ID. Never infer the request contents from a receipt alone.
