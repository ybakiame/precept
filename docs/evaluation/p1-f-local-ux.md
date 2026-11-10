# P1-F local brand/UI evidence

The owner accepted the revised base: “可以以此版本为基础吧”, then requested detailed
logic/UI refinement, modest icons, Daemon state and Windows GUI access to Windows
and WSL Daemons. This authorizes five-page integration; final after-implementation
UI review is tracked separately. No product rename or official logo was selected.

## Integrated implementation (local validation in progress)

The default actual NSH Desktop/TUI now uses the shared theme collection, bilingual
catalog, custom chrome and refined five pages. Separate connection drafts support
Windows named-pipe IPC, Linux Unix IPC and explicit Windows-owner WSL helper
access. Switching targets atomically invalidates pending reads and old scope,
tasks/evidence/time. Invalid drafts cannot silently fall back to synthetic data.
Status is the last query observation, never an always-online assertion.

Raw synthetic images/logs are private under `artifacts/local-p1-f/product/`.
Initial visual inspection found compact summary-card shrink and incomplete VT
capture issues; content sizing and complete redraw consumption were corrected.
Pointer regression needed the new frameless geometry rather than old CSD offsets;
failed exploratory logs are retained privately. These failures were not treated
as acceptance passes. Final tested SHA and full result matrix follow after the
clean host companion run.

Production backend/authorization/protocol/SQLite/fingerprint behavior is unchanged.
The only backend-file diff is inside a test: Tokio may complete a fast Git child
before polling an expired zero timeout, so that assertion permits the authentic
completed output. Unix additionally exercises a real bounded timeout with Git's
built-in infinite `/dev/zero` read, followed by a normal query. No shell, external
fixture executable, repository write or production timeout change is introduced.

GitHub Actions, niri/mobile/macOS, full IME/screen-reader and real OS clipboard
checks are not claimed. ADR-0003 remains Proposed. Historical B0 evidence below
retains its original scope and review status; it is not final implementation evidence.

---

# Historical B0 local brand/UI evidence

Status: **revised B0 theme-collection preview; owner layout review PENDING**. The
owner requested one layout with selectable themes, corrected caption centering,
the supplied concept hierarchy and i18n. No final brand acceptance or complete
five-page redesign is claimed. Start: latest main merge #21,
`0ba227c077c2d4fd0176adc4fbb43730a998c17b`; branch `feat/p1-f-brand-ui`.
No GitHub Actions, niri or mobile tests. ADR-0003 remains Proposed.

## Current revision — theme collection, concept hierarchy and i18n

Current preview code: **`b40b299fa317aa8059cfb0ef3fae8307ef3c59f8`**. Its behavior
was implemented in `72beab957bea380171eba1055c8d6a1c03bc729c`; the following commit
only cleans two preexisting shared-frontend Clippy findings (equivalent conditional
and test-module placement). No production backend, protocol or authorization code
changed. Both actual hosts rebuilt the current code using offline caches.

The owner's new direction supersedes a mutually exclusive A/B/C selection:
Instrument / Ledger / Harbor are now a **theme collection**, each with Light/Dark
tokens. Layout, typography, radii and navigation are identical. Revised Overview
uses Workspaces / Agent access → Effective context → Resume context → unavailable
preference-review boundary. No agent online states or profile review actions are
invented from the supplied concept image. The preview reads the typed built-in
snapshot rather than hardcoded preference counts or repository facts.

English/Simplified Chinese UI catalogs are exhaustive and switch at runtime.
Project/task names, preference values and original recorded evidence stay intact.
Appearance, language and high contrast are session-only; System follows NSH's
runtime appearance without editing system settings or reading another app's
configuration. All of this remains staged in the synthetic preview until the
revised B0 layout is reviewed for full five-page integration.

