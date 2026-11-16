# P1-M5/S1-A — unpublished Windows portable ZIP prototype

2026-11-16. Scope: [Issue #70](https://github.com/ybakiame/agent-control-plane/issues/70),
[S0 contract](../p1/release-readiness-plan.md), parent
[M5 #67](https://github.com/ybakiame/agent-control-plane/issues/67).
**Prototype assembly and developer-host smoke PASS; isolated clean-target acceptance
BLOCKED. Not a release, minimum-OS claim, public-distribution or real-Profile approval.**
The owner confirmed no clean VM/separate installation is available for this stage.
No virtualization component, prerequisite, service or system configuration was installed.

## Source and method

| Input / observation | Exact identity |
| --- | --- |
| Merged S0 main / baseline | `de954327f4fb7209127f7b7373ccb9450d1f58e8` |
| Clean source of **all five** packaged release binaries | `b66034feb146492f3fa89d87a3575cdd1278b6a5` |
| That source tree | `4c7da05a77fdaa00ad2dbc57165a1d406e2ea1f0` |
| Clean smoke harness + loaded-module helper | `7605fb94ef7e0647bc072c48adb0dab64f8b9ce9` |
| Harness executable SHA-256 | `64b60f51541be3e7227c9a95872827cf65627f8e4767903b42073eebdccc9952` |
| Post-stop removal/retention helper | `0b0707636caeb4fdabc83e2a4aad09502e24d2ad` |
| Target / profile | Native `x86_64-pc-windows-msvc`, Cargo `--release --locked`; no cross-compiled or stale executable substitution |
| Builder/test OS | Developer host Windows `10.0.26300.0`, **not clean**, not a supported stable minimum |
| Stock toolchain | Cargo `1.99.1 (5f94df478 2026-08-27)`, rustc `1.99.1 (58cf8f9c3 2026-10-13)`, LLVM 23.1.1 |
| Existing MSVC / SDK | 14.51.36231 / 10.0.26100.0 |
| Developer helpers | PowerShell 7; Node 26.13.1 on Windows; no helper/interpreter shipped |
| NSH source | `b7f31a08478be262fd7b76ec856036fd5b8ffdb0`, unchanged |
| Root lock SHA-256 | `b57d19e43e3b98a4fc4ee49e382d2d4843642ac05b9d6b7d47ce6276b3862bf6` |
| App lock SHA-256 | `ee76804896afbdbc81d819b247d4b44db07563ff29f9db07eb50ee430317e7ee` |

Actor: Codex automated engineering checks and owned-window screenshot inspection.
The owner permitted foreground use of the identified synthetic window; the existing
foreground/PID/title protections remained enabled. This is not independent human
workflow, IME or assistive-technology acceptance. Both native checkouts were clean
at the respective build/smoke inputs. Later changes affect only maintenance tooling
and this evidence, not `crates/`, `apps/operator/`, locks, UI or runtime contracts.
No later HEAD receives a transferred binary/native PASS.

[Recipe and commands](../../tools/portable/README.md) use existing offline caches,
a new target directory per attempt, explicit stock pinned Cargo instead of the
global Cargo frontend, and temporary VS environment setup restored on exit.
Root build selects only daemon/query/MCP; app build selects `acp-operator-ui` with
`native,tui`. Actual Cargo artifact messages select the five names. Cargo's own
hard-linked target/deps outputs are read into **new independent regular files**;
the archive never contains those aliases or a target-directory glob.

The local attempt is under ignored `artifacts/local-p1-m5/s1a-attempt-04/` in the
native Windows checkout; raw logs, ZIP, staged files, module ledgers, VT and PNG
stay local. No binary or raw fixture is attached to GitHub. Private fixture roots
are referenced only by an ignored local pointer, never copied here. The synthetic
state lives outside Git, separately from extracted product files, and is retained.

## Archive and exact provenance

Local filename: `acp-windows-x86_64-prototype-b66034feb146.zip`.
Size **38,654,458 bytes**. ZIP SHA-256:
`7b9c35828e29e853374635acce2d6a29ab15beb085c39db7df338431391dee28`.
The originally assembled generic prototype filename was exclusively renamed to
this SHA-qualified name; SHA-256 was compared before/after, bytes unchanged.
The recipe now derives that suffix from the packaged source manifest.

| Actual product executable | Bytes | SHA-256 |
| --- | ---: | --- |
| `acp-daemon.exe` | 5,257,728 | `8c8baccc3705a99c93b2f1c0c4db46d4473f3511af18e6943e1cfdb6f7a78b1f` |
| `acp-query.exe` | 1,720,832 | `5527cc21ce133a2f952e35cc81513f77346abef33e36bbf6e946581739b00bc3` |
| `acp-mcp.exe` | 1,731,584 | `3d251e41363a113a1cc6b07b20be512f8d519298379144903be2145d4272233b` |
| `acp-nsh-native.exe` | 47,477,760 | `ebd5cc6ff3ff6572d3e173429fe087cd2fc3b0ca3116dac2b54539dc34cd504a` |
| `acp-nsh-tui.exe` | 44,268,032 | `aa85bb47e4ab5635fdc99595d586fd93c9472546d8180e3073ab3896b06db302` |

There are **269 file entries**: these five executables, README, source/build
manifest, dependency ledger, pending-license status, SHA256SUMS and 259 content-addressed
notice texts. SHA256SUMS covers the other 268 files; the external inventory also
hashes SHA256SUMS. Exact relative names/lengths/hashes remain in the ignored local
`archive-inventory.json`, digest
`3e5c17062c502a3cfbe822a23d32e4c362d27a3ca36d062d558228f24884ed6e`.
Manifest digest: `0016063015962ba2b2579536709c46a478fbeba4a6f09efbd46c1f388ea27822`.
Dependency-ledger digest: `9b10f773c204762833e60e5b97e7acdf540851eec4c6e10844d60bccf85a1ddc`.

No snapshot/acceptance/fake executable, Web fixture, log, database, token, agent HOME,
source checkout, helper or toolchain is in the ZIP. Archive metadata is checked for
builder-checkout path leakage. **Binary bytes are not path-free**: a separate scan
found the builder source prefix in all five binaries, including the existing
`acp-ipc` compiled `CARGO_MANIFEST_DIR` runtime guard. This is a remaining packaging
privacy gap, not a passed exclusion or permission to publish. Do not remove that
security check or claim compiler remapping alone fixes string constants. A future
path-remapping/runtime-guard solution needs a small reviewed follow-up before any
external artifact distribution; this task changes no product authority.

## Runtime closure: measured findings and remaining decisions

Actual PE32+ normal and delay DLL import directories were parsed for each artifact;
all are AMD64, Desktop subsystem 2, other four subsystem 3. DLL imports alone do not
establish dynamic COM/plugin closure or an exact OS API floor.

- All five import `VCRUNTIME140.dll` and UCRT API sets. Desktop additionally imports
  `VCRUNTIME140_1.dll`. **No VC runtime is bundled or installed by this prototype.**
  Loaded-module observation found these in the developer host's Windows tree;
  that does not make VC runtime a guaranteed clean-OS component. Before clean
  acceptance, choose an evidence-backed prerequisite/redistribution or reviewed
  build option; no installer or arbitrary System32 DLL copy is authorized.
- UCRT is an OS component on Windows 10+, according to
  [Microsoft's UCRT deployment documentation](https://learn.microsoft.com/en-us/cpp/windows/universal-crt-deployment?view=msvc-170).
  This does not approve Windows 10 or establish ACP's complete minimum OS build.
- Desktop imports Win32 HWND/COM, D3D11/DXGI/D2D/DWrite/DComp, UIAutomation,
  `mfplat.dll`, `mfreadwrite.dll` and `xaudio2_8.dll`. Media-feature availability,
  Windows N/minimal images, GPU/driver and Latin/CJK font coverage remain clean-target
  questions. No media/font assets are bundled, and no feature pack was installed.
- Basic Desktop loaded **74** modules; ConPTY TUI **22**. Both contained only the
  packaged executable and components under the Windows tree; no module from a
  developer cache, source checkout or other external directory was observed.
  The TUI PE imports do not include Desktop's graphics/media libraries. This
  bounded Windows observation is not a claim about Linux GTK linkage.
- No WebView2Loader import or loaded WebView2 module was observed in this basic
  native-root flow. Do not require/install WebView2 solely from a source COM crate
  declaration, or infer that optional WebView functionality is dependency-free.
  [Microsoft distinguishes loader code and the WebView2 Runtime](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution).
- No vendor agent/model starts. Git remains an external live-evidence dependency;
  this synthetic directory has no fabricated Git history. There was no network
  disconnection/firewall change or connection trace: **offline/no-network runtime
  certification is NOT_RUN**, despite offline Cargo builds and no provider calls.

Loaded-module ledger SHA-256: ConPTY
`b3a872eaf8e33699ab680ce0d2d477120d49459c206291049a6a37d540a5fdf3`, HWND
`8a1bb225c7e868ecbd477fba231f69176a78c00f95c405316a45f9e187a9ec9d`.
The ledger records module basenames/hashes and location classes, not absolute paths.
No minimum stable Windows build or complete redistribution policy is selected.

The notice collector records **476 resolved Windows dependency/workspace entries**
across two workspaces, with **35 entries lacking crate-local notice text**. This is
an explicitly labeled superset including build/dev dependencies, **not an SBOM,
linker-derived shipping list or legal completeness claim**. Root first-party text/
ownership grant is still absent; Cargo Apache-2.0 metadata does not resolve it.
Workspace/shared upstream texts, actual linked obligations and any future runtime
asset redistribution need review. Publication remains blocked, with no invented
copyright/license text and no new license decision on behalf of the owner.

## Actual packaged-process checks

`windows-smoke.ps1` at `7605fb9` ran the extracted five binaries, not debug product
outputs. Its private `smoke-03/smoke.json` has **13 PASS** cells:

1. No implicit Profile selection/creation/write grant at startup.
2. An owned synthetic broad-ACL runtime directory and a regular-file runtime path
   refuse without credentials or disruption to the real owned test daemon.
3. A second daemon on the same runtime/endpoint refuses; the first remains healthy.
4. Explicit fresh synthetic v1 creation, selection, exact grant and allowlisted
   canonical project registration; repeat create refuses, without overwrite.
5. Project read credential cannot invoke operator write control.
6. MCP refuses an operator credential in place of its project credential.
7. Actual MCP stdio exposes exactly two tools; effective context succeeds, missing
   Handoff returns an explicit error, invented write tool refuses. No model needed.
8. Explicit write revocation; UI operates with native operator read authority.
9. Real ConPTY render, 160×50 → 100×35 resize, Ctrl+Q normal exit and observed
   alternate-screen/bracketed-paste restoration sequences.
10. Real owned HWND ready/connected state, 920×640 resize, nonuniform window-only
    PNG, screenshot inspection and normal close. No business input or UI change.
11. UI close leaves the daemon healthy.
12. Ctrl+Break to the exact newly created daemon process group exits successfully,
    removes owned capabilities, and permits runtime lease reacquisition; stable
    coordination file is retained. No unrelated process is signaled.
13. Normal and forced-crash restart rotate credentials, refuse old credentials and
    do not adopt the retained Profile automatically. Final normal stop succeeds.

Observed owned principal PIDs: initial daemon **32244**, ConPTY **40784**, HWND
**27936**. GUI/worker report confirms process reaping and reader/writer-thread joins;
TUI confirms exit code 0. Intermediate CLI/MCP/restart PIDs were not all persisted,
so this is not a complete process-creation trace. Actual second-user denial remains
NOT_RUN; broad-ACL refusal is not relabeled as a cross-account attack test.

`hwnd.png` digest: `eebfeb5cf45f5dcb1cb04f3e2a0c333d8c1b5b657f6f69d1095d638227d8b575`.
`terminal.vt` digest: `7eb296915e04604d9208fe3262e799d2bfe3103e153e4d09dea8899dc1f1f44d`.
No screenshot/VT is uploaded. Full IME/paste/AT/user-task, 148-cell matrix and WSLg
were not run for this packaging slice.

After all processes exited, `windows-retention.ps1` at `0b07076` removed exactly
**269 original inventory files** from the owned extraction, with preflight
hash/regular-file/alias/name checks before the first unlink. It retained two newly
created unknown/auth-labeled synthetic sentinels inside extraction; the separate
actual synthetic Profile, auth-labeled sentinel and unknown file retained their
hashes. No recursive directory/data deletion occurred. ZIP and staging tree remain
private; extracting the ZIP again always requires a NEW destination.

## Gate dispositions and verification cadence

| Selected gate | Result / practical limit |
| --- | --- |
| G01 | PASS: same-clean-SHA locked native five-binary build, PE/hash/file-list prototype; path-free artifact/privacy and clean release claim remain BLOCKED as above |
| G02 | **BLOCKED**: owner confirms no independently provisioned clean Windows target; developer-host extraction/start only PASS |
| G03 | PASS: protected owned output, native executable/credential checks, unsafe ACL/path refusal and credential-role separation; actual second-user attack NOT_RUN |
| G04/G05/G06 targeted smoke | PASS at the narrow packaged create/grant/register/revoke, project-bound context/MCP/refusal boundary; full storage/CAS/contract/security suite NOT_RUN, no product changes |
| G07 | PASS: owned endpoint collision, normal stop/lease release/capability cleanup, crash/normal restart and stale credential refusal; complete child trace and clean-target recovery NOT_RUN |
| G08 | PASS: recorded PE + loaded-module inventory on developer host; **BLOCKED** complete clean/offline runtime closure, VC/media/font/optional-feature proof |
| G09 | PASS: exact owned post-stop product removal and synthetic Profile/auth/unknown retention; clean-target repetition NOT_RUN |
| G11 | PASS: source/tree/toolchain/lock/binary/archive provenance inventory; **BLOCKED** completed notices, first-party license decision, builder-path exclusion, signing/release/tag/publication claims |

Commands actually run: recipe build/archive/extraction inventory; Windows
`windows-smoke.ps1`; `windows-retention.ps1`; `node --test tools/portable/windows-package.test.mjs`
(**5 PASS** on Windows and Linux/WSL); PowerShell AST checks and actual helper
execution; `cargo fmt --manifest-path tools/native-acceptance/Cargo.toml -- --check`,
targeted `cargo clippy ... --bin acp-portable-smoke --locked -- -D warnings`,
`cargo test ... --bin acp-portable-smoke --locked` (**4 terminal-observer PASS**
on Windows and Linux/WSL). Source/link/diff checks are recorded in the PR at final
documentation HEAD. Windows builds/smoke used a clean checkout and package source
stability checks. The Linux run compiles/tests only maintenance tooling; it is not
Linux product/Wayland acceptance. No full workspace/148-cell repetition or hosted
Actions/model invocation was performed.

Failed attempts remain local: early entrypoint/toolchain metadata failure; rejection
of Cargo's internal output hard links before changing to a separate byte copy;
registration using a noncanonical root (product correctly denied it); and loaded-
module inspection initially comparing device-prefixed and ordinary local paths.
Subsequent runs use new directories, unchanged authority/assertions, canonical
registration and device-prefix normalization. No failure is overwritten or counted
as a pass. A late owner-authorized foreground helper found the already-closed test
window and safely refused; it touched no other window and is not foreground proof.

## Required follow-up, not release authorization

Before external artifacts: review builder-path privacy without removing the runtime
guard, first-party and actual dependency notices, and a VC runtime closure option.
Then repeat only applicable extraction/permission/launch/stop/removal gates on an
isolated clean Windows installation; establish its exact OS/terminal/runtime claim
from evidence. No new broad installer, auto-update or toolkit is implied.

**Linux S1-B remains a required first-release slice**, not optional: separate Issue
for one clean standalone x86_64 distro/compositor, real native Wayland Desktop plus
PTY TUI, ELF/GTK/glibc/media/font closure, recommended `.tar.gz`, the same five roles,
explicit credentials and safe retention. Reuse the recipe/inventory lessons, report
any TUI/GTK coupling before a packaging split, and never substitute WSLg or promise
cross-distro portability from archive format. Parent M5 stays open; S2/S3/S4 and
owner release/real-Profile go/no-go remain separate. Keep the PR Draft for review.
