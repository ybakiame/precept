# P1-M5/S0 — candidate support and release-readiness contract

Status: **S0 contract accepted; S1 complete (S1-A #70, S1-B #73, S1-C #72, notice review S1-D #76); first-candidate
scope narrowed by owner decision 2026-11-18 (see D1/D5/D6).** Authority:
[S0 #68](https://github.com/ybakiame/agent-control-plane/issues/68),
[M5 #67](https://github.com/ybakiame/agent-control-plane/issues/67),
[ADR-0003](../adr/0003-ui-framework-selection.md),
[ADR-0004](../adr/0004-product-domain-trust.md),
[ADR-0006](../adr/0006-rust-only-runtime.md). The retained single-writer rules in
[ADR-0005](../adr/0005-local-storage-and-write-ownership.md) still apply.

Audited main: **`6d8000153107c67edfa5ff3bc77da15f196623d3`**. M4 and N precedence
#65 are closed at their engineering scope. ACP remains a Rust-only **0.0.x-rc
synthetic engineering prototype**. This proposal does not authorize an installer,
real Profile, public binary, release tag or support commitment. The companion
[gap inventory](../evaluation/p1-m5-s0-gap-inventory.md) maps source and exact prior
evidence to the gates below. Historical reports retain their original verdicts.

*Historical S0 direction (2026-11-16), superseded for standalone Linux by the
2026-11-18 decision below:* the [2026-11-16 owner review](https://github.com/ybakiame/agent-control-plane/pull/69)
and [recorded platform decision](https://github.com/ybakiame/agent-control-plane/issues/68#issuecomment-6916187539)
confirm **Windows x86_64 portable ZIP only**, **standalone Linux x86_64 native
Wayland Desktop + PTY TUI**, and **macOS deferred**. Both Windows HWND Desktop and
ConPTY TUI remain in scope. These are candidate directions, not clean-install
PASS, supported-release claims or authorization to package/publish/use real data.
Other defaults below remain recommendations to finalize at the applicable gate.

**Owner decision 2026-11-18** ([#67](https://github.com/ybakiame/agent-control-plane/issues/67)):
the first candidate narrows to Windows HWND/ConPTY (C1/C2) and the primary Windows GUI →
WSL2 workflow (C6), with WSLg (C5) as a separately labeled secondary target.
**Standalone Linux (C3/C4) is deferred** and is not an M5 blocker; its §1 text below
stays as the direction for a later standalone-Linux slice. G13–G16 are NOT_RUN under
this narrowed claim, and G18 waits for owner-approved budgets.

## 1. Support vocabulary and proposed targets

These labels describe different axes, not an automatic promotion ladder:

- **Candidate:** work direction may be owner-confirmed, while measured floors,
  clean-target verification and a support commitment remain separate.
- **Proven in development:** a named historical native check passed at its SHA.
- **Clean-install verified:** the eventual approved package ran on a recorded clean
  target without the developer's caches/toolchain; currently NOT_RUN everywhere.
- **Support commitment:** an explicit owner choice of OS versions, architecture,
  features, limits and maintenance scope after evidence; currently none.
- **Deferred/excluded from initial candidate:** a product scope decision, labeled
  as confirmed or recommended below; no inference of technical impossibility from
  missing tests.

| ID / surface | Decision / scope status | Existing development evidence | Missing release boundary |
| --- | --- | --- | --- |
| C1 Windows x86_64 Desktop / HWND | **Owner-confirmed initial candidate; portable ZIP only** | E1 native keyboard/modal/theme/resize at `19942b6`; E2 backend at `435f4bd` | Exact stable OS floor, loader/runtime closure, clean ZIP extraction/start and human IME/AT |
| C2 Windows x86_64 TUI / ConPTY | **Owner-confirmed initial candidate alongside C1**, same ZIP direction | E1 real ConPTY and restoration | Supported terminal/version, paste/composition/scaling, clean launch/exit |
| C3 standalone Linux x86_64 Desktop / native Wayland | **Owner decision 2026-11-18 ([#67](https://github.com/ybakiame/agent-control-plane/issues/67)): deferred from the first candidate**; standalone Linux host/VM acceptance is not an M5 blocker and no standalone Linux claim is made | Linux backend source exists; **no standalone distro native acceptance** in E1/E2 | Exact distro/libc/GTK/compositor floor and archive closure; actual native rendering/input/resize |
| C4 standalone Linux x86_64 TUI / PTY | **Owner decision 2026-11-18 ([#67](https://github.com/ybakiame/agent-control-plane/issues/67)): deferred from the first candidate**; standalone Linux host/VM acceptance is not an M5 blocker and no standalone Linux claim is made | E1 is a WSL PTY run, not a standalone host | Terminal/distro selection; standalone PTY restoration and TUI dependency closure |
| C5 WSLg direct native Wayland + WSL PTY | Retain as separately labeled **secondary** integration target ([S1-C #72](https://github.com/ybakiame/agent-control-plane/issues/72): WSLg-provided Weston, local-only Linux `.tar.gz` build/validation; no PR or Release artifact) | E1 `GdkWaylandDisplay` / `GskGLRenderer`, resize, PTY; E2 WSL backend; [S1-C evidence](../evaluation/p1-m5-s1c-wslg-wayland-portable.md): locally built package exercised on a clean Ubuntu 24.04 WSL2 + WSLg target, offline; archive remains ignored and unuploaded; `GdkWaylandDisplay`, Mesa software by default and opt-in d3d12; PTY | Exact Windows/WSL/distro claim and clean evidence if selected; never substitutes for C3/C4 |
| C6 Windows GUI → WSL2 daemon | **Owner decision 2026-11-17: primary product workflow** — full existing Operator and Sessions GUI parity through the persistent typed `acp-wsl-bridge` ([S1-B #73](https://github.com/ybakiame/agent-control-plane/issues/73)) | [S1-B evidence](../evaluation/p1-m5-s1b-windows-wsl-operator.md): HWND GUI → disposable Ubuntu 24.04 WSL2 daemon | Shipping the Linux helper in a package (S1-C), human IME/AT, distro/Windows matrix and release claim |
| C7 macOS / other CPU architectures | **macOS owner-deferred from first candidate**; no other architecture selected | Historical older macOS evidence / source targets only | Current tests NOT_RUN; no macOS/Apple Silicon support or technical-impossibility claim |
| C8 mobile / authenticated browser UI | Remain outside this initial candidate scope | Historical experiments; current offline Web fixture only | No mobile or authenticated browser release claim |

No niri, X11/XWayland or Xvfb prerequisite or acceptance substitution is proposed.
Begin with one clean ordinary standalone Wayland environment for C3 and test both
GUI and PTY TUI there. Exact distro/compositor selection follows S1 evidence;
expand compositor claims only when tested, without a large compositor matrix per
small Issue. WSLg needs an available native Wayland session, not compositor
launch by the application. Missing host access is NOT_RUN/BLOCKED, not PASS.

### Grounded prerequisites, not guessed minimums

The only measured CPU family is x86_64. The Windows development record identifies
OS `10.0.26300.0`; it does **not** establish a Windows 10/11 minimum or a clean
stable-channel baseline. The review recommends a currently maintained **Windows 11
x86_64 candidate**; that is not a confirmed exact OS build/minimum. Leave the floor
**unfrozen** until S1 API/import and clean-target evidence selects it. Likewise no
minimum Linux distro, glibc, kernel, WSL or
terminal version can be extracted from one Arch/WSL development host.

The approved NSH pin is **`b7f31a08478be262fd7b76ec856036fd5b8ffdb0`**, upstream
workspace 0.5.0, not stable 0.5.1. Its pinned Linux manifest uses GTK4 and
GStreamer; workspace `gtk4` enables **`v4_14`**. This is a source/API prerequisite,
not proof that GTK 4.14 is a sufficient complete runtime floor. E1 actually used
GTK 4.24.1. Resolve the exact ELF/shared-library/media/plugin/font closure in S1.
The Windows backend references Win32 graphics and WebView2 COM crates; audit actual
imports/used features before requiring or bundling a WebView2 runtime. A dependency
declaration alone is not proof that every optional subsystem must be installed.

Builder baseline: repository `rust-toolchain.toml` pins **1.99.1** with fmt/Clippy;
core manifests declare rust-version 1.99. Lock root, app and any maintenance-driver
workspaces separately. Linux GUI builds need native GTK/Wayland/xkbcommon and
upstream build/link dependencies; CI's package list is a hint, not the full closure.
Windows development used MSVC/SDK; exact recorded versions are in E1. Windows E1/E2
used a non-stock global Cargo frontend; S1 must choose/document its reproducible
builder instead of implying stock-toolchain verification. Node 24+ and PowerShell
verification helpers are developer tools, not automatically product runtime needs.
Git is used for live repository inspection; missing/unsafe Git evidence must remain
unknown/refused. SQLite uses the `bundled` Rust feature; inspect final linkage before
adding any user SQLite prerequisite. Test Latin/CJK font fallback on clean targets.

## 2. Proposed minimum distribution envelope

**Confirmed Windows shape: one versioned portable ZIP**, with HWND Desktop,
ConPTY TUI and the existing daemon/query/MCP tools. ZIP extraction must need no
installer, Windows service, elevation, PATH edit, scheduled task, auto-update or
persistent registry modification. Product removal preserves Profile/auth/unknown
files; this choice is not permission to produce or publish a package during S0.

**Recommended Linux shape, not yet an owner-selected format: one versioned portable
`.tar.gz` per chosen architecture**, with native Wayland Desktop, PTY TUI and the
same runtime tools plus manifest/hashes/notices and documented libraries. Do not
start DEB/RPM/AppImage work before dependency audit and approval. Linux TUI dependency
independence is a gate, not an assumption from feature names. Inspect each executable;
if TUI unnecessarily requires GUI libraries, report the smallest build/package split
for approval before changing the envelope.

| Contents | Proposed role / boundary |
| --- | --- |
| `acp-daemon`, `acp-query`, `acp-mcp` (`.exe` on Windows) | Explicit native daemon process, operator CLI and two-tool stdio bridge; same candidate revision, no OS service registration |
| `acp-nsh-native`, `acp-nsh-tui` | Shared product model, separate surface entrypoints; no direct SQLite or auto-started daemon |
| Version/source manifest, SHA-256 list, instructions, license/notice texts | Exact Git SHA/tree, toolchain/target, lockfile digests, NSH pin, build flags and dependency provenance |
| Platform dependency files only after inventory | Copy only approved redistributable runtime assets; exact per-file list, no developer cache/target-directory glob |

Do not ship fake agents, acceptance drivers, `acp-ui-snapshot`, generated fixtures,
raw logs, Profiles, credentials, agent HOME or a source checkout as runtime data.
The offline Web packager is a synthetic fixture tool, not a native installer or
authenticated browser gateway. Optional vendor agents and their authentication
remain external; no download/login/model call is required by this minimum envelope.

Separate Desktop/TUI archives are a Linux alternative only if dependency evidence
justifies a reviewed split; do not silently separate or demote TUI. Windows installer
formats are outside the confirmed portable-only direction. Linux OS-specific
installers are not selected. No signing, cloud feed or telemetry is selected here.

### Startup, state separation and removal

1. Operator selects a private **local** runtime directory, endpoint and allowlisted
   project roots. Start the daemon explicitly; no service registration, login task,
   persistent PATH/registry edit, scheduled task, auto-update, automatic privilege
   elevation or broad root discovery.
2. Start with no selected Profile or an explicitly selected validated fresh v1.
   Profile create/select/write-enable remain separate deliberate operator actions;
   packaging must never call them implicitly or adopt/reset an existing database.
3. Operator attaches CLI/GUI/TUI with explicit endpoint and the appropriate native
   capability. MCP uses a distinct project-bound read credential. Operator reads,
   write grant and session context lease are not interchangeable. No tokens in argv,
   logs, archive, repo, shared folder or cross-OS copied state.
4. Closing a UI does not stop the daemon or managed child. Explicit daemon shutdown
   drains/disables writer authority and shuts sessions; verify process exit, lease
   release and recognized credential/socket cleanup separately on each target.
   Crash/restart is a different gate; no blanket `kill` or filesystem cleanup.
5. Removing the archive removes product files only. Preserve user-selected Profile,
   authentication and unknown files. Never recursively delete a user data/runtime
   root, an in-use lock or a database sidecar. Any later explicit data-erasure UX
   needs its own authorization and contract; M3 backup/recovery is not bundled here.

Possible **operator-selected examples**, not implemented defaults: a Profile below
`%LOCALAPPDATA%\ACP\profiles\<selection>\` on Windows or
`$XDG_DATA_HOME/acp/profiles/<selection>/` on Linux; a separate runtime below
`%LOCALAPPDATA%\ACP\runtime\<selection>\` or `$XDG_RUNTIME_DIR/acp-<selection>/`.
Variables must be resolved by the operator; no new resolver/default-path behavior
is authorized. Missing variables, network/interop storage or overlong Unix socket
paths must refuse or request another explicit choice, not weaken privacy. Windows
and WSL daemons keep separate local filesystems and capabilities.

Unix private directories/files require owner-only 0700/0600; Windows uses validated
current-user-only DACLs and refuses unsafe reparse/identity cases. Preserve the
existing native Profile/file binding checks. Install location and executable
permissions must not permit another user to replace a launched binary; actual ACL,
symlink, no-execute and other-user access checks remain release gates. Writable
Profile roots never become binary search paths.

### Diagnostics, integrity and notices

Current daemon errors are sanitized classes; CLI intentionally prints explicitly
requested record content and must not become a background support log. MCP stdout
is protocol-only. UI probes and snapshots are validation tooling, not an approved
crash-report format. S1/S3 must define opt-in, bounded diagnostic capture with no
tokens, raw SQLite errors, private paths, values, Handoff bodies, conversations or
device identifiers; crash dumps can contain memory and must not auto-upload.
Persistent log location/retention and panic handling are currently unverified.

Archive hashes detect byte differences, not signer identity or reproducible builds.
Record build provenance and licenses before claiming either. Root crate metadata
contains Apache-2.0 declarations, while no root LICENSE/NOTICE or full third-party
distribution inventory was found; app package metadata is incomplete for that
purpose. The Web helper copies one upstream NSH license only. Owner must approve
the project's license text/notice policy, and S1 must audit the actual shipped
dependency/font/runtime files. A lockfile is not a completed license or SBOM audit.

## 3. Traceable release gates

G01–G20 are an **inventory for selecting applicable gates by support claim, phase
and actual risk**, not twenty automatic blocking checks per Issue/PR. A later task
records selected gate IDs, target/claim, method and acceptance threshold before
running them; a required selected gate cannot be silently waived. Unselected
inventory entries do not block a documentation-only S0 or an unrelated patch.
Historical PASS in E1/E2 stays attached to those SHAs.
Current candidate results are NOT_RUN unless a prerequisite already blocks them.
S = focused/static, M = one platform/process or bounded human session, L = clean
environment/dependency investigation or multi-surface human work. These are
complexity estimates, not elapsed measurements or delivery promises.

Actors: **O** = product owner `ybakiame` (scope/go-no-go); **E** = assigned engineer
(Codex may implement/run synthetic tests); **H** = separately identified human
reviewer; **T** = clean-target operator. Concrete H/T assignments are still
unconfirmed. For initial engineering gates, recommend independently provisioned,
isolated clean Windows targets, plus **standalone** Linux targets once C3/C4 resumes
(VMs or separate installs; not needed for the narrowed first candidate);
two physical machines are not an automatic requirement. This does not inherit M4's
shared Windows/WSL exception or count WSL as standalone Linux. An independent human
task reviewer is needed at later claimed human-use/release gates, not to finish S0.
A GitHub COMMENT using the author account is not independent
human test evidence. Every future artifact records actual actor, host, method,
clean SHA/tree, binary/package hashes, result and limitations; keep raw data private.

| Gate | Feature / method / host | Actor | Historical evidence / future artifact | Candidate state | Cost |
| --- | --- | --- | --- | --- | --- |
| G01 | Exact locked build and shipped binaries; each selected OS/arch | E | E1/E2; build manifest, import/dependency closure and hashes | NOT_RUN | M |
| G02 | No-toolchain/no-cache offline ZIP extraction/start on Windows; standalone Linux archive launch; C5 separately if claimed | E+T | No prior clean-install evidence; isolated target image/package/dependency log | Windows ZIP: PASS on the clean Sandbox target ([S1-A](../evaluation/p1-m5-s1a-windows-portable.md)); C5 WSLg: PASS on a clean Ubuntu 24.04 WSL2 target ([S1-C](../evaluation/p1-m5-s1c-wslg-wayland-portable.md)); standalone Linux deferred (C3/C4) | L |
| G03 | Native private executable/IPC/capability permissions, second-user and unsafe-root denials | E+T | E2 cooperative/native negatives; packaged ACL/permission process report | NOT_RUN | M |
| G04 | Fresh exact v1, no-overwrite/refusal, sole writer, lease/epoch/grant/revoke/CAS/replay | E | E2 Workspace; release-build real daemon/CLI/process report | NOT_RUN | M |
| G05 | ProjectContract trust, malformed/unavailable/changed fail-closed, selector isolation, private-field exclusions | E | E2 actual IPC/MCP + test-daemon FI; release response/adversarial report | NOT_RUN | M |
| G06 | Exactly two read-only MCP tools, bound projects; session revoke before publication; no automatic model/agent start | E | E2; packaged process/stdio/lease report, no provider needed | NOT_RUN | M |
| G07 | Normal stop, crash/restart, endpoint collision, owned cleanup and retained state; every packaged OS | E+T | E1/E2 scoped process evidence; exit/lock/credential/restoration report | NOT_RUN | M |
| G08 | Offline runtime closure including media/plugin/font assets and no silent network; GUI/TUI individually | E+T | Source inventory only; clean-target import trace and disconnected run | NOT_RUN | L |
| G09 | Remove only listed portable product files; retain Profile/auth/unknown files; occupied/unsafe paths refuse | E+T | Archive file list + synthetic retention comparison; no Windows uninstaller required | NOT_RUN | M |
| G10 | Safe startup/errors/panic/crash diagnostics, byte/time caps, no implicit upload | E+H | Sanitized errors exist; adversarial sentinel/redaction and dump-policy audit | NOT_RUN | M |
| G11 | Version/tag semantics, checksums, builder/source provenance, license/notice completeness | E+O | Three lockfiles/pin exist; package manifest and reviewed notice ledger | PARTIAL: provenance/checksums and the **reviewed notice ledger** PASS ([S1-D](../evaluation/p1-m5-s1d-notice-review.md)); version/tag semantics and signing undecided, so BLOCKED for public distribution only | M |
| G12 | HWND and **standalone** native Wayland mapping/render/resize on selected clean targets; WSLg separately | E+T | E1 WSLg/HWND only; actual backend/renderer + owned captures | HWND PASS (S1-A/S1-B); WSLg PASS separately (S1-C); standalone Wayland deferred (C3/C4) | L |
| G13 | Actual keyboard/focus/destructive modal consent, stale state, uncertain result and manual replay; all claimed surfaces | E+H | E1 paced synthetic native input; human task script and receipts | **NOT_RUN by owner decision 2026-11-18:** outside the narrowed first-candidate claim (no human-use, IME, scaling/locale or AT claim) | L |
| G14 | Rapid paste, Unicode, Windows Chinese IME compose/commit/cancel and chosen Linux input method; TUI bracketed paste | E+H+T | E1 explicitly excludes rapid paste/full IME; exact input/observed state report | **NOT_RUN by owner decision 2026-11-18:** outside the narrowed first-candidate claim (no human-use, IME, scaling/locale or AT claim) | L |
| G15 | Scaling/contrast/fonts/compact viewport, EN/zh-CN and text growth; each chosen GUI/terminal | H+T | E1 scoped locale/theme/size; native human captures and clipping/fallback report | **NOT_RUN by owner decision 2026-11-18:** outside the narrowed first-candidate claim (no human-use, IME, scaling/locale or AT claim) | M |
| G16 | Named assistive technology, labels/roles/read order/focus and main tasks on chosen platforms | H+T | Full AT NOT_RUN; scoped task results with named tool/version and limits | **NOT_RUN by owner decision 2026-11-18:** outside the narrowed first-candidate claim (no human-use, IME, scaling/locale or AT claim) | L |
| G17 | PTY/ConPTY normal/error/cancel terminal restore, resize and consumed shortcuts | E+H | E1 22 streams per TTY; packaged native VT/frame + human inspection | **PASS (automated, scoped)**: packaged ConPTY render, resize, consumed Ctrl+Q and terminal restoration ([S1-A](../evaluation/p1-m5-s1a-windows-portable.md)); packaged WSLg PTY render, resize, Ctrl+Q, alt-screen and bracketed-paste restore ([S1-C](../evaluation/p1-m5-s1c-wslg-wayland-portable.md)). Error/cancel streams and human inspection NOT_RUN | M |
| G18 | Cold/warm startup, RSS/CPU/idle/stop, bounded queue/source/near-4096 Profile reads and 10k presentation stress | E+T | E1 10k component checks; E2 source/IPC caps; repeated release-build measurements | BLOCKED until the owner approves E-proposed budgets (decision 2026-11-18: measure a release-build baseline, propose, then run the gated measurement) | M |
| G19 | C6 explicit Windows→WSL2 targets: bridge protocol limits, daemon absence, unsafe binding, OperatorWrite/session authority, OutcomeUnknown and lifecycle | E+T | Existing helper source/scoped older tests; paired package native integration | NOT_RUN | M |
| G20 | Final clean candidate provenance, claim-selected required gate dispositions and owner decision | O+H+T | New RC evidence ledger referencing scoped prior results | BLOCKED: applicable RC evidence/go-no-go pending | L |

Before performance acceptance, E proposes cold/warm sample count, workload,
percentiles and numeric memory/latency/idle budgets against the selected host;
O approves them **before** pass/fail measurement. Current 1 MiB frames, 16 peers,
32 requests/peer, two-second frame timeout, 16 KiB contract source and 4096-commit
capacity are implementation limits, not user performance targets. A 10k-row UI
fixture is not a 10k-write Profile or proof that the 4096 ceiling is sufficient.
Observe capacity refusal honestly; do not add compaction or weaken integrity checks.

## 4. Owner decisions and go/no-go authority

| Decision | Confirmed direction / recommended default | What remains and when to finalize |
| --- | --- | --- |
| D1 targets/minimums | **Current first-candidate scope (2026-11-18):** C1/C2 Windows HWND/ConPTY and C6 Windows HWND → WSL2 (primary Operator/Session workflow) x86_64; C5 WSLg secondary; macOS deferred. *Historical S0 direction: C1–C4 x86_64, superseded for C3/C4 by the decision at right* | **Decided 2026-11-18:** first candidate = C1/C2 Windows HWND/ConPTY and C6 Windows GUI → WSL2, with C5 WSLg secondary; C3/C4 standalone Linux deferred and not an M5 blocker |
| D2 minimum features | **Recommended:** existing reviewed Profile/review/context/Handoff workflows, both UI surfaces, explicit daemon/CLI and exactly two readonly MCP tools | Finalize precise shipped feature claims in the relevant S1 scope. No onboarder/recovery promise, remote writes or vendor-model certification; existing explicit opt-ins remain |
| D3 packaging | **Confirmed Windows:** portable ZIP only, no installer/service/PATH/elevation/scheduled task/auto-update/persistent registry edit. **Recommended Linux package format:** one portable `.tar.gz` with both surfaces + three runtime tools | Validate the package recipe and dependency closure locally during S1; generated archives stay ignored and are not committed or uploaded. Formal distribution, signing and publication remain deferred to an approved release gate. If TUI cannot avoid GTK, report exact linkage conflict for owner review **before** splitting. No DEB/RPM/AppImage by default |
| D4 test ownership/hosts | **Recommended:** isolated independently provisioned clean Windows targets (standalone Linux targets only when C3/C4 resumes), VM or separate installation; no forced two-physical-machine rule | Assign exact targets/operator in S1; identify independent H only before claimed human-use/release tasks. No M4 exception carryover and no human reviewer prerequisite for S0 |
| D5 security/human/performance scope | **Review direction:** G01–G20 inventory, select by risk/claim per task. **Decided 2026-11-18:** G13–G16 are NOT_RUN under a narrowed first-candidate claim (no human-use/IME/AT claim); G18 budgets are proposed by E from a measured release-build baseline and approved by O before the gated run | Finalize task/actor/tool, sample counts, numeric budgets and pass/fail thresholds **before their RC acceptance run**. Do not invent thresholds in S0 or require every inventory row on each PR |
| D6 version/provenance/notices | Preserve existing 0.0.x-rc positioning. First-party Apache-2.0 confirmed (S1-A). **Third-party notice ledger reviewed** ([S1-D #76](https://github.com/ybakiame/agent-control-plane/issues/76)): permissive policy and resolved notice source for every shipped-closure package; both recipes refuse otherwise. **Recommended:** reconcile crate/MCP/package identifiers in S4 | Exact RC identifier/tag, license text/notice policy and publication remain unapproved; resolve them before the relevant package/distribution gate. Manifest metadata alone is insufficient |
| D7 activation/distribution | **Retained authority:** owner separately decides public distribution and real-profile activation after measured S4; no automatic approval from S0/M5 engineering PASS | RC audience, support commitment, release tag/publish and real-data authorization remain unapproved. This table does not grant any of them |

O may accept a narrower **candidate claim** with documented untested surfaces
before implementation; E cannot relabel a failed required gate or silently remove
TUI/Wayland. Full accessibility or IME support needs corresponding evidence, not
English input or color checks. H records observations, E records tests, O decides
claims and activation. Unknown exact floors, human assignments or numerical budgets
do not prevent finishing the S0 documentation contract; surface them at their
respective S1/RC gate. No pending/default recommendation authorizes work outside S0.

## 5. Proposed slices after S0 acceptance

Do not open/implement these as automatically authorized tasks. Once O accepts the
boundary, create narrow Issues with selected targets and gate IDs; one small PR
per concrete gap where possible.

| Slice | Depends on | Bounded deliverable / gates |
| --- | --- | --- |
| S1 package inventory and assembly | Revised S0 accepted and a separately scoped S1 task; confirmed platform/Windows ZIP direction | Expose and finalize relevant D1–D4/D6 choices from import/dependency/clean-target evidence; portable Windows ZIP, local-only Linux `.tar.gz` recipe/validation, both surfaces; no generated Linux archive upload. Select applicable G01–G03/G07–G11; no installer/service/auto-update |
| S2 native human-use gaps | Approved D1/D2/D4/D5; runnable S1 package for packaged acceptance | Timebox baseline human tasks, fix only observed keyboard/paste/IME/AT/scaling issues, affected-surface smoke (G12–G17). No redesign or toolkit upgrade by default |
| S3 release process/security/performance | S1 release-profile binaries and approved claims; for G18 only, owner-approved budgets | G03–G08/G10/G19 real process/offline/adversarial work. G18 follows as a separate step: measured release-build baseline, E-proposed budgets, owner approval, then the gated run. Do not repeat unrelated UI cases or speculate M3 recovery |
| S4 clean candidate and owner review | S1 and required S2/S3 gates; one clean RC SHA | G20 and approved clean-target checks; reference earlier scoped evidence accurately. A materially changed shared UI or explicitly accepted RC UI gate triggers one complete native matrix |

Routine documents use source/link/diff checks only. Backend changes use focused
unit/invariant + real daemon/IPC/MCP/process gates. Packaging changes test the changed
platform's clean install/teardown; a local UI fix uses affected-surface smoke.
The complete HWND/WSLg/ConPTY/PTY matrix is **not per Issue**: run it once at a major
shared NSH interaction/layout/rendering milestone or explicit RC-level UI gate.
Standalone Linux's selected native acceptance remains separate. No historical PASS
is transferred to a later SHA, package, compositor or human reviewer.

## S0 completion boundary

S0 delivers this reviewable proposal, its grounded gap inventory and current roadmap
alignment in a Draft PR referencing #68/#67. It changes no product/tooling behavior
and runs no installer, full Rust build, GUI/TTY matrix, model or hosted Action.
Owner acceptance freezes the chosen S1–S4 tasks; it does not mark M5 complete or
authorize real-user state, release publishing, migration or backup work.
