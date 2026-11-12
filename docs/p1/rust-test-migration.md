# E4 Python → Rust authority migration
> **Historical evidence only — Python runtime retirement accepted by [ADR-0006](../adr/0006-rust-only-runtime.md).** This matrix records the original E4 migration decision and test provenance at its stated revision. It is no longer a requirement to retain Python runtime, Python tests, Python/Rust parity, or legacy Python schema compatibility. P1-M2 removes the Python product/test paths and establishes a fresh Rust-native Profile v1; the legacy Python v1 and M1 synthetic v2 are separate historical formats, not migration sources. Use [Issue #40](https://github.com/ybakiame/agent-control-plane/issues/40) and ADR-0006 for current acceptance.


The baseline is main `80fdbc57e4ec47557e9130808b401a22ad6b913f`.
The Python implementations from historical PRs #1/#2 are already merged. Their
branches are not cherry-picked. P0 Python runtime/write compatibility stays supported;
Rust remains a read-only daemon/IPC/MCP client of the existing database.

## Replacement matrix

`process::` below means a real Rust test in
`crates/acp-mcp/tests/security_process.rs`, spawning actual daemon and MCP binaries,
using authenticated IPC and a fresh synthetic database. Python constructs only the
disposable v0 fixture; it does not implement these Rust assertions. `core::` means
`crates/acp-core/tests/read_only.rs`; `git::` means the daemon worktree adapter tests.

| Old Python case | Rust authority and independent evidence | Decision |
| --- | --- | --- |
| `test_only_confirmed_is_visible_to_mcp` | `process::confirmed_only_statuses_and_complete_operator_parity`; `core::shared_v1_preferences_documents_and_forbidden_markers`. Complete operator/MCP JSON equality; suggested/rejected/deprecated markers absent. | Remove duplicate Python security test. |
| `test_project_preferences_and_docs_are_isolated` | `process::global_override_and_documents_never_cross_scope`; the same core fixture test. Both project scopes, global override and `include_documents` content isolation. | Remove. |
| `test_project_selector_injection_is_rejected` | `process::project_selector_injection_and_malformed_arguments_are_refused`; `core::shared_v1_denials_and_malformed_arguments`. Both tools reject injected selectors and malformed JSON argument shapes; error markers excluded. | Remove. |
| `test_same_task_id_cannot_cross_project` | `process::same_task_id_and_foreign_only_tasks_are_project_bound`; `core::handoff_checks_both_project_and_task_even_with_faulty_adapter`. Exact task/project, full JSON/evidence comparison. | Remove. |
| `test_other_project_unique_handoff_is_not_found` | The same Rust process task test checks foreign-only task denial and no foreign content. | Remove. |
| `test_unregistered_project_and_mutation_tools_rejected` | `process::unregistered_unknown_and_mutating_tools_have_no_authority`; core malformed/unknown-tool tests. No new tool or write authority. | Remove. |
| `test_real_stdio_subprocess_scoped_and_read_only` | All seven process tests use actual MCP stdio; `process::real_stdio_two_schema_versions_and_notifications_remain_compatible` and `process::bound_ipc_agents_cannot_list_or_escalate_after_restart_and_rotation` add two-tool schema, 2025 negotiation/stateless 2026, notification, direct agent escalation and old/new credentials. | Remove superseded module. |
| `test_handoff_detects_dirty_content_change_without_head_change` | `git::v1_oracle_untracked_same_name_count_changed_bytes_and_modes`; `test_local_service.LocalServiceTests.test_live_git_exact_fingerprint_handoff_oracle_and_warning_parity`. Actual Python digest oracle, unchanged HEAD/count but different bytes, complete operator/MCP warning/evidence equality. | Remove duplicate Store assertion. |
| `test_handoff_detects_tracked_content_change` | `git::v1_oracle_clean_staged_unstaged_binary_rename_unicode_and_head_branch`; the same live service test. Staged/unstaged/binary/rename and full Handoff warning/evidence equality. | Remove duplicate Store assertion. |
| `test_sqlite_migration_adds_nullable_fingerprint` | Rust deliberately has no production write ownership. | **Retain** Python legacy initialization + idempotent schema upgrade proof. |
| `test_handoff_verifies_head_and_worktree` | Rust warning tests also cover revision drift; Python CLI/Store is still supported. | **Retain** minimal Python save/read/HEAD-drift compatibility smoke. |

The seven process cases and existing fingerprint oracle cases passed on actual local
WSL/Linux and native Windows before removal. The added Windows symlink oracle found
an absolute-link target prefix discrepancy; it was corrected and that oracle passed
before final validation. Unix additionally tests raw non-UTF8 names/link targets;
Windows additionally tests absolute/relative symlinks, executable modes and unsafe
directory links on this host, without enabling system privileges/settings.

`core::fingerprint_is_requested_only_after_valid_handoff_scope_check` verifies that
context/document reads and invalid Handoff scope never request a content fingerprint.
The adapter's overflow/deadline/stderr test checks bounded Git process handling;
configured external filters are refused without executing them. See the
[byte contract and limitations](worktree-fingerprint-v1.md).

## Negative verification and retained coverage

`tools/check_rust_migration_mutations.py` uses a disposable source copy and private
Cargo target. Three deliberately compiled mutations must fail their specific tests:
unconditional-null fingerprint, unconfirmed preference exposure, and agent project
listing. Compiler failure never counts as a rejected mutant. The command never
edits the live source, external checkout, user database or system configuration.

The remaining **16 Python tests** are: CLI/operator lifecycle + real Python stdio
(1), Python MCP protocol compatibility (3), reference fixtures (2), Store write/review,
document confinement and legacy migration/HEAD compatibility (6), local runner/config-refusal
classification regressions (4). No Python runtime source is deleted. Baseline
**120 JSON + 78 SQLite** comparisons, recovery checks and **11** service process tests
remain. Exact clean-source counts and host results are recorded in the
[E4 evaluation](../evaluation/p1-e4-local-ux.md).
