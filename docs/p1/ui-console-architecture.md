# NSH read-only console architecture (E4)

This is an optional measured Rust UI spike, outside the P0 Python package and
root Rust workspace. [ADR-0003](../adr/0003-ui-framework-selection.md) remains
**Proposed**. Mobile and production writes remain deferred.

## Ownership and pages

`shared::Model` owns the typed snapshot, selected project, exact queried task,
filter, 12-row page, five sections, disclosure/evidence page, theme, request epoch
and last successful live read time. Snapshots use `Arc`; framework signals do not
clone a 10,000-row dataset on each read. `shared::presentation` formats the same
bounded display content for Desktop/TUI and the optional pinned benchmark.
`nsh/src/components.rs` builds stable inputs/navigation around reactive page bodies.
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
UI closure never starts/stops the standalone daemon. Only the explicit Python
operator CLI writes disposable fixtures. No default user DB is opened.

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
