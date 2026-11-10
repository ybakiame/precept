# P1-F shared design system

Owner-approved foundation: one calm operator product with selectable Instrument,
Ledger and Harbor themes, a custom title bar, English/Simplified Chinese and the
provided concept hierarchy. The actual five-page console now consumes this system.

## Foundation

`shared/src/appearance.rs` defines primitive RGB and semantic background, panel,
inset, text, muted, border, accent/on-accent, success, warning and error colors.
`nsh/src/tokens.rs` adapts these same tokens to Desktop/TUI. No separate production
palette remains. Both modes of every theme and high contrast have computed tests
for 4.5:1 text and 3:1 essential borders; measured values are in the evaluation.
System mode follows NSH's reported appearance without editing OS settings.

| Token | Desktop | Compact / TUI |
| --- | --- | --- |
| Gutters | 16px | 8px / terminal cells |
| Spacing | 4 / 8 / 12 / 16px | 4 / 8px |
| Type | 22px heading, 14px body, 12px metadata | terminal font and Unicode cell widths |
| Radius | 8px panel, 6px control | same meanings; terminal borders |
| Caption | 40×36px target, centered 14px SVG | omitted |
| Content | 12 preference rows, 512-character detail chunks, six evidence entries | same bounds |

Buttons distinguish focus, hover, selection and disabled state. Theme/mode/target
selection has text markers as well as color. All statuses have words. No essential
motion, animation, external font or downloaded artwork is used. Original outline
SVGs accompany the five navigation labels; TUI uses the labels directly.

## Shell and pages

`components.rs` shares cards, controls, navigation, stable inputs, pagination and
error surfaces. Wide windows use a 176px sidebar. Below 900px and in TUI,
navigation becomes a wrapping row and summary cards stack without fixed heights.
Content scrolls; task/filter inputs remain mounted to preserve caret and focus.

- Dashboard: actual target/transport, last query result/time/API/schema/duration,
  unobserved Agent presence, scoped Git/context summary and historical resume
  warnings. Synthetic health explicitly states no Daemon is connected.
- Workspaces (Projects): only the allowlist; selection and current scope are clear.
  Scope changes clear task/evidence, and live data stays hidden during the read.
- Preferences: confirmed-only key/value/scope/source, scoped filter, 12-row pages,
  explicit 512-character disclosure/copy. No review or auto-confirm actions.
- Handoff: exact task ID, recorded objective/next action, current Git uncertainty
  and historical validation in separate cards. Missing and unqueried differ.
- Diagnostics: authenticated read-only IPC, observed runtime/backend, last query,
  bounded metadata and recovery help. No tokens, private paths or document bodies.

Appearance and connection editing are auxiliary session panels, not additional
navigation pages. Copy is disabled while these panels hide project content.
English and Simplified Chinese labels, accessible names and status explanations
update without translating project names, task IDs, values or raw recorded JSON.
Full IME/screen-reader verification and RTL are not claimed.

## Explicit target editing

`shared/src/connection.rs` holds separate applied and draft configurations. Local
IPC needs an absolute private runtime directory and endpoint. Windows WSL access
needs an explicit distribution, owner, existing absolute Linux helper, Linux
runtime and endpoint. Apply validates and starts a read; typing never connects.
Invalid drafts leave the active target unchanged. No discovery, TCP bridge,
automatic Daemon/Agent launch, token transfer or database write is introduced.
Linux hosts use local Unix IPC; WSL target application is disabled outside Windows.
Private paths appear only in the explicitly opened editor, never diagnostics.

Applying a target increments the epoch and clears snapshot, project list/selection,
task, filter, detail/evidence pages and last-read metadata. Late completions cannot
restore a previous target. First successful scope selection is bound for later
reads so allowlist reordering cannot silently move a task lookup. Cancellation,
permission refusal and exact project/task response checks remain fail-closed.

## Native chrome and keyboard

Both native roots use `WindowFrameStyle::None`. A separate drag region and three
centered SVG buttons invoke native minimize/maximize-restore/close. A bottom-right
grip initiates native resize. Windows snap flyouts/all eight resize edges are not
claimed. TUI omits native chrome and restores its terminal on Ctrl+Q.

Alt+1–5 pages, Ctrl/Cmd+F filter, +K task, +R read, +P project, +D light/dark,
+Shift+H contrast, +Shift+C explicit copy, Alt+arrows preference paging, Esc
blur/cancel/close auxiliary panel. Alt+0 appearance, Alt+A/B/C theme and Ctrl/Cmd+L
language are also available. Ctrl+1 remains untouched. Page/filter/task navigation
closes auxiliary panels before focusing stable inputs. No real OS clipboard test
is implied by the in-memory headless clipboard assertions.

See [identity](../brand/identity.md) and [actual local evidence](../evaluation/p1-f-local-ux.md).
