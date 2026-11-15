# P1-M4/S2 precursor — formal operator app migration (spikes/p1-e → apps/operator)

Scope: structural precursor for [Issue #57](https://github.com/ybakiame/agent-control-plane/issues/57)
(S2 shared operator review UX), parent
[Issue #52](https://github.com/ybakiame/agent-control-plane/issues/52). This PR is a
**pure move/rename** of the accepted NSH operator app out of the spike tree into a
formal standalone app workspace. It carries **no functional change**: no product
crate, UI behavior, IPC contract, schema or test logic changed. The S2 feature itself
is a separate follow-up PR built on this structure.

Baseline: merged S1 main `d66d7c6f1dd60cf98238d5122111ed374efc4343` (PR #56).
Owner decisions this session: migrate to `apps/operator/` as a standalone app;
migration as a separate precursor PR; archive the `gpui-kit` comparison crate;
establish a formal `src/ui/` layer + `DESIGN.md` in the S2 feature PR (not here).

| Commit | Content |
| --- | --- |
| `21996ee` | Repository-wide `.gitattributes` LF normalization policy (cross-host precursor) |
| `eadf9bf` | The migration: directory moves, package/crate renames, workspace restructure, identity/wasm renames, script/CI/doc path updates |
| `1b8710e` | Ignore the `artifacts/local-p1-m4/` tree for S2 (matches prior milestone convention) |

**Final tested code SHA: `1b8710ef8881714aad4df902dced0f3da3eee703`.**
`git diff --name-only d66d7c6 1b8710e -- crates/` is empty: no product crate changed.
The Rust source delta is a pure crate-identifier rename (`acp_ui_spike_model` →
`acp_operator_model`, `acp_nsh_spike` → `acp_operator_ui`) plus the driver's binary
path correction; every other changed Rust line contains only a renamed identifier.

## Old → new map

| Old | New |
| --- | --- |
| `spikes/p1-e/shared` (package `acp-ui-spike-model`) | `apps/operator/model` (package `acp-operator-model`) |
| `spikes/p1-e/nsh` (package `acp-nsh-spike`) | `apps/operator/ui` (package `acp-operator-ui`) |
| `spikes/p1-e/tests` | `apps/operator/tests` |
| `spikes/p1-e/gpui-kit` | `docs/archive/gpui-kit-spike` (archived; not built by any gate) |
| two empty per-crate `[workspace]` + two lockfiles | one `apps/operator/Cargo.toml` workspace (`members = ["model","ui"]`) + single `apps/operator/Cargo.lock` |
| wasm `acp_nsh_spike.wasm` | `acp_operator_ui.wasm` |
| nsh.json id `org.example.acp.spike`, name "ACP NSH Rust Spike" | `org.acp.operator`, "ACP Operator" |
| `#[native_module("acp-spike/native")]` | `#[native_module("acp-operator/native")]` |
| binaries `acp-nsh-native` / `acp-nsh-tui` | **unchanged** (driver + docs depend on them) |

Because `model`/`ui` are now one workspace, build output lands in
`apps/operator/target/` (not `apps/operator/ui/target/`); the acceptance driver and
CI web job were updated accordingly. `[profile.dev] debug = 0` was hoisted to the
workspace manifest (Cargo ignores member profiles); this preserves the pre-migration
no-debuginfo build. The lockfile was preserved exactly (513 packages, all 12 NSH
packages pinned at `87c7e8da85d2f15a56107baa6f0fabb419b508b3`), not re-resolved.

Scripts/CI updated: `tools/local_verify.sh`, `tools/windows_local_verify.ps1`,
`tools/native-acceptance/src/main.rs`, `tools/package_ui_web.mjs`,
`tools/check_source.mjs`, `.github/workflows/{ui-spike,rust}.yml`. Live doc paths
updated in the runbook, contract, matrix, architecture, ADR-0003, brand identity and
README; historical evaluation reports keep their original `spikes/p1-e` evidence text
(only broken live links fixed; `p1-m2-s4-retire-python.md` marked historical and
archive-indexed).

## Both-host full gates (clean committed SHA, `git diff --check BASE HEAD` first)

Commands: `tools/local_verify.sh all <new-dir> d66d7c6…` and
`tools/windows_local_verify.ps1 -Suite All -ReportDir <new-dir> -DiffBase d66d7c6…`
(PowerShell 7.6.2). Rust/Cargo 1.99.1 both hosts; locked offline caches.

| Gate | Linux/WSL `1b8710e` | Native Windows `1b8710e` |
| --- | --- | --- |
| branch-diff / diff | PASS | PASS |
| workspace fmt/build/clippy/tests | PASS | PASS |
| shared (model) fmt/clippy/tests | PASS | PASS |
| nsh (ui) fmt/build/clippy/tests | PASS | PASS |
| Workspace tests | 127 | 128 |
| Shared native model tests | 32 | 32 |
| NSH ui tests (lib + real-daemon) | 7 | 7 |

The +1 Windows workspace test is the same platform-specific delta recorded in S5
(then 116/117); counts are unchanged by the migration relative to the S1 baseline.
Linux kernel 6.18.58.1-microsoft-standard-WSL2 x86_64; Windows 10.0.26300.0 x86_64.
`node tools/check_source.mjs`: PASS (218 source paths, 34 archives).
`node --test apps/operator/tests/static-server.test.mjs`: PASS.
Native-acceptance driver fmt/clippy/test: PASS (21+4+1 harness tests).

## Launch smoke (rename did not break launch or rendering)

The full S5 acceptance scenario is **stale relative to S1's CLI contract**: its
direct-CLI `preference-reject … --confirm` step omits the `--proposal-digest` argument
that S1 made required, so the scenario aborts late with a CLI usage diagnostic. This
is pre-existing at the S1 baseline and unrelated to the migration; updating the
harness for the S1/S2 review actions is S2 feature-PR scope (Commit D). The launch
smokes below nevertheless confirm the renamed binaries launch, render and run the
profile create/confirm/lost-reply-replay sequence with correct ownership cleanup.

- **Linux/WSL PTY** (`acp-native-acceptance tui`): real PTY, renamed `acp-nsh-tui`
  launched; captured create-review, busy, confirmed-exact-record, outcome-unknown and
  a valid `receipt-proof.json` (dropped == replayed, `rows_unchanged: true`) before the
  stale-CLI abort.
- **WSLg direct Wayland** (`acp-native-acceptance desktop`): `OWNED_WINDOW_READY
  nonce=a43a2d1341215341 pid=73722`, foreground protection enabled;
  `ACP_GTK backend=GdkWaylandDisplay renderer=GskGLRenderer mapped=true 1080x760`
  (direct Wayland, not X11/XWayland); bounded IO `stdin/stdout/gui_stderr=nonblocking`;
  real rendered PNGs (create-review, busy, confirmed-exact-record, outcome-unknown)
  visually confirmed; `OWNED_PROCESS_CLEANUP name=gui reaped=true reader_joined=true`
  and `DESKTOP_WORKER_CLEANUP reaped=true reader_joined=true writer_joined=true`.

These are launch/render smokes, **not** a full four-surface S2 acceptance. The four
required native surfaces at a clean S2 SHA are delivered in the feature PR.

## Linux binary hashes (SHA-256, `1b8710e`)

| Binary | SHA-256 |
| --- | --- |
| `apps/operator/target/debug/acp-nsh-native` | `3152fb92703cb6bc6edd48f8ac59036c4c19d75ecb55149f1ab4dc93d8a44d13` |
| `apps/operator/target/debug/acp-nsh-tui` | `3513a5d5e53ac7d4a0b3dcd6a5f3e75fb9d226c530c7943fe61743b150a6b5a3` |
| `target/debug/acp-daemon` | `ac395bb68599213e5a42e1ab0a84ddadca912f1c292ccf95c369457830503ffa` |
| `target/debug/acp-query` | `904680860756f06995007d2413893c494e792a52f86127b0766d0c89081afbba` |

## Limits

Hosted GitHub Actions NOT_RUN (`[skip ci]` on every commit). macOS NOT_RUN. The
optional offline Web/wasm smoke was not run (wasm filename/output path verified by
derivation and artifact naming, not an actual wasm build). Raw logs, PNGs and reports
stay in the ignored `artifacts/local-p1-m4/` tree and are not uploaded. No real
Profile, user database, credential or external repository was used; all fixtures are
synthetic. This precursor is not S2 acceptance and not S3 production certification.