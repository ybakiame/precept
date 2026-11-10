# P1-G local agent-session evidence

Status: implementation and targeted local validation in progress; final clean-host
matrix is not yet claimed. Base: clean latest main merge #23,
`52c01b7b6c5b48a2884e77bcd8a8893a5a010e62`; no submodules. Branch
`feat/p1-g-acp-session-runtime`. G0 contract was committed before runtime code.

The owner explicitly authorized an isolated Kimi ACP smoke: “允许隔离 smoke；缺少
认证则记录限制”. Installed Kimi Code CLI 2.5.2 documents `kimi acp`; installed
Codex 0.167.0 and Claude Code 2.1.310 are implementation/CLI inventory, not assumed
ACP servers. Native Windows candidate commands were unavailable in the test process.

Initial actual Kimi run, with temporary HOME and a disposable synthetic project:
ACP v1 initialize PASS; session/new returned AUTH_REQUIRED; prompt and remote model
turn NOT_RUN. This is **partial real interoperability**, not a successful coding
agent prompt. The test did not copy/supply real login configuration, run login or
install an adapter. No fake-agent success is labeled real interoperability.

The deterministic fixture is a real Rust child process, explicitly named
`deterministic-fake-not-coding-agent`. It exercises protocol/lifecycle/permissions,
not model quality. Shared model and headless tests cover scope/epoch/version and
refusal boundaries. The actual WSLg window has observed GdkWaylandDisplay and
rendered the new controls. Final Windows/Wayland/PTY/ConPTY results follow after
clean same-source local verification; failed development attempts remain failures.

Actual development findings: successful idle IPC needed a `{session:null}` envelope
rather than a null result; UI polling initially toggled control disabled state and
caused focus churn/backpressure. Passive in-flight tracking is now separate from
reactive action state; unchanged observations do not repaint. Permission controls
use stable computed projections; explicit start confirmation survives passive polls.
The terminal probe drains pending output while closing, then still requires clean
exit/restoration. Earlier pointer/keyboard assumptions are not acceptance evidence.

Raw synthetic captures, terminal frames, logs, unsigned local builds and measured
host details stay ignored under `artifacts/local-p1-g/`. No real ACP user DB or
external checkout is opened. No production SQLite migration/profile mutation,
Windows→WSL execution bridge, shell command interpolation, network listener,
compositor restart, system configuration change or automatic permission approval.
MCP remains exactly two project-bound read-only tools. ADR-0003 stays Proposed.

Owner review of the new Sessions surface is PENDING; prior P1-F base acceptance is
not claimed as session-control approval. Draft PR only; no automatic merge.
Niri/mobile/macOS, full IME/screen reader and real OS clipboard are NOT_RUN.
See [protocol, configuration and trust boundary](../p1/agent-session-runtime.md).
