# P1-M2/S4 — Python retirement inventory and acceptance contract

Frozen before deletions for [Issue #48](https://github.com/ybakiame/agent-control-plane/issues/48), parent [#40](https://github.com/ybakiame/agent-control-plane/issues/40).
Base: latest merged main `58750f346bf2288d79f9ff29754ca110ebca52d0` (PR #47).
Branch: `feat/p1-m2-s4-retire-python`. Authority: accepted ADR-0006/0004 and retained daemon ownership in ADR-0005.

This is a breaking retirement at validation stage, not a migration, new Schema,
production activation or final M2 acceptance. Profile v1 exact DDL, 4096 receipts,
CAS, human review, lease/epoch/revocation and two-tool read-only MCP stay unchanged.
No real profiles, private auth/runtime directories or external checkouts may be
read, reset or deleted. Only tracked obsolete files are removed; ignored historical
artifacts are not interpreted as current tools or bulk-cleaned.

## Exact tracked Python inventory (64 files)

All tracked Python executables are retired. No maintenance exception or parallel
Python acceptance harness is retained. Removed tool tests that only guard the
removed vendor/Python driver have no live product claim to port.

### Retired product package (5)

Delete; native daemon/query/MCP replace the product.

- `src/agent_control_plane/__init__.py`
- `src/agent_control_plane/cli.py`
- `src/agent_control_plane/context.py`
- `src/agent_control_plane/mcp.py`
- `src/agent_control_plane/store.py`

### Retired product/orchestration tests (8)

Delete; preserve independent current Rust coverage below, not Python parity or vendor-probe self-tests.

- `tests/test_cli.py`
- `tests/test_interop_runtime_guards.py`
- `tests/test_interop_trace.py`
- `tests/test_local_validation.py`
- `tests/test_mcp.py`
- `tests/test_p1_reference_contract.py`
- `tests/test_real_context_optin.py`
- `tests/test_store.py`

### Obsolete parity/migration/process fixture chain (9)

Delete; strict v1 Rust refusal, ownership/process and core/store tests replace live claims.

- `tools/check_caption_alignment.py`
- `tools/check_migration_compat.py`
- `tools/check_p1_parity.py`
- `tools/check_recovered_snapshot.py`
- `tools/check_rust_migration_mutations.py`
- `tools/check_sqlite_parity.py`
- `tools/run_local_service_ci.py`
- `tools/run_process_recovery_ci.py`
- `tools/sqlite_fixture.py`

### Historical vendor/interop experiments (6)

Delete executable probes; keep original Markdown evidence. No model/provider calls in S4.

- `tools/codex_mcp_smoke.py`
- `tools/evaluate_cross_agent.py`
- `tools/interop_trace.py`
- `tools/test_real_acp_runner.py`
- `tools/test_real_acp_smoke.py`
- `tools/test_real_context_smoke.py`

### Legacy UI/native/terminal/WSLg/build/package probes (36)

Delete; retain current NSH shared/component tests, replace required Web packaging/server with Node and local Cargo helpers. Historical physical-window/TTY evidence remains scoped to its old SHA.

- `tools/build_ui_spike.py`
- `tools/capture_brand_previews.py`
- `tools/capture_console_synthetic.py`
- `tools/capture_product_previews.py`
- `tools/gtk_wayland_sample.py`
- `tools/local_console_probe.py`
- `tools/local_verify.py`
- `tools/measure_ui_spike_build.py`
- `tools/native_window_input.py`
- `tools/package_local_console.py`
- `tools/package_ui_spike_web.py`
- `tools/terminal_screen.py`
- `tools/test_agent_session.py`
- `tools/test_agent_session_ui.py`
- `tools/test_brand_chrome.py`
- `tools/test_brand_settings.py`
- `tools/test_connection_switch.py`
- `tools/test_console_integrations.py`
- `tools/test_local_console_native.py`
- `tools/test_local_console_tui.py`
- `tools/test_local_project.py`
- `tools/test_local_service.py`
- `tools/test_session_context.py`
- `tools/test_terminal_screen.py`
- `tools/test_ui_spike_contract.py`
- `tools/test_ui_spike_native.py`
- `tools/test_ui_spike_tui.py`
- `tools/test_wayland_console.py`
- `tools/test_windows_wsl.py`
- `tools/wayland_probe.py`
- `tools/window_capture.py`
- `tools/window_chrome_probe.py`
- `tools/windows_wsl_driver.py`
- `tools/windows_wsl_views.py`
- `tools/wslg_doctor.py`
- `tools/wslg_window.py`

## Other tracked paths and replacements

| Existing path/reference | Action |
| --- | --- |
| `pyproject.toml`, `acpctl`, Python install/PYTHONPATH entrypoints | Delete packaging; current executable names are acp-daemon/acp-query/acp-mcp. No wrapper/alias. |
| `tests/fixtures/p1-preference-contract.v1.json` | Preserve its data unchanged, relocate into Rust core test fixtures and update the two Rust include paths. This is logical resolver data, not a legacy DB Schema. |
| `crates/acp-core/examples/contract_reader.rs`, `crates/acp-store/examples/sqlite_reader.rs` | Delete obsolete external parity adapters; retain independent Rust domain/store tests. |
| `crates/acp-daemon/src/worktree.rs` Python oracle subprocess | Replace with fixed v1 digest vectors plus native Git behavioral/security assertions; do not change Fingerprint bytes or bounds. |
| `.github/workflows/ci.yml` | Remove standalone Python package job. |
| `.github/workflows/rust.yml` | Cargo build/fmt/Clippy/all-target tests on native OS matrix; remove Python parity/process runners and nonexistent acp-rehearsal target. |
| `.github/workflows/ui-spike.yml` | Accepted NSH/shared native/component gates; optional existing Node/Playwright offline Web, no historical GPUI/hybrid/Xvfb matrix or Python. Hosted native GUI/TTY is not claimed. |
| `tools/windows_local_verify.ps1` | Replace Python forwarding with direct Cargo gates and optional process-local MSVC setup; no installations, default Profile or agent/model launch. |
| Linux developer entrypoint | Small direct Cargo shell helper plus documented exact commands, no Python/PowerShell requirement. |
| `spikes/p1-e/tests/web-smoke.mjs` | Replace Python HTTP subprocess with loopback static Node test server; keep external-request denial and offline/keyboard/resize assertions on current NSH selectors. |
| Web asset packaging | Small Node script, pinned upstream renderer, fail if wrong revision/missing assets; no daemon gateway or credentials. |
| Active README/AGENTS/interfaces/security/product/architecture/roadmap/spike README | Rewrite current commands/ownership/versions and link Rust-only runbook. No legacy install or migration step. |
| Old docs/p1 runbooks, old evaluation reports and comparison diagrams | Mark archival, preserve dated SHAs/outcomes and explanatory Python quotations. Deleted script references become historical code text, not broken live links. Current contracts stay current. |
| Root/spike gitignore | Remove obsolete Python/package patterns; keep private ACP artifacts/data protections. Do not clean ignored files. |

## Live security/acceptance replacement map

Deletion alone is not parity evidence. Existing tests remain enabled; GAP rows are
merge gates to implement in S4, then report their actual results.

| Former claim/tool | Independent live replacement / required work |
| --- | --- |
| P0 preference ordering/status/project isolation and bounded documents | `acp-core/tests/read_only.rs`, `typed_context.rs`; `acp-store/tests/sqlite.rs`; retained logical fixture |
| P0 review/delete/Handoff writes and no data loss | `acp-write` transaction tests; `acp-daemon/tests/ownership.rs::native_cli_lifecycle_seven_commands_preview_cas_and_exact_retry`; complete logical-state equality on denial |
| Legacy Schema/parity/migration | Delete compatibility expectations; preserve strict Python-v1/M1-v2 refusal in Rust store/daemon fixtures, no loader |
| Git drift, binary/rename/unicode/untracked modes/limits/no external helper execution | GAP: retire runtime Python oracle while retaining native Git cases and independent fixed Fingerprint v1 vectors, symlink containment and bounds |
| Service/CLI/MCP real subprocesses and cross-project/malformed/tool denials | `acp-daemon/tests/ownership.rs`, `acp-mcp/tests/security_process.rs`, `acp-ipc` native framing/role tests; no Python oracle |
| Lease contention, crash/restart, alias/file identity, write revoke/drain/CAS/receipts | Existing independent ownership groups plus deterministic `ownership_tests.rs`; `acp-ipc` lost-response test |
| Established session revoke/expiry/publication and two tools | `acp-ipc::session_lease` tests + real fake-agent/MCP ownership group; GAP: re-home essential legacy session lifecycle/permission/protocol/auth/root-drift negative scenarios into Rust real-process tests |
| NSH target/project/task/stale success+failure/review invalidation/native-only writes | `spikes/p1-e/shared` 32 tests and NSH real-daemon component test; retain 10k bounded mounting |
| Historical native HWND/Wayland/PTY/ConPTY input/capture | Remove Python drivers. No replacement physical interaction claim: S4 local-only rerun only if an independent native driver is available; otherwise NOT_RUN with S3 original evidence linked. Not a headless PASS. |
| Offline Web no network/keyboard/resize | GAP: Node static-server/packaging replacement and current-selector Playwright smoke; rerun if retained |
| Vendor/model opt-in probe guards | Retire probes and tests together; no active model probe to approve or compatibility promise to preserve; native session permissions remain independently tested |
| Source/CI/helper integrity | GAP: executable scan for zero tracked Python/packaging, obsolete targets and broken active links/commands; classify historical quotations explicitly |

## Acceptance and delivery

At one clean exact product/tooling SHA on Windows and Linux/WSL: workspace
fmt/build/Clippy -D warnings/all-target tests; shared fmt/Clippy/native tests; NSH
fmt/build/Clippy/all-target native,tui tests; real process security groups above.
Use fresh temporary synthetic roots only. Local helpers must propagate failed gates
and never auto-create a selected Profile, enable writes, run a real agent or dispatch
Actions. Revalidate supported Web separately with Node/Playwright/WASM; distinguish
its offline loopback asset server from a product network listener.

Check CI YAML, extant script/manifest references and all active docs. Report exact
source, commands, platforms and PASS/FAIL/BLOCKED/NOT_RUN; raw logs stay ignored.
Full gates run once after substantive changes, repeat only affected checks if needed.
No hosted Actions, model/vendor, macOS, mobile or niri claims.

Submit one Draft PR, Closes #48 / Refs #40, all commits [skip ci]. Do not merge or
close #40. S5 final integrated M2 acceptance is a separate issue after S4 merges.

Implementation closure: [S4 evidence](../evaluation/p1-m2-s4-retire-python.md)
records the exact Rust replacements, clean native source/results, source/CI scans,
offline Web verification and actual GUI/TTY NOT_RUN boundary. The inventory and
replacement plan above retain their pre-deletion scope; they are not current
instructions to run any retired path.
