# S3 owned native acceptance driver

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

`ACP_S5_APPEARANCE=dark` selects the retained S2 baseline flow's initial appearance;
the default is `light`. Other values are refused. The S3 presentation cells also
select **both** Light and Dark through actual focused settings buttons in each
locale, capturing compact and large layouts. Screenshot inspection remains an
independent requirement; a state probe is not pixel-content verification.

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

The default run retains the S2 create/grant/Busy, exact review/delete, saved
Handoff, keyboard, locale and restoration flow, then runs independent fresh S3
fixtures. A failed fixture is recorded and reaped without suppressing later
independent cells. Stored NOT_RUN Handoff history is not fresh Git evidence.

| S3 cells | Actual input and independent observation |
| --- | --- |
| Empty; project/global pending pages; stale-page disclosure | Observed native Tab focus then Enter on scoped queue/paging controls; CLI checks exact scope, ascending IDs, 12-row cap and commit watermark; a separate CLI commit between pages induces the rendered disclosure |
| Exact row/provenance, Confirm/Reject/Deprecate/Delete, peer demotion, bounded history | Retained real Alt/Tab/Enter/Space flow; exact daemon inspect and decision/receipt checks; valid lifecycle history is at most two rows, never fabricated to reach eight |
| Wrong ID, blind Enter, Escape/reopen, exact ID, deletion privacy | Native destructive modal and native initial-focus/retype probes; unchanged transaction sequence for refusals; deleted inspect is NotFound and observed delete receipt is identity-only |
| Selection, scope, project, live source, draft source, target | Real selection/field/navigation/settings input invalidates inspected or prepared authority; independent CLI checks no transaction. Scope controls are concealed while prepared, so scope switching exercises a current exact inspection, not a fictional reachable prepared control |
| Epoch, revocation, status/CAS | Independent CLI changes grant epoch, revokes between inspect/apply or rejects the prepared target; actual UI apply refuses and CLI proves no unintended transaction |
| Generation binding | Contained relay changes only the generation in an actual UI mutation; the real daemon refuses Unauthorized before writer admission. This is process-level fault injection, not typing or an in-place generation change |
| Delayed read after navigation | Relay holds a real inspect reply until actual project navigation, then releases it within the existing read deadline; UI discards the old inspected identity |
| OutcomeUnknown, no automatic replay, exact replay, changed bytes, stale epoch | Relay consumes a real committed Confirm reply. A bounded 500 ms observation records exactly one submission/decision. Actual explicit retry hashes immutable params/operation ID, compares the original receipt and unchanged decision history; a contained changed-channel request returns real IdempotencyConflict; an independently advanced epoch refuses old retry |
| EN/zh-CN × Light/Dark × compact/large | Real settings/language input and owned captures for every combination; no headless or callback substitute |

`ledger.json` declares every required cell as PASS/FAIL/BLOCKED/NOT_RUN and
references the single append-only `events.jsonl` stream by zero-based event range.
Events contain monotonic elapsed milliseconds and UTC Unix milliseconds, actual
chords, redacted text lengths, observed GUI probes **or** terminal frame text,
CLI response hashes, capture-relative filenames and owned UI PID/nonce. They do
not record tokens, private paths or field values. Cell `owned_ui_event` references
the corresponding PID/nonce event. Full ledgers are checkpointed at cell
begin/end/final only; event appends are flushed immediately and bounded to 32,768
events, 2,048 per cell, 4 KiB per event and 8 MiB total.

All cells start NOT_RUN; a started but unfinished cell remains FAIL. Missing
observable native focus is BLOCKED, never a guessed platform Tab count or an
automatic business-action retry. Overall success requires every declared cell
PASS, normal clean-source before/after checks and successful retained baseline.
`report.json` retains binary hashes and the per-cell summary. A failure preserves
the partial ledger, event stream, previous captures, last terminal/probe output
and a best-effort last owned frame before cleanup. Independent fixtures live in
`s3-CELL/`; no failed exploratory artifact is overwritten.

The driver creates a new private regular `state/focus.events` file inside each
owned disposable root and passes `ACP_NATIVE_FOCUS_FILE` only to that direct UI
child. The optional diagnostic sink is cached once by the UI, opens existing
regular non-symlink/non-reparse files append-only, flushes fixed catalog-key
focus events and refuses growth beyond 256 KiB. It creates no file, control,
focus stop, product state or authority. Existing Windows scoped Enter/Space
focus references are preserved. There is no normal-run file work, extra process,
stderr/ConPTY redirection or detached observer thread.

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

The fixed-flow VT observer handles ConPTY's erase-character (`CSI X`) before its
cursor-jump spaces, and retires both halves of overwritten/erased wide glyphs.
Otherwise old draft text can falsely obscure the actually rendered status.

Owned captures use a two-second composition/nonuniform-frame readiness budget;
the existing capture RPC deadline still bounds OS calls. Every attempt rechecks
foreground/nonce/PID/bounds, clears the private bitmap, and uses only PrintWindow.
Uniform or alpha-only blank frames are refused; no keyboard or mutation action
is resent and there is no desktop-capture fallback. Color variation is not content
proof: the PNGs still require visual inspection.

A normal run requires clean source before and after and records the Git SHA plus
actual driver/daemon/query/NSH SHA-256. `ACP_S5_DEVELOPMENT=1` explicitly labels
exploratory runs DEV_PASS and allows dirty source; those are never final acceptance.
Run all four surfaces again at the final code SHA and keep artifacts private.
