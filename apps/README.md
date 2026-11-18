# ACP Operator — NSH Desktop/TUI and optional offline Web

The operator app. NSH Rust is the accepted UI direction (ADR-0003). `apps/` is its
own Cargo workspace (`apps/Cargo.toml`, `apps/Cargo.lock`, output in `apps/target/`),
separate from the product crates in `crates/`, so NSH dependencies never enter the
daemon's lockfile.

```text
apps/
├── core/       acp-operator-core    state, presentation, i18n, themes; pure, no I/O (also wasm)
├── client/     acp-operator-client  native IPC + Windows -> WSL2 bridge; bins acp-wsl-bridge, acp-ui-snapshot
├── ui/         acp-operator-ui      NSH Desktop/TUI/Web; bins acp-nsh-native, acp-nsh-tui
│   └── src/ui/                      the app's design system (styled controls, tokens, motion)
├── web-smoke/                       Node static-server and Playwright Web smoke
└── DESIGN.md                        design standards and motion inventories
```

`core` never depends on `client`; `ui` uses `client` only with its `ipc` feature
(`native`/`tui` imply it), so the Web build stays offline and synthetic. Until
2026-11, `core` and `client` were one crate (`acp-operator-model`) under
`apps/operator/`; historical reports keep those paths. The `gpui-kit` comparison
spike is archived at [`docs/archive/gpui-kit-spike`](../docs/archive/gpui-kit-spike);
it is historical source, not a supported build/CI candidate or second renderer.

Use the [current developer runbook](../docs/p1/rust-only-development.md) for
Cargo/native helper commands. Root native binaries must be built before the NSH
real-daemon component integration test. Desktop/TUI features are `native,tui` (from the workspace: `--features acp-operator-ui/native,acp-operator-ui/tui`).
Current operator scope and CLI-only gates are in the
[operator guide](../docs/p1/native-operator-workflows.md).

## Review keyboard flow

Alt+O opens the operator workspace; Alt+Left/Right switches Queue, Detail and
History. Load the scoped suggested queue, select a row and inspect the exact
revision, proposal fields, digest and peer-demotion disclosure before deciding.
Alt+Shift+Y/N prepares confirm/reject, W prepares deprecate, Q opens the separate
delete tier, and V explicitly applies a prepared operation. If inspection is
missing or stale, a decision chord only refreshes Detail; invoke it again after
reviewing the result to prepare. It never applies automatically.

Delete requires retyping the exact preference ID in its focused input before V.
Escape cancels consent. Profile/grant/target/project/task/scope/selection/source or
watermark drift invalidates review and delayed replies. The daemon independently
rechecks the pinned project root at mutation admission; clients receive no root
identity in the existing read DTOs and cannot observe a filesystem root change
until a daemon read/write refuses it.

An uncertain commit retains only the exact same-epoch request for deliberate
Alt+Shift+Z replay. Changing its authority or scope discards replay authority.
Receipts prove transactions, not reviewer identity; history channel labels and
source/origin claims are not authentication or independent verification.

The optional Web target is an offline synthetic fixture, without native IPC or a
server/gateway to the daemon. Its pinned renderer packaging and Node/Playwright
smoke are documented in the runbook. Loopback HTTP is test-only static asset serving.
No profile data, runtime capability or agent authentication is served.

Historical comparison, GUI capture and PTY results retain their original source
revisions under `docs/evaluation`. S4 removes those executable drivers. Headless
components are not proof of a mapped native window, physical keyboard, Wayland or
terminal restoration. Native GUI/TTY checks are local-only unless actual evidence
at the tested revision says otherwise; hosted Actions are not inferred to pass.
