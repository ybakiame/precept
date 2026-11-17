# P1-M5/S1-A — unpublished Windows portable ZIP prototype

2026-11-17. Scope: [Issue #70](https://github.com/ybakiame/agent-control-plane/issues/70),
[S0 contract](../p1/release-readiness-plan.md), parent
[M5 #67](https://github.com/ybakiame/agent-control-plane/issues/67).
**Prototype assembly, developer-host packaged smoke, exact removal and disposable
clean-target (Windows Sandbox) acceptance PASS. Not a release, minimum-OS claim,
public-distribution or real-Profile approval.** Windows Sandbox was already enabled on
the developer host; nothing was installed, uninstalled or configured on the host. The
VC++ redistributable was installed only inside the discarded sandbox.

## Owner decisions recorded in this revision

| Topic | Decision | Implementation |
| --- | --- | --- |
| VC++ runtime | **Documented prerequisite, not bundled.** App-local copies would not be serviced by Windows Update. | `bin/` holds only the five executables; the manifest records `prerequisites[]`; README gives install link and a read-only check |
| First-party license | **Apache-2.0**, copyright holder `ybakiame`; explicitly confirmed by the owner on 2026-11-17 in response to PR #71 review item 4 | Root `LICENSE` (verbatim canonical text, SHA-256 `cfc7749b…3d30`), `NOTICE`, README section, license field on the two operator crates; shipped as `ACP/LICENSE.txt` and `ACP/notices/NOTICE.txt` |
| `acp-ipc` builder path | **Remove the compiled-in build-checkout rule** and keep the generic `.git` ancestor refusal | `7a19db2`, plus the regression tests that rule never had |
| Clean target | **Windows Sandbox** instead of uninstalling VC++ on the daily machine; Windows N editions out of scope for the target audience | `windows-sandbox.ps1` + `sandbox-acceptance.ps1` |
| OS scope | The tested clean target is **Windows 11 Enterprise, Windows Sandbox image build 26100**. Older Windows versions are outside the current validation and support claim; no broader compatibility is asserted, and no additional OS run is required for S1-A | This report |

## Source and method

| Input / observation | Exact identity |
| --- | --- |
| Merged S0 main / baseline | `de954327f4fb7209127f7b7373ccb9450d1f58e8` |
| Clean source of archive, smoke harness, removal helper and Sandbox tooling | `729eb51d5662722037ad015578712084d758efa6` (tree `e6fae078bef75fc0a622cedf973f977a33295f80`) |
| Harness executable SHA-256 | `8227ede09141b13c70a2cfe38be1de9b79181030033cac7771c92f1cf51a35bb` |
| Clean target | Windows Sandbox, Windows 11 Enterprise **build 26100** image (host 26300), user `WDAGUtilityAccount` (administrator), networking/clipboard/audio/video disabled, vGPU on (host AMD GPU) |
| Target / profile | Native `x86_64-pc-windows-msvc`, Cargo `--release --locked` |
| Builder/test OS | Developer host Windows `10.0.26300.0`, **not clean**, not a supported stable minimum |
| Stock toolchain | Cargo `1.99.1 (5f94df478 2026-08-27)`, rustc `1.99.1 (58cf8f9c3 2026-10-13)`, LLVM 23.1.1 |
| MSVC tools / SDK | 14.51.36231 / 10.0.26100.0 |
| Developer helpers | PowerShell 7; Node 26.13.1; no helper/interpreter shipped |
| NSH source | `b7f31a08478be262fd7b76ec856036fd5b8ffdb0`, unchanged |
| Root / app lock SHA-256 | `b57d19e4…3b862bf6` / `ee768048…0317e7ee` (unchanged) |

Actor: automated engineering checks plus screenshot inspection of the owned synthetic
window, with the existing foreground/PID/title protections. This is not independent
human, IME or assistive-technology acceptance. The native Windows checkout was clean
at `729eb51` for the build, host smoke, removal and Sandbox run; all come from that one
revision.

[Recipe](../../tools/portable/README.md): offline existing caches, a new target
directory per attempt, explicit stock pinned Cargo, and a temporary VS environment
restored on exit. Cargo runs with inherited `RUSTFLAGS`, encoded flags,
`RUSTC`/wrappers, `CARGO_BUILD_*`, `CARGO_PROFILE_*` and `CARGO_TARGET_*` removed,
and with `--remap-path-prefix` for checkout, Cargo home and build output. Any remaining
checkout, home, Cargo-home or output prefix in a binary (ASCII or UTF-16, either
separator) **refuses the build**. Root build: daemon/query/MCP. App build:
`acp-operator-ui` with `native,tui`.

Local attempt: ignored `artifacts/local-p1-m5/s1a-attempt-08/` (and
`s1a-attempt-08-sandbox-01/`) in the native Windows checkout. Logs, ZIP, staging, module
ledgers, VT and PNG stay local and unattached.

## Archive layout and exact provenance

Local file `acp-windows-x86_64-prototype-729eb51d5662.zip`, **38,643,993 bytes,
256 entries**, SHA-256
`ca6b9f23cdc62fedcfe3ae928916bde510355a62bae0b8dc0bb9139f996eadb7`. All entries are
under `ACP/` (checked before extraction):

```
ACP/
├── bin/        acp-daemon.exe, acp-query.exe, acp-mcp.exe, acp-nsh-native.exe, acp-nsh-tui.exe
├── runtime/    manifest.json
├── notices/    NOTICE.txt, dependency-notices.json, 246 <sha256>.txt license texts
├── README.txt
└── LICENSE.txt Apache-2.0, byte-identical to the canonical text
```

| `ACP/bin/` file | Bytes | SHA-256 |
| --- | ---: | --- |
| `acp-daemon.exe` | 5,254,144 | `3eec9c8db0d75dbbbc20887d8367d2841d283be37d68a0d444e9d137878baa9e` |
| `acp-query.exe` | 1,718,272 | `122f362e993d4b4f694739c3f26cc78f5f153df374d843b0406cedac96b88cb6` |
| `acp-mcp.exe` | 1,729,024 | `fc70d0eb50a3d843c4bc12e6d4db5d221af2b20591ba8735887ea32f3c1a69fd` |
| `acp-nsh-native.exe` | 47,441,408 | `6ed3e60f4bfa95fce0b215fce939eea353141ec0a592e226dbc6ae83975ec6df` |
| `acp-nsh-tui.exe` | 44,235,776 | `e79c9f92070c99eb01f626aaf41d3873db363981ade941aeee922fcc1b4ae375` |

All five executables are unsigned. Product code is identical to attempt-07 (only tooling
and README changed), yet the binary hashes differ: the recipe is reproducible, the
binaries are **not bit-reproducible**, as already stated. `ACP/runtime/manifest.json`
(`337a1d56ee10e658e80f3c07117b647f9f37acd5279268c42adf60dfb53ecc7f`) records source
SHA/tree, locks, build commands, placeholder-only rustflags, builder, PE imports,
license, prerequisites and size/SHA-256 for the 255 other files. The ignored external
`archive-inventory.json` (`dce6da62e283efe90ffa7e58849a4099d9068045ed48d9f7fcf3cd26cb0a6f3f`)
also covers the manifest.

**Builder paths: resolved.** The recipe's hard gate passed. An independent byte count of
the account name in every extracted executable found **0** ASCII and **0** UTF-16
occurrences. The text files in the archive also contain no checkout, home or output
prefix. No snapshot/acceptance/fake executable, fixture, log, database, token, agent
HOME, source checkout, helper or toolchain is in the ZIP.

## Runtime closure and prerequisites

Actual PE32+ normal and delay imports were parsed. All are AMD64, Desktop subsystem 2,
the other four subsystem 3.

- **VC++ runtime (prerequisite).** All five executables import `vcruntime140.dll`; the
  Desktop also imports `vcruntime140_1.dll`. No `msvcp140*`. Debug CRT imports refuse.
  The manifest requires the **Microsoft Visual C++ v14 Redistributable (x64) ≥
  14.51.36231**, the MSVC build tools version. Microsoft states the installed
  redistributable "must be the same or later than the version of the MSVC Build Tools
  used to create your application". The README links the permalink
  `https://aka.ms/vc14/vc_redist.x64.exe` and gives a read-only registry query. On this
  host that query reports v14.51.36247 installed. **On the clean Sandbox target, all
  five executables fail with `0xC0000135` (DLL not found) before the redistributable is
  installed and run after it** (see below).
  [Microsoft guidance](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist?view=msvc-170).
- **Loaded-module proof.** For the daemon (13 modules), ConPTY TUI (23) and HWND
  Desktop (74), the smoke asserted three things: each imported VC++ DLL loaded exactly
  once from the Windows installation; nothing except the executable loaded from
  `ACP\bin`; and nothing loaded from outside the package or the Windows tree. The
  missing-prerequisite path ran on the clean Sandbox target, not on the daily machine.
- UCRT (`api-ms-win-crt-*`) is an OS component on Windows 10+ and is not listed. The
  README cites the redistributable's own Windows 10/11 and Server 2016+ support, but
  states that ACP's exact minimum build is not established.
- The Desktop also imports HWND/COM, D3D11/DXGI/D2D/DWrite/DComp, UIAutomation,
  `mfplat.dll`, `mfreadwrite.dll` and `xaudio2_8.dll`. Windows N and minimal-image
  media availability, GPU/driver and fonts remain clean-target questions. No WebView2
  import or module was observed. Offline/no-network certification is NOT_RUN.

**Notices.** The ledger covers the normal-dependency closure of the shipped packages:
402 linked entries (core 53, operator 349) plus 30 proc-macro crates labeled
compile-time. Build/dev dependencies and non-shipped members such as `acp-ui-snapshot`
are excluded. First-party entries point to `LICENSE.txt`/`NOTICE.txt`. **21
third-party entries lack crate-local notice text** and need review. This is not a
linker-derived SBOM or a legal-completeness claim.

## Clean-target acceptance (Windows Sandbox)

`windows-sandbox.ps1` (host, PowerShell 7) verifies the ZIP hash and the Microsoft
signature of the building Visual Studio's `vc_redist.x64.exe`, maps them read-only into
a fresh Sandbox with networking disabled, and waits. `sandbox-acceptance.ps1` (inside,
Windows PowerShell 5.1, the only PowerShell there) follows README steps and shuts the
Sandbox down. Private `result.json` (`8200e2e9…50d16819`), **12 PASS / 0 FAIL**:

1. Clean target: no VC++ v14 registry key and no `System32\vcruntime140.dll`.
2. ZIP hash, entry-name check, extraction into a new directory creating only `ACP\`;
   all 255 manifest files verified; `bin` holds exactly five executables; the shipped
   README contains the prerequisite link and the private-state snippet used below.
3. **Missing prerequisite:** all five exit `0xC0000135` (STATUS_DLL_NOT_FOUND), no dialog.
4. Signed redistributable installed inside the Sandbox only: v14.51.36247 ≥ 14.51.36231.
5. Daemon in its own console: no implicit Profile/write grant, health OK, VC++ from System32.
6. Private state directory created **with the README snippet**; explicit Profile create,
   repeated create refused, write enable, registration with the canonical root.
7. MCP stdio: exactly `get_effective_context`, `get_handoff`; context OK; write tool refused.
8. Write grant revoked.
9. HWND Desktop renders (screenshot inspected), both VC++ DLLs from System32, normal
   close exit 0, daemon still healthy.
10. ConPTY TUI renders, VC++ from System32, Ctrl+Q exit 0.
11. Ctrl+C in the daemon's own console: exit 0, capabilities removed, endpoint closed.
12. Exact removal of 256 files; `bin` kept for an unknown file; Profile and state retained.

**Product finding (fixed in docs):** Profile creation refuses ordinary folders. Explorer
and `New-Item` folders inherit SYSTEM/Administrators ACEs, and under an elevated
administrator the owner becomes `BUILTIN\Administrators`; the product requires owner =
current user with exactly one ACE (`UnsafeRuntime`). That is the intended contract, but
the README gave users no way to satisfy it. The README now documents a PowerShell snippet,
and the Sandbox run executes exactly that snippet. A product-side helper that creates the
private directory is a possible later usability improvement, not done here.

Limits: one Sandbox image (Windows 11 Enterprise build 26100) on this host's hardware and
vGPU, not a separate physical installation; Windows PowerShell 5.1 acceptance instead of the Rust
harness (the TUI resize, terminal-restore sequences and module-ledger hashes are covered
by the host smoke only); no second user.

## Packaged-process checks (developer host)

`windows-smoke.ps1` at `729eb51` first verified every manifest-listed file, then ran
the extracted `ACP\bin` binaries. Private `smoke-01/smoke.json`
(`1f507426…66daba1`) has **14 PASS** cells:

1. No implicit Profile selection, creation or write grant at startup.
2. Broad-ACL and regular-file runtime paths refuse without disrupting the owner.
3. Endpoint collision refuses; the first daemon stays healthy.
4. Explicit synthetic v1 create, exact grant and canonical allowlisted registration; a
   repeated create refuses.
5. Project credential cannot write.
6. MCP refuses an operator credential.
7. MCP exposes exactly two read tools: context OK, missing Handoff errors, invented
   write tool refuses.
8. Explicit write revocation.
9. ConPTY render, resize, Ctrl+Q exit and terminal restoration.
10. HWND ready, 920×640 resize, window-only PNG inspected, normal close.
11. VC++ prerequisite loaded from the system by daemon, ConPTY and HWND (above).
12. UI close leaves the daemon healthy.
13. Ctrl+Break graceful stop, capability cleanup and lease release.
14. Normal and crash restart rotate and refuse stale credentials, with no Profile
    adoption.

Initial daemon PID **25648**. Module counts: daemon 13, ConPTY 23, HWND 74, all from the
package executable or the Windows tree. `hwnd.png` `3392312c…70c61c05`, not uploaded.

**Exact removal:** all **256** inventory files were preflighted (allowlisted name,
regular file, no alias, hash) before the first unlink, then removed. The helper removed
the empty `ACP/runtime` and `ACP/notices` non-recursively. It retained `ACP/bin` and
`ACP` because an unknown sentinel was inside, and kept a second sentinel beside `ACP/`.
The separate synthetic Profile, auth-labeled and unknown files kept their hashes.

## Gate dispositions

| Selected gate | Result / practical limit |
| --- | --- |
| G01 | PASS: same-clean-SHA locked five-binary build, sanitized inputs, `ACP/` layout, **builder-path-free binaries** (hard gate); signing/release claims NOT_RUN |
| G02 | **PASS (disposable clean target)**: Windows 11 Enterprise Sandbox build 26100, with and without the prerequisite, extraction/verify/start/connect/stop/remove (12 PASS). Scope is that target only; older Windows is outside the current validation/support claim. A separate physical installation was NOT_RUN |
| G03 | PASS: protected output, executable/credential checks, unsafe ACL/path refusal, credential-role separation; actual second-user attack NOT_RUN |
| G04/G05/G06 targeted | PASS at the packaged create/grant/register/revoke and MCP/refusal boundary |
| G07 | PASS: collision, graceful stop/lease release, crash/normal restart, stale credential refusal |
| G08 | PASS: PE imports, recorded prerequisite, missing-prerequisite failure and post-install load on the clean target; offline/no-network operation passed with Sandbox networking disabled. Font/GPU breadth beyond one host remains NOT_RUN |
| G09 | PASS on host and clean target: exact removal, empty-directory cleanup, Profile/auth/unknown retention |
| G11 | **PARTIAL — BLOCKED for public distribution.** PASS: source/toolchain/lock/binary/archive provenance, verbatim first-party Apache-2.0 `LICENSE` and `NOTICE` shipped and hash-checked, narrowed notice ledger. Open: **21 third-party entries lack crate-local notice text and are pending review**; signing and publication undecided. The first-party Apache-2.0 grant (copyright `ybakiame`) is explicitly owner-confirmed. Accurately recorded, this does not block merging a private prototype |

## Validation commands

| Check | Platform | Result |
| --- | --- | --- |
| `node --test tools/portable/windows-package.test.mjs` | Windows; Linux/WSL | **11 PASS** each |
| PowerShell AST parse, all 6 helpers | Windows | PASS |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings` (root) | Linux/WSL | PASS |
| `cargo test --workspace --locked` (root, incl. real-process daemon/IPC/MCP) | Linux/WSL | **153 PASS**, 0 failed, 2 ignored |
| `cargo clippy -p acp-ipc --all-targets -- -D warnings`; `cargo test -p acp-ipc` | Windows | PASS; **8 PASS**, including the new checkout/worktree test |
| Mutation check: disabling the `.git` rule makes the new test fail | Linux/WSL | confirmed |
| Harness fmt/Clippy `-D warnings`/tests | Windows; Linux/WSL | PASS; 4 PASS |
| Operator workspace Clippy `-D warnings` | Linux/WSL | PASS |
| Operator workspace tests | Linux/WSL | **72 PASS, 4 FAIL**: `operator::tests` delete-tier, deprecate, prepare-digest and selected-list wait on "Prepared operation…" and time out. They fail identically at `84e748d`, before this revision. These are pre-existing and not caused by this change; tracked as follow-up |
| `windows-package.ps1` (attempt-08), `windows-smoke.ps1` (smoke-01), `windows-retention.ps1` | Windows | PASS |
| `windows-sandbox.ps1` (attempt-08 sandbox-01) | Windows Sandbox | **12 PASS** |
| PowerShell AST, PS 7 helpers and PS 5.1 Sandbox script | Windows | PASS |
| Harness env isolation (review item 1, `ae9689d`): fmt, Clippy `-D warnings`, tests incl. real child process with mixed-case `ACP_*` | Windows; Linux/WSL | PASS, 36 tests each; the unit and process tests fail when the prefix check is made case-sensitive (mutation-checked) |
| Packaged smoke with `Acp_Ui_Gallery` leaked into the parent environment (`smoke-02`, fresh re-extraction of the same attempt-08 ZIP, inventory verified) | Windows | **14 PASS**; HWND capture shows the operator UI, not the Gallery |

Not run: operator workspace tests on Windows, the full 148-cell UI matrix, standalone
Linux/WSLg, a physical clean installation, hosted Actions, models.

## History (retained locally, not counted as passes)

- **attempt-04** (`b66034f`): flat 269-entry archive, no VC++ runtime, notice superset,
  unsanitized Cargo inputs, builder paths. 13 smoke PASS.
- **attempt-05** (`45f544a`): refused, because an Authenticode helper emitted nested JSON.
- **attempt-06** (`6d3999b`): app-local VC++ DLLs. smoke-01 hit the same nested-JSON
  defect in the module helper; smoke-02 passed 14 cells. **Superseded** by the owner
  decision to require the redistributable; the helper was removed.
- **attempt-07** (`f1f8aad`): host smoke 14 PASS. Sandbox runs 01–10 exposed tooling
  defects, each fixed and retained: `Expand-Archive` unavailable in the image; a vacuous
  PASS over zero binaries (now refused); a PowerShell `--help` parameter-binding error;
  state directory without private ACL (the README gap above); Ctrl+Break returning
  `ERROR_INVALID_FUNCTION` (now Ctrl+C in the daemon's own console, as documented);
  a hand-built canonical path and pre-grant binding (registration `Unauthorized`, correct
  CAS behavior); the .NET Framework UTF-8 BOM on MCP stdin (console code page 65001,
  `-32700`, correct refusal); a Sandbox that did not shut itself down. Run 10 passed 12/12.
- **attempt-08** (`729eb51`): this report.

## Required follow-up, not release authorization

- Review the 21 third-party entries without notice text and the remaining notice
  obligations, then decide on signing.
- Investigate the 4 pre-existing Linux operator UI test failures separately.
- **Linux S1-B remains a required first-release slice**: clean standalone x86_64 distro,
  native Wayland Desktop plus PTY TUI, ELF/GTK/glibc/media/font closure, `.tar.gz`, and
  the same `ACP/{bin,runtime,notices}` and prerequisite lessons. WSLg never substitutes.
  Parent M5 stays open; keep the PR Draft.
