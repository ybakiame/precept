# P1-N / #65 — production ProjectContract integration evidence

## Verdict and review boundary

**Local backend integration PASS; M4 remains BLOCKED pending owner runtime review.**
The owner approved D1–D4 in [PR #66](https://github.com/ybakiame/agent-control-plane/pull/66)
on 2026-11-16 11:02 UTC. Production `context.get` and the existing project-bound
MCP tool now apply the [accepted boundary](../p1/project-contract-integration.md).
This is implementation plus author-run local evidence, not an independent approval,
production activation or permission to merge. PR #66 remains stacked on Draft
[PR #61](https://github.com/ybakiame/agent-control-plane/pull/61); #65/#60/#52 stay open.

Contract-only proposal: `c0b7042ae143e54a8c354dc140bc6a61b553546f`.
Core parser/bridge: `a00261b`. Production integration/test source:
**`435f4bd325d569e3bd065fc6b5d096aec1d79f0a`**, tree
**`fd17ed77b1c12d2406e3e462b91647d9e8771e78`**.
Both native Git checkouts were clean at that exact tested revision. Later
documentation commits do not claim rebuilt or retested host binaries.

## Changed behavior and actual test classification

The optional exact root `ACP.contracts.md` is bounded to 16 KiB and one versioned
JSON fence. Strict parsing rejects unknown/duplicate fields, unsupported versions,
invalid selectors, excessive lists/depth and malformed text. There is no discovery,
YAML/prose inference, cache fallback, scaffolding or repository write.

The production daemon supplies fresh authorized sources to frozen typed v1;
confirmed project/global preferences cannot override applicable accepted contracts.
Pending or definitely nonmatching contracts preserve exact legacy fallback,
including multiline and >4096-byte valid Profile values. For genuinely affected
keys, unrepresentable values or unknown applicability block that key. Contradictory
accepted contracts have no effective winner. No Profile DDL or writer behavior changed.

`selector` labels are optional on existing IPC/MCP context reads. They never
authenticate an agent, override project/root binding, confer permissions or add a
tool. Invalid populated requests fail instead of becoming unrestricted. Whole-source
invalidity/unavailability/change returns empty effective preference/contract output
with an explicit sanitized status; it cannot be confused with `not_configured`.

| Evidence | Linux/WSL | Native Windows | Classification |
| --- | --- | --- | --- |
| New core parser/bridge tests | 9 PASS | 9 PASS | Component + actual owned filesystem; Linux symlink/permission refusal, Windows native reparse-junction refusal |
| New daemon integration tests | 3 PASS | 3 PASS | Independently launched **production binaries**, native Unix socket / named pipe IPC and actual MCP stdio bridge |
| New source-publication checkpoint parent | PASS, 12 cases | PASS, 12 cases | Separate **test-built** daemon child, actual native IPC/MCP; contained process fault injection |
| Entire Rust Workspace | **152 PASS**, 0 FAIL | **153 PASS**, 0 FAIL | Includes existing schema, CAS/replay, write ownership, root drift, lease expiry/revoke/publication, MCP denials and Fingerprint regressions |
| Workspace fmt / bins build / Clippy `-D warnings` | PASS | PASS | Clean source, locked offline dependencies |
| Source/link/workflow/SDK-pin checks | PASS, 248 paths / 34 archives | PASS, 248 paths / 34 archives | At tested code revision; evidence document added afterward |
| Branch and worktree diff checks | PASS | PASS | Baseline `95a3905ae8bcded2084ad0af22edb918d55513b5` |

Each Workspace has two ignored entries: the existing read-cost benchmark and the
subprocess daemon entry. The latter is actually launched by its parent tests:
four old write-admission cases plus twelve new source-publication cases per OS.
It is not an extra counted test PASS or a hook in the product daemon.

The three new integration tests prove actual IPC/MCP masking, compatible fallback,
inactive/foreign exclusion, operator confirmation leaving the contract unchanged,
private source/decision/provenance exclusion, exactly two tools, selectors and
malformed/project/root-override denial. Same-HEAD dirty source edits change live
behavior. Absent, malformed, unsupported, oversized, directory and unreadable
sources are distinguished; Unix permissions and Windows exclusive file sharing
exercise actual unreadable-source responses without changing system configuration.
DB logical content remains unchanged by reads.

The new checkpoint runs after normal context assembly, before final root/source
recheck. The parent waits for the exact child PID and then modifies, deletes,
renames, replaces or adds the source, or displaces the registered root. All six
cases run through **both IPC and a separately launched production MCP bridge**.
Source changes return `source_changed` with empty effective collections; root
drift yields native `UnsafeRuntime` or the bridge's sanitized refusal. No stale
contract/preference result is published. Exact DB bytes and logical records,
receipts and decisions remain unchanged. Every child is owned, terminated and
reaped by a drop guard, including failure paths. Only `cfg(test)` contains the
checkpoint/entry; the product binary has no test environment switch.

## Reproduction and native environment

Both commands ran from their respective genuine repository roots on the same
physical Windows/WSL installation, using the owner's M4-only exception:

```text
# Linux/WSL; process-local CARGO_NET_OFFLINE=true, CARGO_BUILD_JOBS=4
tools/local_verify.sh core artifacts/local-p1-m4/n-435f4bd-linux-gates 95a3905ae8bcded2084ad0af22edb918d55513b5
node tools/check_source.mjs
# Native Windows PowerShell; process-local offline/jobs=4/RUSTFLAGS=-Cdebuginfo=0
tools/windows_local_verify.ps1 -Suite Core -ReportDir artifacts/local-p1-m4/n-435f4bd-windows-gates -DiffBase 95a3905ae8bcded2084ad0af22edb918d55513b5
node tools/check_source.mjs
```

The helpers run workspace formatting, locked binary build, all-target Clippy with
warnings denied, and `cargo test --workspace --all-targets --locked`. Development
runs used targeted core/daemon tests before the one final full Workspace gate per
OS. Initial development compile/lint mistakes were corrected before that clean
cohort; they are not claimed as passing validation. No checks were deleted or
weakened, no business action was retried to obtain acceptance.

Linux/WSL: rustc 1.99.1, Cargo 1.99.1, x86_64 kernel
6.18.58.1-microsoft-standard-WSL2. Native Windows: 10.0.26300.0 x64, rustc 1.99.1,
observed global Cargo **1.102.0 / huacnlee-cargo 0.103.0-1**, PowerShell 7 and
global Git 2.56.0.windows.4. This is not stock Windows Cargo evidence. No toolchain
was installed or system setting changed. Native Git fetched the stacked branch
explicitly from WSL; the Windows checkout's configured origin was preserved.

SHA-256 fingerprints of tested product binaries:

| Binary | Linux/WSL | Native Windows |
| --- | --- | --- |
| acp-daemon | `c6ba77732e88a351bce93356b76a6238f3d16661d684794aad54f30630ecd0a0` | `5228830ab230a7b383e21514a90abf6a576b53013558d1e57ecd39ced3bffeec` |
| acp-mcp | `e6bb45fcd70ec45d50a2afc9fbf83b0d92a8e6860c5fd0ac2078173f00ddd939` | `a342242dff3212962a0a408af24372721daa8b5f0d9d7207ba6d064f3a93946b` |
| acp-query | `5ad5e926620c14b29fc1f4e31b596aa7596ee050b9d2bcd5a0d00103177b18f7` | `bb85b281f1e1bce9ab83965971c6bf86255146e570b5c899cfc83581bc7c8503` |

Logs, command lists, native gate reports and summaries remain ignored and local
in the relative artifact directories above, separately in each checkout. No raw
Profile data, private paths, transcripts or credentials are uploaded.

## UI evidence, limitations and handoff

Per the updated owner policy in #65 and PR #61's re-review, this backend-only
change does **not** rerun the 148-cell HWND/WSLg/ConPTY/PTY matrix. NSH code, SDK pin
and UI behavior were not changed. The one-time clean UI cohort remains precisely
at `19942b6e8ba58e990e60095bf6440d572cda6123`, with the original classification,
binary identities and prior failed/partial cohorts preserved in the
[S3 ledger](p1-m4-s3-integrated-acceptance.md). It is not evidence for new UI binaries
at `435f4bd`. New GUI rendering/input, niri/mobile, macOS, clean-machine installation,
IME/assistive technology, models and hosted GitHub Actions are NOT_RUN.

Repository `accepted` is a local repository claim, not authenticated GitHub review.
Ordinary AGENTS/ADRs/config retain authority but are not machine-parsed contracts.
An absent optional source is visibly not configured; no history cache distinguishes
never-configured from removal between independent reads. Final bounded rechecks
detect the tested read interval, not edits after the check, an atomic filesystem/
SQLite snapshot or hostile same-user tampering. Source/policy limits remain as
approved; no permission expansion or universal enforcement claim is made.

The implementation addresses #61's production-masking gap with actual process
evidence. Owner review of these results and M4/#60/#52 closure is still required.
Both PRs stay Draft, no merge or issue closure is performed. No real user Profile,
auth directory, external development checkout, network/model call or automatic
repository rewrite was part of validation. All commits use `[skip ci]`.
