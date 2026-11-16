# Windows portable prototype (M5/S1-A)

Developer-only tooling for [Issue #70](https://github.com/ybakiame/agent-control-plane/issues/70),
after the [S0 contract](../../docs/p1/release-readiness-plan.md). No installation,
release, real Profile, provider, agent HOME or platform configuration is required
or authorized. Product source/UI/schema/authority are unchanged.

Use a clean committed **native Windows x86_64 checkout**, existing pinned stock
Rust 1.99.1/MSVC/SDK, PowerShell 7, Node and Git. Helpers temporarily select the
stock toolchain instead of any global Cargo fork, work offline with existing caches
and restore the caller's environment. Missing tools/cache fail, never install or
change pins/locks. They do not provision a VM or call hosted workflows.

Create an existing owned output parent under ignored `artifacts/local-p1-m5/`.
Each attempt name must be NEW. The recipe exclusively creates/protects its private
output, builds into a new Cargo target, copies only actual Cargo artifact messages
for five product executables into independent regular files, audits x86_64 PE
normal/delay DLL imports and collects a **resolved dependency superset** notice
ledger. Cargo's internal artifact hard links are not copied as archive aliases.
No target-directory glob or fake/test driver is shipped.

```powershell
./tools/portable/windows-package.ps1 -OutputParent artifacts/local-p1-m5 -Name zip-attempt-01
./tools/portable/windows-smoke.ps1 -PackageDir artifacts/local-p1-m5/zip-attempt-01/extracted -ReportDir artifacts/local-p1-m5/zip-attempt-01/smoke-01
./tools/portable/windows-retention.ps1 -AttemptDir artifacts/local-p1-m5/zip-attempt-01 -SmokeName smoke-01
```

Retain failed attempt directories; use a new name for the next run. Raw build logs
and tool metadata can contain private developer paths and remain ignored. The ZIP
contains only the explicit file inventory, provenance, notices and local instructions;
no Node/PowerShell helper, Rust toolchain, snapshots, raw logs, credentials or data.
The ZIP name carries its source SHA suffix; the exact archive hash/file list live beside it. `manifest.json` identifies clean
source SHA/tree, both locks, target/flags and builder. This is a reproducible recipe,
not a deterministic-output, signed, completed-SBOM or complete-license claim.
First-party grant/text and third-party obligations remain owner/review gates.
Existing binaries can embed builder paths, including the compiled IPC runtime guard;
archive metadata path checks do not prove path-free executable bytes. Keep all
artifacts private until that separate exclusion is resolved without deleting guards.

`windows-smoke.ps1` builds only the separate non-shipping harness and runs its
focused lint/tests. It launches **extracted** product binaries against a new private
synthetic fixture outside Git, explicitly creates a fresh Profile, enables writes
only to register its allowlisted synthetic project, then revokes them. It tests
MCP tools/context/refusals, stale capabilities, endpoint collision, normal/crash
restart and runtime lease release. Desktop and ConPTY reuse existing bounded owned
workers; no 148-cell matrix, IME/AT certification, model or system changes. A GUI
capture/resize needs the owned window foreground; the worker never steals focus.
Click it if requested. Its title nonce/PID bind the exact window. Only synthetic
capture/text/module evidence is retained. Graceful stop targets the new daemon's
process group, never other user processes; forced kill is separately labeled crash.
The fixture is retained on success/failure; its private path is only in ignored output.

For post-stop removal, use the exact original `archive-inventory.json`; preflight
all listed files/hashes before removing any. Aliases, altered files and forged
members refuse. **Unknown/Profile/auth files are retained**. No recursive cleanup
or user-data erasure exists. The operation is developer tooling, not a shipped
uninstaller. An example for a disposable extracted package, after smoke exits:

```powershell
node tools/portable/windows-package.mjs --remove-owned-product artifacts/local-p1-m5/zip-attempt-01/extracted artifacts/local-p1-m5/zip-attempt-01/archive-inventory.json
```

Local checks: `node --test tools/portable/windows-package.test.mjs`, PowerShell AST
syntax checks, targeted harness fmt/Clippy/tests, recipe and packaged process smoke.
A developer-host PASS never satisfies the independent clean-target, second-user,
offline runtime closure or stable minimum Windows version gate. No VM availability
means BLOCKED/NOT_RUN, not installing a VM or changing the daily machine.

Linux **S1-B remains required**: use S1-A file-list/provenance/retention lessons to
plan an independently scoped archive/ELF dependency slice with both native Wayland
Desktop and PTY TUI on one clean standalone x86_64 distro/compositor. A `.tar.gz` is
recommended pending GTK/libc/media closure; WSLg cannot satisfy standalone acceptance.
Do not automatically implement Linux packaging, a compositor matrix or new UI stack.
