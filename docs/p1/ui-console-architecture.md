# NSH console architecture (E4 / S3 operator extension)

The implemented NSH Rust console lives at `apps/operator`, one Cargo workspace
outside the root Rust workspace with `model` and `ui` members.
[ADR-0003](../adr/0003-ui-framework-selection.md) now **accepts NSH**
as the product Desktop/TUI direction; behavior below is unchanged. Source paths
are relative to `apps/operator` (`model` was `shared`, `ui` was `nsh`).
[Product IA](../product-ui.md) describes later workflows, not implemented pages.
Mobile and production writes remain deferred.

## Ownership and pages

`model::Model` owns the typed snapshot, selected project, exact queried task,
filter, 12-row page, five sections, disclosure/evidence page, theme, request epoch
and last successful live read time. Snapshots use `Arc`; framework signals do not
clone a 10,000-row dataset on each read. `model::presentation` formats the same
bounded display content for Desktop/TUI and the optional pinned benchmark.
`ui/src/components.rs` builds stable inputs/navigation around reactive page bodies.
There is no second desktop/TUI business model.

| Page | Content |
| --- | --- |
| Dashboard | Observed repository facts, confirmed preference count, exact-task summary and warnings |
| Projects | Daemon-allowlisted navigation; no repository discovery or task enumeration |
| Preferences | Confirmed effective rows with scope/source, page-specific filter, 12-row pagination and bounded detail/copy |
| Handoff | Exact-task objective, next action, current fingerprint or explicit unavailable state and resume warnings; historical evidence disclosure |
| Diagnostics | Read-only transport, observed GTK/runtime facts and bounded contract metadata disclosure |

Evidence/contracts are collapsed by default and paged six entries at a time;
visible content is bounded to 48 lines, 512 characters per line, with explicit
truncation labels. Controls and bidi embedding/isolate characters are removed
from display/copy. Copy requires an explicit action and copies only that visible
sanitized page. No token or clipboard inspection is part of these tests.

## Asynchronous reads and failure

Every request receives a monotonically increasing epoch plus selected project and
exact task. Starting a live query hides old details immediately. Project navigation
remains available during loading through the last validated allowlisted names.
Project switches clear the task. A completion applies only if its epoch/project/
queried-task still match; stale successes and stale failures are ignored. Schema,
ready state, project scope, task response and confirmed-only rows are validated
before accepting success. Refused reads clear details and project navigation.
The last successful read time is retained as historical information after failure;
it does not claim a continuously healthy connection.

Blocking IPC and Windows `wsl.exe --exec` work runs through NSH's blocking worker,
never the UI thread. Existing bounded response size, helper deadline, operator
capability, UID/Named Pipe checks and project-bound two-tool MCP remain unchanged.
UI closure never starts/stops the standalone daemon. S3 adds the separate native
operator workflow below; normal context snapshots and the Windows→WSL helper stay
read-only. No default user DB is opened, and clients never use a retired runtime or direct SQLite as a
fallback.

## Keyboard and layout

A 176px sidebar becomes wrapping page tabs at compact widths and in terminals.
Only the current page shows its task/filter/paging controls. The content pane
scrolls independently. Semantic palette tokens cover light/dark/high contrast and
focus/hover/disabled/connection states. The same
layout is exercised at GTK window allocations 1080×760 and 640×480, and terminals
160×50 and 80×30. Inputs remain mounted when page content changes. Buttons and
inputs have visible focus borders and labels. Dark/light and a high contrast
black/white content mode are explicit local state; no theme preference persists.
This is not a screen-reader or full WCAG certification.

- Alt+1–5 selects pages (also while editing); Ctrl/Cmd+P selects the next project.
- +R reconnects; +K focuses exact task lookup; +F focuses preferences filter.
- +D changes light/dark; +Shift+H toggles high contrast.
- +Shift+C copies the visible page; Alt+Left/Right changes preference pages.
- Tab/Shift+Tab follows native focus; Escape blurs inputs; Enter submits task lookup.
- TUI Ctrl+Q/C closes; plain `q` remains editable. Bracketed paste is enabled and
  disabled on exit, with raw/alternate screen restoration. CJK paste and terminal
  resize are tested; full IME composition and assistive technology remain NOT_RUN.

## Probe boundary

`ACP_UI_PROBE=1` emits content-free phase/count/section/timing diagnostics. Linux
reads the actual GTK display type, renderer type, mapping/allocation and frame
clock; environment variables alone are never backend proof. An optional validated
random 16-hex `ACP_UI_PROBE_ID` identifies only the owned synthetic test window.
Normal TUI probes remove the marker environment so diagnostics cannot corrupt the
terminal. No probe adds a listener, shell, daemon write or agent-facing tool.

See [Wayland troubleshooting](wslg-wayland-troubleshooting.md),
[local commands](local-codex-e2.md) and [E4 evidence](../evaluation/p1-e4-local-ux.md) and [UX specification](ui-ux-spec.md).


## P1-G native Sessions auxiliary surface

The reviewed five context pages remain intact. An auxiliary Sessions panel uses the
same model/tokens/catalog and stable prompt input, with observed daemon state and
explicit operator controls. It requires a native local target; the WSL helper stays
read-only. Session reads/actions use independent scoped completions, ephemeral events
and explicit start review. See [runtime contract](agent-session-runtime.md). Context
copy excludes session prompts/events. Full IME/screen-reader behavior remains unverified.


## P1-M2/S3 operator panel

Connections → Profile operator, or Alt+O, opens one shared auxiliary panel on
Desktop/TUI. Profile, Preference records and Saved Handoff tabs preserve the five
context pages. The panel reads Profile state independently of context loading:
an unregistered project/context failure does not falsely imply daemon disconnection.
Last operator response, current observed grant, waiting acknowledgement, failure,
last acknowledged receipt and uncertain operation identity are separate states.

`model::operator` owns exact scoped records/revisions, binding, prepared review,
immutable pending mutation and request guards. `ui::operator` supplies retained
inputs, ordinary semantic buttons, escaped full review content and keyboard entry
points. Target/project/task/request/scope changes and changed exact records invalidate
review/completions. Loading and uncertain outcome prevent duplicate/new submissions;
uncertainty retains one exact same-epoch request for an explicit retry. Scope changes
never rebind that retry. Ending retry clears client blocking state without claiming
that the original operation failed or rolling it back.

Explicit Profile create/select and grant enable/disable, suggestion and exact
suggested-record confirm/reject use authenticated native IPC. Confirmation displays
ID, scope, key/value/source/status/revision and warns about atomic peer deprecation.
Disable is acknowledged only after daemon drain. Registration, deprecate/delete and
Handoff put are documented CLI-only gates; exact task/revision history is inspectable
without pretending it is current Git or verification evidence. No fake pending queue,
automatic approval, full ReviewDecision engine, repository editing or startup grant.

Alt+Shift U/M/R/E/X select create/select/inspect/enable/disable; L/I/Y/N/S select
records/record/confirm/reject/suggest; V applies the exact prepared review and Z
retries the retained uncertain request. Alt P/K/W/F/T focus Profile path/key/value/
source/exact task. Existing page and Sessions shortcuts remain unchanged. Escape
cancels review and returns to ordinary navigation; hidden inputs blur. The connection
panel keeps an explicit entry point without expanding the existing compact sidebar.

See the [S3 contract and CLI usage](native-operator-workflows.md) and
[platform-specific evidence](../evaluation/p1-m2-s3-native-operator.md).
