# Native Wayland diagnosis and local acceptance

E3 treats **direct WSLg Weston**, **niri**, and **X11/XWayland** as distinct cases.
An environment variable is not backend proof. Neither Xvfb, an X11-only test nor
a Windows-native NSH window can pass Linux Wayland acceptance.

## What was wrong and what changed

The previous local probe forced `GDK_BACKEND=x11`, searched windows with xdotool
and captured the X11 root through MSS. That path never tested Wayland. The actual
pinned NSH GTK application already renders with automatic GTK selection and
explicit child-only `GDK_BACKEND=wayland` in the tested WSLg session. No upstream
NSH version or framework lockfile change was necessary.

The new probe observes the **actual** GTK `GdkWaylandDisplay`, GSK renderer type,
mapped window/allocation, model/layout timing and GTK after-paint callbacks.
A temporary bounded protocol trace additionally observed Wayland surface commits,
`wp_presentation` feedback and `xdg_toplevel.configure` during real resize. GTK
paint/model markers alone do not prove compositor presentation or visible text;
owned synthetic-window OS captures and keyboard assertions supply separate checks.

WSLg RDP/RAIL windows append a distro suffix to their OS title and include GTK
transparent CSD shadows. Detection now uses a unique validated nonce and accepts
that suffix without logging it. Captures exclude titlebar/shadows/corners and
include only the opaque synthetic content interior, while requiring the owned
window to be foreground. No desktop-wide or live-project capture is allowed.

`SetWindowPos` on the RAIL proxy only clipped the existing buffer in this session;
it did not cause GTK resize. The probe now performs a real pointer border drag,
then requires fresh **exact GTK allocations** of 640×480 and 1080×760. Ordinary
application resize behavior is unchanged. Window input is real Win32 OS input
forwarded through RAIL/Wayland to GTK, not direct model mutation.

The previous E2 X11 `BadMatch` capture failure remains a historical diagnostic,
not a proven NSH widget crash or universal absence of WSL GUI support. A fresh
synthetic X11 capture diagnostic succeeded on this session and did **not** reproduce
that error. It is explicitly excluded from E3 acceptance. No X11 renderer repair
or universal WSLg driver fix is claimed.

GTK theme-parser warnings also occur in the minimal GTK control. A **temporary
child-only** `XDG_CONFIG_HOME` removes those warnings, classifying them as inherited
user GTK configuration rather than an NSH-only failure. The app/probe does not
replace that configuration, suppress its warnings globally, or change user themes.

## Doctor and reproduction

Run from the WSL owner session with existing GTK4/native prerequisites and cached
locked Rust dependencies. These commands neither install libraries nor configure
a compositor. Diagnostics belong outside real project checkouts:

```sh
PYTHONPATH=src python tools/wslg_doctor.py --probe \
  --out artifacts/local-p1-e3/doctor
cargo build --manifest-path spikes/p1-e/nsh/Cargo.toml \
  --release --features native,tui --bins --offline --locked
# Normal launch: GTK chooses its backend; no forced X11.
spikes/p1-e/nsh/target/release/acp-nsh-native --synthetic-rows 10000
# Diagnostic choice applies to this child only.
GDK_BACKEND=wayland ACP_UI_PROBE=1 \
  spikes/p1-e/nsh/target/release/acp-nsh-native --synthetic-rows 10000
```

The doctor reports socket reachability/type/owner/mode and fixed classifications,
not private runtime paths, DBus addresses, usernames or device IDs. WSLg's socket
is mounted from its system distro; its server PID may be invisible in the client
PID namespace. Report that limitation instead of interpreting it as a dead server.
The report records WSLg build revisions, GTK/Mesa/NSH versions and linked library
names. The ctypes GTK positive control has no NSH dependency and verifies its
actual display/renderer/mapped state in both automatic and explicit Wayland modes.

To compare Windows `wsl.exe --exec`, pass both explicit owner settings:

