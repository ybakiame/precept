# Portable package tooling

Developer-only tooling for local package and clean-target validation. No
installation, release, real Profile, provider, agent HOME or platform configuration
is required or authorized. Product source/UI/schema/authority are unchanged.

Use a clean committed **native Windows x86_64 checkout**, existing pinned stock
Rust 1.99.1/MSVC/SDK, PowerShell 7, Node and Git. Helpers temporarily select the
stock toolchain instead of any global Cargo fork, work offline with existing caches
and restore the caller's environment. Missing tools/cache fail, never install or
change pins/locks. They do not provision a VM or call hosted workflows.

Create an existing owned output parent under ignored `.tmp/artifacts/local-p1-m5/`.
Each attempt name must be NEW. The recipe exclusively creates/protects its private
output, builds into a new Cargo target, copies only actual Cargo artifact messages
for five product executables into independent regular files, audits x86_64 PE
normal/delay DLL imports and collects notices for the **normal-dependency closure**
of the shipped packages (build/dev dependencies excluded; proc-macros labeled
compile-time; still not a linker-derived SBOM). Cargo's internal artifact hard links
are not copied as archive aliases. No target-directory glob or fake/test driver is shipped.

Archive layout (one top-level directory, so extraction never scatters files):

```
Precept/
├── bin/          the five product executables only
├── runtime/      manifest.json: provenance, prerequisites, size/SHA-256 of every other file
├── notices/      NOTICE.txt, dependency-notices.json, <sha256>.txt license texts
├── README.txt    includes the prerequisite and verification instructions
└── LICENSE.txt   the committed root LICENSE (verbatim Apache-2.0, hash-checked)
```

The Microsoft Visual C++ v14 Redistributable (x64) is a **documented user
prerequisite, not bundled** (owner decision): app-local copies would not be serviced
by Windows Update. The recipe derives the required DLLs from the actual PE imports,
refuses debug CRTs and records `prerequisites[].minimum_version` = the MSVC build
tools version (the installed redistributable must be the same or later). Smoke
asserts each process loads the VC++ runtime from the Windows installation and
nothing but the executable from `Precept\bin`.

Cargo runs with inherited `RUSTFLAGS`/wrapper/target/profile overrides removed and
`--remap-path-prefix` for checkout, Cargo home and build output. Any remaining
checkout, home, Cargo-home or build-output bytes (ASCII or UTF-16) in a binary
**refuse the build**.

```powershell
./tools/portable/windows-package.ps1 -OutputParent .tmp/artifacts/local-p1-m5 -Name zip-attempt-01
./tools/portable/windows-smoke.ps1 -PackageDir .tmp/artifacts/local-p1-m5/zip-attempt-01/extracted -ReportDir .tmp/artifacts/local-p1-m5/zip-attempt-01/smoke-01
./tools/portable/windows-retention.ps1 -AttemptDir .tmp/artifacts/local-p1-m5/zip-attempt-01 -SmokeName smoke-01
./tools/portable/windows-sandbox.ps1 -AttemptDir .tmp/artifacts/local-p1-m5/zip-attempt-01 -Name sandbox-01
```

`windows-sandbox.ps1` is the disposable clean-target acceptance. It needs Windows
Sandbox already enabled (it never enables, installs or uninstalls anything on the host)
and refuses if another Sandbox is running. It maps the ZIP and the Microsoft-signed
`vc_redist.x64.exe` of the building Visual Studio read-only into a fresh, networkless
Sandbox. There, `sandbox-acceptance.ps1` (Windows PowerShell 5.1) proves the five
executables fail with `0xC0000135` without the prerequisite, installs it inside the
Sandbox only, then follows the README: private state directory, daemon in its own
console, CLI, MCP, HWND, Ctrl+C stop, exact removal. The Sandbox shuts itself
down; results stay in the ignored `<attempt>-sandbox-NN/output/`. Do not click inside
the Sandbox window while it runs.

Retain failed attempt directories; use a new name for the next run. Raw build logs
and tool metadata can contain private developer paths and remain ignored. The ZIP
contains only the explicit file inventory, provenance, notices and local instructions;
no Node/PowerShell helper, Rust toolchain, snapshots, raw logs, credentials or data.
Windows ZIP names use `x64`; a prototype also carries its source SHA suffix. The exact archive hash/file list live beside it.
`Precept/runtime/manifest.json` identifies clean source SHA/tree, both locks, target/flags,
builder, VC++ redistributable version/signer and every other file's hash. This is a reproducible recipe,
not a deterministic-output, signed, completed-SBOM or complete-license claim.
First-party license: Apache-2.0 (`LICENSE`, `NOTICE`). Third-party obligations remain a review gate.
Product code embeds no build-host path; the IPC runtime refuses any `.git` checkout
ancestor generically instead of a compiled-in source path. Keep all
artifacts private until that separate exclusion is resolved without deleting guards.

