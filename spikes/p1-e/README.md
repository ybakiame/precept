# P1-E: isolated, read-only UI experiments

Issue #14; base `d92bcadddcf33963def536698268db4996b1c889`.
These are optional workspaces, not production dependencies or a UI framework decision.
All commands below run from the repository root with Rust **1.99.1**.

| Candidate | Exact source | Backend in this experiment |
| --- | --- | --- |
| GPUI Kit 0.9.0 | `7591a35eced761e3613c01e71ff04c10129dae1a` | GPUI Fast 0.1.13, real Kit DataTable/scroll |
| NSH development snapshot, manifest 0.4.0 | `87c7e8da85d2f15a56107baa6f0fabb419b508b3` | Rust-only GTK/AppKit/Win32, ratatui, Rust WASM/DOM |
| Hybrid | those same revisions | Rust NSH AppView embedded in the Kit host |

NSH release v0.4.0 (`e626a8d`) lacks TUI. The pinned newer source is intentional.
The release CLI `nsh 0.4.0 (e626a8d)` was used only to actually try
`nsh init <temporary-directory> --template rust`; application builds use the
exact Git dependencies above, not that release's prebuilt renderer.
Each workspace has its own Cargo.lock. The ACP workspace/lockfile is unchanged.
The Kit spike lock explicitly resolves gpu-allocator's broad Windows binding range
to windows 0.62.2, matching wgpu-hal 29's DX12 types in the hybrid. A broad Cargo
update can choose an older binding and recreate the Windows type mismatch; verify
that relationship when refreshing this optional lock. No production dependency is changed.

## Build and run

Install native framework prerequisites first. On Ubuntu 24.04/Debian 13 the
native jobs list the exact packages in `.github/workflows/ui-spike.yml` (GTK
4.14+, Fontconfig/Freetype/X11/Wayland/xkbcommon, GStreamer, clang/libclang).
macOS needs Xcode Command Line Tools; Windows needs MSVC/Windows SDK.

```sh
cargo test --manifest-path spikes/p1-e/shared/Cargo.toml --features native --locked
cargo build --manifest-path spikes/p1-e/gpui-kit/Cargo.toml --locked
cargo run --manifest-path spikes/p1-e/gpui-kit/Cargo.toml --locked
cargo test --manifest-path spikes/p1-e/gpui-kit/Cargo.toml --features ui-tests --locked
cargo test --manifest-path spikes/p1-e/nsh/Cargo.toml --lib --locked
cargo run --manifest-path spikes/p1-e/nsh/Cargo.toml --features native --bin acp-nsh-native --locked
cargo run --manifest-path spikes/p1-e/nsh/Cargo.toml --features tui --bin acp-nsh-tui --locked
cargo run --manifest-path spikes/p1-e/gpui-kit/Cargo.toml --features hybrid --locked
```

No location flags means the **fixed offline synthetic snapshot**, captured from
a disposable Python-created database through the real daemon. Only synthetic
root paths were normalized. No real Git/session state or fingerprint is invented.
`shared` contains the typed model, selection/filter/theme/view-state logic and
the optional authenticated operator IPC client. All frontends consume it.

For live **synthetic** IPC, explicitly start the disposable foreground daemon
using [the P1-D demo](../../docs/p1/local-service.md). Append to any native `cargo
run` above (after `--`):

```text
--runtime-dir <private DEMO/run> --endpoint <Unix socket path or Windows pipe label>
```

The client loads the existing operator capability privately through acp-ipc;
no token value is an argument, asset, environment variable or log. The daemon
enforces its allowlist unchanged. A failed connection clears displayed data;
it never falls back to a synthetic success. Reconnect is read-only and runs off
the UI thread. Closing a UI never sends a daemon shutdown message. Task `shared`
is the fixed read-only handoff query in this narrow trial. There are no writes,
profile discovery, UI SQLite opens, automatic daemon launch or browser IPC gateway.
Offline status explicitly says captured synthetic health; it is not a claim of
a connected daemon. NSH native/TUI/hybrid use empty in-memory framework storage
and disable its disk image cache, rather than reading a framework profile.

## Web: offline Rust app only

Clone NSH to a separate directory and check out the exact revision above. Its
renderer JS is copied unmodified with its license; app state/handlers stay Rust.

```sh
rustup target add wasm32-unknown-unknown
cargo build --manifest-path spikes/p1-e/nsh/Cargo.toml --target wasm32-unknown-unknown --lib --locked
python tools/package_ui_spike_web.py --source <pinned-nsh-checkout> \
  --wasm spikes/p1-e/nsh/target/wasm32-unknown-unknown/debug/acp_nsh_spike.wasm \
  --out spikes/p1-e/nsh/dist
npm ci --prefix spikes/p1-e/tests
npx --prefix spikes/p1-e/tests playwright install chromium --with-deps
node spikes/p1-e/tests/web-smoke.mjs spikes/p1-e/nsh/dist spikes/p1-e/artifacts
```

Use `CHROMIUM=/path/to/chromium` to use an existing browser. The test serves only
static synthetic assets on a loopback ephemeral port, blocks external requests,
then disconnects the browser network before testing selection/keys/theme/states.
It does not serve daemon data. Browsers cannot use native IPC; authenticated web
integration requires a separately designed boundary and is deliberately absent.

## Evidence gates

```sh
cargo build --workspace --bins --locked
cargo build --manifest-path spikes/p1-e/shared/Cargo.toml --features native --locked
PYTHONPATH=src python tools/test_ui_spike_contract.py
# Linux terminal test; pip install pyte
python tools/test_ui_spike_tui.py --binary spikes/p1-e/nsh/target/debug/acp-nsh-tui --out spikes/p1-e/artifacts
# Native display required; pip install mss psutil. Linux also needs xdotool.
PYTHONPATH=src python tools/test_ui_spike_native.py --candidate nsh \
  --binary spikes/p1-e/nsh/target/debug/acp-nsh-native --out spikes/p1-e/artifacts
```

Native smoke runs an explicitly created synthetic daemon, enumerates the actual
OS window, waits for fixed model-ready markers, rejects a blank screenshot,
resizes on Linux, terminates only the UI and verifies daemon health and identical
DB bytes. Markers contain no returned data or credentials. GPUI/NSH component
tests are separate headless state/input tests, not native-window claims.
On Linux run Xvfb and the probe in the same IPC namespace (`xvfb-run` does this).
Separate managed exec calls isolate SysV SHM and can produce a blank GTK window;
that failed experiment is not RUN_PASS. Hosted macOS/Windows without an
enumerable interactive desktop report NOT_RUN, not fabricated runtime parity.

[Measured comparison](../../docs/p1/ui-spike-evaluation.md) and
[Proposed ADR](../../docs/adr/0003-ui-framework-selection.md) distinguish actual
results, blocked experiments, missing coverage and decision criteria.
