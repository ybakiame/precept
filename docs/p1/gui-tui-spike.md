# UI Spike: GPUI Kit vs NSH / TUI

> **Historical record — not a current runbook.** Original dates, source revisions
> and outcomes below are preserved. S4 retired the executable prototype and probes;
> use [Rust-only development](rust-only-development.md) for current commands.

Status: exploratory only. Do not select a renderer based only on README claims or screenshots.

Issue #14 implementation: [comparison and executed evidence](ui-spike-evaluation.md),
[isolated prototypes and commands](../../apps/README.md), and
[Proposed ADR-0003](../adr/0003-ui-framework-selection.md). Framework selection remains open.

## Upstream references

- GPUI (pre-1.0; APIs can change): https://github.com/zed-industries/zed/tree/main/crates/gpui
- GPUI Kit: https://github.com/longbridge/gpui-kit and https://gpui-kit.com/docs/
- NSH (Rust/TSX, native desktop, GPUI embedding and a TUI backend): https://github.com/longbridge/nsh and https://nsh.dev/docs/
- NSH TUI details and dependency versions must be pinned from a real checkout; do not assume NSH replaces a PTY session manager.

## Build exactly the same narrow UI in two throwaway apps

Use the **same mocked daemon/IPC fixtures** for both candidates:

1. Workspaces list with selected project and live HEAD/branch/dirty marker.
2. Effective Context Inspector: confirmed preference with source/scope, project contract references, empty/sensitive states and conflict indicators.
3. Handoff list/detail including prior validation statuses and stale fingerprint warning.
4. Consent/review **display only**. Do not create an actual preference confirmation write API before authorization is designed.
5. Keyboard-first interactions: project switch, filter, focus order, copy a redacted result, help/escape. Include thousands of events for virtualization/responsiveness testing.

## Evidence matrix

| Measurement | GPUI + GPUI Kit | NSH desktop + TUI/GPUI |
| --- | --- | --- |
| Linux build and runtime | test locally | test locally |
| Windows build and runtime | test locally/CI | test locally/CI |
| macOS build and runtime | test on machine/CI | test on machine/CI |
| Startup latency / idle memory | measure | measure |
| Incremental update and 10k rows | measure | measure |
| Keyboard, IME, screen reader | manual + automated where available | manual + automated where available |
| Error overlays, logs, hot reload | test | test |
| Packaging / signed builds | test | test |
| Terminal and PTY display semantics | document limits | document limits |
| Dependency stability / lockfile | pin snapshot | pin snapshot |

For unavailable machines, record **NOT_RUN** rather than inferring parity from published platform support. Upstream GPUI is evolving and NSH was recently introduced; track exact Git revisions rather than using floating dependencies.

## Decision rule

Prefer GPUI Kit if the main product is a rich desktop control console and it provides demonstrably better focus, tables, inspector layout and packaging. Prefer NSH when a **proven** shared GUI/TUI model reduces maintenance without losing input/accessibility functionality. A third outcome (GPUI desktop + a separate small Ratatui TUI) is acceptable if forcing a single UI abstraction costs more than it saves.

Keep the UI a presentation-only client. Daemon/Python P0 parity comes first; do not import storage code or direct database access into UI tests.

Deliver one comparative report containing versions, commands, tested targets, failures, screenshots (of synthetic data), measured results and a proposed ADR amendment.
