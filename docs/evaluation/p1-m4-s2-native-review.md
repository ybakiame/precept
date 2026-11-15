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

Historical S0/S1/S3/S5 reports retain their original revisions and scope. Hosted
Actions, macOS, niri, mobile, real-model turns, installation/release, full IME and
screen-reader certification are NOT_RUN and are not implied by these checks.
Parent #52 remains open for S3; this work cannot certify S3 or production readiness.