| Current B0 check | Actual result |
| --- | --- |
| Shared native-feature tests | 12 PASS on WSL and Windows, including complete catalogs and six palettes/high contrast; scope/request tests preserved |
| NSH fmt / Clippy, shared fmt / Clippy | PASS locally on both actual hosts; preexisting shared lint findings corrected without changing behavior |
| NSH component tests | 4 PASS on current WSL code, including all 834 pages of 10k rows, bounded nodes, filter/detail and scope behavior |
| Python compatibility/local safety | 16 PASS on WSL |
| Test-only VT cell renderer | 3 PASS on WSL and Windows: intact CJK/styles and replacement of each half of a wide glyph |
| Actual preview windows / PTY / ConPTY | PASS on both hosts: Chinese Light 3 themes × 3 pages × both sizes; English Dark Harbor Overview/Appearance × both sizes; all use current rebuilt binaries |
| Native and terminal language/theme switching | PASS on WSLg/PTY and Windows/ConPTY, actual Ctrl+L / Alt+0 / Alt+B plus visible Chinese/English states |
| Caption alignment | PASS in owned-window pixels: all three vectors within 0.5px of target centers on the measured host allocations |
| Custom chrome | PASS on both hosts: real drag, maximize/restore, minimize/restore, close; no system caption / GTK decorated=false |
| Full selected-design acceptance | PENDING: all five pages, full actual 10k performance matrix and final clean companion-source acceptance are not claimed by B0 |

Both locale catalogs preserve scope. The headless regression changes theme,
appearance and language, discloses original validation JSON, then switches project
and proves the previous task/objective cannot return. Locale changes do not rewrite
project values. No clipboard read or real-profile database access is involved.

The first actual TUI language/page switch exposed pyte's retained empty wide-cell
stub and `display` IndexError. A **test-only** screen view now renders replaced
halves as spaces while retaining intact two-cell glyphs and attributes. Three
deterministic VT regressions plus real PTY/ConPTY switching pass. Application bytes,
terminal configuration and third-party installation were not patched. Raw initial
failure and final logs remain private.

Current private evidence is under `artifacts/local-p1-f/revision-2/`:

- `windows-light/`, `wslg-light/`: three-theme Chinese Overview/Handoff/Appearance
  PNGs at 1080×760 / 640×480 and terminal frames at 160×50 / 80×30.
- `windows-dark-en/`, `wslg-dark-en/`: English Dark Harbor Overview/Appearance
  samples; the theme picker also shows all three dark palette cards.
- `windows-interaction/`, `wslg-interaction/`: actual settings-switch captures,
  PTY/ConPTY frames, `settings-result.json` and `chrome-result.json`.
- `contrast.json`, `source-files.json`, self-contained `preview-gallery.html` and
  private build/test logs. Source-file copies are checked against the marked
  disposable Windows checkout; it is not represented as a clean same-SHA checkout.

Exact captured unsigned preview binary hashes (all four capture reports checked
against the rebuilt files):

| Binary | SHA-256 |
| --- | --- |
| Linux desktop | `3061c741edf4a3fe934b4f192cc7f1d0c0d19483cfcec05cd6bc3ca9b422ab9e` |
| Linux TUI | `a546f064184a0863b2e04e67f36df3f6f6e3cf5d714526647594d3c0f11fbdad` |
| Windows desktop | `8d1ac812d7106d5e688c1d52a082bb72ac48cc98c062f73776b62288c081f258` |
| Windows TUI | `777e69d6197e48f8072aa76bdf673b91b0e3433a327605caf45d0bcb11a14d9b` |

Reproducible preview entry points (native Windows uses `.exe`):

```sh
acp-nsh-native --brand-preview c --locale zh-CN --appearance light
acp-nsh-tui --brand-preview c --preview-page appearance --locale zh-CN
python tools/test_terminal_screen.py
python tools/test_brand_settings.py --desktop <desktop-bin> --tui <tui-bin> --out artifacts/local-p1-f/revision-2/interaction
python tools/capture_brand_previews.py --desktop <desktop-bin> --tui <tui-bin> --out artifacts/local-p1-f/revision-2/captures --locale zh-CN --appearance light
```

The preexisting root zero-timeout assertion failure documented below remains
unmodified and must be resolved/dispositioned before final acceptance. No new root
Fingerprint/MCP authority is introduced or removed. niri/mobile, native clipboard,
full IME/screen readers, remote inference and real external repositories remain
NOT_RUN in this preview iteration. No Actions or raw-data upload.

## Historical initial B0 scope and decision gate

Three independent visual directions use synthetic-only actual NSH Desktop/TUI
previews. Default console behavior and production backend are unchanged. Preview
arguments reject IPC location, credentials, project and WSL helper options; the
preview does not initialize native inputs or access a user database/repository.
Illustrative recorded checks are labeled historical; current verification is
explicitly Unknown/NOT_RUN. No fabricated measurement or live connection.

