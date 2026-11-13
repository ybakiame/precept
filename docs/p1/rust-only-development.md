# Rust-only development and local gates

Current runtime: fresh Rust-native Profile v1, native daemon/query/MCP and accepted
NSH Desktop/TUI. Read [AGENTS](../../AGENTS.md), [architecture](../architecture.md)
and [retirement/replacement map](python-retirement.md). No interpreter/package install,
legacy database bootstrap, migration, default Profile or model call is required.

## Native gates

Prerequisites: the manifests' supported Rust toolchain (currently rustc 1.99), Git
and native platform build libraries. Windows needs existing MSVC/SDK; Linux NSH
needs GTK4, Wayland/xkbcommon, clang and the upstream platform's build libraries.
Helpers do not install prerequisites or modify persistent system configuration.
Use locked caches with `CARGO_NET_OFFLINE=true` when available; do not delete locks
or install a different dependency version to bypass a missing cache.

From a clean committed checkout, use a **new** report directory with an existing
parent under the ignored ACP artifacts tree. Helpers refuse dirty source and an
existing output directory, stop at the first failed command and preserve logs.
Reports include exact source and command/status; they contain no real fixture data.

```bash
mkdir -p artifacts/local-p1-m2
CARGO_NET_OFFLINE=true tools/local_verify.sh all artifacts/local-p1-m2/s5-linux 05035e21434085faf368ddec5a06904a99ef4a35
```

```powershell
$env:CARGO_NET_OFFLINE = 'true'
./tools/windows_local_verify.ps1 -Suite All -ReportDir artifacts/local-p1-m2/s5-windows -DiffBase 05035e21434085faf368ddec5a06904a99ef4a35
```

Linux helper modes: `core`, `ui`, `all`; Windows: `-Suite Core|UI|All`.
Windows toolchain setup affects only the helper process. The UI mode first builds
root binaries for the real-daemon integration test. All helpers run these extant
Cargo commands as applicable:

Both helpers require an explicit full baseline commit. They refuse HEAD-equal,
empty, missing, invalid, ambiguous and non-ancestor deltas, and first run
`git diff --check BASE HEAD`; a clean worktree alone is insufficient. The baseline
above is the merged S4 main for S5. Use a new artifact directory on every run.

```text
cargo fmt --all -- --check
cargo build --workspace --bins --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo fmt --manifest-path spikes/p1-e/shared/Cargo.toml -- --check
cargo clippy --manifest-path spikes/p1-e/shared/Cargo.toml --all-targets --features native --locked -- -D warnings
cargo test --manifest-path spikes/p1-e/shared/Cargo.toml --features native --locked
cargo fmt --manifest-path spikes/p1-e/nsh/Cargo.toml -- --check
cargo build --manifest-path spikes/p1-e/nsh/Cargo.toml --features native,tui --bins --locked
cargo clippy --manifest-path spikes/p1-e/nsh/Cargo.toml --all-targets --features native,tui --locked -- -D warnings
cargo test --manifest-path spikes/p1-e/nsh/Cargo.toml --all-targets --features native,tui --locked
```

Root tests launch independent Rust daemon/CLI/MCP/fake-agent processes against owned
temporary synthetic projects/Profiles. No real vendor executable, key or user DB is
used. NSH tests include real components, stale state guards and 10k bounded mounting;
these are headless, not native GUI/TTY acceptance. The optional ignored S1 performance
measurement is not a skipped security test.

## Source, CI and Node checks

Use Node 24+ and the exact lockfile (Playwright plus YAML parser for CI validation):

```text
npm ci --prefix spikes/p1-e/tests
node tools/check_source.mjs
node --test spikes/p1-e/tests/static-server.test.mjs
```

The source check rejects tracked prototype executables/packaging, active interpreter
or removed-target invocations, broken local Markdown links and invalid workflow YAML
or missing local run-step paths. `docs/archive-index.json` explicitly classifies
historical documents, whose marked quotations retain old SHAs/results. No historical
command is part of current setup. The frozen inventory is a reference-only deletion
map, not a promise those paths still exist.

## Optional offline Web smoke

This is a separate synthetic browser check. Use a new output directory and the
exact existing upstream NSH revision; obtaining build/cache dependencies is explicit.
No native daemon, credential or remote-write bridge is needed.

```text
rustup target add wasm32-unknown-unknown
cargo build --manifest-path spikes/p1-e/nsh/Cargo.toml --target wasm32-unknown-unknown --lib --locked
node tools/package_ui_web.mjs PINNED_NSH_CHECKOUT spikes/p1-e/nsh/target/wasm32-unknown-unknown/debug/acp_nsh_spike.wasm spikes/p1-e/nsh/dist
npx --prefix spikes/p1-e/tests playwright install chromium
node spikes/p1-e/tests/web-smoke.mjs spikes/p1-e/nsh/dist spikes/p1-e/artifacts
```

`PINNED_NSH_CHECKOUT` must be an explicit checkout at
`87c7e8da85d2f15a56107baa6f0fabb419b508b3`, including its renderer/license assets.
The packager refuses an existing output rather than mixing stale assets. The smoke
uses a loopback-only static Node server, blocks external requests and exercises the
actual Rust WASM app offline, keyboard and compact resize. It is not desktop evidence.

## Actual native UI and reporting

S5's [owned Rust/PowerShell driver](../../tools/native-acceptance/README.md) supplies
actual Windows/ConPTY and direct WSLg Wayland/PTY input and captures. Use a clean
committed source, build all product binaries first, then run the native scenarios
sequentially with respect to builds; Windows refuses replacement of a running exe.
Never infer native PASS from component tests or borrow historical S3 observations.
The [S5 verdict](../evaluation/p1-m2-s5-rust-only-integrated-acceptance.md) records the
exact tested code SHA, binary hashes, private report paths and evidence levels.
The operator guide includes Alt+Shift+H for the exact saved-Handoff read; verify
the actual saved revision/history is displayed. Stored history is not fresh evidence.

Retired capture/input drivers are not required or restored. A manual desktop run
uses the native binary with an explicit endpoint and `--operator`; it must never
activate a real Profile implicitly. X11/Xvfb is not native Wayland acceptance.

Hosted workflows describe future gates; do not dispatch them during quota exhaustion.
Use `[skip ci]`, record clean SHA/platform/commands and actual PASS/FAIL/BLOCKED/NOT_RUN,
keep raw artifacts private and create a Draft PR. Do not close parent #40 before S5.
