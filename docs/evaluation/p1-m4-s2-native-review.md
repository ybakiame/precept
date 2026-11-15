# P1-M4/S2 — shared native review repair and local acceptance

Scope: [Issue #57](https://github.com/ybakiame/agent-control-plane/issues/57),
parent [#52](https://github.com/ybakiame/agent-control-plane/issues/52).
Baseline: merged S1 `d66d7c6f1dd60cf98238d5122111ed374efc4343`.
Engineering validation only: owned synthetic fresh Profile v1, no real profile,
model/provider call, production activation, migration or release.

## Repairs and authority

- Require a current exact inspect before preparing confirm/reject/deprecate/delete.
  A first decision chord with missing/stale inspection performs only a read;
  preparation and explicit apply remain separate inputs.
- Fence observed profile/grant/selection/source/scope/task/connection/watermark
  drift, late completions and retained uncertain requests. Delete rechecks exact
  ID retype, revision, digest and current consent at apply.
- Match S1 preference receipt identity: preference IDs are globally allocated and
  their `RecordVersion.project` is empty, including project-scoped deletions.
  Keep exact ID/resulting-revision/deleted checks; do not change Writer or DTOs.
- Recheck pinned canonical project roots inside the existing daemon mutation
  admission callback. Real-process regressions cover all four decisions after
  root loss, unchanged logical state/receipts on refusal, restoration and a valid
  explicit request. No new role, endpoint, writer operation or schema change.
- Existing client DTOs contain no root identity (`project.list` is name-only).
  The client fences observable source drift; the daemon independently refuses
  invalid roots at admission. This is not a filesystem watcher or a claim against
  hostile same-user filesystem races.
- Remove the obsolete duplicate list inspector and receipt presentation; Detail
  owns exact content and decisions. Queue empty/unobserved/drift states differ.
  Preserve full immutable source fields, explicit digest expansion, localized
  source/channel/receipt/contract caveats and truthful peer-demotion counts.
- Native driver waits for fresh action-specific inspect/digest probes, not a
  previously observed row revision. Its dedicated same-key scenario checks
  actual confirm/deprecate/demote revisions and shell channel through CLI reads,
  wrong digest refusal, compact wrong-ID/blind-Enter refusal and deleted history.
- WSLg arrow injection carries physical scan codes and explicit E0 navigation
  flags. Actual Wayland resize uses guarded owned title-bar/grip drags and waits
  for GTK dimensions, rather than accepting a resized RDP-host HWND alone.
- Keep the semantic native modal outside the scrolling document, retain it across
  retype changes, and request input focus from native `on_open`. Escape clears only
  the destructive tier and its exact-ID consent while retaining the workspace.
  Desktop acceptance now requires that lifecycle event plus fresh retype/readiness
  observations instead of treating `delete_armed=true` as visible-dialog evidence.

## Intermediate observations (not final acceptance)

At clean code `9ec35306d7ac90c2bd9080d8c854ab30434fb9a7`, both complete native
Cargo gate helpers passed:

| Check | Linux/WSL | Native Windows |
| --- | --- | --- |
| Root workspace fmt/build/Clippy/tests | PASS, 128 tests | PASS, 129 tests |
| Shared model fmt/Clippy/tests | PASS, 53 tests | PASS, 53 tests |
| NSH fmt/build/Clippy/tests | PASS, 24 library + 2 real-daemon tests | PASS, 24 library + 2 real-daemon tests |
| Driver fmt/Clippy/tests/build | PASS, 27 tests | PASS, 27 tests |

The one ignored root test is the existing opt-in synthetic performance sample,
not a skipped security regression. Raw gate evidence: ignored
`artifacts/local-p1-m4/s2-linux-9ec3530/`, `s2-windows-9ec3530/` and
`s2-driver-windows-9ec3530/` on their respective hosts.

Commands:

```text
CARGO_NET_OFFLINE=true tools/local_verify.sh all artifacts/local-p1-m4/s2-linux-9ec3530 d66d7c6f1dd60cf98238d5122111ed374efc4343
tools/windows_local_verify.ps1 -Suite All -ReportDir artifacts/local-p1-m4/s2-windows-9ec3530 -DiffBase d66d7c6f1dd60cf98238d5122111ed374efc4343
cargo fmt --manifest-path tools/native-acceptance/Cargo.toml -- --check
cargo clippy --manifest-path tools/native-acceptance/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path tools/native-acceptance/Cargo.toml --locked
cargo build --manifest-path tools/native-acceptance/Cargo.toml --locked
```

Windows uses the real Git checkout, installed global Git, existing MSVC Build
Tools and pinned standard Cargo/rustc/rustdoc 1.99.1, selected only in the child
process. Its pre-existing helper modification was identical to the committed arrow
fix and was preserved in a named stash before fast-forwarding; it was not dropped.
Initial Windows build attempts exposed mixed 1.98/1.99 generated artifacts.
Explicit compiler paths plus process-local `RUSTFLAGS=-Cdebuginfo=0` gave fresh
fingerprints without deleting caches or local data. No global configuration or
tool installation was changed.

Observed hosts: Linux `6.18.58.1-microsoft-standard-WSL2` x86_64 and native Windows
`10.0.26300.0` x86_64. Actual tested Rust/Cargo: 1.99.1.

Subsequent native exploration correctly found failures despite passing headless
checks; these attempts are retained, not relabelled:

- `s2-pty-9ec3530`: FAIL; duplicate inspection content pushed the digest below the
  160×50 viewport. Removing the obsolete inspector and exercising the expanded
  review viewport corrected that visible path.
- `s2-pty-layout1` / `s2-pty-layout3`: DEV_PASS for the actual PTY lifecycle,
  including visible 80×30 destructive dialog and wrong-ID refusal; dirty-source
  observations, not final clean-SHA acceptance.
- `s2-wslg-layout1`: FAIL; real GTK resize reached only 1080×898 because the drag
  destination hit the monitor edge. Guarded title-bar repositioning then produced
  actual 1080×1000 GTK configure observations.
- `s2-wslg-layout2`: FAIL; fixed Tab counts did not activate exact reinspection.
  The existing decision chord's read-only first step replaced that assumption.
- `s2-wslg-layout3`: FAIL; actual GTK resized to 640×480, but the destructive
  dialog was absent from owned-window pixels despite `delete_armed=true`.
  Headless state and a successful capture API were therefore not treated as
  native dialog/focus proof.
- `s2-wslg-layout7`: FAIL; native modal lifecycle/layout was still absent while
  the modal lived inside the scrolled document.
- `s2-wslg-layout8`: FAIL at the assumed single-Tab Cancel activation. Moving the
  modal outside the scroll produced native open/focus events, a 640×480 modal root
  and 320×234 panel. The owned-window PNG visibly shows the focused `999` field,
  mismatch warning, impact disclosure and disabled Delete. Cancellation was not
  marked PASS; subsequent driver steps use the specified semantic Escape action
  rather than platform-dependent Tab-stop counts.
- `s2-wslg-layout9` / `s2-pty-layout4`: DEV_PASS for the complete actual-input
  scenario after the modal ownership and Escape fixes. WSLg PNGs visibly show
  focused exact-ID arming and compact mismatch refusal; PTY captures show the
  same impact text and refusal. The WSLg History capture still placed its records
  below the viewport, so this was not accepted as final human-visible history
  evidence. Explicit navigation/new-result reveal scrolling addresses that path
  without moving input focus or reacting to retype edits/cancellation.
- `s2-wslg-layout10`: FAIL after the Chinese history/detail captures; explicit
  grant enable correctly cleared the previous selection/inspect at the new epoch.
  The driver had incorrectly expected them to survive. It now reloads the list,
  reselects and obtains a fresh exact inspect instead of restoring stale consent.
  Its English and Chinese History PNGs visibly show real decision rows.
- `s2-wslg-layout11` / `s2-pty-layout5`: DEV_PASS, including English/zh-CN exact
  content/history, compact localized destructive disclosure, wrong-ID refusal,
  unchanged revision/status/commit sequence, tier-only Escape and clean child exit.
  Native PNGs and parsed PTY frames were visually inspected. These remain dirty
  exploratory evidence, not the final clean-SHA gate.
- Clean `9f6defcf7b2dd837cb6a3751f1238b9e4e23d70a` full gates: FAIL on both
  hosts at the real review-component test's History pointer activation in the
  1080×760 viewport. All preceding fmt/build/Clippy/root/shared gates, 27 UI unit
  tests and the original UI IPC test passed. Scrolling to the document's end had
  placed the tab under the fixed header. The fix defers a measured tab-header or
  preview-start anchor to the next frame and fences superseded navigation.
  The unchanged real-daemon review regression then passed; no assertion was
  weakened and no pointer activation was substituted with a chord.
- Clean `be42a9267ff824065fa29487b9865523852ef59a`: both full host gate
  helpers PASS, including 27 UI unit + 2 real-daemon tests; both driver gate sets
  PASS (27 tests); source checker PASS (235 paths, 34 archives).
  Linux PTY light/dark and WSLg light/dark completed the actual-input scenario.
  However, the dark WSLg `review-zh-delete-refused.png` contained 307,200 opaque
  black pixels. Its JSON PASS was not accepted as rendered evidence or relabelled.
  The worker now waits for composition/nonuniform owned pixels within two seconds
  and refuses blank output; a native C# buffer check proves uniform and alpha-only
  rejection plus real RGB variation acceptance (`s2-capture-pixels.txt`).
- `s2-conpty-be42a92-light`: FAIL at creation-preview observation. Actual ConPTY
  bytes contained erase-character `CSI X` followed by cursor jumps over spaces;
  the observer ignored erasure, retaining old path characters inside the status.
  Add proper erasure and wide-glyph retirement rather than weakening the predicate.
  Four focused VT tests pass, including the observed redraw and partial CJK cells.
- `s2-hwnd-be42a92-light`: FAIL at queue activation; the assumed focus path invoked
  Reconnect instead. Field shortcuts now defer focus until the selected pane's
  visibility update, fence closed/busy/uncertain panes and log content-free real
  focus events. The exact destructive-focus regression remains PASS.
- `s2-wslg-frame1-dark` / `s2-pty-erasure1`: DEV_PASS with the new capture guard,
  deferred field focus and VT erasure. The dark compact Chinese refusal PNG was
  visually inspected and is rendered, not black. These are exploratory results.
- Clean `148bfefdccaf1d663d39a282dd89efdcfa4cce3e`: Linux full gates,
  driver gates (29 tests), source checks and PTY light/dark PASS. Windows smoke
  still failed at queue keyboard activation. An owned Crossterm/ConPTY diagnostic
  confirmed normal Tab/Enter modifiers and separate press/release events, so
  keyboard delivery was not replaced with a callback or a different shortcut.
  The attempted app-level `focusable` override was not supported by the component
  builder and was removed; it is not a compatibility layer.

## NSH source decision

The old pin's Win32 Tab handler visits only text fields and its `Focus` command
only calls `text_input::focus`. The required button keyboard path needs the
upstream generic focus implementation, not more Tab-count guesses.

[NSH v0.5.1](https://github.com/longbridge/nsh/releases/tag/v0.5.1) resolves to
`224d9a795c7ac9d10d75750de9351adac5549675`; that release still has the input-only
Win32 focus command. The owner chose a fixed upstream implementation rather than
maintaining a local backport, conditional on its base being at least 0.5.
[Upstream fix b7f31a0](https://github.com/longbridge/nsh/commit/b7f31a08478be262fd7b76ec856036fd5b8ffdb0)
and its [parent manifest](https://github.com/longbridge/nsh/blob/57341a82ec66bb57f355c5f716dd3d7f038d6de8/Cargo.toml#L10)
both declare workspace version **0.5.0** (Rust minimum 1.97).

Active NSH crates and offline renderer packaging now share immutable
`b7f31a08478be262fd7b76ec856036fd5b8ffdb0`; this is a 0.5-based upstream revision,
**not** the 0.5.1 release tag. No cached SDK source, local SDK patch, alternate
framework, product IPC/schema/permission or mutation contract is introduced.
The existing real-daemon review regression passed on this revision before native
acceptance. Historical spike/archive pins and their evidence remain unchanged.

## Final clean-source acceptance

Runtime/test code: **`4d757591dac8150beafc60b869741b60cd6c8254`**, clean on
both real Git checkouts. Evidence collection completed **2026-11-15T08:47:32Z**.
The later delivery commit changes this report only; no native result is attributed
to that documentation-only revision.

### Remaining native focus repairs

- Retained concealed panes now set both display-none and invisible. Win32 had
  visited the hidden Handoff input after Suggest, despite its absent pixels.
  Owned-window captures and read-only UI Automation focus observations isolated
  that path; no UI Automation Invoke/SetFocus operation drove the scenario.
- The pinned upstream implementation supplies generic Win32 focus/raw keys, not
  default Enter/Space activation of press targets. The shared app `ui::Button`
  now supplies standard Windows activation using the same focused-key lifecycle
  as NSH base controls. Bindings exist only while that enabled, non-loading
  control owns focus and never consume editing keys from an input or sibling.
  This is app control behavior, not a cached SDK modification or a driver callback.
- Native focus chains are independently calibrated: load/digest then queue-row
  stop counts are PTY 3/4, ConPTY 4/5, GTK 3/3 and HWND 3/4. Optional PNG/VT
  traces preserve actual per-stop observations; native tests still send OS keys.
  Enter loads/selects and Space expands the immutable digest. No input retries,
  guessed cross-backend offsets or stale-state success fallback were introduced.

Earlier `d2198f5`/`571ba25` native failures remain in the local evidence, including
hidden focus stops, missing native activation and the incorrectly shared row
count. They are not final acceptance. The normal ConPTY key-delivery diagnostic,
read-only focus observer and temporary Windows launcher were removed after use;
their generated logs/caches remain ignored local artifacts.

### Native Cargo and presentation gates

| Check | Linux/WSL | Native Windows |
| --- | --- | --- |
| Root workspace fmt/build/Clippy/tests | PASS; 128 passed, 1 existing opt-in sample ignored | PASS; 129 passed, same 1 ignored |
| Shared model fmt/Clippy/tests | PASS; 53 tests | PASS; 53 tests |
| NSH fmt/build/Clippy/tests | PASS; 27 library + 2 real-daemon integration tests | PASS; 27 library + 2 real-daemon integration tests |
| Driver fmt/Clippy/tests/build | PASS; 29 tests | PASS; 29 tests |
| Branch/final whitespace checks | PASS | PASS |

Clippy ran with `--all-targets -- -D warnings`; no regression assertion was
weakened. Driver tests include four VT redraw/wide-cell regressions, 21 worker
protocol/deadline/refusal regressions and four owned-process lifecycle regressions.
The source/link/workflow checker passed: **235 source paths, 34 explicit archives**.

Raw gate directories, relative to each checkout's `artifacts/local-p1-m4/`:
`s2-linux-4d75759`, `s2-driver-linux-4d75759`, `s2-windows-4d75759` and
`s2-driver-windows-4d75759`. Linux `source.txt`/`commands.txt` and Windows
`report.json` retain exact revision, commands, flags and platform metadata.

```text
CARGO_NET_OFFLINE=true tools/local_verify.sh all artifacts/local-p1-m4/s2-linux-4d75759 d66d7c6f1dd60cf98238d5122111ed374efc4343
tools/windows_local_verify.ps1 -Suite All -ReportDir artifacts/local-p1-m4/s2-windows-4d75759 -DiffBase d66d7c6f1dd60cf98238d5122111ed374efc4343
cargo fmt --manifest-path tools/native-acceptance/Cargo.toml -- --check
cargo clippy --manifest-path tools/native-acceptance/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path tools/native-acceptance/Cargo.toml --locked
cargo build --manifest-path tools/native-acceptance/Cargo.toml --locked
node tools/check_source.mjs
```

The final offline Wasm build and six-asset renderer package also passed with the
same NSH pin (`s2-web-package-4d75759`), using the already installed LLVM 22 clang
through process-local `CC_wasm32_unknown_unknown=/usr/lib/llvm22/bin/clang`.
This is compile/package proof, not a browser-surface acceptance claim.

### Actual native input and visible surfaces

All eight reports name the code SHA above, `worktree_dirty=false`,
`historical_source_substitution=false` and `status=PASS`. Direct Wayland reports
`GdkWaylandDisplay` / `GskCairoRenderer`; DISPLAY was removed, not routed through
X11 or niri. GUI sessions ran sequentially under nonce/PID/foreground protection.
GUI ownership logs confirm GUI/worker reaping and their pipe reader/writer joins;
terminal sessions exited cleanly.

Native command entrypoints (absolute new report paths, listed below):

```sh
ACP_S5_APPEARANCE=light tools/native-acceptance/target/debug/acp-native-acceptance tui "$PWD/artifacts/local-p1-m4/s2-pty-independent-smoke"
ACP_S5_APPEARANCE=dark tools/native-acceptance/target/debug/acp-native-acceptance tui "$PWD/artifacts/local-p1-m4/s2-pty-4d75759-dark"
# Run for appearance=light and appearance=dark; windows_helper is the committed
# tools/native_window.ps1 in the owner's real Windows checkout.
GSK_RENDERER=cairo ACP_S5_APPEARANCE="$appearance" tools/native-acceptance/target/debug/acp-native-acceptance desktop "$PWD/artifacts/local-p1-m4/s2-wslg-independent-$appearance" "$windows_helper"
```

```powershell
# From the real Windows checkout, for appearance=light and appearance=dark.
$env:ACP_S5_APPEARANCE = $appearance
.\tools\native-acceptance\target\debug\acp-native-acceptance.exe tui (Join-Path $PWD "artifacts\local-p1-m4\s2-conpty-4d75759-$appearance")
.\tools\native-acceptance\target\debug\acp-native-acceptance.exe desktop (Join-Path $PWD "artifacts\local-p1-m4\s2-hwnd-4d75759-$appearance") (Join-Path $PWD 'tools\native_window.ps1')
```

Evidence directories below are relative to that host checkout's
`artifacts/local-p1-m4/`. PIDs are the disposable UI children, not an adopted
desktop/terminal session.

| Surface | Appearance | Result | Owned UI PID / HWND nonce | Evidence directory |
| --- | --- | --- | --- | --- |
| Linux PTY | light | PASS | 435774 / no HWND | `s2-pty-independent-smoke` |
| Linux PTY | dark | PASS | 436619 / no HWND | `s2-pty-4d75759-dark` |
| WSLg Wayland | light | PASS | 436129 / `a43a2eaa8b85d708` | `s2-wslg-independent-light` |
| WSLg Wayland | dark | PASS | 436478 / `a43a2de470bca36b` | `s2-wslg-independent-dark` |
| Windows ConPTY | light | PASS | 35640 / no HWND | `s2-conpty-4d75759-light` |
| Windows ConPTY | dark | PASS | 41128 / no HWND | `s2-conpty-4d75759-dark` |
| Windows HWND | light | PASS | 37688 / `a43a2f1f42ee084f` | `s2-hwnd-4d75759-light` |
| Windows HWND | dark | PASS | 32432 / `a43a2d1b7d313701` | `s2-hwnd-4d75759-dark` |

The common scenario exercises explicit Profile creation/grant, Busy, current exact
inspect, Queue/Detail/History through real Alt+left/right, full-digest expansion,
same-key peer demotion, confirm/deprecate, real decision history, wrong digest,
Conflict, committed lost reply with explicit immutable same-request replay,
revocation and disconnect. Delete requires exact-ID retype and explicit apply;
wrong `999` and blind Enter leave revision/status/commit sequence unchanged.
Escape cancels only the destructive tier; reopening starts empty. Deleted content
and decisions are NotFound while the identity-only receipt remains.

English/zh-CN, 1080×1000/640×480 desktop and 160×90/80×30 terminal captures cover
exact content/history and localized destructive impact/refusal. Owned PNGs and
parsed terminal frames were inspected: full digest and real history are visible,
the compact input is focused, mismatch text is readable and Delete is disabled.
Dark localized captures contain rendered content, not the earlier black frame.
Existing radius/type/spacing/alignment/token styling is preserved; focus,
disabled, selected and error states are visible. No new motion was introduced;
static captures do not certify hover/press animation timing or full accessibility.

Each directory retains `report.json`, native probes or `terminal.vt`, captures,
`receipt-proof.json` and independent CLI `review-history-proof.json`.
History confirms r1→r2 / deprecate r2→r3 at commits 11/12, with the actual
`desktop` or `tui` client-asserted channel; these are not reviewer identity claims.

Binary SHA-256 values observed at native completion agree across the light/dark
and GUI/TTY reports on each host:

| Host | Binary | SHA-256 |
| --- | --- | --- |
| Linux | `acp-native-acceptance` | `d80817355840c4cc4daef4189992b2d29ec6721ae3fd48a954188680da7a726b` |
| Linux | `acp-daemon` | `f4625a279ebd1ea2e4dce7730122b8e801dd8d1a625a38b9c7f492a5a6d925cb` |
| Linux | `acp-query` | `132d4cac6d2c74f37dddd5c29c4bb796b211da941deb6c3daa209a4a60cbb73b` |
| Linux | `acp-nsh-native` | `f237a8d843aae8153d65b0fc91cf7db0a7cb43b7d0a39f85e68c203adf8aaef2` |
| Linux | `acp-nsh-tui` | `3b7c7dbde90dd66aa2c0102ae7914ae7de4cc9ad81954e8ea1a8f0d31313e8b0` |
| Windows | `acp-native-acceptance.exe` | `4909e34314a69c674dcf7d7f1df02d31985834b6493f77eef8299246b39f23ee` |
| Windows | `acp-daemon.exe` | `d84e01a6defcf68ae0722d75b9466429f5121e6d218c2dbc29863fd319978d84` |
| Windows | `acp-query.exe` | `37e89851498591ebc6eb991f396616804b5f2307ac730432945696d085915850` |
| Windows | `acp-nsh-native.exe` | `4ae73a26833f909e82a8df44f3e67dd94c7069c9a1c2a72f99bde70a40555926` |
| Windows | `acp-nsh-tui.exe` | `a25f994c723eda8a3f637093194e1ad83776b099ca0c8cec482f312c2920d593` |

## PR #59 review follow-up — Web pin and ADR synchronization

Review-fix code: **`22a8a2c8826f53fdb07aa935fb307d72ff9ab920`**. The original
native matrix remains evidence for `4d757591`; no new native run is claimed for
this CI/verifier/documentation-only change. Rust sources, manifests and lockfile
are unchanged.

The reviewed blocking defect was real: the Web workflow checked out `87c7e8d`
while Cargo and the packager required `b7f31a0`. The workflow now fetches the same
immutable revision and runs `node tools/check_source.mjs` immediately after its
locked npm dependency install, before the Wasm build. That existing verifier
compares the canonical Cargo `nsh-core` revision, all active NSH dependencies, the
packager's enforced SHA and the Web workflow's renderer checkout. It performs
validation only; no second resolver, SDK patch or historical-pin rewrite is added.

ADR-0003 now names the selected 0.5-based revision, explicitly distinguishes it
from stable v0.5.1, preserves `87c7e8d` as historical/pre-S2 evidence and links this
source-decision report. Framework selection and product authority are unchanged.

| Local check | Result |
| --- | --- |
| Source/link/YAML/NSH-pin verifier | PASS; 235 paths, 34 archives |
| Disposable Git-fixture mutations | PASS; aligned inputs accepted; old workflow pin, mutable workflow ref, old packager pin and divergent Web-crate pin each refused |
| Exact workflow renderer clone/checkout | PASS; `b7f31a08478be262fd7b76ec856036fd5b8ffdb0` |
| Locked Wasm build and six-entry offline package | PASS |
| Static-server confinement/MIME regression | PASS; 1 test |
| Real offline NSH Wasm/browser smoke | PASS on clean `22a8a2c`; five pages, filter/detail, task validation, keyboard Tab/Enter, themes, resize and read-only operator |

The local sequence used existing Node **26.13.1**, Chromium **157.0.8103.38** and
LLVM 22 clang. The committed job specifies Node 24 and Playwright's downloaded
Chromium; that hosted environment was **NOT_RUN**, not inferred from this local
pass. Existing local compiler/browser prerequisites were reused rather than
installing OS packages. No daemon connection, native IPC or browser gateway was
used; the smoke observed zero external requests and blocked non-loopback traffic.
Dark and compact/light actual-browser PNGs were visually inspected.

```text
npm ci --prefix apps/operator/tests
node tools/check_source.mjs
CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CC_wasm32_unknown_unknown=/usr/lib/llvm22/bin/clang cargo build --manifest-path apps/operator/ui/Cargo.toml --target wasm32-unknown-unknown --lib --locked --offline
git clone https://github.com/longbridge/nsh apps/operator/vendor/nsh
git -C apps/operator/vendor/nsh checkout b7f31a08478be262fd7b76ec856036fd5b8ffdb0
node tools/package_ui_web.mjs apps/operator/vendor/nsh apps/operator/target/wasm32-unknown-unknown/debug/acp_operator_ui.wasm artifacts/local-p1-m4/pr59-review-web-dist
node --test apps/operator/tests/static-server.test.mjs
CHROMIUM=/usr/sbin/chromium node apps/operator/tests/web-smoke.mjs artifacts/local-p1-m4/pr59-review-web-dist artifacts/local-p1-m4/pr59-review-web-browser
```

Fresh ignored output directories were used instead of overwriting an existing
local `ui/dist`. The browser result/PNGs are in `pr59-review-web-browser`;
`pr59-review-pin-cases.json` records isolated refusal cases. Their temporary Git
fixture was removed. These are author-run local checks, not independent review
or hosted CI evidence.

PR #58 was still Open/Draft when its review was read. Its structural precursor
must be merged by the owner first; only after the base advances can #59's reduced
diff/mergeability be rechecked. No self-merge, squash of #58, issue closure or S3
acceptance is implied. Keep #59 Draft and #57/#52 open.

Historical S0/S1/S3/S5 reports retain their original revisions and scope. Hosted
Actions, macOS, niri, mobile, real-model turns, installation/release, full IME and
screen-reader certification are NOT_RUN and are not implied by these checks.
Parent #52 remains open for S3; this work cannot certify S3 or production readiness.