Candidate identity and tradeoffs: [identity](../brand/identity.md).
Implementation staging: [design system](../p1/ui-design-system.md).
The initial A/B/C selection question was superseded by the owner's theme-collection
instruction above. Revised layout review remains pending; full five-page local
acceptance and Draft PR follow that review.

## Historical initial B0 evidence

All raw synthetic screenshots, terminal frames, logs, measurements and host data
stay under gitignored `artifacts/local-p1-f/`. Native capture uses only the owned
HWND via PrintWindow, never a desktop crop/fallback. WSLg must observe actual
GdkWaylandDisplay and map/paint; X11/Xvfb is not a substitute.

Preview source revision: **`266f576eeb3974d1c0444a8139ad84b02a1d09fd`**. This
contains the three candidates plus the owner's custom-title-bar revision. Exact
NSH/shared Rust source-file hashes match between the primary checkout and the
explicitly marked, disposable Windows test checkout. The Windows development copy
is not presented as a clean same-SHA checkout. Subsequent evidence-only commits do
not change the tested UI code. This is a **development B0 matrix**, not final
selected-design acceptance.

| B0 observation | Result / scope |
| --- | --- |
| Windows native candidate windows | PASS: A/B/C × Dashboard/Handoff × 1080×760 / 640×480; owned-window pixels reviewed |
| WSLg native candidate windows | PASS: same candidate/page/size matrix; actual GdkWaylandDisplay, mapped/after-paint, GskGLRenderer, GTK decorated=false |
| Actual window chrome | PASS on both hosts: app-drawn bar, pointer drag, maximize/restore, minimize/restore and close; no system Windows caption |
| Actual Wayland resize | PASS: frameless bottom-right grip produces observed GTK 640×480 and 1080×760 allocation changes |
| PTY / ConPTY candidate adaptation | PASS: all three candidates, both pages, 160×50 / 80×30, Ctrl+Q and clean terminal restoration |
| NSH preview boundary/interaction tests | 2 PASS on WSL and Windows: reject live/arbitrary inputs; candidate/page changes retain historical/Unknown semantics |
| NSH component suite, including existing state/10k and new preview tests | 4 PASS on WSL after custom-title-bar revision; all 834 preference pages mounted, bounded nodes, filter/detail checks preserved |
| NSH fmt / Clippy / offline release bins | PASS locally on WSL and Windows |
| Remaining Python compatibility/local-safety tests | 16 PASS on WSL |
| Root fmt / Clippy / bin build | PASS on WSL |
| Root Rust tests | Initial 47 PASS; subsequent development run FAIL in an unchanged zero-timeout assertion (details below) |
| Independent JSON / SQLite comparison | 120 / 78 PASS on WSL |
| Synthetic backup / process recovery / service E2E | PASS / 11 PASS / 11 PASS on WSL |
| Compiled Rust fingerprint/core/IPC guard mutations | 3 rejected, PASS on WSL |
| Owner direction choice | PENDING; custom title bar is an explicit revision, not selection of A/B/C |
| Full selected-design host/performance/Codex acceptance | PENDING; no final E4/E4-compatible acceptance claim |

Native clipboard, full IME/screen reader, macOS, three-sample startup/RSS and local
Codex smoke are NOT_RUN in this B0 preview pass. The 10k component test is not a
substitute for final actual-window filtered/paged 10k workloads. No niri, mobile,
real external project or real ACP user database is exercised here. E4 evidence is
historical only. The selected design still requires the full local regression
matrix, exact clean code revision and private before/after owner review.

### Actual observed problems and revisions

- Initial compact candidates spent too many rows on identity/navigation. Combining
  the name treatment and navigation, hiding repeated subtitles and reducing compact
  panel padding now exposes repository facts plus preference scope/source together.
- Owner requested a custom title bar. All candidates now use a frameless opaque
  window, app-drawn caption/buttons and a separate native drag region/resize grip.
  Controls were tested with actual owned-window pointer actions, not image inference.
- Initial new headless test used a substring as an exact selector; it failed while
  the expected banner was present. Corrected the selector and reran successfully.
