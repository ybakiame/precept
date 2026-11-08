# ADR-0001: Core before UI (GPUI Kit / NSH under evaluation)

- Status: Proposed
- Date: 2026-11-08

## Context

The target product is a developer-centric Agent Control Plane: shared preferences, current project contract resolution, reproducible task handoffs and cross-harness visibility. GPUI + GPUI Kit provides Rust desktop GUI primitives; `longbridge/nsh` exposes a native UI model including a TUI backend and integration with GPUI. Their integration, packaging and accessibility need measurements across relevant OSs rather than assuming either can transparently replace a traditional terminal multiplexer.

## Decision proposal

1. Implement and test the context/permission domain independent of any UI toolkit.
2. Use Python 3.11+ stdlib as a **temporary P0 reference implementation** where Rust toolchain is unavailable; migrate to a Rust context core after locking the semantics and tests.
3. Keep the long-lived Daemon separate from GUI lifetime; use MCP as the agent-facing context API and ACP or bounded CLI/PTY adapters for session control.
4. Spike GPUI + GPUI Kit versus NSH's GPUI/TUI backend before selecting a GUI/TUI implementation. Do not make either a P0 dependency.

## Tradeoffs

Python accelerates protocol verification but adds migration work. NSH could reuse UI code across terminal/desktop but is a moving dependency; direct GPUI Kit may yield better desktop control with more TUI-specific work. Terminal I/O, accessibility, Windows/Linux support, packaging and user input permissions need empirical checks.

## Acceptance for a GUI decision

- A minimal project list/context inspector uses the same domain API as CLI and MCP.
- Keyboard focus, large outputs, screen reader behavior and reduced-motion are explicitly tested.
- A closed GUI does not terminate agent sessions owned by Daemon.
- Test macOS, Windows and Linux target support; record errors and versions rather than claiming parity from documentation.

Reference upstream:
- https://github.com/longbridge/nsh
- https://github.com/longbridge/gpui-kit
