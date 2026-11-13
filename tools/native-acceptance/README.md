# S5 owned native acceptance driver

Repository maintenance only; not a product launcher, terminal emulator or transport.
See the [frozen matrix](../../docs/p1/rust-only-integrated-acceptance.md) and
[development gates](../../docs/p1/rust-only-development.md). No Python is required.

Build the root daemon/query and NSH native,tui binaries from the same clean commit
before running this standalone manifest. Its only ACP dependency is the IPC client;
it cannot open a writable Profile. Run from the repository root:

```text
cargo fmt --manifest-path tools/native-acceptance/Cargo.toml -- --check
cargo build --manifest-path tools/native-acceptance/Cargo.toml --locked
cargo clippy --manifest-path tools/native-acceptance/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path tools/native-acceptance/Cargo.toml --locked
```

Choose a **new absolute** report directory below the ignored artifacts tree. On
Linux, run `tools/native-acceptance/target/debug/acp-native-acceptance tui NEW_OUTPUT`.
On native Windows, run the corresponding `.exe` with the same arguments from an
existing process-local MSVC/Cargo/Git environment. This creates a real PTY/ConPTY,
feeds terminal keyboard sequences and handles the terminal's cursor query. Captures
contain actual child VT output, a bounded parsed view, and restoration assertions.

For `desktop`, append the absolute **Windows path** to `tools/native_window.ps1`.
Windows runs a native HWND; Linux uses the existing WSLg `wayland-0` with X11 removed
and invokes the installed Windows PowerShell through WSL interop. Pass the script
from a same-source native Windows checkout, since Windows cannot execute a Linux
script path. No dependency installation or compositor launch is performed.

The window worker matches a unique synthetic title nonce and exact native child
PID, or the WSLg host `msrdc` plus the nonce (WSLg may append a distro suffix). It
never forces focus. `FOREGROUND_REQUIRED` means click the owned window and leave it
in the foreground during input. Only fixed keyboard chords and ASCII synthetic
fields are accepted; Ctrl+1 is excluded. Captures use only that owned window via
PrintWindow. Verify the PNGs visually; neither a probe nor a successful capture
API return alone proves rendered content. GUI shutdown is an owned WM_CLOSE.

Each run creates only its own `acp-s5-*` temporary root, synthetic alpha/beta
projects, private runtime and explicitly reviewed Profile. CLI-only setup is done
with the native CLI. A bounded local authenticated relay consumes one **actual**
committed mutation reply and disconnects; it forwards no invented responses. Its
private receipt artifacts prove identical explicit replay and unchanged row count.
It is fault injection for this driver, never a product relay or network service.
No default DB, user auth, external project or model is accessed. Exact children
are reaped before their owned temporary files are removed. A crash can leave owned
temporary artifacts; never clean unrelated directories or retained credentials.

The scenario covers create/grant/Busy, suggested/confirm/reject, exact record and
saved Handoff, Unknown/retry, stale Conflict, Escape/project review invalidation,
revocation/Unauthorized, five pages, Sessions, theme/locale/resize and disconnect.
Per-page captures and content-free native probes supplement independent CLI state
and receipt checks. Shared-model/process tests remain responsible for specific
late completion and task/scope/target races; do not claim every race was injected
through OS input. Stored NOT_RUN Handoff history is not fresh Git evidence.

A normal run requires clean source before and after and records the Git SHA plus
actual driver/daemon/query/NSH SHA-256. `ACP_S5_DEVELOPMENT=1` explicitly labels
exploratory runs DEV_PASS and allows dirty source; those are never final acceptance.
Run all four surfaces again at the final code SHA and keep artifacts private.
