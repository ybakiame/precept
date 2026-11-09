# ADR-0003: compare read-only UI frontends before selecting a framework

Status: **Proposed**
Date: 2026-11-09
Authority: Issue #14; this document does not accept a framework or production rollout.

## Context

The existing Rust local service has an explicit authenticated operator interface
and project-bound MCP. A UI must stay a read-only client with an independent
lifetime. Browser runtimes cannot reach its Unix sockets/Windows named pipes.
Desktop density, accessibility and operational packaging need empirical evidence;
framework platform lists and a successful compiler invocation are insufficient.

## Proposal

Keep a framework-neutral typed presentation model and unchanged native IPC
semantics. Evaluate GPUI Kit, Rust-only NSH and Kit hosting an NSH AppView in
independent optional workspaces. Use the same disposable snapshot and workflows,
including errors/loading/unknown fingerprint, effective confirmed preference
provenance, handoff warnings and opaque recorded validation evidence. Native
clients receive explicit endpoint paths; offline web carries only synthetic data.
No UI writes, SQLite access, browser gateway, automatic service start or new
production authority is part of this trial.

Provisional recommendation: retain **NSH Rust as the cross-target candidate**,
with **GPUI Kit as the dense desktop benchmark**. Keep the hybrid as a feasible
fallback if NSH alone cannot meet desktop density/accessibility needs; do not
commit to its extra renderer/engine cost before focus and theme interoperation
are measured. This is an experiment ordering, not a final product framework.

## Decision weights and acceptance gates

Desktop usability/density/accessibility 30%; native IPC/security 25%; TUI/web/mobile
reuse 20%; packaging/build/maintenance/churn 15%; measured startup/RSS/bundle/UI
responsiveness 10%. Do not turn missing measurements into zeroes or publish an
aggregate winner. The [comparison](../p1/ui-spike-evaluation.md) lists evidence
and counterevidence for each dimension.

Before an Accepted decision: verify keyboard/focus/IME and screen readers on the
three actual desktop OSs, package and launch on clean machines, test identical
10k-row UI workloads (not just a model filter), compare isolated release builds
and repeated startup/RSS samples, validate simulator/device backends separately,
and resolve the browser's authenticated data design without weakening the daemon.
Hybrid additionally needs host/embedded focus traversal, theme synchronization,
resize/unmount and per-surface lifecycle tests. Owner review of those results is
required for a later framework decision; this PR remains Draft and unmerged.

## Counterevidence and consequences

NSH release 0.4.0 lacks TUI; this trial needs a newer exact development commit.
Its Linux backend brings GTK/GStreamer even with JS disabled. Rust changes rebuild
instead of TSX hot reload; TSX was not benchmarked and receives no cost/benefit
score. Kit has a real virtualized table, but its WASM docs label support
showcase-only and mobile uses a separate compatibility baseline. The hybrid
aligns GPUI Fast versions but increases binary/dependency burden and leaves
interaction boundaries to the host. Software-rendered Xvfb results do not establish
physical-GPU performance, hardware accessibility or universal OS support.
