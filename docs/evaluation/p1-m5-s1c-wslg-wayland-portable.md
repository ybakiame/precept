# P1-M5/S1-C — WSLg + Weston Wayland Linux Desktop, PTY TUI and portable `.tar.gz`

2026-11-18. Scope: [Issue #72](https://github.com/ybakiame/agent-control-plane/issues/72),
[S0 contract](../p1/release-readiness-plan.md), parent
[M5 #67](https://github.com/ybakiame/agent-control-plane/issues/67). Follows
[S1-A](p1-m5-s1a-windows-portable.md) (Windows ZIP) and
[S1-B](p1-m5-s1b-windows-wsl-operator.md) (primary Windows GUI → WSL2 workflow).

**Results: a local-only Linux x86_64 portable `.tar.gz` was assembled and exercised; every check
passed on one disposable, clean Ubuntu 24.04 WSL2 user distribution with WSLg:**
- ELF and runtime dependency closure.
- Missing-dependency diagnostics.
- Packaged daemon/CLI/MCP/PTY/Wayland smoke (14 cells), with both the default Mesa
  software renderer and the opt-in WSL GPU (d3d12) driver.
- Archive policy, collision and removal checks (11 cells).

All of it ran **inside a disconnected network namespace**.

This revision also fixes a cross-platform UI defect found during the WSLg captures.
The inverted color-mix weight turned the hover/press wash of every Default button
near-white with an unreadable label. A real-pointer hover check now guards it, and
it was proven on WSLg and on Windows HWND. The README documents the optional WSLg
GPU switch.

This validates exactly that WSLg deployment target. It is **not** a standalone
Linux, other-distribution, compositor, release or public-distribution claim. No real
Profile was used.

## Owner decisions applied

| Topic | Decision | Implementation |
| --- | --- | --- |
| Distributions | **Builder + new clean target**: the existing disposable `acp-s1b-ubuntu` builds; a new disposable `acp-s1c-target` receives only the declared runtime packages | Daily `archlinux` distro and the WSLg system distro untouched |
| OS libraries | Never bundled; the archive declares them as user-distribution packages | Recipe refuses non-system interpreters and any RPATH/RUNPATH |
| Compositor | The Weston that WSLg provides; no second Weston, niri or X11/XWayland fallback | GUI launched with `GDK_BACKEND=wayland` and no `DISPLAY` |
| License | Apache-2.0, copyright `ybakiame` (owner-confirmed in S1-A) | `ACP/LICENSE.txt`, `ACP/notices/NOTICE.txt` |

## Source and environment

| Input / observation | Exact identity |
| --- | --- |
| Baseline main | `a237427` (PR #74 merge) |
| Clean source of archive, smoke harness and every target run | `dd0940405117be5784f0ad6b6cecd2e35ef9a244` (tree `9ddd235770a77fcf3e1c7726354f494af17ce879`); later commits: `7c6c95d` adds the same hover cell to the Windows smoke (its HWND run below), plus documentation |
| Harness executable SHA-256 | Linux `78b1b1da90f480b2745cbdff8be5251dce5a579bd04f785d2f72d92010849ddd`; Windows HWND (`7c6c95d`, debug) `5182b2febba67f38b45f5191e88be83d739dca95b0ffe0f55c0592d159d362f5` |
| Windows host | `10.0.26300.9818`, AMD GPU |
| WSL / kernel / WSLg | WSL `3.0.5.0`, kernel `6.18.58.1-1`, WSLg `1.0.79`, MSRDC `1.2.7391`, Direct3D `1.611.1`, DXCore `10.0.26100.1` |
| WSLg system distro (`/mnt/wslg/versions.txt`, read-only) | Azure Linux 3.0, built 2026-06-25; wslg `6942bb53`, weston `a7e35fda`, mesa `23.1.0`, FreeRDP `c4030980`, DirectX-Headers `v1.608.0` |
| Builder | `acp-s1b-ubuntu`: Ubuntu 24.04.5 LTS, glibc `2.39-0ubuntu8.12`, Node `v18.19.1`; dev headers `libgtk-4-dev`, `libgstreamer1.0-dev` and `libgstreamer-plugins-base1.0-dev` installed there only |
| Clean target | `acp-s1c-target`: fresh `wsl --install Ubuntu-24.04 --name acp-s1c-target --no-launch`, Ubuntu 24.04.5 LTS, 523 packages, non-root user `acp` (uid 1000, groups `acp` only), no Rust/Node/dev packages |
| Toolchain | Stock Cargo `1.99.1 (5f94df478 2026-08-27)`, rustc `1.99.1 (58cf8f9c3 2026-10-13)`, LLVM 23.1.1; target `x86_64-unknown-linux-gnu`, `--release --locked` |
| NSH source | `b7f31a08478be262fd7b76ec856036fd5b8ffdb0`, unchanged |
| Root / app lock SHA-256 | `b57d19e4…3b862bf6` / `ee768048…0317e7ee`, unchanged |

Actor: automated engineering checks, plus inspection of the owned synthetic window
captures and the PTY frame. The existing nonce/PID/foreground protections were active.
This is not independent human, IME or assistive-technology acceptance.

[Recipe](../../tools/portable/linux-package.mjs):
- Requires clean committed source, the verbatim Apache-2.0 `LICENSE` and the NSH pin.
- Builds both workspaces into a new owned target directory, with inherited Cargo
  overrides removed and `--remap-path-prefix` for checkout, Cargo home and build output.
- Copies only the six Cargo-reported product executables.
- Refuses any builder-path bytes in binaries or metadata.

Local attempts and generated package outputs are ignored under
`artifacts/local-p1-m5/s1c-attempt-*` in the builder, plus private run directories
in the target. Logs, the `.tar.gz`, inventory/checksum sidecars, captures, VT and
library maps stay local and unattached. Only the packaging recipe, tests and this
evidence report are committed; the generated archive is not a PR or release asset.

## Local archive and provenance

The archive below is a local test output, not a repository deliverable. It is
built for clean-target extraction and acceptance; the generated archive and sidecars
remain in ignored local artifacts. Formal release packaging, signing and publication
are deferred until an explicitly approved release stage.

`acp-linux-x86_64-prototype-dd0940405117.tar.gz`: **42,316,349 bytes, 269 entries,
265 files**. SHA-256
`20b3c1ea1bda719e8fa653d13ac3fc54cd62c2a6d09ccd5b2ef2bc1b339f3af0`. The external
inventory SHA-256 is `703f395d…b26c`; the in-archive `runtime/manifest.json` SHA-256
is `67e637ce…2bc0`.

```
ACP/
├── bin/          acp-daemon acp-query acp-mcp acp-nsh-native acp-nsh-tui acp-wsl-bridge (0755)
├── runtime/      manifest.json: provenance, ELF facts, declared packages, hashes of every other file
├── notices/      NOTICE.txt, dependency-notices.json, 254 deduplicated license texts
├── README.txt    dependencies, extraction, verification, private state, start/stop/remove
└── LICENSE.txt   verbatim Apache-2.0
```

The tar is written with sorted names, numeric owner 0:0 and the commit mtime. The
recipe then lists it under the same policy the target enforces and verifies a fresh
extraction against the inventory. `acp-wsl-bridge` is the existing S1-B helper binary,
not a duplicate implementation.

| Binary | Bytes | SHA-256 | DT_NEEDED | Highest GLIBC |
| --- | ---: | --- | ---: | --- |
| `acp-daemon` | 6,865,320 | `77948368…8ead` | 4 | 2.39 |
| `acp-query` | 2,376,648 | `3b4792c0…9aa7` | 4 | 2.34 |
| `acp-mcp` | 2,531,984 | `218dec77…ead9` | 4 | 2.34 |
| `acp-nsh-native` | 60,356,760 | `7ccd6e67…9375` | 19 | 2.39 |
| `acp-nsh-tui` | 56,346,696 | `7e4ab6ba…18c3` | 4 | 2.39 |
| `acp-wsl-bridge` | 2,950,512 | `fe632d5c…401a` | 4 | 2.34 |

The daemon, query, MCP and bridge binaries are byte-identical across three
independent clean builds (`13120f2`, `c218030`, `dd09404`). The two UI binaries
changed only with the color fix. This is an observation on one builder, not a
reproducible-build claim.

Every binary has the same ELF facts:
- ELF64 x86_64, interpreter `/lib64/ld-linux-x86-64.so.2`.
- **No RPATH/RUNPATH.**
- `GCC_4.2.0` is the highest libgcc version needed.
- No GLIBCXX/CXXABI needs.

The four-entry binaries need only `libc.so.6`, `libm.so.6`, `libgcc_s.so.1` and the
loader. **The TUI links no GUI library**, so no package split is needed.

## Runtime dependency closure

**Static:** a strict ELF reader (`tools/portable/elf.mjs`) reads the interpreter,
DT_NEEDED, RPATH/RUNPATH and version needs. It was cross-checked against `readelf`
on real binaries. The direct DT_NEEDED libraries map to their owning packages via
`ldd` → `dpkg -S` on the builder. The resulting declared set, which the README gives
as one `apt-get install` line, is:

`libc6 libcairo2 libfontconfig1 libgcc-s1 libglib2.0-0t64 libgraphene-1.0-0
libgstreamer-plugins-base1.0-0 libgstreamer1.0-0 libgtk-4-1 libharfbuzz0b
libpango-1.0-0 libpangocairo-1.0-0 libpangoft2-1.0-0`. The glibc floor is `GLIBC_2.39`.

**Runtime (execution tracing):** the smoke reads `/proc/<pid>/maps` of each owned
process and classifies every shared object:
- **user distribution**: `/usr/lib`, `/lib` and similar
- **WSL-provided**: `/usr/lib/wsl/`, the GPU/DXCore integration
- **refused**: anything under the extracted `ACP/` tree or elsewhere

| Process | User distribution | WSL-provided | From archive |
| --- | ---: | --- | --- |
| daemon | 4 | none | none |
| PTY TUI | 4 (no GTK/GStreamer/Pango/Cairo/Wayland) | none | none |
| Wayland Desktop, default | 104 | none | none |
| Wayland Desktop, `GALLIUM_DRIVER=d3d12` | 106 | `libd3d12`, `libd3d12core`, `libdxcore`, `amdxc64` (host AMD user-mode driver) | none |

Not visible to DT_NEEDED, but observed in the Desktop: Mesa `libgallium-25.2.8` and
`libLLVM.so.20.1`, `libEGL_mesa`, `libdconfsettings` (GIO module), the Vulkan loader
probing `libvulkan_asahi` and `libvulkan_gfxstream`, and `libxcb-dri3`. Ubuntu's GTK 4
is built with both backends, so 12 X11 client libraries are mapped **as linkage**. No
X11 connection is possible: the owned GUI's `/proc/<pid>/environ` has no `DISPLAY`
and has `GDK_BACKEND=wayland`, and GTK reports `GdkWaylandDisplay`.

The clean target already had these from its base image, and they are recorded as
**observed, not declared**:
- Wayland, Mesa and graphics: `libwayland-client0 1.22.0`, `libegl-mesa0` and
  `libgl1-mesa-dri 25.2.8`, `libvulkan1 1.3.275`.
- Themes, fonts and settings: `adwaita-icon-theme 46.0`, `fonts-dejavu-core 2.37`,
  `fontconfig-config 2.15.0`, `libdconf1 0.40.0`.

**GPU finding:**
- **Default: software rendering.** EGL on WSLg's Weston gets no DRM device from the
  compositor ("failed to get driver name for fd -1"), Zink finds no device, and Mesa
  falls back to its software renderer with no WSL GPU library mapped.
- **Not a permission issue.** A labeled diagnostic temporarily added `acp` to `render`,
  then reverted it; rendering stayed software.
- **Opt-in GPU works.** With `GALLIUM_DRIVER=d3d12` set by the user, Mesa uses WSL's
  d3d12 driver and GTK selects `GskNglRenderer`.
- **Archive stays neutral.** The archive sets no environment variable. On the
  owner's decision, the README now documents this optional, per-process switch and
  how to fall back to software rendering.

**Install delta** (`apt-get install --no-install-recommends` on the README line, the
strictest test of sufficiency):
- 7 new packages: `libgtk-4-1`/`libgtk-4-common 4.14.5+ds-0ubuntu0.10`,
  `libgraphene-1.0-0 1.10.8`, `libgstreamer-plugins-base1.0-0 1.24.2-1ubuntu0.7`,
  `liborc-0.4-0t64 0.4.38`, `libgles2 1.7.0`, `libcairo-script-interpreter2 1.18.0`.
- 7 in-place upgrades: naming `libc6` and `libglib2.0-0t64` explicitly upgraded
  glibc `8.8 → 8.12` and GLib `3.8 → 3.10`. The pre-install binaries already ran on
  glibc `8.8`, since the floor is `2.39`.

## Clean-target acceptance (`acp-s1c-target`)

Order of the evidence run, as the normal user `acp`, following the shipped README:
1. Root first purges the earlier attempts' 7 packages, returning the target to its
   original 523-package set (names verified identical; only the 7 upgrades remain).
2. Copy into a new `0700` directory. Check the archive against the SHA-256 recorded
   outside it. Extract with `tar --keep-old-files` into a new `0700` directory (modes
   `755` for `ACP` and executables, `644` for files). Run the README's own per-file
   verification. PASS.
3. **Missing dependencies (before install):** the daemon, query, MCP, TUI and bridge
   run (`--help`, exit 0). The Desktop exits **127** and the loader names
   `libgstaudio-1.0.so.0`. PASS.
4. Root installs the README line with `--no-install-recommends`, in the target only.
5. Root starts `unshare -n` and drops to uid 1000 (`setpriv`); the namespace has only
   `lo`, no route, DNS or TCP. **Offline** runs:
   - smoke with the default renderer, 5.4 s
   - smoke with `GALLIUM_DRIVER=d3d12`, 5.1 s
   - the archive cells

**Packaged smoke: 14/14 PASS in both renderer runs** (identical cells):

| Cell | What it proves |
| --- | --- |
| no-implicit-profile | fresh daemon: nothing selected, writes disabled |
| unsafe-permissions-and-file-runtime-refused-without-owner-disruption | `0755` runtime dir and file-as-runtime refused; owner keeps serving |
| endpoint-collision-preserves-owner | second daemon on the same Unix endpoint exits non-zero |
| explicit-synthetic-profile-grant-register-no-overwrite | explicit create/enable/register; repeat create refused |
| project-credential-cannot-write | agent credential gets `Unauthorized` for writes |
| mcp-refuses-operator-credential | MCP refuses an operator capability |
| packaged-mcp-exact-two-read-tools-context-missing-handoff-write-refusal | exactly two read-only tools; unknown write tool refused |
| write-grant-explicit-revocation | explicit disable |
| packaged-pty-render-resize-normal-restore | PTY render, resize, Ctrl+Q, alt-screen and bracketed-paste restore |
| packaged-wayland-render-resize-navigate-hover-normal-close | Wayland render, resize to 920×640, Alt+3 input → Preferences, real-pointer hover on "Sessions" keeps a readable wash, normal close |
| libraries-from-user-distribution-only | `/proc/<pid>/maps` provenance for the daemon, TUI and Desktop, as classified above |
| ui-close-leaves-daemon-running | daemon healthy after both UIs close |
| graceful-stop-cleans-owned-credentials-releases-runtime-lease | SIGINT to the owned daemon; credentials removed; lease reacquirable |
| normal-and-crash-restart-no-adoption-stale-credential-refused | restart after stop and after SIGKILL; nothing adopted; old credentials refused; Profile bytes unchanged |

Both UIs used the same WSL-local daemon over the authenticated Unix socket, not
the Windows bridge. No separate TCP-listener inspection ran here; the namespace had
no external network.

**Archive cells: 11/11 PASS** (`acp-linux-portable-smoke archive`, offline):
- **Real archive:** passes the listing policy (269 entries).
- **Hostile archives**, built with GNU tar in the target, are refused before
  extraction:
  - traversal (`ACP/../x`): unsafe path
  - absolute path: unsafe path
  - symlink: link entry
  - setuid: unexpected mode
  - outside `ACP/`: unsafe path
- **Fresh extraction** matches the 265-file external inventory.
- **Collision:** re-extraction with `--keep-old-files` fails and a locally edited
  file keeps its content.
- **Removal:**
  - Changed product file: refused before any unlink, all files present.
  - Running product (owned daemon): refused before any unlink. Linux would otherwise
    let a running executable be deleted.
  - After stop: exactly 265 files removed. `ACP/bin` and `ACP` are retained because of
    an unknown file. The synthetic Profile and auth files are unchanged.

**Wayland facts:**
- Display: `WAYLAND_DISPLAY=wayland-0`, `XDG_RUNTIME_DIR=/mnt/wslg/runtime-dir`,
  `WSL_DISTRO_NAME=acp-s1c-target`, `GdkWaylandDisplay`.
- GTK renderer: `GskGLRenderer` on Mesa software by default; `GskNglRenderer` on
  Mesa d3d12 when opted in.
- The captures (920×640) show the synthetic Preferences page. Fonts, icons and the
  client-side frame render correctly inside the WSLg RAIL window.

**Hover defect found and fixed:**
- **Symptom.** Earlier d3d12 captures showed a light "Sessions" fill (`216,227,248`)
  with an unreadable label. The owner identified it as pointer hover, also seen on
  other buttons.
- **Cause.** NSH's `Hsla::mix(other, factor)` keeps `factor` of `self`. Our
  `tokens::mix(a, b, t)` passed `t` through. The Default button's "7% hover / 13%
  press wash" was therefore 93% / 87% text color. A selected operator row was 90%
  accent instead of a 10% tint.
- **Fix.** `tokens::mix` now inverts the weight once (`64f2ea2`). A unit regression
  test covers it and was mutation-checked.
- **Guard.** The window worker gained a guarded `hover` action:
  - the window must be owned, in the foreground and not occluded
  - the original pointer position is restored on `unhover`, on close and on worker
    exit

  Captures can also sample one pixel. The Linux and Windows packaged smokes hover
  the sidebar "Sessions" button and assert the wash stays near the dark rest fill.

| Package | Surface | Rest | Hover | Cell |
| --- | --- | --- | --- | --- |
| `c218030` (pre-fix) | WSLg | `44,59,86` | `216,227,248` | **FAIL** (as intended) |
| `729eb51` (S1-A, pre-fix) | Windows HWND | `44,59,86` | `216,227,248` | **FAIL** (as intended) |
| `dd09404` | WSLg software / d3d12 | `44,59,86` | `50,70,105` | PASS / PASS |
| `7c6c95d` Windows ZIP | Windows HWND (developer host) | `44,59,86` | `50,70,105` | PASS, smoke 14/14 |

The fixed hover capture shows a subtle lift with a clearly readable label.

**Performance observations (no gate):**

| Renderer | First `phase=Ready` | GTK mapped | First painted frame |
| --- | ---: | ---: | ---: |
| default (software) | 0.34 s | 0.63 s | 0.70 s |
| d3d12 | 0.06 s | 0.20 s | 0.38 s |

Typing latency, long sessions and multi-monitor/DPI behavior on WSLg were NOT_RUN.

## Gate dispositions

| Selected gate | Result / practical limit |
| --- | --- |
| G01 | **PASS**: one clean SHA, locked builds of all six binaries, sanitized inputs, builder-path-free binaries and metadata, ELF facts and hashes recorded. Signing and release claims NOT_RUN |
| G02 | **PASS (WSLg target only)**: fresh Ubuntu 24.04 WSL2 distribution with no toolchain; extract/verify/start/connect/stop/remove offline. **Standalone Linux archive launch NOT_RUN** and still required for that claim |
| G03 | **PASS**: owner/mode checks on product files, private `0700` runtime, unsafe-permission and file-runtime refusal, credential-role separation. An actual second-user attack was NOT_RUN |
| G07 | **PASS**: endpoint collision, SIGINT stop with credential cleanup and lease release, normal and crash restart without adoption, stale credential refusal, retained state |
| G08 | **PASS (WSLg target)**: static ELF closure, declared packages, missing-library diagnosis, post-install run with classified `/proc` library provenance covering dynamically loaded GIO/EGL/Vulkan/d3d12 modules, GUI and TUI measured separately, disconnected run. Media/audio playback was not exercised, because the UI does not play media. Font/GPU breadth beyond this target NOT_RUN |
| G09 | **PASS**: exact inventory removal; changed and occupied products refused before unlink; unknown/Profile/auth retained; collision refused |
| G11 | **PARTIAL — BLOCKED for public distribution.** PASS: source/toolchain/lock/binary/archive provenance, verbatim Apache-2.0 `LICENSE` and `NOTICE`. Open: in the Linux closure, **18 third-party entries lack crate-local notice text** and are pending review (21 in the Windows closure), and signing is undecided |
| G12 | **WSLg PASS, standalone NOT_RUN**: actual `GdkWaylandDisplay` mapping, render, resize, keyboard and real-pointer hover input, and close through WSLg's Weston, with both software (default) and d3d12 renderers and owned captures. The standalone native Wayland target remains NOT_RUN, and WSLg does not substitute for it |
| G17 | **PASS (automated) / human NOT_RUN**: PTY render, resize, consumed Ctrl+Q and terminal restore (alt-screen and bracketed-paste exit sequences). Error/cancel streams and human inspection beyond the captured frame were NOT_RUN |

## Validation commands

| Check | Platform | Result |
| --- | --- | --- |
| `node --test tools/portable/{elf,linux-package,windows-package}.test.mjs` | Linux/WSL | **17 PASS** (ELF facts match `readelf`) |
| Same | Windows | **16 PASS, 1 skipped** (the `readelf` comparison is Linux-only) |
| Harness `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` | Linux/WSL (`7c6c95d`); Windows (`ca53bd9`, plus `windows-smoke.ps1`'s own fmt/Clippy/test of `acp-portable-smoke` at `7c6c95d`) | PASS; 36 tests |
| `native_window.ps1` PowerShell AST parse | Windows | PASS, 0 errors |
| Operator UI (`acp-operator-ui`, `native,tui`): Clippy `-D warnings`, tests | Linux/WSL | PASS; **28 + 1 + 1** (includes the new `mix` regression test; reverting the fix makes it fail) |
| `linux-package.mjs` attempt-09 | builder | PASS: extraction inventory, listing policy |
| `missing-deps`, smoke-default, smoke-d3d12, archive-01 | clean target | PASS, 14/14, 14/14, 11/11 |
| `windows-package.ps1` + `windows-smoke.ps1` (`s1c-hover-zip-01`, smoke-04) | Windows developer host | PASS, 14/14 including HWND hover |
| Root workspace; operator model | — | Not rerun: their sources are unchanged since PR #74 (`a237427`), where they passed; daemon/query/MCP/bridge binaries are byte-identical |

Not run: standalone Linux host/VM, other distributions or compositors, the full
148-cell UI matrix, second user, IME/AT, hosted Actions, models.

## History (retained locally, not counted as passes)

- **attempt-01–03**: recipe refusals:
  - The loader in DT_NEEDED was not mapped from `ldd`'s bare line (fixed in two
    commits).
  - A transient `cargo metadata` crate download failed.
- **attempt-04** (`7ae7fdf`): first archive; exploratory.
- **attempt-05** (`dcb542a`): first target extraction under `umask 077`. Modes
  `0700`/`0600` are safe, but the smoke's exact-`0755` check was too strict; it was
  relaxed to owner + user-execute + no group/other write.
- **attempt-06** (`110f98f`): target runs found three harness defects, each fixed:
  - The report directory wasn't created `0700`.
  - A wrong "no libX11 mapped" oracle; Ubuntu GTK links X11. It was replaced by
    environment plus backend proof.
  - The symlink refusal reason was mislabeled.

  That run's smoke-04 passed 14/14 and its archive-01 passed 11/11 online.
- **attempt-07** (`13120f2`): smoke 14/14 and archive 11/11 offline. A draft of this
  report then claimed hardware GL. Checking the mapped libraries showed Mesa software
  rendering, and showed that the harness counted `/usr/lib/wsl` as user distribution.
  Both were fixed in `c218030`.
- **attempt-08** (`c218030`): smoke 14/14 with both renderers and archive 11/11,
  offline. It showed the intermittent light button fill that turned out to be the
  hover defect.
- **attempt-09** (`dd09404`): this report, after the `mix` fix and the hover cell.

## Required follow-up, not release authorization

- Review the third-party entries without notice text (18 Linux, 21 Windows), then
  decide on signing.
- **Standalone Linux** (host or VM, native compositor) remains required before any
  Linux support claim. It also needs a decision on whether to declare graphics/font
  packages that the WSL image happened to provide.
- Human, IME and AT acceptance on WSLg, if WSLg is ever offered beyond secondary use.
- Parent M5 stays open for later approved readiness gates. This PR reviews the local packaging implementation and WSLg acceptance evidence; it is not a release or distribution authorization.
