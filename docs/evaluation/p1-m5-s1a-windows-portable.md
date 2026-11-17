# P1-M5/S1-A — unpublished Windows portable ZIP prototype

2026-11-17. Scope: [Issue #70](https://github.com/ybakiame/agent-control-plane/issues/70),
[S0 contract](../p1/release-readiness-plan.md), parent
[M5 #67](https://github.com/ybakiame/agent-control-plane/issues/67).
**Prototype assembly, developer-host packaged smoke and exact removal PASS; isolated
clean-target acceptance BLOCKED. Not a release, minimum-OS claim, public-distribution
or real-Profile approval.** The owner confirmed no clean VM/separate installation is
available. No virtualization component, prerequisite, service or system configuration
was installed.

## Owner decisions recorded in this revision

| Topic | Decision | Implementation |
| --- | --- | --- |
| VC++ runtime | **Documented prerequisite, not bundled.** App-local copies would not be serviced by Windows Update. | `bin/` holds only the five executables; the manifest records `prerequisites[]`; README gives install link and a read-only check |
| First-party license | **Apache-2.0**, copyright holder `ybakiame` | Root `LICENSE` (verbatim canonical text, SHA-256 `cfc7749b…3d30`), `NOTICE`, README section, license field on the two operator crates; shipped as `ACP/LICENSE.txt` and `ACP/notices/NOTICE.txt` |
| `acp-ipc` builder path | **Remove the compiled-in build-checkout rule** and keep the generic `.git` ancestor refusal | `7a19db2`, plus the regression tests that rule never had |

## Source and method

| Input / observation | Exact identity |
| --- | --- |
| Merged S0 main / baseline | `de954327f4fb7209127f7b7373ccb9450d1f58e8` |
| Clean source of archive, smoke harness and removal helper | `f1f8aadd1aa9e8bdf333e519ea60f85b2a8b1398` |
| Harness executable SHA-256 | `8227ede09141b13c70a2cfe38be1de9b79181030033cac7771c92f1cf51a35bb` |
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
at `f1f8aad` for the build and the smoke. The package, harness and helpers come from
that one revision.

[Recipe](../../tools/portable/README.md): offline existing caches, a new target
directory per attempt, explicit stock pinned Cargo, and a temporary VS environment
restored on exit. Cargo runs with inherited `RUSTFLAGS`, encoded flags,
`RUSTC`/wrappers, `CARGO_BUILD_*`, `CARGO_PROFILE_*` and `CARGO_TARGET_*` removed,
and with `--remap-path-prefix` for checkout, Cargo home and build output. Any remaining
checkout, home, Cargo-home or output prefix in a binary (ASCII or UTF-16, either
separator) **refuses the build**. Root build: daemon/query/MCP. App build:
`acp-operator-ui` with `native,tui`.

Local attempt: ignored `artifacts/local-p1-m5/s1a-attempt-07/` in the native Windows
checkout. Logs, ZIP, staging, module ledgers, VT and PNG stay local and unattached.

## Archive layout and exact provenance

Local file `acp-windows-x86_64-prototype-f1f8aadd1aa9.zip`, **38,643,575 bytes,
256 entries**, SHA-256
`a6a5a5c29464621327d9d853e7beeaa62cf5be0b6d076bc956dfa25e268492a8`. All entries are
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
| `acp-daemon.exe` | 5,254,144 | `94869d92ae9430d2060c9de6f9ece6b55c36b5c88684754eced16bfbcff1cb40` |
| `acp-query.exe` | 1,718,272 | `f74ff1824b0ebeff9ee04d368764417835c66dfd97f2fdda8085e7af39dcf85c` |
| `acp-mcp.exe` | 1,729,024 | `dc4a94720396adea3ec21f72fe62ebaa29606e0943420c58a91293c3c55e5a1b` |
| `acp-nsh-native.exe` | 47,441,408 | `4aab19cdfb772fc6b2a7cea8222590282e3d68004bfc40332431fae0074b2609` |
| `acp-nsh-tui.exe` | 44,235,776 | `38bb58e24fd377353b7abffe6b1c722f3486ec105162fdd0b9bd22060b1c2c58` |

All five executables are unsigned. `ACP/runtime/manifest.json`
(`e62c54519a74955256d3e82d313fcbf03caa82d71270c5a2a0b877d4ab44d574`) records source
SHA/tree, locks, build commands, placeholder-only rustflags, builder, PE imports,
license, prerequisites and size/SHA-256 for the 255 other files. The ignored external
`archive-inventory.json` (`b722d81272a9434450fd1d4e28df7165795098573bb9c105276185b703eff672`)
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
  host that query reports v14.51.36247 installed.
  [Microsoft guidance](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist?view=msvc-170).
- **Loaded-module proof.** For the daemon (13 modules), ConPTY TUI (23) and HWND
  Desktop (74), the smoke asserted three things: each imported VC++ DLL loaded exactly
  once from the Windows installation; nothing except the executable loaded from
  `ACP\bin`; and nothing loaded from outside the package or the Windows tree. The
  missing-prerequisite failure path is **NOT_RUN**. This host has the redistributable
  installed, and removing it would change the daily machine.
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

## Packaged-process checks

`windows-smoke.ps1` at `f1f8aad` first verified every manifest-listed file, then ran
the extracted `ACP\bin` binaries. Private `smoke-01/smoke.json`
(`56c0d610…36d63852c0`) has **14 PASS** cells:

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

Initial daemon PID **28932**, HWND PID **22140**. `hwnd.png` `78a13b1e…f7d2040a`,
`terminal.vt` `281b08d9…57919a22d`, neither uploaded.

**Exact removal:** all **256** inventory files were preflighted (allowlisted name,
regular file, no alias, hash) before the first unlink, then removed. The helper removed
the empty `ACP/runtime` and `ACP/notices` non-recursively. It retained `ACP/bin` and
`ACP` because an unknown sentinel was inside, and kept a second sentinel beside `ACP/`.
The separate synthetic Profile, auth-labeled and unknown files kept their hashes.

## Gate dispositions

| Selected gate | Result / practical limit |
| --- | --- |
| G01 | PASS: same-clean-SHA locked five-binary build, sanitized inputs, `ACP/` layout, **builder-path-free binaries** (hard gate); signing/release claims NOT_RUN |
| G02 | **BLOCKED**: no independent clean Windows target. Developer-host extraction/start PASS. A clean run needs the VC++ prerequisite installed per README, plus one run without it to confirm the documented failure |
| G03 | PASS: protected output, executable/credential checks, unsafe ACL/path refusal, credential-role separation; actual second-user attack NOT_RUN |
| G04/G05/G06 targeted | PASS at the packaged create/grant/register/revoke and MCP/refusal boundary |
| G07 | PASS: collision, graceful stop/lease release, crash/normal restart, stale credential refusal |
| G08 | PASS: PE imports, recorded prerequisite and asserted system VC++ load for three surfaces; **BLOCKED** clean/offline closure, missing-prerequisite path, media/font |
| G09 | PASS: exact removal, empty-directory cleanup, Profile/auth/unknown retention; clean-target repetition NOT_RUN |
| G11 | PASS: provenance, Apache-2.0 first-party license and NOTICE, narrowed notice ledger; **BLOCKED** third-party notice review (21 without text), signing and publication |

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
| `windows-package.ps1` (attempt-07), `windows-smoke.ps1` (smoke-01), `windows-retention.ps1` | Windows | PASS |

Not run: operator workspace tests on Windows, the full 148-cell UI matrix, standalone
Linux/WSLg, hosted Actions, models.

## History (retained locally, not counted as passes)

- **attempt-04** (`b66034f`): flat 269-entry archive, no VC++ runtime, notice superset,
  unsanitized Cargo inputs, builder paths. 13 smoke PASS.
- **attempt-05** (`45f544a`): refused, because an Authenticode helper emitted nested JSON.
- **attempt-06** (`6d3999b`): app-local VC++ DLLs. smoke-01 hit the same nested-JSON
  defect in the module helper; smoke-02 passed 14 cells. **Superseded** by the owner
  decision to require the redistributable; the helper was removed.
- **attempt-07** (`f1f8aad`): this report.

## Required follow-up, not release authorization

- Run the clean Windows target in two configurations: with the redistributable installed
  per README, and without it to confirm the documented failure. Establish the exact OS
  claim from that evidence.
- Review the 21 third-party entries without notice text and the remaining notice
  obligations, then decide on signing.
- Investigate the 4 pre-existing Linux operator UI test failures separately.
- **Linux S1-B remains a required first-release slice**: clean standalone x86_64 distro,
  native Wayland Desktop plus PTY TUI, ELF/GTK/glibc/media/font closure, `.tar.gz`, and
  the same `ACP/{bin,runtime,notices}` and prerequisite lessons. WSLg never substitutes.
  Parent M5 stays open; keep the PR Draft.
