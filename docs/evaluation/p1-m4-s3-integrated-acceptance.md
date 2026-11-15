# P1-M4/S3 — Integrated acceptance evidence ledger

## Verdict and evidence boundary

**M4 BLOCKED.** This is an in-progress evidence checkpoint, not completed native
acceptance or release authorization. Production `ContextResolver` does not consume
structured ProjectContracts; its repository documents and authority warning do not
prove the automatic conflict masking required by the review contract. The separate
pure typed-context resolver implements masking. Its unit tests must not be relabelled
as production IPC/MCP enforcement. No parser, new policy authority or contract waiver
is introduced under S3. Parent [#52](https://github.com/ybakiame/agent-control-plane/issues/52)
remains open.

[S3 #60](https://github.com/ybakiame/agent-control-plane/issues/60) requires fresh
four-surface evidence. S2 results remain historical baseline only. GitHub Actions,
release/production activation, clean-machine installation, macOS, mobile, full IME,
full assistive technology and real-user durability are **NOT_RUN** in this S3 work.
Offline Web evidence cannot substitute for native evidence.

## Source and host provenance

- Base: merged S2 `6aa3bd414ee9472537b46f370ffed87694ff3166`.
- Branch: `feat/p1-m4-s3-integrated-acceptance`.
- Clean Windows process-test checkpoint:
  `e0cd4cc05fcdac1fdec2df6d948a6df9ace82737`, tree
  `f1f37ed126b65986238717c81548d5202d035774`.
- Linux exploratory regressions ran while the new test sources were uncommitted;
  they are development evidence, not a clean-SHA integrated acceptance PASS.
- Separate real Windows and Linux/WSL Git checkouts and native binaries on the same
  physical Windows/WSL2 installation. No independent physical-machine or independent
  human execution claim. Windows fetched the branch explicitly from the WSL Git
  checkout; its origin configuration and original stash were not changed.
- NSH remains `b7f31a08478be262fd7b76ec856036fd5b8ffdb0`, upstream workspace
  **0.5.0**, not stable 0.5.1. No SDK fork/cache patch or upgrade.

| Host/tool | Observed version/environment |
| --- | --- |
| Both native Rust toolchains | rustc 1.99.1 `58cf8f9c3`; Cargo 1.99.1 `5f94df478` |
| Both Node runtimes | v26.13.1 |
| Linux/WSL | Linux 6.18.58.1-microsoft-standard-WSL2, x86_64, Arch Linux distribution |
| Linux compiler / GTK | GCC 16.2.1 20260810; GTK 4.24.1; direct Wayland socket present |
| Windows | 10.0.26300.0, x86_64; Git 2.56.0.windows.4 |
| Windows PowerShell / .NET removal probe | PowerShell 7.6.2; .NET 10.0.8 |
| Windows process-test controls | stock rustup-resolved Cargo/rustc/rustdoc; `RUSTFLAGS=-Cdebuginfo=0`; `CARGO_NET_OFFLINE=true`; existing MSVC DevShell, no installation/global environment edits |

## Targeted corrections, separately reviewable commits

1. `b9775e9`: remove private preference `source` from the shared effective-context
   projection for all roles. Exact Operator inspection retains source, origin and
   proposal digest. Effective confirmed values and repository documents remain
   deliberately shared. Core regression covers project override/global fallback,
   both project bindings and document inclusion settings.
2. `6bf7cf7`: wrap Windows verifier setup/guards/gates in caller-environment
   restoration on success and failure. PowerShell coerces `$null` to an empty
   string; .NET 9+ retains empty environment entries. Use `[NullString]::Value`
   to actually remove newly introduced variables. The fixture now compares the
   environment before its own cleanup rather than hiding helper leaks.
3. `e0cd4cc`: eight adversarial process regressions and one precisely controlled
   admission test; correct stale Profile contract inventory to six tables/two
   indexes and mutation-envelope version 2. No DDL, migration or compatibility change.

The independent source review identified private-source disclosure and the runtime
ProjectContract masking gap. It ran no host/process tests. The source disclosure
correction has subsequent execution evidence below; no independent re-review PASS
is inferred from that correction.

## Executed targeted checks

| Check | Linux/WSL | Native Windows | Evidence class |
| --- | --- | --- | --- |
| Core private-source projection regression | PASS, 1 test, development tree | NOT_RUN separately | Pure core unit; not native UI |
| New S3 adversarial suite | PASS, 8 tests, development tree | PASS, 8 tests, clean `e0cd4cc` | Real daemon/CLI/IPC/MCP subprocesses; capacity/identity fixtures explicitly injected |
| Exact root drift at final admission | PASS, 1 test, development tree | PASS, 1 test, clean `e0cd4cc` | In-process duplex framing + real SQLite/filesystem + existing cfg(test) barrier; not separate OS daemon/native input |
| Windows verifier guard/environment fixture | Not a Linux runtime test | PASS, 10 cases | Native Git/helper, Cargo function stub for control flow only; no Cargo build claim |
| Windows process launcher restores caller environment | Not applicable to that launcher | PASS, changed variables = 0 | Actual process environment comparison, no values logged |
| Full committed workspace/shared/NSH gates against merged base | NOT_RUN | NOT_RUN | Required before integrated acceptance |
| Source verifier and current acceptance-driver gates | NOT_RUN | NOT_RUN | Required before integrated acceptance |

Commands actually executed for the development/process checkpoint:

```text
cargo test -p acp-core --test read_only effective_context_omits_private_sources_for_project_and_global_fallbacks --locked
cargo build --workspace --bins --locked
cargo test -p acp-daemon --test s3_review_acceptance --locked
cargo test -p acp-daemon --bin acp-daemon s3_root_drift_between_initial_check_and_final_admission_refuses_review --locked
```

Windows used the owned ignored `s3-windows.ps1` launcher, mode `Process`, with
`-ReportDir artifacts/local-p1-m4/s3-e0cd4cc-windows-process`. It ran the last three
commands with the exact toolchain/flags above. `tools/test_windows_verifier.ps1`
used native Windows Git with the current helper copied into an owned temporary
repository; it exercised committed whitespace, fixed branch, empty delta, missing
base, invalid base, HEAD-equal, ambiguous ref, nonancestor, existing report and dirty
source, asserting environment restoration on every success/refusal path.

### Windows process-checkpoint binary SHA-256

Only binaries built for this checkpoint are attributed; stale UI/driver artifacts
are not included.

| Binary | SHA-256 |
| --- | --- |
| acp-daemon.exe | `8411f51f089bf5ed45480431ad3878a8d63b2f684d9352fc3ef5ea24f33d2395` |
| acp-query.exe | `5e8b3ff645cd7ee85b34c76554e84827d17dbb1e80ea92e93d72528bd7998a36` |
| acp-mcp.exe | `f4e529342cf9ee704128f5fe9fcbe5d22989f2ee15e03b8de063b7027a4bc885` |

## C. Adversarial case-to-evidence map

All named existing tests below are a coverage map, **not a fresh execution claim**.
Only the targeted runs above have current execution status at this checkpoint.

| S3 cell | New executable evidence | Existing coverage / honest limits |
| --- | --- | --- |
| Roles, independent write grant, session/channel denial | `s3_session_review_denial_and_client_channels_never_supply_authority` | Existing daemon exact-review/write-role/session tests and MCP two-tool/security suites; labels never supply authority |
| Agent effective-context privacy | `s3_confirmed_private_sources_never_leak_through_real_ipc_or_mcp` | Normal daemon-created confirmed global fallback/project override, foreign/pending/rejected/deprecated exclusion; independent expected values through agent IPC, Operator context and MCP; exact Operator inspection retains private fields |
| Binding and strict decode | `s3_stale_request_bindings_refuse_before_replay_and_strict_decode` | Stale profile/generation/epoch before replay; unknown envelope/binding/command fields and method |
| Identity drift | `s3_contained_identity_corruption_never_rebinds_an_existing_writer` | Explicit owned SQLite fixture corruption, followed by real process requests; not a normal authorized state transition |
| Canonical roots, foreign registration | `s3_symlink_or_junction_swapped_root_and_foreign_registration_refuse` | Real Unix symlink/Windows junction; unchanged logical state after refusal; existing independent-daemon/lease/root-binding suites map second-writer and substitution boundaries |
| Root TOCTOU at final admission | `s3_root_drift_between_initial_check_and_final_admission_refuses_review` | Precise cfg(test) barrier; in-process transport, not independent daemon process. Existing revocation/drain barrier has the same evidence-class limit |
| Concurrent CAS and exact replay | `s3_concurrent_cli_cas_and_identical_replay_commit_one_decision` | Independent CLI submissions; single decision/commit; identical original receipt; existing writer/daemon suites cover transitions, mismatch vs CAS, atomic peers and deletion |
| Receipt ceiling / boundedness | `s3_contained_4096_fixture_process_capacity_conflict_and_exact_replay` | Structurally valid 4096-receipt fixture injected in one owned transaction; real IPC replay/conflict, real CLI Capacity, cross-epoch denial. No claim of 4096 normal CLI mutations |
| Queue order, bound and watermark | `s3_intervening_commit_changes_page_watermark_without_snapshot_claim` | Real inter-page commit, ≤12 rows, ordered next page, newer watermark; not a snapshot guarantee |
| History ≤8 | Existing exact-review process test plus strict reader SQL `LIMIT 8` | Valid terminal-state chains permit at most two decisions. No fabricated eight-decision valid fixture; query cap is source evidence |
| Fresh v1, no compatibility, causal history and deletion | Existing store frozen-DDL/schema-drift/causal-chain suites and writer transition/atomicity/deletion suites | Six tables/two indexes and `user_version=1`; no migration/dual reader/Python. Deletion is not forensic erasure |
| Read-only/direct/second writer | Existing read-only SQLite connection test plus independent-daemon lease/process tests | Application ownership/read-only connections, not an OS sandbox against a hostile same-user raw SQLite connection |
| Confirmed-only, project/global fallback, Handoff history | New privacy/capacity process tests plus existing core/MCP suites | Stored historical PASS remains historical; does not become current verification |
| Repository ProjectContract precedence | Typed-context resolver unit tests only | **BLOCKED for production automatic masking:** runtime context exposes documents/authority but has no structured contract consumer. Confirmation does not approve/rewrite a contract |
| Offline/browser/Windows→WSL exclusion | Existing model no-IPC/no-write/native-input tests and source review | Model/source class, not native typing or authenticated browser proof; current execution NOT_RUN |

## B. Four-surface native ledger

At this checkpoint all required native cells are **NOT_RUN** for the new S3 code.
None inherits S2 PASS. The driver must record individual case IDs, actual OS input,
observed render/receipt, owned PID/nonce, SHA/binary hashes, timeline, captures and
contained-interference mechanisms, and preserve partial evidence on failure.

| Required cells | HWND | WSLg direct Wayland | ConPTY | PTY |
| --- | --- | --- | --- | --- |
| Empty/project/global queues; 12-row paging; watermark/staleness | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| Exact inspect/digest/source/origin/project; history/peer/channel caveats | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| Confirm/Reject/Deprecate/Delete; peers; wrong ID/blind Enter/Escape/reopen; deleted content/history | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| EN/zh-CN; light/dark; compact/large; Tab/Enter/Space/Alt; modal focus/restoration | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| Scope/project/selection/binding/target/source/status drift; stale revision/digest; revoke/disconnect; delayed reads | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |
| OutcomeUnknown/no-auto-retry; immutable same-epoch manual replay; altered bytes/stale epoch | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN |

Missing required input is BLOCKED, never NOT_APPLICABLE. GUI needs actual owned
pixels plus GTK backend/renderer evidence; TTY needs actual frames/events and
restoration. Probe/model/headless output alone is not a native PASS.

## Failures retained

- Initial Windows helper invocation used the wrong WSL share name and did not run
  the test. Corrected to the observed Arch Linux distribution; not a product failure.
- First restoration implementation left newly introduced MSVC environment variables
  present with empty values. Native fixture failed. A bounded synthetic probe on
  PowerShell 7.6.2/.NET 10.0.8 established ordinary `$null` retains a key while
  `[NullString]::Value` removes it. Corrected helper/fixture/owned launcher; subsequent
  10-case fixture and real process-launch environment comparison passed.
- Independent source review found confirmed `source` disclosure and overstated
  production contract masking. Private-source projection is corrected and tested;
  contract masking remains an explicit M4 blocker, not silently weakened wording.

## Private local artifacts

Raw evidence is ignored, not uploaded or committed. Fixtures contain synthetic data
only; no existing personal Profile, authentication state or unrelated worktree data
is adopted, reset or deleted.

- Linux development checks, helper failures and final 10-case fixture:
  `artifacts/local-p1-m4/s3-helper/` in the WSL checkout.
- Windows clean checkpoint:
  `artifacts/local-p1-m4/s3-e0cd4cc-windows-process/` and adjacent
  `s3-e0cd4cc-windows-process-environment.json` in the Windows checkout.
- Owned launcher source: `artifacts/local-p1-m4/s3-windows.ps1` in the WSL checkout.

A completed S3 ledger still requires the same clean committed candidate on both
hosts, the full gate commands mandated by #60, driver negatives and all actual native
cells. No parent closure or engineering PASS is authorized by these targeted runs.
