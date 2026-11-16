# P1-M5/S0 — candidate support and release-readiness contract

Status: **Proposed for owner approval; documentation only.** Authority:
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

## 1. Support vocabulary and proposed targets

These labels describe different axes, not an automatic promotion ladder:

- **Candidate:** proposed work scope; owner has not committed support.
- **Proven in development:** a named historical native check passed at its SHA.
- **Clean-install verified:** the eventual approved package ran on a recorded clean
  target without the developer's caches/toolchain; currently NOT_RUN everywhere.
- **Support commitment:** an explicit owner choice of OS versions, architecture,
  features, limits and maintenance scope after evidence; currently none.
- **Excluded from initial candidate:** a proposed product decision, not a claim
  that missing tests establish technical impossibility or upstream non-support.

| ID / surface | Proposal | Existing development evidence | Missing release boundary |
| --- | --- | --- | --- |
| C1 Windows x86_64 Desktop / HWND | Primary candidate | E1 native keyboard/modal/theme/resize at `19942b6`; E2 backend at `435f4bd` | Minimum stable OS version, loader/runtime closure, install and human IME/AT |
| C2 Windows x86_64 TUI / ConPTY | First-class candidate alongside C1 | E1 real ConPTY and restoration | Supported terminal/version, paste/composition/scaling, clean launch/exit |
| C3 standalone Linux x86_64 Desktop / native Wayland | Candidate requiring its own evidence | Linux backend source exists; **no standalone distro native acceptance** in E1/E2 | Select one distro/libc/compositor baseline; dependencies, native rendering/input and clean installation |
| C4 standalone Linux x86_64 TUI / PTY | First-class candidate alongside C3 | E1 is a WSL PTY run, not a standalone host | Select terminal/distro; standalone PTY restoration and dependency closure |
| C5 WSLg direct native Wayland + WSL PTY | Separately named integration candidate | E1 `GdkWaylandDisplay` / `GskGLRenderer`, resize, PTY; E2 WSL backend | Supported Windows/WSL/distro combination and clean VM/installation; cannot certify C3/C4 |
| C6 Windows GUI → WSL daemon reads | Bounded integration candidate | Existing typed read-only WSL helper/source and earlier scoped evidence | Same-version Windows/Linux packages and explicit read-only cross-boundary smoke; no WSL writes or managed execution |
| C7 macOS / other CPU architectures | Recommend exclude from first RC claims | Historical older macOS evidence / source targets only | Current tests NOT_RUN; not unsupported by inference |
| C8 mobile / authenticated browser UI | Recommend outside M5 initial scope | Historical experiments; current offline Web fixture only | No product release path or current certification |

No niri, X11/XWayland or Xvfb prerequisite or acceptance substitution is proposed.
Use one owner-selected ordinary standalone Wayland environment for C3, not a new
compositor matrix. WSLg needs an available native Wayland session, not compositor
launch by the application. Missing host access is NOT_RUN/BLOCKED, not PASS.

### Grounded prerequisites, not guessed minimums

The only measured CPU family is x86_64. The Windows development record identifies
OS `10.0.26300.0`; it does **not** establish a Windows 10/11 minimum or a clean
stable-channel baseline. Leave that floor **unfrozen** until owner selection and S1
API/loader validation. Likewise no minimum Linux distro, glibc, kernel, WSL or
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

Recommend **one versioned local archive per chosen OS/architecture, containing
Desktop and TUI together**, plus the three runtime tools. This retains both primary
surfaces without a new package manager/service framework. Linux TUI dependency
independence is a gate, not an assumption from feature names. Inspect each executable;
if TUI unnecessarily requires GUI libraries, report the smallest build/package split
for approval before changing the envelope.

| Contents | Proposed role / boundary |
| --- | --- |
| `acp-daemon`, `acp-query`, `acp-mcp` (`.exe` on Windows) | Explicit native service, operator CLI and two-tool stdio bridge; same candidate revision |
| `acp-nsh-native`, `acp-nsh-tui` | Shared product model, separate surface entrypoints; no direct SQLite or auto-started daemon |
| Version/source manifest, SHA-256 list, instructions, license/notice texts | Exact Git SHA/tree, toolchain/target, lockfile digests, NSH pin, build flags and dependency provenance |
| Platform dependency files only after inventory | Copy only approved redistributable runtime assets; exact per-file list, no developer cache/target-directory glob |

Do not ship fake agents, acceptance drivers, `acp-ui-snapshot`, generated fixtures,
raw logs, Profiles, credentials, agent HOME or a source checkout as runtime data.
The offline Web packager is a synthetic fixture tool, not a native installer or
authenticated browser gateway. Optional vendor agents and their authentication
remain external; no download/login/model call is required by this minimum envelope.