- The existing `git_output_overflow_deadline_and_stderr_are_bounded_and_reaped`
  test first passed, then failed at the assertion that `Duration::ZERO` must return
  None for a fast Git status. A process may already have completed at the first
  future poll. No production backend or root test code was changed to mask this.
  The development report retains **12 PASS / 1 FAIL**, plus the earlier full-pass
  log. Resolve or explicitly disposition this existing timing assertion before
  final acceptance; it is not relabeled PASS or silently suppressed.

### Reproducible local preview commands

Use cached toolchains and the existing process-local Windows compiler setup; no
package installation or system configuration change is performed.

```sh
cargo build --offline --locked --release --manifest-path spikes/p1-e/nsh/Cargo.toml --features native,tui --bins
spikes/p1-e/nsh/target/release/acp-nsh-native --brand-preview a
spikes/p1-e/nsh/target/release/acp-nsh-tui --brand-preview b --preview-page handoff
python tools/capture_brand_previews.py --desktop spikes/p1-e/nsh/target/release/acp-nsh-native --tui spikes/p1-e/nsh/target/release/acp-nsh-tui --out artifacts/local-p1-f/wslg
python tools/test_brand_chrome.py --binary spikes/p1-e/nsh/target/release/acp-nsh-native --out artifacts/local-p1-f/wslg
PYTHONPATH=src python -m unittest discover -s tests -v
PYTHONPATH=src python tools/local_verify.py --core --service --migration-mutations --report-dir artifacts/local-p1-f/b0-core
```

For Windows use the native `.exe` bins and `--out artifacts/local-p1-f/windows`.
WSLg input/capture needs the explicit existing `ACP_WINDOWS_PYTHON` and owned
`ACP_WINDOWS_TEST_ROOT`; test-only Python dependencies are provided through the
existing private PYTHONPATH. No implicit host/helper discovery or token copying.

Private locations inside `artifacts/local-p1-f/`:

- `before-wslg/`, `before-windows/`: unchanged E4 synthetic baseline captures.
- `windows/`, `wslg/`: final custom-title-bar candidate PNGs, PTY/ConPTY text frames,
  `report.json` and `chrome-result.json`.
- `preview-gallery.html`: self-contained local comparison viewer for these actual
  captures; it is not a substitute frontend or a live backend connection.
- `contrast.json`, `preview-source-files.json`: palette calculations/source hashes.
- `b0-core/`: development gate logs/report, including the unsuppressed failure.
- `owner-preview.json` in the owned Windows artifact directory: only the synthetic
  preview's process metadata; owner choice remains PENDING.

Locally built preview binaries, **unsigned development only**:

| Binary | SHA-256 |
| --- | --- |
| Linux desktop | `d0baf39fd5693810ee1edd47fc05834eb1839819b86dbe0da8df7c476d5b3f6c` |
| Linux TUI | `74ed00883149aca58dec2aea2ed1769b11c1a30836efe95db25a6fbd660b58bc` |
| Windows desktop | `df5c343d12033480655c4acdc6ccae616fff1b35214b6caad85afb1038bf5bd9` |
| Windows TUI | `5b5187bbf2e1af4ba0d3c9aec7f5c5bafce5bc9935997993c282111eefd0c795` |

These hashes match the corresponding preview capture reports. No signed release,
installer, final selected-design archive or upload is claimed.

## Owner-authorized historical branch cleanup

After fresh fetch and checking PR head tips/merged state and worktree use, twelve
old remote branches were atomically deleted with exact-tip lease guards. The
eleven tips listed in the [E4 audit](../p1/e4-branch-audit.md) were unchanged; the
additional E4 branch tip was `5e23b5ad39011528c03b6acd43463a67fddc54e5` (merged #21).
The two divergent historical refs match the completed E4 content/migration audit;
the other ten are ancestors of current main. No unresolved unique work was lost.

Remote now has only main; the new design branch has not been pushed at this stage.
Local has main plus `feat/p1-f-brand-ui`. Old local E4 was deleted with normal
`git branch -d` after verifying containment in main. Detached/prunable worktree
metadata is intact. No force-push of main or repository-wide setting change.
Private exact before/after refs and command results: `branch-audit/` in the P1-F
artifact directory. GitHub PR review records remain intact.
