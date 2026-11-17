# P1-M5/S1-A — unpublished Windows portable ZIP prototype

2026-11-17 (supersedes the 2026-11-16 attempt-04 evidence, summarized under
[History](#history)). Scope: [Issue #70](https://github.com/ybakiame/agent-control-plane/issues/70),
[S0 contract](../p1/release-readiness-plan.md), parent
[M5 #67](https://github.com/ybakiame/agent-control-plane/issues/67).
**Prototype assembly, developer-host packaged smoke and exact removal PASS; isolated
clean-target acceptance BLOCKED. Not a release, minimum-OS claim, public-distribution
or real-Profile approval.** The owner confirmed no clean VM/separate installation is
available. No virtualization component, prerequisite, service or system configuration
was installed.

## Source and method

| Input / observation | Exact identity |
| --- | --- |
| Merged S0 main / baseline | `de954327f4fb7209127f7b7373ccb9450d1f58e8` |
| Clean source of the whole archive (five binaries, DLL selection, notices) | `6d3999bcecb7e930ce40159ee54aa9b41947854d` |
| That source tree | `0be843f6dcf3144fe9e35823148f8e9a876d4048` |
| Clean smoke harness + loaded-module helper | `e080e584c594eb44c3fb2694124cb69070ed093f` |
| Harness executable SHA-256 | `a96abb203fbf9addfe456c2e804fdb0296bd52469878dfa3613ddf8458f61bd0` |
| Removal/retention helper | unchanged since `45f544a`; run from `e080e58` |
| Target / profile | Native `x86_64-pc-windows-msvc`, Cargo `--release --locked` |
| Builder/test OS | Developer host Windows `10.0.26300.0`, **not clean**, not a supported stable minimum |
| Stock toolchain | Cargo `1.99.1 (5f94df478 2026-08-27)`, rustc `1.99.1 (58cf8f9c3 2026-10-13)`, LLVM 23.1.1 |
| MSVC tools / redistributable / SDK | 14.51.36231 / 14.51.36231 (`Microsoft.VC145.CRT`) / 10.0.26100.0 |
| Developer helpers | PowerShell 7; Node 26.13.1 on Windows; no helper/interpreter shipped |
| NSH source | `b7f31a08478be262fd7b76ec856036fd5b8ffdb0`, unchanged |
| Root / app lock SHA-256 | `b57d19e4…3b862bf6` / `ee768048…0317e7ee` (unchanged; full values in manifest) |

Actor: automated engineering checks plus screenshot inspection of the owned synthetic
window. Existing foreground/PID/title protections stayed enabled. This is not independent
human workflow, IME or assistive-technology acceptance. The native Windows checkout was
clean at both the build and smoke inputs. Commits after `6d3999b` change only maintenance
tooling and this report; no later HEAD receives a transferred binary/native PASS.

[Recipe and commands](../../tools/portable/README.md): offline existing caches, a new
target directory per attempt, explicit stock pinned Cargo, temporary VS environment
restored on exit. **New in this revision:** Cargo runs with inherited `RUSTFLAGS`,
encoded flags, `RUSTC`/wrappers, `CARGO_BUILD_*`, `CARGO_PROFILE_*` and `CARGO_TARGET_*`
removed (empty wrapper values override config), and with `--remap-path-prefix` for the
checkout, Cargo home and owned build output. Root build selects daemon/query/MCP; app
build selects `acp-operator-ui` with `native,tui`. Actual Cargo artifact messages choose
the five names and are copied into independent regular files.

Local attempt: ignored `artifacts/local-p1-m5/s1a-attempt-06/` in the native Windows
checkout. Raw logs, ZIP, staging, module ledgers, VT and PNG stay local. No binary or
fixture is attached to GitHub. The private fixture path appears only in an ignored pointer.

## Archive layout and exact provenance

Local file `acp-windows-x86_64-prototype-6d3999bcecb7.zip`, **38,755,444 bytes,
258 entries**, SHA-256
`b00cebb472f819e3eee295d88f6551c86c8645a035ebe2d6ebe5d8e7dcb10e57`.
Every entry is below one top-level `ACP/` directory (checked before extraction):

```
ACP/
├── bin/        acp-daemon.exe, acp-query.exe, acp-mcp.exe, acp-nsh-native.exe,
│               acp-nsh-tui.exe, VCRUNTIME140.dll, VCRUNTIME140_1.dll
├── runtime/    manifest.json
├── notices/    dependency-notices.json, microsoft-visual-cpp-runtime.txt, 246 <sha256>.txt
├── README.txt
└── LICENSE.txt
```

| `ACP/bin/` file | Bytes | SHA-256 | Signature |
| --- | ---: | --- | --- |
| `acp-daemon.exe` | 5,253,120 | `99290a74e0e70fd0ff634c2e8c4ed0cd316d63d78e64e6de1ab00e56054f0bb2` | none |
| `acp-query.exe` | 1,718,784 | `480771e1fa8df6dca9317a89ad653c146a244b5787e18711eb818d02cb82c317` | none |
| `acp-mcp.exe` | 1,729,536 | `e77f02755bfd90c4bc36ce223fae0e7a5aed4ca5ca386428d7cc9f2b82f14a46` | none |
| `acp-nsh-native.exe` | 47,441,920 | `4b4de6b07d56512e1a72de0bac7c5416dabfd28ff6f74460a03528b1c3c1265f` | none |
| `acp-nsh-tui.exe` | 44,236,288 | `e7acc481d52f25a7272e741831827c3c9019dd2fbe22fc954bf5a07a37085979` | none |
| `VCRUNTIME140.dll` 14.51.36247.0 | 178,616 | `d1f4225df2cd877dbf130d5668a021dce3f94118455ff5ec952061c30afc9ce7` | Microsoft, Valid |
| `VCRUNTIME140_1.dll` 14.51.36247.0 | 50,112 | `a7146c08f89fe5b04541ab507cdb59ff7b44534d4ba3c668a426c6450a03434e` | Microsoft, Valid |

`ACP/runtime/manifest.json` (SHA-256 `c3a3c49de5738a617e8729cdebabddf29b50de81e71fe275449acdf29130b574`)
records source SHA/tree, locks, build commands, placeholder-only rustflags, builder,
PE imports, signer subject/thumbprint (`2650247A…98A1D6E`), the redistributable version
and CRT folder, and size/SHA-256 for all 257 other files. The ignored external
`archive-inventory.json` (`6679284c945756c10f1917188a045d9da66479c6d2f4040bb8cabc0cc5a92d59`)
also covers the manifest. Archive text files were scanned for checkout, home and build
output prefixes: none found.

No snapshot/acceptance/fake executable, Web fixture, log, database, token, agent HOME,
source checkout, helper or toolchain is in the ZIP.

**Builder path bytes.** With remapping, each executable contains exactly **one**
builder path string, and no UTF-16 occurrence: the existing `acp-ipc`
`env!("CARGO_MANIFEST_DIR")` runtime guard (`crates/acp-ipc/src/lib.rs`), which path
remapping cannot reach. Registry/Cargo-home/source-location paths are gone. The manifest
records this per binary as booleans. This remaining privacy gap is **BLOCKED for external
distribution** and needs a reviewed product change (for example, relying on the existing
`.git` ancestor refusal in release builds). This slice does not remove that guard.

## Runtime closure

Actual PE32+ normal and delay imports were parsed. All are AMD64, Desktop subsystem 2,
the other four subsystem 3.

- **VC++ runtime: now bundled app-locally.** The recipe derives the set from the actual
  imports: `vcruntime140.dll` (all five) and `vcruntime140_1.dll` (Desktop only). No
  `msvcp140*` or other VC library is imported, so none is shipped. Debug CRT imports
  refuse. Copies come unmodified from the same Visual Studio's
  `VC\Redist\MSVC\14.51.36231\x64\Microsoft.VC145.CRT` folder. Microsoft lists
  `VC\redist` content as Distributable Code (unmodified, subject to license terms) and
  documents app-local deployment as possible but not recommended for servicing reasons.
  [Redistribution list](https://learn.microsoft.com/visualstudio/releases/2026/redistribution),
  [deployment guidance](https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files?view=msvc-170).
  The archive therefore owns VC++ security updates: a fix requires a rebuilt archive.
  README, notice and manifest say so.
- **Loaded-module proof.** Smoke asserted for the daemon (12 modules), ConPTY TUI (23)
  and HWND Desktop (74) that each imported VC++ DLL was loaded **from `ACP\bin`** with
  the packaged SHA-256, and that no module came from outside the package and the Windows
  tree. On this host a system-wide VC++ runtime also exists, so this proves load order
  (application directory first), not behavior on a host without it.
- UCRT (`api-ms-win-crt-*`) is an OS component on Windows 10+ and is not bundled. This
  does not establish ACP's minimum OS build.
- Desktop still imports Win32 HWND/COM, D3D11/DXGI/D2D/DWrite/DComp, UIAutomation,
  `mfplat.dll`, `mfreadwrite.dll` and `xaudio2_8.dll`. Media availability on Windows N or
  minimal images, GPU/driver and font coverage remain clean-target questions. No WebView2
  import or loaded module was observed in the basic flow.
- Offline/no-network runtime certification remains NOT_RUN.

**Notices.** The ledger now covers the **normal-dependency closure of the shipped
packages**: 402 linked package entries (core 53, operator 349) plus 30 proc-macro crates
labeled compile-time. Build and dev dependencies are excluded, as are non-shipped
workspace members such as `acp-ui-snapshot`. 34 entries lack crate-local notice text.
Attempt-04 had 476 entries, as a resolved-workspace superset. This is still **not a
linker-derived SBOM or legal completeness claim**. `LICENSE.txt` states
`PENDING OWNER DECISION`. The repository has no committed first-party license text, and
the recipe copies a root `LICENSE*` when one exists instead of inventing a grant.

## Packaged-process checks

`windows-smoke.ps1` at `e080e58` ran the extracted `ACP\bin` binaries. Before any launch
it verified every file listed in `runtime/manifest.json`. Private `smoke-02/smoke.json`
(`0c774e18…3a093ae8`) has **14 PASS** cells:

1. No implicit Profile selection/creation/write grant at startup.
2. Broad-ACL runtime directory and regular-file runtime path refuse without disrupting
   the owned daemon.
3. A second daemon on the same runtime/endpoint refuses; the first stays healthy.
4. Explicit fresh synthetic v1 creation, exact grant and canonical allowlisted project
   registration; a repeated create refuses without overwrite.
5. Project credential cannot invoke operator write control.
6. MCP refuses an operator credential in place of its project credential.
7. MCP stdio exposes exactly two tools: context succeeds, a missing Handoff errors, an
   invented write tool refuses.
8. Explicit write revocation.
9. ConPTY render, 160×50 → 100×35 resize, Ctrl+Q exit, alternate-screen/bracketed-paste
   restoration.
10. HWND ready/connected, 920×640 resize, window-only PNG inspected, normal close.
11. App-local VC++ runtime loaded by daemon, ConPTY and HWND (see above).
12. UI close leaves the daemon healthy.
13. Ctrl+Break to the owned process group: graceful exit, capability cleanup, lease release.
14. Normal and forced-crash restart rotate credentials, refuse stale ones, and do not
    adopt the retained Profile.

Initial daemon PID **25176**, HWND PID **31000**; worker/reader/writer joins confirmed.
`hwnd.png` `4e845112…92814c841`, `terminal.vt` `b49460a8…62fb81c`. Neither is uploaded.

**Exact removal** (`windows-retention.ps1`): after all processes exited, the helper
preflighted all **258** inventory files (name allowlist, regular file, no alias, hash)
before the first unlink, then removed them. It removed the empty `ACP/runtime` and
`ACP/notices` directories non-recursively. It retained `ACP/bin` and `ACP` because an
unknown sentinel was inside `ACP/bin`, and kept a second sentinel beside `ACP/`. The
separate synthetic Profile, auth-labeled and unknown files kept their hashes.

## Gate dispositions

| Selected gate | Result / practical limit |
| --- | --- |
| G01 | PASS: same-clean-SHA locked five-binary build with sanitized Cargo inputs, `ACP/` layout, PE/hash/file list; **BLOCKED** path-free binaries (one guard string) and release claims |
| G02 | **BLOCKED**: no independent clean Windows target; developer-host extraction/start PASS. The VC++ runtime is now bundled, so this is ready for a clean run |
| G03 | PASS: protected output, executable/credential checks, unsafe ACL/path refusal, credential-role separation; actual second-user attack NOT_RUN |
| G04/G05/G06 targeted | PASS at the packaged create/grant/register/revoke and MCP/refusal boundary; full suites NOT_RUN, no product changes |
| G07 | PASS: endpoint collision, graceful stop/lease release, crash/normal restart, stale credential refusal |
| G08 | PASS: PE imports plus asserted app-local VC++ load for three surfaces; **BLOCKED** clean/offline closure, media/font/optional features |
| G09 | PASS: exact post-stop removal with empty-directory cleanup and Profile/auth/unknown retention; clean-target repetition NOT_RUN |
| G11 | PASS: source/tree/toolchain/lock/binary/DLL-signature/archive provenance and narrowed notice ledger; **BLOCKED** first-party license decision, notice review, builder-path exclusion, signing/publication |

Commands run: `node --test tools/portable/windows-package.test.mjs` (**10 PASS** on
Windows and Linux/WSL); PowerShell AST parse of all seven helpers (PASS on Windows);
`cargo fmt --check`, `cargo clippy --bin acp-portable-smoke --locked -- -D warnings` and
`cargo test --bin acp-portable-smoke --locked` (**4 PASS**, Windows and Linux/WSL);
`windows-package.ps1` (attempt-06); `windows-smoke.ps1` (smoke-02);
`windows-retention.ps1`. No full workspace run, 148-cell matrix, hosted Actions or
model invocation.

Failures retained locally, none counted as a pass:

- **attempt-05.** The build succeeded, then the recipe refused because the Authenticode
  helper emitted a nested JSON array. The DLL signatures themselves were Valid. Fixed in
  `6d3999b`.
- **attempt-06/smoke-01.** The loaded-module helper had the same nesting defect, and the
  harness panicked on the first ledger. The ledger already showed the app-local load.
  Fixed in `e080e58`, then smoke-02 ran against the same extracted package.

## History

Attempt-04 (source `b66034f`, 2026-11-16) had a flat 269-entry archive with no VC++
runtime, the whole-workspace notice superset, unsanitized Cargo inputs and no path
remapping. It passed 13 developer-host smoke cells. Its evidence stays attached to that
source and is superseded here.

## Required follow-up, not release authorization

Before external artifacts: review the `acp-ipc` build-checkout guard (the only remaining
builder path); make the first-party license decision and review actual notices, including
Microsoft license terms for the VC++ DLLs; decide signing. Then repeat the
extraction/permission/launch/stop/removal gates on an isolated clean Windows installation
without a VC++ redistributable, and set the exact OS/terminal claim from that evidence.

**Linux S1-B remains a required first-release slice**: one clean standalone x86_64
distro with native Wayland Desktop plus PTY TUI, ELF/GTK/glibc/media/font closure,
recommended `.tar.gz`, the same five roles and the same `bin/runtime/notices` lessons.
WSLg never substitutes. Parent M5 stays open; keep the PR Draft.
