# E4 operator console UX specification

Scope: one local-first, read-only NSH Rust Desktop/TUI client. ADR-0003 is Proposed;
mobile and production writes remain deferred. The owner reviews actual synthetic
windows before Draft PR finalization. Raw before/after captures stay private.

## Baseline observations and direction

E3 renders correctly on direct WSLg Wayland and Windows. Its controls occupy most
of the compact viewport: task/filter/paging are always shown, even on Diagnostics;
Dashboard repeats a full preference page, raw status prose competes with titles,
and selected project/navigation have little hierarchy. The 80×30 terminal spends
too many rows on chrome. These are observed layout problems, not usability scores.

Use a restrained slate/blue visual system, semantic surfaces and visible focus.
No decorative charts, fabricated history, raster assets or new framework dependency.

| Token | Desktop | Compact / terminal |
| --- | --- | --- |
| Gutters | 16 px | 8 px; terminal cell-aware rendering |
| Spacing | 4 / 8 / 12 / 16 px | 4 / 8 px |
| Navigation | 176 px sidebar, project + pages | compact wrapping page tabs + explicit project switch |
| Type | 22 px page title, 14 px body, 12 px metadata | 18 px title, 14 px body; native terminal metrics |
| Surfaces | background / panel / inset | same semantic colors; no translucent overlays |
| State | primary blue; warning amber; error red + text | same meaning with explicit words |
| Themes | light / dark / high contrast | all text, controls, border/focus tokens follow the selected mode |
| Focus | 2 px visible ring, fixed bounds | Tab order follows the visible controls |

```
wide:    [ACP / project / pages] [page title           Reconnect / appearance]
         [                    ] [source + last successful read / current state]
         [                    ] [page-specific input and content              ]
compact: [ACP · selected project            Reconnect / appearance]
         [Dashboard Projects Preferences Handoff Diagnostics]
         [read source / current state]
         [only this page's inputs and scrollable content]
```

## Page behavior

- Dashboard: compact daemon/selected-project/Git summary, confirmed count and
  bounded Handoff/resume warnings. Link to full pages. No filter, task editor or
  full 12-row preference table in this overview; no invented task history.
- Projects: allowlisted names only, selected scope and observed repository facts;
  switch clears exact task and old data. Unselected project details are not fetched.
- Preferences: visible scoped filter, 12 structured key/value/scope/source rows,
  numbered paging and explicit row detail/copy. Loading/error/empty have distinct
  explanations. Long values are bounded with a detail disclosure, never silently lost.
- Handoff: exact task editor and Lookup; objective/next action, current fingerprint
  vs recorded evidence, drift warnings and six-entry numbered evidence disclosure.
  Empty task is instructional; missing task is scoped Empty. No task enumeration.
- Diagnostics: current connection/auth state, historical last read, transport mode,
  safe runtime/backend limitations and reconnect advice. No tokens/private paths.

## State and interaction inventory

One framework-neutral model owns scope, query epochs, pagination/disclosure/theme
and last read. Page changes preserve the stable filter/task input entities while
showing only relevant editors. Requests run off the UI thread. Reconnect/project/
task changes supersede old requests; cancellation invalidates the epoch and hides
old content. Auth denial clears results. A previous read time never means online.

| State | Presentation / action |
| --- | --- |
| Loading | scope + query-in-progress, allow navigation/cancel; hide old results |
| Ready live | LIVE_READ_ONLY + last successful read, manually refreshable |
| Ready offline | OFFLINE_SYNTHETIC, no suggestion of a running daemon |
| Empty | exact page/scope reason and next meaningful action |
| Invalid task | inline length/control constraint, retain edited input |
| Disconnected | local daemon unavailable, explicit Reconnect; no autostart |
| Unauthorized | private operator access refused; no stale detail or token display |
| Unknown Git/fingerprint | explicit Unknown / unavailable; no clean/verified claim |

Keyboard: Alt+1–5 pages; Ctrl/Cmd+P next project; +R reconnect; +F opens Preferences
and focuses filter; +K opens Handoff and focuses exact task; +D light/dark;
+Shift+H contrast; +Shift+C explicit visible copy. Alt+Left/Right preference pages.
Tab/Shift+Tab follows visual order; Escape blurs editors/cancels an in-flight read;
Enter submits exact task. TUI Ctrl+Q/C restores terminal; plain q is text.
Pointer tests cover navigation, disclosure and paging as well as real resize.

## Acceptance checklist

- [ ] Before/after actual Windows and native WSLg synthetic views at 1080×760 and
  640×480; private PTY/ConPTY frames at 160×50 and 80×30.
- [ ] Compact content remains usable with less chrome and no horizontal clipping.
- [ ] Five pages, project/task/filter/paging/detail/copy, focus, CJK and all themes.
- [ ] Delayed/out-of-order requests, cancel, denial and reconnect cannot restore stale scope.
- [ ] Actual GdkWaylandDisplay, map/paint, OS input/resize, reconnect and 10k workload;
  >=3 startup/RSS samples per surface, separately defined timing observations.
- [ ] Root/parity/recovery, Rust fingerprint oracle and real Rust MCP security gates.
- [ ] Owner reviews actual local synthetic windows before Draft PR finalization.

niri, macOS/mobile, full IME and screen readers remain NOT_RUN unless actually
executed. No Actions, system configuration edits, user DB migration or real data upload.