```sh
python tools/wslg_doctor.py --wsl-distribution '<distribution>' \
  --wsl-user '<owner>' --out artifacts/local-p1-e3/environment-comparison
```

Run the doctor **inside** a real interactive or niri-launched terminal with
`--context interactive-terminal` / `--context niri-terminal` for those comparisons.
Codex subprocess and Windows exec were compared locally; an interactive-terminal
capture was not fabricated. The owner chose not to start niri for this run after
confirming the direct Wayland window rendered. Thus **niri is NOT_RUN**; installed
niri alone is not an active/verified niri session. The application keeps GTK's
normal backend choice. The automated RAIL input/capture adapter certifies direct
WSLg only; an arbitrary/niri compositor is BLOCKED pending scoped native input/
capture tooling. Do not launch a compositor inside another session to fake coverage.

## Full local gate and measurements

The WSLg OS adapter needs explicit `ACP_WINDOWS_PYTHON` (absolute WSL-mounted path
to the installed native Python) and `ACP_WINDOWS_TEST_ROOT` (owned ACP test checkout
containing `.local-acp-test-root` and existing test-only MSS dependencies under
`artifacts/local-p1-e2/python`). No Windows path or WSL owner is inferred from a
parent workspace. Set `ACP_WSL_DISTRIBUTION` / `ACP_WSL_USER` explicitly to also
exercise the existing Windows-owner helper. No token crosses that boundary.

```sh
PYTHONPATH=src:/absolute/test-only/python-packages \
  python tools/local_verify.py --ui-full --wslg \
  --report-dir artifacts/local-p1-e3/final-wsl
```

`--wslg` selects UI/native Wayland gates plus the doctor; `--ui-full` also selects
core/service/parity/recovery and synthetic local Codex/Windows-owner integration.
The PowerShell driver accepts `-WSLg` / `-UIFull`; run Linux Wayland gates **inside
WSL**, not as a fabricated Windows pass. Reports contain PASS/FAIL/BLOCKED/NOT_RUN,
and an E3 host acceptance field. Missing/broken Wayland or actual X11 fallback
cannot become PASS. A Linux development archive additionally requires an explicit
passing Wayland report whose binary SHA-256 matches the exact packaged executable.
Windows host evidence is a separate local report.

Visible OS-window polling is an upper bound including the native Python helper
startup. Model ready/layout and GTK after-paint times come from the Linux process's
monotonic clock. The 10k filter records model acknowledgement and later actual
capture completion separately; neither is presented as an exact compositor frame
latency. Terminal timing waits for the expected text on the painted PTY/ConPTY screen.

## Renderer fallback and boundaries

The tested default reports `GskGLRenderer`; Mesa logs show software fallback, so
**hardware acceleration is not verified**. As a deliberate application-only
software diagnostic, the following reports `GskCairoRenderer` on native Wayland:

```sh
GDK_BACKEND=wayland GSK_RENDERER=cairo ACP_UI_PROBE=1 \
  spikes/p1-e/nsh/target/release/acp-nsh-native --synthetic-rows 1
```

This is opt-in and is not a default/global workaround or GPU-acceleration claim.
A failed Wayland socket must fail explicit Wayland startup; automatic GTK fallback
to X11 is detected and rejected by the acceptance adapter. Render/no-frame failures
fail the bounded deadline. No system packages, drivers, `.wslconfig`, session files,
sockets or user databases are changed. No `sudo`, compositor restart, `wsl --update`
or `wsl --shutdown` is required or executed.

Primary references: [GTK runtime/backend/renderer controls](https://docs.gtk.org/gtk4/running.html),
[WSLg architecture](https://github.com/microsoft/wslg),
[WSLg environment and mounts](https://github.com/microsoft/wslg/blob/main/samples/container/Containers.md),
[niri documentation](https://niri-wm.github.io/niri/),
[pinned NSH Linux backend](https://github.com/longbridge/nsh/tree/87c7e8da85d2f15a56107baa6f0fabb419b508b3/crates/nsh-platform-linux).
