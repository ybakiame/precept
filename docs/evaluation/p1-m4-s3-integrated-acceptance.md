# P1-M4/S3 — Integrated acceptance evidence ledger

2026-11-16 P1-N follow-up: the owner approved the separate #65 design, and
[PR #66's integration evidence](p1-n-project-contracts.md) records production
contract enforcement at clean `435f4bd` on both native OS environments. This S3
ledger preserves its original cohort verdict and failed/partial history below;
it is not a claim that the new backend SHA reran the four-surface UI matrix.
M4 remains blocked pending owner runtime review; both PRs remain Draft.

## Verdict

**M4 BLOCKED.** Production `ContextResolver` does not consume structured
ProjectContracts. Its repository documents and authority warning do not implement
the automatic conflict masking stated in the [review contract](../p1/operator-review-contract.md).
The separate typed-context resolver implements masking as a pure API; its unit
tests are not production IPC/MCP enforcement evidence.

[PR #61](https://github.com/ybakiame/agent-control-plane/pull/61) remains Draft for
[S3 #60](https://github.com/ybakiame/agent-control-plane/issues/60).
Parent [#52](https://github.com/ybakiame/agent-control-plane/issues/52) stays open.
No self-merge, contract waiver, document parser, new write authority, schema
compatibility, production activation or release authorization is introduced.

The original exact root-admission checkpoint remains **in-process** evidence.
Review cohort C adds a separately launched, test-built daemon using actual native
IPC: its checkpoint regression is **PASS on Windows and Linux/WSL**, classified
as contained process fault injection, not an instrumented product daemon or GUI.
Independent human acceptance, GitHub Actions, clean-machine installation, macOS,
mobile, full IME/assistive technology and real-user durability are **NOT_RUN**.
Offline Web evidence is not native acceptance or an authenticated browser UI.

## Review disposition — 2026-11-16

The product owner retained the ProjectContract precedence requirement and chose
a separate, contract-first integration task: [#65](https://github.com/ybakiame/agent-control-plane/issues/65).
No parser is added or requirement waived by PR #61. M4 stays **BLOCKED** until
production IPC/MCP enforcement and its affected S3 regressions are reviewed.
Neither #60 nor parent #52 may be closed by this revision.

For this M4 phase only, the owner explicitly permits two native Git checkouts on
the same physical Windows/WSL2 installation instead of independent physical
hosts. This is an approved environment exception, **not** independent-host proof
or independent human acceptance. Other matrix, clean-SHA, foreground, ownership
and native rendering requirements remain in force.

The current Windows checkout is the operator-designated native Git repository
and uses globally installed Windows Git. Its configured origin is preserved;
explicit fetches from the WSL checkout do not change that configuration. No real
Profile, authentication directory or external project is part of validation.

The new `process_tests::s3_independent_daemon_admission_checkpoint` launches a
separate **test-built** daemon process running the normal `run`/listener/peer/
Profile/Writer path. The parent uses actual Unix-socket/named-pipe IPC, waits for
the child PID's checkpoint between initial and final admission, displaces the
synthetic root, and releases the child. Confirm/Reject/Deprecate/Delete must
return `UnsafeRuntime` with byte-identical database and unchanged logical rows,
receipts and decisions; restored-root exact inspection must match. Only `cfg(test)`
contains the checkpoint and subprocess entry. This is contained independent
process fault injection, not GUI keyboard evidence or an instrumented product
daemon. The prior in-process result remains separately classified below.

## Current complete review cohort C

Final code/test revision: **`19942b6e8ba58e990e60095bf6440d572cda6123`**, tree
**`d18ec29232cd266b51c3716e07c24b04c4d405b1`**. Both native checkouts were clean
at this revision. All builds/tests finished before C's native sessions; no binary
was rebuilt during a session. All four final reports record dirty=false,
clean_after=true and unchanged binary fingerprints. Later evidence-only commits
do not substitute a new tested binary revision. No A/B evidence is stitched into C.

```text
tools/local_verify.sh all artifacts/local-p1-m4/s3-19942b6-linux-gates 6aa3bd414ee9472537b46f370ffed87694ff3166
tools/windows_local_verify.ps1 -Suite All -ReportDir artifacts/local-p1-m4/s3-19942b6-windows-gates -DiffBase 6aa3bd414ee9472537b46f370ffed87694ff3166
cargo fmt --manifest-path tools/native-acceptance/Cargo.toml -- --check
cargo build --manifest-path tools/native-acceptance/Cargo.toml --locked
cargo clippy --manifest-path tools/native-acceptance/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path tools/native-acceptance/Cargo.toml --locked
node tools/check_source.mjs
```

These ran in each respective repository root with process-local controls below.
Native invocations used no development flag or selected-case filter:

```text
# Linux/WSL: WINDOWS_HELPER_PATH resolves to tools/native_window.ps1 in the designated Windows checkout.
tools/native-acceptance/target/debug/acp-native-acceptance tui "$PWD/artifacts/local-p1-m4/s3-19942b6-pty"
tools/native-acceptance/target/debug/acp-native-acceptance desktop "$PWD/artifacts/local-p1-m4/s3-19942b6-wslg" "$WINDOWS_HELPER_PATH"
# Native Windows PowerShell:
& .\tools\native-acceptance\target\debug\acp-native-acceptance.exe tui "$PWD\artifacts\local-p1-m4\s3-19942b6-conpty"
& .\tools\native-acceptance\target\debug\acp-native-acceptance.exe desktop "$PWD\artifacts\local-p1-m4\s3-19942b6-hwnd" "$PWD\tools\native_window.ps1"
```

| Full gate at C | Linux/WSL | Native Windows |
| --- | --- | --- |
| Baseline diff / Workspace fmt, build, Clippy `-D warnings`, tests / final diff | PASS; **139 tests** | PASS; **140 tests** |
| Shared model fmt, Clippy, tests with native feature | PASS; **53 tests** | PASS; **53 tests** |
| NSH fmt, native+tui build, Clippy, tests including 10k rows and real IPC | PASS; **29 tests** | PASS; **29 tests** |
| Native driver fmt, build, Clippy, tests | PASS; **29 tests** | PASS; **29 tests** |
| Source inventory, links, workflows, immutable NSH pins | PASS; **240 paths / 34 archives** | PASS; **240 paths / 34 archives** |
| Independent daemon admission checkpoint; all four review actions | PASS; process FI | PASS; process FI |

Each Workspace run has two explicitly ignored entries: the existing performance
test and the subprocess daemon entry, which the new parent regression actually
launches four times. The latter is an entry point, not an additional claimed test
PASS. The earlier Windows environment control-flow fixture remains its historical
ten-case result, not a new Cargo/test claim at C.

| Complete native surface at C | Automated cells | Baseline / final identity | Cleanup / restoration |
| --- | --- | --- | --- |
| Windows native HWND | **37/37 PASS** | PASS / PASS | 22/22 owned driver/window closures |
| Direct WSLg native Wayland | **37/37 PASS** | PASS / PASS | 22/22 owned driver/window closures |
| Windows ConPTY | **37/37 PASS** | PASS / PASS | 22/22 VT streams restore alternate screen |
| Linux/WSL PTY | **37/37 PASS** | PASS / PASS | 22/22 VT streams restore alternate screen |

This is **148 automated native cells**, not M4 PASS or independent human approval.
Current GUI captures were inspected for exact immutable fields, destructive focus,
wrong-ID refusal, visible queue-drift warning and all eight EN/zh-CN × Light/Dark ×
compact/large combinations per GUI. Compact captures show the form and modal;
detail/history can remain below the fold. This is not simultaneous visibility of
all content or full accessibility/IME certification. Raw report `visual_review`
stays NOT_RUN for independent inspection; this paragraph records scoped agent
image inspection only.

C's actual GTK observation is **`GdkWaylandDisplay` / `GskGLRenderer`**, mapped
windows, after-paint events and acknowledged resize; DISPLAY is removed. The
historical Cairo observations below do not describe C. No niri, X11/XWayland or
Xvfb substitute was used. C uses the existing GTK 4.24.1, GCC 16.2.1, Windows
10.0.26300.0, VCTools 14.51.36231, SDK 10.0.26100.0, Node v26.13.1,
PowerShell 7.6.2/.NET 10.0.8 and global Windows Git 2.56.0.windows.4.

Both compilers are repository-pinned **rustc 1.99.1**. Linux Cargo is **1.99.1**;
Windows C's logged frontend is **Cargo 1.102.0 / huacnlee-cargo 0.103.0-1**.
Windows's earlier `3f4c534` gate logged frontend 1.101.0 / 0.102.0-9. These are
observed existing global tools, not installations by this task or a claim of
stock Windows Cargo 1.99.1 acceptance. Exact host metadata stays in the private
`s3-19942b6-{linux,windows}-driver/host.json` files. Controls were offline Cargo,
four build jobs, existing LLVM22 on Linux process PATH and `RUSTFLAGS=-Cdebuginfo=0`
with the existing Windows MSVC environment; no persistent setting changed.

The input correction observes actual Desktop field focus after a non-activating
Tab and the field shortcut, then types once. PTY/ConPTY and Desktop text use
25 ms character pacing. Exact content, receipt, stale-state, foreground and
ownership assertions are retained; no character, mutation or consent is retried.
Rapid/bulk-paste input and IME composition are not certified by these flows.

Private evidence: `artifacts/local-p1-m4/s3-19942b6-{linux,windows}-gates/`,
`s3-19942b6-{linux,windows}-driver/`, and `s3-19942b6-{hwnd,wslg,conpty,pty}/` in
the respective native checkouts. `s3-19942b6-evidence-index.json` in WSL hashes
**928 native artifacts** and records binary identity, per-surface counts and
cleanup/restoration. Windows raw evidence remains in the designated Windows
checkout. Nothing from these ignored directories is uploaded.

## Source cohorts

| Cohort | Exact revision | Evidence |
| --- | --- | --- |
| Merged S2 baseline | `6aa3bd414ee9472537b46f370ffed87694ff3166` | Baseline only; no inherited S3 PASS |
| Full gates | `55769ff27f935a1ce55ec32f4aafc63e80646ab6` | Both hosts' full Cargo, driver and source checks |
| A: complete native matrix | `44b989f53b4cc22d6a93c6e306ee982feb5b2a9d` | Same clean tree `e2cde24778206d2ce4fde32e27f0886def0e0b37`; 37 automated cells per surface |
| B: native disclosure correction | `57386fa02adfe8a37b662a9e516c2b3251a39aad` | Same tree `6f4465604f59f949294e50c57e6d271b4cf945a0`; affected UI checks and two queue cases per surface |
| Review preflight | `75d40c4eb7222b711cd21b603370194b2402b161` | Branch-diff failed on an inherited trailing blank line; no full gate PASS |
| Review exploratory full run | `3f4c5340c5287befd16f568a744a7b8beae40c27` | Full gates PASS on both OS environments; native failures/refusal preserved below |
| C: final complete review | `19942b6e8ba58e990e60095bf6440d572cda6123` | Same clean tree `d18ec29232cd266b51c3716e07c24b04c4d405b1`; full gates and 37/37 on all four surfaces |

Branch: `feat/p1-m4-s3-integrated-acceptance`. Documentation commits after these
cohorts do not recertify binaries. Windows and Linux/WSL are genuine separate Git
checkouts/native binaries on the same physical Windows/WSL2 installation—not
independent physical machines or independently operated hosts. Historical A/B
Windows sync preserved its origin configuration and original stash; C explicitly
fetched into the newly designated native repository and preserved its origin.

For historical A/B, owner selected minimal smoke before completing four surfaces and no repeat of
unaffected product gates. Git object fingerprints record unchanged crates, model,
test dependencies, root manifests/toolchain and verification helpers. A retains
the full-gate evidence at `55769ff`; B reruns affected UI checks, not unchanged
daemon/storage/MCP/model suites.

**No fresh complete 37/37 run at B is claimed.** Eight clean scoped B invocations
each report `DEV_PARTIAL_PASS`, one selected cell and 36 NOT_RUN cells. They cover
project drift and global paging on all four surfaces. A supplies the complete
matrix; B supplies only the affected correction. All commits use `[skip ci]`.

## Historical A/B host and SDK provenance

| Item | Observed version/environment |
| --- | --- |
| Both Rust toolchains | rustc 1.99.1 `58cf8f9c3`; Cargo 1.99.1 `5f94df478` |
| Both Node runtimes | v26.13.1 |
| Linux/WSL | Arch Linux; Linux 6.18.58.1-microsoft-standard-WSL2; x86_64 |
| Linux compiler / GTK | GCC 16.2.1 20260810; GTK 4.24.1 |
| Actual WSLg rendering | `GdkWaylandDisplay`, `GskCairoRenderer`, mapped windows and after-paint events; DISPLAY removed; no niri/XWayland/Xvfb |
| Windows | 10.0.26300.0; x86_64; Git 2.56.0.windows.4 |
| MSVC / SDK | compiler 19.51.36252.0; tools 14.51.36231; SDK 10.0.26100.0 |
| PowerShell / .NET | PowerShell 7.6.2; .NET 10.0.8 |
| Linux verification controls | `CARGO_NET_OFFLINE=true`, `CARGO_BUILD_JOBS=4`, existing LLVM22 tools on process PATH |
| Windows controls | Stock rustup-resolved Cargo/rustc/rustdoc; `RUSTFLAGS=-Cdebuginfo=0`; `CARGO_NET_OFFLINE=true`; existing MSVC DevShell; caller environment restored |
| Exercised sizes | GUI large 1080×1000 / compact 640×480; TTY large 160×90 / compact 80×30, plus baseline sizes |

NSH remains approved immutable
[`b7f31a08478be262fd7b76ec856036fd5b8ffdb0`](https://github.com/longbridge/nsh/commit/b7f31a08478be262fd7b76ec856036fd5b8ffdb0):
upstream workspace **0.5.0**, not stable 0.5.1. No SDK fork/cache patch, toolchain
installation or persistent OS/environment change. Fixtures use fresh private
disposable Profiles/roots/credentials/endpoints, never a real Profile or model.
The zero-byte `/.rpi/.resource-lease` is preserved and precisely ignored; other
`.rpi` files are not hidden.

## Historical executed gates

```text
tools/local_verify.sh all artifacts/local-p1-m4/s3-55769ff-linux-gates 6aa3bd414ee9472537b46f370ffed87694ff3166
tools/windows_local_verify.ps1 -Suite All -ReportDir artifacts/local-p1-m4/s3-55769ff-windows-gates-r2 -DiffBase 6aa3bd414ee9472537b46f370ffed87694ff3166
```

| Gate at `55769ff` | Linux/WSL | Native Windows |
| --- | --- | --- |
| Baseline-to-HEAD whitespace | PASS | PASS |
| Workspace fmt / binary build / Clippy, warnings denied | PASS | PASS |
| Workspace tests | PASS: 138; 1 existing ignored performance test | PASS: 139; 1 existing ignored performance test |
| Shared model fmt / Clippy | PASS | PASS |
| Shared model tests, native feature | PASS: 53 | PASS: 53 |
| NSH fmt / native+tui build / Clippy | PASS | PASS |
| NSH tests | PASS: 29 | PASS: 29 |
| Final whitespace | PASS | PASS |
| Source verifier | PASS: 239 paths, 34 explicit archives | PASS: 239 paths, 34 explicit archives after locked Node prerequisites |
| Driver fmt / build / Clippy / tests | PASS: 29 tests | PASS: 29 tests |

Corrected-driver checks also ran for A. Driver tests comprise four VT regressions,
21 worker-protocol negatives and four owned-process cases: malformed/oversized,
silent/EOF/crash, stale/duplicate/late/unsolicited replies, nonce/request mismatch,
stalled/cancelable stdin, held stdout, unfit bounded IO and panic/adoption cleanup.
These are mock-worker/process ownership tests, not native GUI proof.

B ran only affected UI gates on both hosts:

```text
cargo fmt --manifest-path apps/operator/ui/Cargo.toml -- --check
cargo build --manifest-path apps/operator/ui/Cargo.toml --features native,tui --bins --locked
cargo clippy --manifest-path apps/operator/ui/Cargo.toml --all-targets --features native,tui --locked -- -D warnings
cargo test --manifest-path apps/operator/ui/Cargo.toml --all-targets --features native,tui --locked
```

Result: **PASS, 29 UI tests per host**. Linux affected gates had only untracked
diagnostic PNGs outside the intended ignored path; tracked sources were committed
B. They are not described as a clean-worktree helper run. The owned files were
preserved inside ignored evidence, and all four final Linux scoped native runs
were repeated with dirty=false/clean_after=true. No product suite was repeated
merely to relocate PNGs.

Earlier targeted checks: one core privacy regression, eight process regressions
and one admission unit per host, and ten Windows helper control-flow cases. The
last uses actual Git/helper but a Cargo function stub—not a Cargo test result.

## Historical A/B native matrix

Normal complete invocation (no development selector):

```text
tools/native-acceptance/target/debug/acp-native-acceptance tui ABSOLUTE_NEW_REPORT_DIR
tools/native-acceptance/target/debug/acp-native-acceptance desktop ABSOLUTE_NEW_REPORT_DIR WINDOWS_HELPER_PATH
```

Actual directories: `artifacts/local-p1-m4/s3-44b989f-{pty,wslg}` in WSL and
`s3-44b989f-{conpty,hwnd}` in Windows. The helper was the same-source Windows
checkout's `tools/native_window.ps1`. WSLg used DISPLAY removed,
`GDK_BACKEND=wayland`, `GSK_RENDERER=cairo`. Every normal run reports dirty=false,
clean_after=true, unchanged before/after binary hashes and no selected case.

| A surface | Automated cells | Baseline | Actual backend |
| --- | --- | --- | --- |
| Windows Desktop | 37 PASS | PASS | Native HWND |
| Linux Desktop | 37 PASS | PASS | Direct WSLg Wayland/Cairo |
| Windows TUI | 37 PASS | PASS | ConPTY |
| Linux TUI | 37 PASS | PASS | PTY |

**148 automated PASS results are not unconditional pixel/engineering PASS.**
Image inspection found the stale badge absent on both GUIs at A. It remained
absent after a 1300 ms capture-only settling probe, beyond the 500 ms entrance
plus 11×50 ms stagger. A temporary row wrapper did not fix it and was not retained.
B changes only that disclosure to localized warning text. Clean B PNGs visibly
show the warning and `+1` for project/global queues; TTY frames show the same text.

In the indexed ledger below **A** means the complete A input/state run; **B** means
the clean affected B invocation. The GUI stale-disclosure **visual** result at A
was FAIL despite automated model PASS; original failures remain retained.

| Cell | HWND | WSLg | ConPTY | PTY |
| --- | --- | --- | --- | --- |
| profile-owned-flow | PASS A | PASS A | PASS A | PASS A |
| empty-queue | PASS A | PASS A | PASS A | PASS A |
| project-paging | PASS A | PASS A | PASS A | PASS A |
| global-paging | PASS B | PASS B | PASS B | PASS B |
| stale-page-disclosure | PASS B | PASS B | PASS B | PASS B |
| exact-inspect-provenance | PASS A | PASS A | PASS A | PASS A |
| confirm-peer-demotion | PASS A | PASS A | PASS A | PASS A |
| reject | PASS A | PASS A | PASS A | PASS A |
| history-bounded | PASS A | PASS A | PASS A | PASS A |
| deprecate | PASS A | PASS A | PASS A | PASS A |
| delete-wrong-id | PASS A | PASS A | PASS A | PASS A |
| delete-blind-enter | PASS A | PASS A | PASS A | PASS A |
| delete-escape-reopen | PASS A | PASS A | PASS A | PASS A |
| delete-exact-id | PASS A | PASS A | PASS A | PASS A |
| deleted-content-history | PASS A | PASS A | PASS A | PASS A |
| keyboard-restoration | PASS A | PASS A | PASS A | PASS A |
| selected-id-change | PASS A | PASS A | PASS A | PASS A |
| scope-change | PASS A | PASS A | PASS A | PASS A |
| project-change | PASS A | PASS A | PASS A | PASS A |
| profile-epoch-change | PASS A | PASS A | PASS A | PASS A |
| profile-generation-change | PASS A | PASS A | PASS A | PASS A |
| target-change | PASS A | PASS A | PASS A | PASS A |
| source-change | PASS A | PASS A | PASS A | PASS A |
| source-field-change | PASS A | PASS A | PASS A | PASS A |
| status-change | PASS A | PASS A | PASS A | PASS A |
| stale-revision | PASS A | PASS A | PASS A | PASS A |
| altered-digest | PASS A | PASS A | PASS A | PASS A |
| revoke-inspect-apply | PASS A | PASS A | PASS A | PASS A |
| disconnect | PASS A | PASS A | PASS A | PASS A |
| delayed-read-navigation | PASS A | PASS A | PASS A | PASS A |
| unknown-no-auto-replay | PASS A | PASS A | PASS A | PASS A |
| manual-exact-replay | PASS A | PASS A | PASS A | PASS A |
| replay-changed-bytes | PASS A | PASS A | PASS A | PASS A |
| replay-stale-epoch | PASS A | PASS A | PASS A | PASS A |
| en-light-dark-compact-large | PASS A | PASS A | PASS A | PASS A |
| zh-light-dark-compact-large | PASS A | PASS A | PASS A | PASS A |
| tab-enter-space-alt | PASS A | PASS A | PASS A | PASS A |

### Evidence mechanisms and limits

- Actual bounded Tab observation then Enter drives queue/settings controls, not
  callbacks or guessed platform counts. Independent CLI proves exact scope,
  ascending IDs, ≤12 rows, watermarks and inter-page commits; no snapshot guarantee.
- Inspection/decisions compare exact immutable row/digest/revision/scope/source/
  origin and peer effects. Wrong ID, blind Enter, Escape/reopen use the real modal.
  Deleted inspect/history is NotFound; receipt retains identity only. No forensic
  erasure or human-identity assertion.
- Tab/Enter/Space/Alt and Alt+Right/Left transitions ran on every surface. E0
  navigation mapping remains unchanged; synthetic page callbacks are not used.
- Generation/request-byte alteration and lost/delayed replies are contained
  authenticated relay faults forwarding real daemon responses, not native typing
  of protocol fields. Altered digest is explicitly CLI-only. Other interference
  uses actual navigation/input and independent CLI processes.
- Lost Confirm reply has one submission/decision during a bounded 500 ms
  no-auto-retry observation. Manual immutable replay compares hashes/original
  receipt and unchanged history/counter. Changed bytes/new epoch refuse. This is
  bounded observation plus source/model evidence, not an infinite-time claim.
- History SQL caps at eight; valid terminal-state chains permit at most two
  decisions. No fabricated eight-decision valid fixture.
- EN/zh-CN Light/Dark compact/large GUI PNGs were inspected, plus compact wrong-ID
  dialogs, paging and corrected warnings. Compact captures may show the form with
  details below the fold; they do not prove simultaneous visibility of all fields.
  These are scoped agent image inspections, not independent human/IME/AT approval.
- GUI A cleanup records 22 closed drivers per host, all reaped/joined and no
  incomplete marker. Each TTY has 22/22 raw streams with alternate-screen restore;
  close asserts child success. Windows caller environments restore absent keys
  as well as prior values; user/system settings are unchanged.

Each `ledger.json` indexes `events.jsonl` with actual chords, observed probes/VT,
CLI hashes, UTC/monotonic time, capture names, owned PID/nonce and mechanisms.
The local evidence index hashes retained artifacts and indexes cleanup/restoration.
Probes alone do not prove pixels: the stale badge demonstrated that distinction.

## Adversarial authority/security

| Obligation | Result and evidence class |
| --- | --- |
| Operator reads vs independent live grant; agent/session denial | PASS, native daemon/CLI/IPC/session/MCP processes on both hosts |
| Exactly two project-bound MCP tools; private source/history/provenance exclusion | PASS, actual IPC/MCP; independent expected confirmed fallback/override values; exact Operator inspect retains private fields |
| Stale bindings, strict decode, foreign/unallowlisted/symlink/junction roots, revoke, second writer | PASS, real process suites and explicitly contained identity-corruption fixtures |
| Precise root drift at final admission | Historical component FI PASS; C independent test-built daemon process FI PASS on Windows and Linux/WSL, actual native IPC and unchanged bytes/rows/receipts/decisions |
| Fresh exact v1; digest vs CAS; transitions; atomic peers/receipts/history/deletion | PASS, strict store/writer units plus real CLI/IPC lifecycle/concurrency |
| Queue/history/4096 ceiling, Capacity, Busy, replay/conflict | PASS, process queries, explicitly injected valid 4096-receipt fixture, writer/IPC units and real SQLite contention |
| Confirmed-only, project/global fallback, historical Handoff PASS | PASS, core/MCP/process suites; historical evidence never becomes fresh verification |
| Production ProjectContract automatic masking | BLOCKED: no production structured-contract consumer; typed-context unit PASS is not runtime enforcement |
| Client-reported CLI/Desktop/TUI channel never authorizes | PASS, process denials and normally authorized labelled decisions; channel is not human identity |
| Browser/offline/Windows→WSL stay read-only | PASS within model/source and native target-invalidation evidence; authenticated browser/remote-write acceptance NOT_RUN |
| Direct SQL and sole writer | PASS for production read-only connection/independent daemon lease; no sandbox claim against hostile same-user raw SQLite access |

Eight new `crates/acp-daemon/tests/s3_review_acceptance.rs` regressions passed on
both hosts in full gates:

- `s3_concurrent_cli_cas_and_identical_replay_commit_one_decision`
- `s3_session_review_denial_and_client_channels_never_supply_authority`
- `s3_stale_request_bindings_refuse_before_replay_and_strict_decode`
- `s3_contained_4096_fixture_process_capacity_conflict_and_exact_replay`
- `s3_symlink_or_junction_swapped_root_and_foreign_registration_refuse`
- `s3_confirmed_private_sources_never_leak_through_real_ipc_or_mcp`
- `s3_contained_identity_corruption_never_rebinds_an_existing_writer`
- `s3_intervening_commit_changes_page_watermark_without_snapshot_claim`

`ownership_tests::s3_root_drift_between_initial_check_and_final_admission_refuses_review`
is the distinguished component checkpoint test. Existing daemon lease/restart/
substitution/root/revocation, store DDL/causality/refusal, writer transition/
rollback/deletion/capacity, core typed-context, MCP scope/privacy/two-tool, model
stale/no-write and WSL native-input cases also ran. Full logs retain exact names;
none is relabelled as physical keyboard proof.

Independent source review found source disclosure and the production-masking
discrepancy, ran no host tests and returned NOT CLEAR. Subsequent tests prove the
source correction, not an independent re-review or human acceptance PASS.

## Reviewable corrections

| Commit | Correction |
| --- | --- |
| `b9775e9` | Omit private preference source from context for all roles; retain Operator inspection; core regression and explicit masking gap |
| `6bf7cf7` | Restore Windows helper environment on setup/success/failure; `[NullString]::Value` genuinely removes new variables on current PowerShell/.NET |
| `e0cd4cc` | Eight process regressions and one admission unit; correct existing six-table/two-index inventory and envelope format 2, no DDL change |
| `55769ff` | Bounded 37-cell ledger and opt-in content-free focus observation; existing controls/ConPTY handles preserved |
| `44b989f` | Remove false physical-cycle inference from repeated labels; retain 128-Tab bound; development selector cannot claim full acceptance |
| `57386fa` | Native queue drift becomes readable localized warning text, without changing queue/permission semantics or SDK |
| `75d40c4` | Independent test-built daemon checkpoint over native IPC; owner decisions retain #65 blocker and explicitly allow same-physical-host environments |
| `3f4c534` | Remove inherited trailing blank line; preserve failed branch preflight |
| `19942b6` | Synchronize actual input focus and pace synthetic text; preserve exact assertions and avoid native sessions during builds |

No local confirmation approves a ProjectContract. Agent, prompt, session, Handoff,
unverified source/origin and client channel never self-authorize.

## Binary SHA-256

Current C fingerprints are recorded in the private evidence index and table
below. Executables were not rebuilt during C's native sessions. Historical A/B distinguish
complete-run binaries from the corrected UI; other binaries remain unchanged.

| C binary | Linux/WSL | Native Windows |
| --- | --- | --- |
| daemon | `883363045fc8821b229a8e4cabfc6128613bde9ba6101777bf2de2eb193a06da` | `000433387c5fb3e61c19ebb78b54404c0fd35da9c9d59394724c09c569050159` |
| query | `212b80609606bbe6d184d3e5dc92644ecc8582d10099738485dc95b01d6e694f` | `57df17d22e3eed2805f6df4a328a34eee1e8f83b33799f21025eea7416d1e2f6` |
| MCP | `baacb5d0f24b95c61874ba4855842cb559e083341827f841152770bc094a9ef6` | `cdab23a6a7015a10ce2ac541befb71879daf1dada02b6c92f52eb191fb4e60af` |
| NSH Desktop | `98c74bc0af8732b64460a9b5dd9d5d248fea67d426b63f79e3c8cd56f62adefa` | `87ac827ef0740a2265abb1fa063c0f60ae842867016ae0db2f0b54759a5067f3` |
| NSH TUI | `1616f8a0ebfb7d98700545c471461ea09c33bd24d4a6b6c489f87704f82684a4` | `7f94767ce9ddc9effe137e6314ae56f326fcacaec1386b66ab57ad7873860dcb` |
| acceptance driver | `1ff38e3f134046ab79f9962d0f3c46730219021cb44a76fa24f624f3fa81d61d` | `3818c8974f5c54db20dd086ad3cf663e64a64e89f0199e801ec8083321c1feb2` |

| Binary / cohort | Linux/WSL | Windows |
| --- | --- | --- |
| daemon, A/B | `fae31e195dcaeabb9356ba80adc7480f36e2fe5ec416f9d280ed84e82ec0423f` | `584b74c19a905e0702d74ca93b2359fadee304d001f8b2f097db0a1d272d5eb0` |
| query, A/B | `759b748a0be72bc364c54c1f8c82ae3496d6cdca53bad648ff8166e092575b08` | `abaa27472dd00db8b855af1217ce104ac424e58e2c8e70b88c46f1ef89500bde` |
| MCP, full gates | `c2ddffba3d4b81348877d35ef141b3cf1eacbaff46d072098a3cf06439b6dd6f` | `3a76c7877d5e71db450f9c854beeaf4874e696b48c5fdb1dc9a7ea47426af6e7` |
| native UI, A | `514a298fd76faefd4131259d87a4808bb4020a6a22c5a0ec0c2a556680b85d67` | `37f1e3b48e6c9411785aaa5a13a68f07299275ab6ea37c80ccc13c77dbbff57a` |
| TUI, A | `69134ba27e0edae7417c0daacf3d08862ac936b4583cbd63e101aaa0b8a4af26` | `1bf6d2df768d2cc2e3dbf659e3308aa0777b698b3003f6a4b4f4b6530ef0c9e1` |
| driver, A/B | `5a459b58118038c8f9af0a73bec35daff4b94348b2a2679ee774d5b94e60a95e` | `d0dc911c85852d803987cfa96ae12e528dacbe63e8cfb9dec6ebbf3d15207390` |
| native UI, B | `213b858f3994e2684b53942f0f736ed26f23b2b004ebee8ad4b3f089ff29b0f5` | `17ff7b3521cb08de95c70a71828748ba4f5afff7ac22ab9141ed9bf4909cbce1` |
| TUI, B | `639354c5ba36fc310489852d6f4c82208e781176dd9952f98ca62099e2cd9b9e` | `5b2e55b67d70d7eab84819c4087457bd286ccc9ce8685b8f9b3ba65801e206da` |

## Failures retained

- Review preflight `75d40c4` failed baseline whitespace on the report's inherited
  trailing blank line; corrected in `3f4c534`. No suite was passed by suppressing it.
- `3f4c534` complete gates passed on both OS environments, but native PTY ended
  **19 PASS / 4 FAIL / 14 NOT_RUN**, and WSLg **20 PASS / 2 FAIL / 15 NOT_RUN**.
  Exact fields visibly lost characters with bulk/4 ms input; original assertions
  and images remain. C's focus synchronization and single-send pacing fix this
  acceptance path, not rapid-input stress behavior.
- `3f4c534` ConPTY logged all 37 cells PASS but exited 101 without a final report:
  it was started before the remaining NSH gates ended, and the final binary check
  observed a replaced TUI executable. It remains **INCOMPLETE**, not native PASS.
  Its original ledger/log and `final-refusal-diagnosis.json` are retained. All C
  native sessions start only after the respective builds/tests have completed.
- Wrong initial WSL share: no test executed; corrected to observed Arch Linux.
- Initial environment restore retained empty MSVC keys. Native probe established
  PowerShell `$null` coercion/.NET empty-variable behavior; corrected ten-case
  fixture passes.
- Owned launcher script-scoped Cargo dispatch failed from its nested helper
  before Cargo tests; corrected process-local executable dispatch.
- Windows source check lacked local YAML; `npm.cmd` was also an incorrect shim
  assumption. Existing `npm.exe ci --prefix apps/operator/tests --ignore-scripts`
  installed three locked packages, then native source verification passed.
- Initial driver Clippy had two lints; corrected before acceptance.
- First PTY at `55769ff` had baseline PASS but false focus-cycle BLOCKED cells.
  Focused PTY/ConPTY smoke preceded clean A complete runs.
- First WSLg batch was cancelled after owner questioned repeated tests; incomplete
  cells are not PASS and last owned GUI PID was confirmed absent. Owner then
  explicitly selected full four-surface execution after minimal proof, without
  repeating unaffected product gates.
- Native stale-badge pixels failed despite automated PASS; settled capture and
  row-wrapper attempt did not fix it. Only verified text correction is retained.
- One affected WSLg attempt lost foreground before useful interaction and failed;
  it reaped/joined children. GUI work was subsequently serialized, never forced.
- Relocating a capture-only helper generated three nonce-owned PNG groups in an
  unintended nested artifact path. They were moved intact into ignored evidence,
  not deleted. Dirty exploratory runs are excluded from final clean B native
  attribution; four Linux scoped cases were repeated after relocation.
- Throwaway capture/host scripts were removed after recording hashes. Raw logs,
  images, receipts, VT/event streams and source fingerprints remain preserved.

No suppressed errors, fabricated daemon responses, hidden contract weakening,
legacy/Python runtime restoration or mock-to-native PASS conversion.

## Private local evidence

These are local ignored path references, not uploaded artifacts or dead hosted
repository links. WSL retains historical indexes/copies of Windows A/B evidence;
C Windows raw evidence remains in the designated native Windows checkout.
The [driver README](../../tools/native-acceptance/README.md) documents invocation,
isolation, diagnostics, ownership and evidence formats.

- Full gates: `artifacts/local-p1-m4/s3-55769ff-linux-gates/`,
  `s3-55769ff-windows-gates-r2/`, `s3-55769ff-windows-driver/`.
- Corrected driver: `s3-44b989f-windows-driver/`; Linux logs under `s3-helper/`.
- Complete native: `s3-44b989f-{hwnd,wslg,conpty,pty}/` under the same root;
  `s3-44b989f-evidence-index.json` hashes artifacts and indexes ownership/cleanup.
- Corrected UI gates: `s3-57386fa-{linux,windows}-ui/`.
- Clean scoped B: `s3-57386fa-clean-{pty,wslg}-{stale-page-disclosure,global-paging}/`
  and `s3-57386fa-{conpty,hwnd}-{stale-page-disclosure,global-paging}/`;
  `s3-57386fa-supplemental-index.json` preserves explicit selected/NOT_RUN status.
- Failures, helper fixture, prerequisite resolution, paired-host metadata and
  immutable product input fingerprints: `s3-helper/`; original complete-run
  attempts `s3-55769ff-{pty,wslg}/`; settled/failed-wrapper/corrected development
  captures and owned relocated PNGs remain in their diagnostic directories.

**Disposition:** scoped privacy, harness and native-disclosure defects are corrected
with the evidence above. Runtime masking and unexecuted evidence classes remain
visible to owner review; this ledger does not authorize M4 PASS or closing #52.