`windows-smoke.ps1` builds only the separate non-shipping harness and runs its
focused lint/tests. It launches **extracted** product binaries against a new private
synthetic fixture outside Git (`-PackageDir` is the extraction root holding `Precept/`), explicitly creates a fresh Profile, enables writes
only to register its allowlisted synthetic project, then revokes them. It tests
MCP tools/context/refusals, stale capabilities, endpoint collision, normal/crash
restart and runtime lease release. The Desktop reuses existing bounded owned
workers; no 148-cell matrix, IME/AT certification, model or system changes. A GUI
capture/resize needs the owned window foreground; the worker never steals focus.
Click it if requested. Its title nonce/PID bind the exact window. Only synthetic
capture/text/module evidence is retained. Graceful stop targets the new daemon's
process group, never other user processes; forced kill is separately labeled crash.
The fixture is retained on success/failure; its private path is only in ignored output.

For post-stop removal, use the exact original `archive-inventory.json`; preflight
all listed files/hashes before removing any. Known product directories are then
removed only when empty (non-recursive `rmdir`). Aliases, altered files and forged
members refuse. **Unknown/Profile/auth files are retained**. No recursive cleanup
or user-data erasure exists. The operation is developer tooling, not a shipped
uninstaller. An example for a disposable extracted package, after smoke exits:

```powershell
node tools/portable/windows-package.mjs --remove-owned-product .tmp/artifacts/local-p1-m5/zip-attempt-01/extracted .tmp/artifacts/local-p1-m5/zip-attempt-01/archive-inventory.json
```

Local checks: `node --test tools/portable/windows-package.test.mjs`, PowerShell AST
syntax checks, targeted harness fmt/Clippy/tests, recipe and packaged process smoke.
A developer-host PASS never satisfies the independent clean-target, second-user,
offline runtime closure or stable minimum Windows version gate. No VM availability
means BLOCKED/NOT_RUN, not installing a VM or changing the daily machine.

Standalone Linux x86_64 native Wayland acceptance remains a separate future gate;
WSLg does not satisfy it. S1-C validates the WSLg integration and a local-only Linux
package recipe on Ubuntu 24.04 WSL2. Do not infer general Linux support, add a compositor
matrix, or upload the generated archive as a PR/Release asset.

## Linux x86_64 `.tar.gz` (M5/S1-C)

Run on a clean committed checkout inside a native Linux x86_64 builder with the
pinned stock toolchain and Node. The default musl build ships only daemon/CLI/MCP
binaries; pass `x86_64-unknown-linux-gnu` to include the GTK4 Desktop. Precept uses
no media playback, so NSH 0.7.1 does not require GStreamer development packages:

```sh
node tools/portable/linux-package.mjs "$PWD" "$PWD/.tmp/artifacts/local-p1-m5" musl-attempt-01 "$HOME/.cargo/bin/cargo"
node tools/portable/linux-package.mjs "$PWD" "$PWD/.tmp/artifacts/local-p1-m5" gnu-attempt-01 "$HOME/.cargo/bin/cargo" x86_64-unknown-linux-gnu
```

The generated `.tar.gz`, inventory and checksum sidecars stay under the ignored local
artifact directory. Do not commit or upload them; formal distribution and signing are
deferred until an explicitly approved release stage.

The same `Precept/{bin,runtime,notices}` layout ships six executables (including the S1-B
`precept-bridge` helper). `elf.mjs` records the interpreter, DT_NEEDED, RPATH/RUNPATH
(refused) and symbol-version needs; direct DT_NEEDED owners (`dpkg -S`) become the
declared runtime packages written into the README `apt-get` line. OS-owned libraries
are never bundled. The tar is sorted, owned 0:0 and timestamped at the commit; the
listing policy refuses links, devices, traversal and unexpected modes.

`precept-linux-portable-smoke` (tools/native-acceptance) runs in the target:
`missing-deps BIN_DIR NEW_REPORT` before the packages are installed, the packaged
smoke `EXTRACTED_PARENT NEW_REPORT WINDOW_WORKER` (Wayland through WSLg via the
Windows window worker), and `archive ARCHIVE INVENTORY NEW_WORK_DIR` for listing
policy, hostile archives, collision and exact removal.