Alternatives for owner D3: separate Desktop and TUI archives reduce headless
dependencies but duplicate manifests/testing; an OS installer adds integration and
uninstall obligations before evidence exists. Prefer the archive first; no installer
format, signing, auto-update, cloud feed or telemetry is selected by S0.

### Startup, state separation and removal

1. Operator selects a private **local** runtime directory, endpoint and allowlisted
   project roots. Start the daemon explicitly; no service registration, login task,
   persistent PATH edit, automatic privilege elevation or broad root discovery.
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

Gate definitions below are **proposed required gates for chosen targets**, not
newly executed tests. Historical PASS in E1/E2 stays attached to those SHAs.
Current candidate results are NOT_RUN unless a prerequisite already blocks them.
S = focused/static, M = one platform/process or bounded human session, L = clean
environment/dependency investigation or multi-surface human work. These are
complexity estimates, not elapsed measurements or delivery promises.

Actors: **O** = product owner `ybakiame` (scope/go-no-go); **E** = assigned engineer
(Codex may implement/run synthetic tests); **H** = separately identified human
reviewer; **T** = owner-provided clean-target operator. H/T are **unassigned** until
owner acceptance. A GitHub COMMENT using the author account is not independent
human test evidence. Every future artifact records actual actor, host, method,
clean SHA/tree, binary/package hashes, result and limitations; keep raw data private.

| Gate | Feature / method / host | Actor | Historical evidence / future artifact | Candidate state | Cost |
| --- | --- | --- | --- | --- | --- |
| G01 | Exact locked build and shipped binaries; each selected OS/arch | E | E1/E2; build manifest, import/dependency closure and hashes | NOT_RUN | M |
| G02 | No-toolchain/no-cache offline extract/install/start; clean Windows, standalone Linux and C5 separately if claimed | E+T | No prior clean-install evidence; target image/package/dependency log | BLOCKED: targets/package unchosen | L |
| G03 | Native private executable/IPC/capability permissions, second-user and unsafe-root denials | E+T | E2 cooperative/native negatives; packaged ACL/permission process report | NOT_RUN | M |
| G04 | Fresh exact v1, no-overwrite/refusal, sole writer, lease/epoch/grant/revoke/CAS/replay | E | E2 Workspace; release-build real daemon/CLI/process report | NOT_RUN | M |
| G05 | ProjectContract trust, malformed/unavailable/changed fail-closed, selector isolation, private-field exclusions | E | E2 actual IPC/MCP + test-daemon FI; release response/adversarial report | NOT_RUN | M |
| G06 | Exactly two read-only MCP tools, bound projects; session revoke before publication; no automatic model/agent start | E | E2; packaged process/stdio/lease report, no provider needed | NOT_RUN | M |
| G07 | Normal stop, crash/restart, endpoint collision, owned cleanup and retained state; every packaged OS | E+T | E1/E2 scoped process evidence; exit/lock/credential/restoration report | NOT_RUN | M |
| G08 | Offline runtime closure including media/plugin/font assets and no silent network; GUI/TUI individually | E+T | Source inventory only; clean-target import trace and disconnected run | NOT_RUN | L |
| G09 | Remove only product files; retain Profile/auth/unknown files; occupied/unsafe paths refuse | E+T | No native uninstaller; synthetic file-ledger comparison | NOT_RUN | M |
| G10 | Safe startup/errors/panic/crash diagnostics, byte/time caps, no implicit upload | E+H | Sanitized errors exist; adversarial sentinel/redaction and dump-policy audit | NOT_RUN | M |
| G11 | Version/tag semantics, checksums, builder/source provenance, license/notice completeness | E+O | Three lockfiles/pin exist; package manifest and reviewed notice ledger | BLOCKED: D6/notice gaps | M |
| G12 | HWND and **standalone** native Wayland mapping/render/resize on selected clean targets; WSLg separately | E+T | E1 WSLg/HWND only; actual backend/renderer + owned captures | NOT_RUN | L |
| G13 | Actual keyboard/focus/destructive modal consent, stale state, uncertain result and manual replay; all claimed surfaces | E+H | E1 paced synthetic native input; human task script and receipts | NOT_RUN | L |
| G14 | Rapid paste, Unicode, Windows Chinese IME compose/commit/cancel and chosen Linux input method; TUI bracketed paste | E+H+T | E1 explicitly excludes rapid paste/full IME; exact input/observed state report | NOT_RUN | L |
| G15 | Scaling/contrast/fonts/compact viewport, EN/zh-CN and text growth; each chosen GUI/terminal | H+T | E1 scoped locale/theme/size; native human captures and clipping/fallback report | NOT_RUN | M |
| G16 | Named assistive technology, labels/roles/read order/focus and main tasks on chosen platforms | H+T | Full AT NOT_RUN; scoped task results with named tool/version and limits | BLOCKED: human/AT scope unassigned | L |
| G17 | PTY/ConPTY normal/error/cancel terminal restore, resize and consumed shortcuts | E+H | E1 22 streams per TTY; packaged native VT/frame + human inspection | NOT_RUN | M |
| G18 | Cold/warm startup, RSS/CPU/idle/stop, bounded queue/source/near-4096 Profile reads and 10k presentation stress | E+T | E1 10k component checks; E2 source/IPC caps; repeated release-build measurements | BLOCKED: numeric budgets/hosts unapproved | M |
| G19 | C6 explicit Windows→WSL read targets, daemon absence, unsafe binding and mutation/session denial | E+T | Existing helper source/scoped older tests; paired package native integration | NOT_RUN | M |
| G20 | Final clean candidate provenance, all required gate dispositions and owner decision | O+H+T | New RC evidence ledger referencing scoped prior results | BLOCKED: D1–D7 and preceding gates | L |

