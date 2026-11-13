# P1-M2/S4 — Rust-only source and validation evidence

Authority: [Issue #48](https://github.com/ybakiame/agent-control-plane/issues/48),
parent [#40](https://github.com/ybakiame/agent-control-plane/issues/40), accepted
ADR-0006/0004 and retained daemon ownership in ADR-0005.
Branch: `feat/p1-m2-s4-retire-python`. Delivery remains Draft for human review.
Date: 2026-11-13. All process tests use owned temporary synthetic state; no real
Profile, credentials, vendor/model or external development checkout was used.

## Source and contract order

Base main is merged PR #47:
`58750f346bf2288d79f9ff29754ca110ebca52d0`.

| Commit | Scope |
| --- | --- |
| `808a18e299ecf143e02d76929f49b73f19c0b352` | Exact inventory/replacement matrix and acceptance plan, committed before removals |
| `be3b6c72539414071668cc4af33083d157a892ec` | Retire package, tests and all tracked executable probes; preserve fixture; Rust fingerprint/session replacements and root-failure revoke fix |
| **`989ac4cf0b43d2d02caa58bf1e47e35f8608dcf5`** | Final tested product/tooling: direct Cargo helpers, native CI definitions, source/YAML checks, Node static server/packaging/Web smoke, current docs and NSH host clock |

Both complete native gate runs started from clean tracked worktrees at **989ac4c**.
The final Web smoke also records this clean SHA. Later evidence edits are
documentation only; no test result is relabeled as a later report commit.
Every new branch commit uses `[skip ci]`; hosted Actions were not dispatched.

## Retirement and replacements

The [frozen inventory](../p1/python-retirement.md) lists all **64** retired Python
files: 5 package modules, 8 tests and 51 tools. Root packaging/entrypoint, obsolete
external parity examples and the standalone package CI workflow are removed.
No tracked executable Python maintenance exception, wrapper, migration/import,
legacy reader, rollback owner or parallel runtime is retained.

The logical preference fixture is byte-for-byte unchanged at
`crates/acp-core/tests/preference-resolution.v1.json`; both Rust consumers now use
it. It is domain test data, not legacy Profile storage compatibility. Exact fresh
Profile v1 refusal tests for old schemas remain in Rust.

- Fingerprint v1 no longer launches an interpreter oracle. Independent fixed
  SHA-256 vectors cover binary/tracked data, Unicode/raw names, modes, link targets/
  ordering and bounds; native Git tests retain drift, stages, modes, rename,
  HEAD/branch, symlink containment, overflow, deadlines and external-filter refusal.
  Fingerprint bytes and budgets are unchanged.
- Eight new Rust session process groups replace essential retired driver claims:
  explicit start/scope/generation, unshared default, malformed initialize/auth/
  metadata, auth-required observation, reverse-auth refusal, allow-once/default
  deny/stale decisions, real **30-second permission expiry**, protocol failure,
  cancellation/deadline, established-peer revoke, root failure, lease expiry and
  hard restart. Actual native daemon, deterministic fake agent and IPC are used.
  The fake agent's test-only context flag allows negative protocol modes to receive
  the real read-only attachment without calling a vendor.
- Existing independent CLI/ownership/MCP groups, store/writer tests, role and lease
  tests, shared NSH **32** tests and NSH real-daemon component/10k tests remain
  enabled. Retiring vendor-probe guard tests does not claim fresh model parity.
- CI definitions now invoke Cargo workspace and accepted shared/NSH checks directly.
  The former GPUI/hybrid/Xvfb/Python probe matrix is removed. Optional offline Web
  uses Node; the source job validates actual YAML and extant run-step scripts.
- Linux shell and Windows PowerShell helpers run the documented Cargo commands,
  preserve exact-source reports and stop on failed gates. Neither creates a default
  Profile/grant, invokes a real agent, installs tools or dispatches Actions.
- **33** explicitly catalogued historical documents have archival banners; original
  dates, SHAs and PASS/FAIL outcomes remain historical. Deleted executable links
  are explanatory retired-path text. Current README/AGENTS/interfaces/security/
  product/architecture/roadmap/NSH runbook describe the active runtime.
- Pre-retirement local bytecode/package metadata remains ignored to avoid uploading
  or indiscriminately deleting local generated files. It is not built, imported or
  packaged. No ignored auth/runtime/profile directory was cleaned.

## Security findings resolved

The ported root-failure process case exposed an early rejection path: complete
Profile validation could reject an unavailable root before the later session
binding check, leaving the session lease active. The daemon now invalidates session
context on that earlier read refusal and on failed/timed-out prompt root validation.
Established peers and new handshakes are denied; Stop remains available and logical
Profile state is unchanged. No validation check or expected rejection was relaxed.

Linux exercises an actually renamed/missing synthetic root. Windows can pin a live
child's cwd, so its test injects the equivalent unavailable registered root in the
owned synthetic database, snapshots that intentional fixture change, then asserts
no further state change and revoked authority. No real DB or OS sharing configuration
is touched. Cancel revocation is asserted immediately at acknowledgement, before
the ordinary two-second idle framing deadline, separately from delayed child reaping.

Web revalidation exposed a retained WASM startup panic from native
`std::time::Instant`. Only the load-duration clock changes to NSH's existing host
clock, preserving native behavior and using the browser clock on WASM. There is no
new UI workflow, layout, permission or framework dependency.

## Exact local gates

| Host | Environment |
| --- | --- |
| Linux/WSL | Arch Linux, x86_64, WSL2 6.18.54.1-microsoft-standard-WSL2 |
| Native Windows | Windows 11 10.0.26300.0, AMD64, isolated owned checkout |
| Both native hosts | Rust 1.99.1 (58cf8f9c3), Cargo 1.99.1 (5f94df478), locked local caches |
| Web/source checks | Linux Node 26.13.1, Playwright 1.65.0, Chromium 151.0.7922.34; YAML 2.9.1 test dependency |

The public native entrypoints actually ran: Linux
`CARGO_NET_OFFLINE=true tools/local_verify.sh all artifacts/local-p1-m2/s4-linux-final`
and Windows
`./tools/windows_local_verify.ps1 -Suite All -ReportDir artifacts/local-p1-m2/s4-windows-final`.
Source objects were transferred into the owned Windows checkout locally, then
native Git checked out the exact SHA. The first native fetch attempt rejected a
Linux-only origin path; that setup failure is not counted as a gate PASS.
MSVC/SDK/PATH configuration was process-local.

At clean **989ac4cf0b43d2d02caa58bf1e47e35f8608dcf5**:

| Command | Linux/WSL | Native Windows |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | PASS |
| `cargo build --workspace --bins --locked` | PASS | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS | PASS |
| `cargo test --workspace --all-targets --locked` | **PASS, 116** | **PASS, 117** |
| `cargo fmt --manifest-path spikes/p1-e/shared/Cargo.toml -- --check` | PASS | PASS |
| `cargo clippy --manifest-path spikes/p1-e/shared/Cargo.toml --all-targets --features native --locked -- -D warnings` | PASS | PASS |
| `cargo test --manifest-path spikes/p1-e/shared/Cargo.toml --features native --locked` | **PASS, 32** | **PASS, 32** |
| `cargo fmt --manifest-path spikes/p1-e/nsh/Cargo.toml -- --check` | PASS | PASS |
| `cargo build --manifest-path spikes/p1-e/nsh/Cargo.toml --features native,tui --bins --locked` | PASS | PASS |
| `cargo clippy --manifest-path spikes/p1-e/nsh/Cargo.toml --all-targets --features native,tui --locked -- -D warnings` | PASS | PASS |
| `cargo test --manifest-path spikes/p1-e/nsh/Cargo.toml --all-targets --features native,tui --locked` | **PASS, 7** | **PASS, 7** |
| `git diff --check` | PASS | PASS |

One S1 manual performance measurement remains explicitly ignored on each host;
there are **zero ignored security/acceptance tests**. The retired language-parity/
package tests were not run as a supported S4 gate. Root tests retain the 15 existing
real ownership/CLI/preview groups, seven MCP process groups and the new eight session
groups. NSH totals include the real-daemon component test and the unchanged
10,000-row/834-page bounded mounting regression.

Both helpers refused an existing report directory before any gates could overwrite
it; the Windows report digest was unchanged. Cargo dependency inspection still
shows only `acp-daemon` as the normal dependent of `acp-write`.

## Source and optional Web evidence

Source/server/WASM/Web gates ran on Linux at clean 989ac4c. Lockfile installation
was reconfirmed during documentation-only reporting, with no product/tool changes:

| Command/check | Result |
| --- | --- |
| `npm ci --prefix spikes/p1-e/tests` | PASS; Playwright unchanged, only required YAML parser added |
| `node tools/check_source.mjs` | PASS: 192 active/historical tracked paths checked, zero tracked prototype executables/packaging, 33 explicit archives, active refs/Markdown links/YAML/manifest/run-step targets valid |
| `node --test spikes/p1-e/tests/static-server.test.mjs` | PASS, 1 real HTTP group: loopback assets/WASM MIME/HEAD; traversal, escaping symlink, malformed paths, directories and writes denied |
| `cargo build --manifest-path spikes/p1-e/nsh/Cargo.toml --target wasm32-unknown-unknown --lib --locked` | PASS |
| `node tools/package_ui_web.mjs PINNED_NSH_CHECKOUT WASM NEW_OUTPUT` | PASS, exact upstream revision 87c7e8da85d2f15a56107baa6f0fabb419b508b3 |
| `CHROMIUM=EXISTING_CACHED_CHROMIUM node spikes/p1-e/tests/web-smoke.mjs STATIC_ROOT NEW_OUTPUT` | PASS: actual Rust WASM + browser DOM, offline five-page navigation, project switch, Tab/Enter, appearance, filter/details, exact-task invalid/missing/recovery, disabled write controls and 1080×760→640×480 resize |

Web requested only `/`, `/start.js`, `/nsh.js` and its synthetic WASM asset:
**zero external requests**, no WebSocket, native IPC, daemon gateway or credentials.
Screenshots were inspected locally and remain ignored. The pinned renderer does
not mirror input labels into DOM aria-labels; tests select the sole visible scoped
input and assert the resulting behavior. This is not an accessibility certification.

The first WASM build needed the existing clang 22 bin directory in process PATH;
the wasm standard-library component was added to the user Rust toolchain.
No system configuration/library installation was performed. Initial stale selectors
and the actual startup panic were resolved before the final clean-source smoke.
Node tooling is not installed on native Windows: Windows Node/Web checks are
**NOT_RUN**, with no interpreter fallback. Cargo/native component checks above do
not depend on Node. Source/YAML and optional Web CI jobs target Linux explicitly.

Private ignored evidence under `artifacts/local-p1-m2/`:

- `s4-linux-final/`: exact source/toolchain, enumerated command list, status TSV and logs.
- `s4-windows-final/`: copied native machine-readable report (all 12 gates PASS) and logs.
- `s4-source-final.log`, `s4-node-final.log`, `s4-wasm-final.log`.
- `s4-web-final/web-result.json`: clean SHA, browser identity, assertions and observed request paths.
- `s4-helper-refusal.log`, `s4-native-guard.log`: report overwrite refusal.
- Earlier development failures remain separate and are not presented as acceptance passes.

## NOT_RUN and remaining scope

- **NOT_RUN:** hosted GitHub Actions, current actual Windows/WSLg mapped windows,
  PTY/ConPTY input/restoration, macOS, niri, mobile, model/vendor calls, clean-machine
  installation/release and full accessibility/IME.
- Original S3 native GUI/TTY evidence stays at
  [a99b218 in the S3 report](p1-m2-s3-native-operator.md). S4 headless/browser tests
  are not substituted for native Wayland/HWND/TTY acceptance.
- Profile v1 DDL/4096 cap, strict verification, daemon-exclusive writer, lease/
  epoch/grants, seven commands, exact operator previews, two-tool read-only MCP and
  Windows-to-WSL/offline/Web write refusal are preserved. No GUI redesign or new
  product capability is introduced.
- No real Profile/auth/runtime directory was accessed or reset. No executable
  legacy runtime/compatibility or migration requirement remains in active source.
- After S4 review and merge, open a separate **S5 final integrated M2 acceptance**
  issue. Parent #40 remains open; S4 does not activate production writes or release.

Keep this delivery **Draft** and wait for human review; do not automatically merge.
