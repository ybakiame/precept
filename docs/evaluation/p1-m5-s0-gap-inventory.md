# P1-M5/S0 — source, evidence and release gap inventory

Audit date: 2026-11-16. Scope: [#68](https://github.com/ybakiame/agent-control-plane/issues/68),
parent [#67](https://github.com/ybakiame/agent-control-plane/issues/67).
Auditor: Codex, author-run source/document review on Linux/WSL; not independent
human, clean-machine or new product runtime acceptance. The companion
[release-readiness proposal](../p1/release-readiness-plan.md) defines candidate
targets C1–C8, gate inventory G01–G20, owners/costs and the confirmed/pending
portions of D1–D7.

**Disposition:** S0 supplies an owner-reviewable contract; **M5 release readiness
is not established**. No package was built/installed/published and no real Profile
was opened. The owner has confirmed Windows x86_64 portable ZIP, standalone Linux
x86_64 native Wayland + PTY TUI, and macOS deferral. Exact support floors/claims,
other recommended defaults and subsequent task details remain to be finalized
with evidence. Current product is Rust-only 0.0.x-rc synthetic engineering validation.

### Owner review amendment — 2026-11-16

The [PR #69 review](https://github.com/ybakiame/agent-control-plane/pull/69) at
`2b916280ddc352dce26fdcb8299e517c06b9c7f8` and the
[Issue #68 decision record](https://github.com/ybakiame/agent-control-plane/issues/68#issuecomment-6916187539)
confirm initial **Windows HWND + ConPTY in a portable-only ZIP**, **standalone
Linux native Wayland Desktop + PTY TUI**, and **macOS deferred**. WSLg stays a
separate integration environment, not standalone/compositor certification. No
niri/XWayland/Xvfb dependency, Windows installer/service/elevation/PATH/registry
mutation, scheduled task or auto-update is in this direction.

The review recommends one Linux `.tar.gz`, a maintained Windows 11 candidate and
one ordinary isolated clean standalone Linux distro/compositor. These exact floors,
Linux package details, human IME/AT assignments, numeric budgets, license texts,
tag/audience and release claims are **not newly owner-approved**. Finalize them
with S1/RC evidence at the appropriate gate; they do not block finishing S0.
G01–G20 are a risk/claim selection inventory, not twenty mandatory checks per PR.
No packaging code or actual acceptance run is authorized by this amendment.

## 1. Source baseline, closure and evidence identities

Audited merged main: **`6d8000153107c67edfa5ff3bc77da15f196623d3`**, tree
**`4dd6540f674fcb7d4d4cd69295c48a43ec41e24b`**. Local main was updated by fast-forward
and the S0 branch started there. GitHub confirms [PR #61](https://github.com/ybakiame/agent-control-plane/pull/61)
merged at `e286c2606ed94031bc85ecd46925f2f0b62b1838`, then
[PR #66](https://github.com/ybakiame/agent-control-plane/pull/66) at `6d800015…`.
[M4 #52](https://github.com/ybakiame/agent-control-plane/issues/52) and
[N precedence #65](https://github.com/ybakiame/agent-control-plane/issues/65) are
closed; the new M5 parent explicitly records engineering-only acceptance. That
closure is owner disposition, not a newly observed physical-host or human test.

| Evidence ID | Exact identity | What it establishes | What it does not establish |
| --- | --- | --- | --- |
| E1 | Tested UI/native code `19942b6e8ba58e990e60095bf6440d572cda6123`; tree `d18ec29232cd266b51c3716e07c24b04c4d405b1` | [S3 ledger](p1-m4-s3-integrated-acceptance.md): 37 automated cells on each HWND/direct WSLg Wayland/ConPTY/WSL PTY; actual captures, input, restoration; 139/140 Workspace, 53 model, 29 NSH, 29 driver tests per respective OS | Standalone Linux, installation, rapid paste/full IME/AT, independent human review or new-SHA native PASS |
| E2 | Tested backend code `435f4bd325d569e3bd065fc6b5d096aec1d79f0a`; tree `fd17ed77b1c12d2406e3e462b91647d9e8771e78` | [N report](p1-n-project-contracts.md): 152 WSL/153 Windows Workspace tests; new actual daemon/IPC/MCP and twelve source-publication FI cases per OS | New UI binaries, package provenance/installation, actual vendor-model behavior or real data durability |
| E3 | Evidence-only head `3fb61626869418f580239487231ac201e98721b1`; merged main `6d800015…` | `git rev-parse` confirms both trees equal `4dd6540f…`; merges retain the reviewed source/doc contents | Tree equality/merge is not another build, binary reproducibility or certification run |
| E4 | [M2/S5](p1-m2-s5-rust-only-integrated-acceptance.md), tested `ddc12ac8c17215014e04cc65ed802819b1bc8376` | Earlier synthetic Rust-only integration and retirement baseline | Current M4/N behavior, packages, fresh device/host/model coverage |

At this main, `git diff 19942b6 main -- apps/operator` is empty. The UI source is
unchanged, but its linked core can differ; E1 remains E1, not a binary claim at
main/E2/S0. E1 and E2 used genuine native Windows and WSL checkouts on one physical
installation, under an **M4-only owner exception**. For initial M5 engineering
gates, the revised plan recommends isolated independently provisioned clean Windows
and standalone Linux targets (VM or separate install); it does not force two
physical machines or inherit that M4 exception. Concrete hosts/operators remain
unassigned. Their Windows global Cargo frontend also
differs from stock Cargo. Failed/partial E1 cohorts and older BLOCKED report text
are preserved, rather than retroactively relabeled after issue closure.

## 2. Verifiable source inventory

Paths below are tracked baseline files unless explicitly marked absent or pinned
upstream. Reading an implementation is source evidence, not a new execution PASS.

| Source / inspected path | Implemented truth | Release gap / gate |
| --- | --- | --- |
| [AGENTS](../../AGENTS.md), [product](../product.md), [architecture](../architecture.md), ADR-0003/0004/0005/0006 | Local-first Rust product, daemon single writer, readonly clients/two MCP tools, explicit creation and separate grants; no Python/legacy migration | Maintain these constraints in packages; architecture/README contain older milestone prose, not current certification (G04–G06) |
| [Root Cargo workspace](../../Cargo.toml), [toolchain](../../rust-toolchain.toml), `Cargo.lock` | Eight runtime crates, fixed 1.99.1 fmt/Clippy toolchain; core package version 0.1.0 and rust-version 1.99 | Product 0.0.x-rc vs package/tool metadata need owner D6; locked dependency/build provenance is not reproducible output proof (G01/G11) |
| [Operator workspace](../../apps/Cargo.toml), [README](../../apps/README.md), [UI manifest](../../apps/ui/Cargo.toml), `apps/operator/Cargo.lock` | One model/ui workspace; `acp-nsh-native` and `acp-nsh-tui`; pinned immutable NSH across platform crates; actual native,tui features | No native package manifest/dependency bundle; `acp-ui-snapshot` is developer tooling, not a shipped product default (G01/G08) |
| [Desktop entry](../../apps/ui/src/main.rs), [TUI entry](../../apps/ui/src/tui.rs), [shared UI](../../apps/ui/src/lib.rs), `ui/build.rs` | Native platform selection, ephemeral storage/cache off, explicit inputs; raw/alternate-screen restore guard, bracketed paste events; build script only sets WASM stack size | Feature/API presence does not prove IME/paste/AT; no native installer/version resource from this build script; crash/panic restore not universal (G07/G12–G17) |
| [Model native adapter](../../apps/client/src/native.rs), [operator model](../../apps/core/src/operator.rs) | Explicit endpoints and operator actions; typed read-only Windows→WSL helper; shared review invalidation and uncertain-result handling | No general remote writer/agent execution; paired package/path/quoting and read-only denial require G19 |
| [Daemon](../../crates/acp-daemon/src/main.rs), [Profile owner](../../crates/acp-daemon/src/profiles.rs), [CLI](../../crates/acp-query/src/main.rs) | Explicit daemon start, allowlists, optional Profile/config; create/select/write-enable distinct; shutdown disables writes/sessions; CLI returns deliberately requested data | No default Profile/onboarding/service installer; startup/teardown permissions/real-use diagnostics on clean targets remain G02/G03/G07/G10 |
| [IPC](../../crates/acp-ipc/src/lib.rs), [Unix transport](../../crates/acp-ipc/src/transport/unix.rs), [Windows transport](../../crates/acp-ipc/src/transport/windows.rs) | Private modes/current-user DACL, local sockets/named pipes, peer checks, lease/capability cleanup; no TCP | Current-user cooperative boundary, not hostile same-user sandbox; executable ACL and distinct-user packaged cases need G03; Unix endpoint bound is 100 path bytes |
| [Store](../../crates/acp-store/src/lib.rs), [Profile contract](../p1/profile-v1-runtime-contract.md), [Writer](../../crates/acp-write/src/writer.rs), [ownership](../p1/daemon-profile-ownership.md) | Fresh exact v1 readonly reader, private daemon writer, CAS/receipts/transitions, capacity refusal and native file binding; bundled SQLite feature | No user-data preservation/recovery/release guarantee; do not weaken full verification or build M3 by default (G04/G18) |
| [Contract adapter](../../crates/acp-core/src/contracts/mod.rs), [accepted boundary](../p1/project-contract-integration.md), [MCP](../../crates/acp-mcp/src/main.rs) | Bounded optional repository claims, live typed precedence, safe projection/status, exact two bound read tools; MCP stdout protocol-only | Accepted is a repository assertion; arbitrary prose unparsed, no last-good cache or post-check tamper protection; package/runtime tests G05/G06 |
| [Native development](../p1/rust-only-development.md), [Linux verifier](../../tools/local_verify.sh), [Windows verifier](../../tools/windows_local_verify.ps1) | Clean-SHA build/fmt/Clippy/tests, logs and environment restoration; helpers use existing prerequisites | Verification helpers are not product installation or runtime dependencies; no locked distribution closure (G01/G02/G08) |
| [Rust workflow](../../.github/workflows/rust.yml), [UI workflow](../../.github/workflows/ui-spike.yml) | Root Linux/macOS/Windows and UI Windows/Ubuntu job definitions, Node checks, offline Web fixture | Definitions are not executed evidence; latest hosted Actions NOT_RUN. CI Linux dev packages omit some pinned upstream media requirements; no release job (G01/G08) |
| [Web packager](../../tools/package_ui_web.mjs), [Web tests](../../apps/web-smoke/package.json) | Copies exact pinned renderer/WASM and one NSH Apache license; loopback synthetic fixture | No native distribution path, authentication gateway, complete notice ledger or production Web support (G11) |
| [Acceptance driver](../../tools/native-acceptance/README.md), `tools/native-acceptance/Cargo.lock`, [Windows worker](../../tools/native_window.ps1) | Third locked workspace, bounded owned process/window nonce/foreground checks; temporary installed English layout for synthetic input | Test-only fakes/relays/probes; paced ASCII/UTF-8 is not IME/rapid-paste evidence; don't ship them or require PowerShell for product use (G13/G14/G17) |
| `docs/deployment.md`, native installer/uninstaller/release scripts, root LICENSE/NOTICE, full license/SBOM manifest | **Not found in tracked baseline**; core Cargo license declarations exist; app license metadata incomplete | Portable extraction/removal and notice/dependency evidence are missing (G02/G09/G11). Absence of a Windows installer is intentional under ZIP-only direction, not a requirement to add one |

Pinned upstream inspected locally at exactly
[`b7f31a08478be262fd7b76ec856036fd5b8ffdb0`](https://github.com/longbridge/nsh/tree/b7f31a08478be262fd7b76ec856036fd5b8ffdb0):
root `Cargo.toml`, `crates/nsh-platform-linux/Cargo.toml` and
`crates/nsh-platform-windows/Cargo.toml`. The source declares workspace 0.5.0,
GTK4 `v4_14`, Linux GStreamer/app/audio dependencies and Windows graphics/WebView2
COM crates. This is exact source evidence, not a dynamic import/plugin audit or
reason to install every optional upstream feature. No cache or dependency was changed.

## 3. Candidate facts and gaps

| Area | Verified historical/source fact | Current release state / required next proof |
| --- | --- | --- |
| Windows HWND + ConPTY | Owner-confirmed portable ZIP candidate; E1 x86_64, OS 10.0.26300.0, MSVC tools 14.51.36231 / SDK 10.0.26100.0; scoped native input/render/restoration | Clean ZIP extraction/start NOT_RUN; maintained Windows 11 is a recommendation, exact OS/terminal/runtime floor awaits S1 evidence; applicable G01–G03/G12–G17 |
| Native Linux/Wayland + PTY | Owner-confirmed standalone x86_64 candidate; backend/TTY source exists; E1 uses Arch **inside WSL**, GTK 4.24.1, kernel 6.18.58.1-microsoft-standard-WSL2 | Both GUI/TUI need one clean standalone distro/compositor first; exact GTK/libc/terminal unchosen; `.tar.gz` recommended. NOT_RUN; no X11 substitute; applicable G02/G08/G12/G17 |
| WSLg | E1 actual GdkWaylandDisplay/GskGLRenderer and actual acknowledged resize; not niri | Separate integration, not standalone Linux certification; clean WSL/Windows version combination NOT_RUN |
| Cross-OS reads | Source allows bounded explicit Windows→WSL owner reads | Exact packaged binary/path/endpoint combination NOT_RUN; no transferred grant or filesystem sharing by implication (G19) |
| Platforms beyond x86_64 Windows/Linux | Owner deferred macOS from first candidate; manifests/old CI include it; older mobile/Web experiments remain distinct | Current macOS/mobile/other-arch tests NOT_RUN; no Apple Silicon support claim or inference of technical impossibility; mobile/authenticated Web remain outside initial scope |
| Resource/performance | 1 MiB frame, 16 peers, 32 requests/peer, 2 s frame timeout; 16 KiB source; 4096 commit capacity; E1 10k component presentation | No approved cold/warm/RSS/idle/p95 budgets or clean release-profile measurements (G18); do not equate a synthetic row count with Profile write capacity |
| Keyboard/IME/accessibility | E1 physical/native synthetic keyboard, theme/locale and scoped screenshots; input deliberately switches away from Chinese IME | Human IME compose/cancel, bulk paste, AT tools and scaling/font matrix NOT_RUN; owner must name reviewer/scope (G13–G17) |
| Integrity/privacy | Native auth/ownership/source failure gates pass at E2; no automatic telemetry/listener | Packaged permissions, crash dumps, log limits/redaction, no-cache/offline behavior and notices NOT_RUN (G03/G08/G10/G11) |
| Lifecycle | Explicit current-v1 creation and daemon ownership exist; no migration or compatibility fallback | Portable extract/remove/clean-target state preservation NOT_RUN; Windows installer is excluded. Backup/restore/retention is conditional M3 only; no speculative prerequisite |

The exact manifest/feature combination and development runtime can have a wider
dependency closure than CI package lists suggest. Before choosing a Linux archive
floor, inspect linked binaries plus plugin/font loading. Before promising a lean
TUI, inspect its binary independently of the GUI build. Before requiring an OS
runtime on Windows, inspect actual imports. No dependency installer was executed
on the user's everyday machine during S0.

## 4. Prioritized disposition and owner handoff

1. **Confirmed direction and staged choices (D1–D7 / G20):** Windows portable ZIP,
   Linux standalone native Wayland/PTY and macOS deferral are recorded as actual
   decisions. Finalize exact floors/package dependencies in S1, human tasks/actors
   and numeric budgets before relevant RC runs, and license/tag/audience before
   their release gate. Do not hold S0 for these unmeasured details or self-approve
   remaining defaults; scope each task by applicable gates.
2. **Packaging blockers (G01/G02/G08/G09/G11):** native layout, loader closure,
   clean offline startup/removal, version consistency and license texts are absent
   or unverified. Start one bounded S1 platform task only after owner acceptance.
3. **Human-use gaps (G12–G17):** obtain standalone Wayland and real Windows/Linux
   IME/paste/AT observations before claiming those behaviors; preserve first-class
   TUI. Fix actual failures in narrow S2 PRs, not a speculative redesign.
4. **Release-build gaps (G03–G07/G10/G18/G19):** reuse contracts but execute targeted
   packaged security/process/performance checks in S3. M4 historical PASS is baseline
   only. Assign the isolated clean Windows and standalone Linux targets/operator;
   no two-physical-machine requirement or M4 exception carryover is inferred.
5. **Final RC (G20):** S4 depends on the selected S1/S2/S3 gates and one clean
   candidate identity. Required FAIL/BLOCKED/NOT_RUN prevents engineering PASS;
   publishing and real Profile activation need a separate owner decision even then.

Do not open S1–S4 issues or implement packaging/services/data lifecycle merely by
finishing this inventory. The risk-based cadence in #68 is binding: documents use
source/link/diff checks; backend/security uses corresponding real-process tests;
localized UI uses affected-surface smoke. One full four-native-surface UI matrix
is reserved for a material shared NSH milestone or explicitly accepted RC UI gate,
not every Issue. No old result is promoted to a new source/package SHA.

## 5. This S0's validation class

Source reads used tracked manifests/locks, public pinned upstream source and
GitHub issue/merge metadata. No build, installation/uninstall, user database,
auth file, external development checkout, model, native window/TTY or hosted
workflow was accessed/executed. Source review is not runtime PASS.

Required documentation checks (run locally for the S0 commits, with exact head
and results recorded in the Draft PR):

```text
git diff --check 6d8000153107c67edfa5ff3bc77da15f196623d3 HEAD
node tools/check_source.mjs
git diff --exit-code 6d8000153107c67edfa5ff3bc77da15f196623d3 HEAD -- docs/evaluation/p1-m4-s3-integrated-acceptance.md docs/evaluation/p1-n-project-contracts.md
```

The last check preserves the two historical reports byte-for-byte. Confirm only
the two new S0 documents and roadmap alignment are in the branch delta. Rust,
native Windows/WSL builds, installer trials and the 148-cell UI suite are **NOT_RUN
for S0 by design**; no code/tooling changed. M5 stays open and the PR stays Draft,
with `[skip ci]` commits and no automatic merge, release tag or public artifact.

Initial clean proposal validation at
**`3cc5097d8b7591433263803dca696f7a5fda09c5`** on Linux/WSL: baseline-to-HEAD
`git diff --check` **PASS**; `node tools/check_source.mjs` **PASS**, 251 source paths /
34 explicit archives (including local Markdown links, workflow targets and NSH pins).
Only the two new documents were present in that commit. Final roadmap alignment,
historical-report byte preservation and final clean-head checks are recorded with
the exact delivery SHA in the Draft PR; none implies a product test rerun.
