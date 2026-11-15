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

`ACP_S5_APPEARANCE=dark` repeats the same native scenario in dark appearance;
the default is `light`. Other values are refused. Reports record the actual
requested appearance; screenshot inspection remains a separate requirement.

For `desktop`, append the absolute **Windows path** to `tools/native_window.ps1`.
Windows runs a native HWND; Linux uses the existing WSLg `wayland-0` with X11 removed
and invokes the installed Windows PowerShell through WSL interop. Pass the script
from a same-source native Windows checkout, since Windows cannot execute a Linux
script path. No dependency installation or compositor launch is performed.

Every worker call is bounded. One request settles with exactly one validated reply
inside one deadline that covers both the write and the reply (init 300 s, capture 180 s,
other actions 60 s). Each request carries a monotonic `rpc_id` on the wire and the reply
must name it; any reply carrying a nonce must name this owned window. A hung, malformed,
oversized, truncated, silent, early-exiting, duplicated, unsolicited or wrong-nonce
worker fails closed: the run reports `DESKTOP_RPC_FATAL` with a sanitized cause plus the
sequence number and action, terminates and reaps only the owned worker, and joins **both**
IO threads instead of detaching a blocked one, so a late reply can never acknowledge a
later action. The stdin write is a cancelable job, so a worker that stops consuming stdin
cannot wedge the harness on a full pipe. Neither thread may block past one poll: Unix uses
`O_NONBLOCK` on both ends, while Windows sets it only on the write end and polls the read
end with `PeekNamedPipe`, because `PIPE_NOWAIT` is not honored for the read end of an
anonymous pipe. `DESKTOP_IO_MODE stdin=… stdout=… gui_stderr=…` prints the mode actually
achieved per direction. `tests/desktop_worker.rs` proves all of this against the owned
`acp-fake-desktop-worker` binary on either host with no real window; that is driver
evidence only and never a native GUI PASS.

Bounded pipe IO is a precondition, not a preference. The setters in `src/pipe.rs` can
only return `nonblocking` or `peek-poll`: there is no `blocking` value to keep going with.
If the mandatory setup fails, `WorkerRpc::spawn` refuses the session
(`SpawnError::Unfit { pid, reaped, detail }`) after terminating and reaping exactly the
child it launched, and a program that never started is reported as
`SpawnError::Launch` rather than as a half-built session.

The GUI window process is owned from the moment `Desktop::start` spawns it
(`src/process.rs`): adoption happens before any initialization step can fail, and its
stderr probe pump runs on the same cooperative bounded reader. A refused `init`, a window
that never appears, a bounded worker fault, a refused adoption or a panic therefore
unwinds through ownership, which terminates and reaps that child and bounded-joins its log
thread, printing `OWNED_PROCESS_CLEANUP name=gui … reaped=… reader_joined=…` (plus an
explicit `OWNED_PROCESS_TEARDOWN_INCOMPLETE` when either is false). `OWNED_DRIVER_CLOSED`
reports `gui_reaped` and `gui_reader_joined` next to the worker's own three fields.
`tests/owned_process.rs` drives those paths with an owned mock GUI, including a panic in
the middle of initialization; the refusal paths that neither host can produce on demand go
through the documented `*_with_*sabotage` seams instead of being claimed.

The window worker matches a unique synthetic title nonce and exact native child
PID, or the WSLg host `msrdc` plus the nonce (WSLg may append a distro suffix). It
never forces focus. `FOREGROUND_REQUIRED` means click the owned window and leave it
in the foreground during input. Only fixed keyboard chords and ASCII synthetic
fields are accepted; Ctrl+1 is excluded. Captures use only that owned window via
PrintWindow. Verify the PNGs visually; neither a probe nor a successful capture
API return alone proves rendered content. GUI shutdown is an owned WM_CLOSE.

The Windows worker maps every injected virtual key with `MapVirtualKeyW`
(`MAPVK_VK_TO_VSC`) and explicitly marks Right/Left/Home/End as E0 extended keys
on both press and release; the mapping API omits E0 for these navigation VKs.
Without E0, WSLg delivers Alt+Right as Alt+KP_Right rather than the shortcut's
Alt+Right. The review scenario exercises the actual Alt+arrow chords
on both surfaces and requires History → Detail → Queue → Detail state changes;
native Tab/arrow navigation is not a substitute for this shortcut regression.

WSLg resize uses the app's own title bar (if repositioning is needed to fit the
monitor work area) and bottom-right grip. Each drag checks window-at-pointer
ownership before pressing and nonce/process/foreground ownership throughout,
then releases the button and restores the pointer. A monitor too small for the
required viewport is refused. Windows-host-only `SetWindowPos` is not a Wayland
resize and is never a fallback. The driver requires the actual GTK `width`/`height`
acknowledgement before accepting the resized state.

Windows may remember a Chinese IME separately for each window. As explicitly
authorized for this acceptance task, before native Windows input the worker checks
the owned window thread's layout and requests an **already installed** English
layout using WM_INPUTLANGCHANGEREQUEST. It verifies the result and foreground again
before typing, emits content-free layout observations, and restores the original
layout on worker exit if that exact window still exists. It never installs/loads
a layout, activates another window or changes persistent input/system settings.
No installed English layout means refusal. This does not certify IME composition
input; Chinese UI rendering remains a separate assertion. WSLg host layout is not
changed by this native-Windows-only step.

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

S2 review checks wait for fresh, action-specific probes: exact inspected ID/revision
after an inspect, and `digest_expanded=true` after deliberate expansion. A previously
observed row revision cannot acknowledge either action. The scenario also confirms
a same-key peer demotion, checks actual decision revisions and `desktop`/`tui`
channel labels through independent CLI reads, and refuses history inspection after
deletion. A first decision chord without exact inspection performs only that read;
preparation and application remain separate explicit inputs.
For deletion, Desktop waits for native `on_open` with a mounted initial-focus
reference, not merely an armed model flag. Input waits require fresh retype-length
and readiness probes; no typed ID or proposal value is logged. Both shells cancel
the destructive tier with Escape, retain the workspace and reopen with fresh consent.
The locale pass re-reads retained content/history in zh-CN, then explicitly enables
the synthetic grant, reloads selection/inspection in its new epoch, and exercises
compact wrong-ID refusal without a preference write. TTY startup logs the actual
owned PTY/ConPTY child PID; GUI startup logs the owned PID and title nonce.

A normal run requires clean source before and after and records the Git SHA plus
actual driver/daemon/query/NSH SHA-256. `ACP_S5_DEVELOPMENT=1` explicitly labels
exploratory runs DEV_PASS and allows dirty source; those are never final acceptance.
Run all four surfaces again at the final code SHA and keep artifacts private.