Before performance acceptance, E proposes cold/warm sample count, workload,
percentiles and numeric memory/latency/idle budgets against the selected host;
O approves them **before** pass/fail measurement. Current 1 MiB frames, 16 peers,
32 requests/peer, two-second frame timeout, 16 KiB contract source and 4096-commit
capacity are implementation limits, not user performance targets. A 10k-row UI
fixture is not a 10k-write Profile or proof that the 4096 ceiling is sufficient.
Observe capacity refusal honestly; do not add compaction or weaken integrity checks.

## 4. Owner decisions and go/no-go authority

| Decision | Recommendation for approval | Required confirmation |
| --- | --- | --- |
| D1 targets/minimums | C1/C2 primary; C3/C4 real standalone candidates; C5 separate integration; C6 reads only | Choose exact stable OS/distro/arch/terminal/Wayland/WSL baselines; no implied broad support or M4 host-exception carryover |
| D2 minimum features | Existing native explicit Profile/review/Handoff/context workflows, both UI surfaces and two-tool MCP | Select claimed workflows and known limits; vendor managed sessions remain separately opted in, no vendor certification |
| D3 packaging | One portable versioned archive per selected OS with Desktop + TUI + three runtime tools | Accept archive vs split vs installer; approve runtime dependency/permission policy after S1 audit |
| D4 test ownership/hosts | Synthetic VM/clean target controlled by T; named H for human tasks | Supply/assign targets and actors; decide host independence for M5 explicitly, not by inheriting M4's exception |
| D5 security/human/performance scope | G01–G20 for chosen claims; IME/paste and explicit AT disposition before a human-use RC claim | Approve test tasks, AT/platform coverage and numerical budgets; an untested required gate blocks PASS |
| D6 version/provenance/notices | Preserve 0.0.x-rc positioning; choose exact prerelease tag and synchronize displayed/package versions later | Resolve crate 0.1.0 vs product/MCP version metadata and own/third-party notices; no tag/signing/update action now |
| D7 activation/distribution | O retains separate release and real-data go/no-go decisions after S4 | Explicit audience, support commitment and authorization; M5 engineering PASS alone cannot activate production or publish |

O may accept a narrower **candidate claim** with documented untested surfaces
before implementation; E cannot relabel a failed required gate or silently remove
TUI/Wayland. Full accessibility or IME support needs corresponding evidence, not
English input or color checks. H records observations, E records tests, O decides
claims and activation. No pending decision authorizes work outside S0.

## 5. Proposed slices after S0 acceptance

Do not open/implement these as automatically authorized tasks. Once O accepts the
boundary, create narrow Issues with selected targets and gate IDs; one small PR
per concrete gap where possible.

| Slice | Depends on | Bounded deliverable / gates |
| --- | --- | --- |
| S1 package inventory and assembly | Approved D1–D4/D6 | Choose one platform first, then the other; resolve runtime closure/licenses/version manifest, archive/extract/stop/remove on clean synthetic target (G01–G03/G07–G11). No OS service/auto-update |
| S2 native human-use gaps | Approved D1/D2/D4/D5; runnable S1 package for packaged acceptance | Timebox baseline human tasks, fix only observed keyboard/paste/IME/AT/scaling issues, affected-surface smoke (G12–G17). No redesign or toolkit upgrade by default |
| S3 release process/security/performance | Approved budgets/claims; S1 release-profile binaries | G03–G08/G10/G18/G19 real process/adversarial/measurement work; do not repeat unrelated UI cases or speculate M3 recovery |
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
