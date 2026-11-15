# ACP Operator — NSH Desktop/TUI and optional offline Web

This is the formal location of the operator app, moved from `spikes/p1-e`.
NSH Rust is the accepted UI direction (ADR-0003). `model` (package
`acp-operator-model`, formerly `shared`) and `ui` (package `acp-operator-ui`,
formerly `nsh`) are members of the single `apps/operator` Cargo workspace and share
one presentation/operator model. `tests` holds the Node static-server and Web smoke
checks. The `gpui-kit` comparison spike is archived at
[`docs/archive/gpui-kit-spike`](../../docs/archive/gpui-kit-spike); it is historical
source, not a supported build/CI candidate or second renderer.

App binaries keep the `acp-nsh-native` and `acp-nsh-tui` names. Because `model` and
`ui` now share one workspace, build output lands in `apps/operator/target/` and the
lockfile is the single tracked `apps/operator/Cargo.lock`.

Use the [current developer runbook](../../docs/p1/rust-only-development.md) for
Cargo/native helper commands. Root native binaries must be built before the NSH
real-daemon component integration test. Desktop/TUI features are `native,tui`.
Current operator scope and CLI-only gates are in the
[operator guide](../../docs/p1/native-operator-workflows.md).

The optional Web target is an offline synthetic fixture, without native IPC or a
server/gateway to the daemon. Its pinned renderer packaging and Node/Playwright
smoke are documented in the runbook. Loopback HTTP is test-only static asset serving.
No profile data, runtime capability or agent authentication is served.

Historical comparison, GUI capture and PTY results retain their original source
revisions under `docs/evaluation`. S4 removes those executable drivers. Headless
components are not proof of a mapped native window, physical keyboard, Wayland or
terminal restoration. Native GUI/TTY checks are local-only unless actual evidence
at the tested revision says otherwise; hosted Actions are not inferred to pass.
