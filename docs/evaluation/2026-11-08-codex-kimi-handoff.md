# Codex / Kimi Code — MCP context and Handoff E2E

- **Date:** 2026-11-08
- **Evidence source:** manually reported observations from two coding-agent sessions, not an independently instrumented or CI-driven harness run
- **Project:** Lanpath (`lanpath`)
- **Project HEAD:** `01c930b57db92ea61ff6bf6d9e615ccdc5d1ba61`
- **Branch:** `feat/gpui-desktop-validation`
- **Reported worktree:** dirty, change_count = 2
- **Harness A:** Kimi Code + Kimi K3.1 Pro
- **Harness B:** Codex + GPT-6.1 Sol
- **MCP:** project-scoped `acp-lanpath` stdio server
- **Scope:** read-only context + read-only task handoff retrieval; neither agent changed source files or ran builds

## Context-sharing trial

Both harnesses successfully invoked `get_effective_context(include_documents=true)` and independently reported the same project, HEAD, branch, dirty state, and two pending worktree entries. Both extracted materially consistent development constraints from the current project instructions; wording and emphasis differed.

Before preference confirmation, both reported an empty confirmed-preferences list. After an operator explicitly confirmed `interaction.language=zh-CN` (global; source: `manual-e2e-test`), both returned the confirmed value. The local `acpctl context lanpath` output agreed. This establishes read consistency for the sampled sessions, **not** autonomous preference adherence.

## Handoff recovery trial

For `task_id=acp-e2e-001`, both harnesses successfully invoked `get_handoff` and agreed on:

- Task objective: test cross-harness recovery.
- Previous steps: MCP context retrieval and preference propagation completed.
- Saved HEAD and branch matched the current checkout.
- Both saved and current worktree were dirty, with two reported changes.
- Historical `MCP context sharing` check was PASS; `Android device validation` was NOT_RUN.
- Neither treated historical PASS as proof that the current worktree, platform or device was verified.
- Both recommended read-only inspection of the actual changed files, without resetting or overwriting edits.

**Finding:** `HEAD + branch + dirty + change_count` cannot establish that the **contents** of two uncommitted changes are identical. A distinct snapshot/fingerprint is needed to detect some kinds of drift.

## Outcome and limitations

**Observed PASS:** two-harness context retrieval consistency; explicit confirmed-preference read-through; read-only Handoff interpretation; accurate validation-scope and dirty-worktree caveats.

**Not yet established:** resumed code editing and test execution, negative tests for unconfirmed or rejected preferences in real harnesses, cross-project isolation against real clients, adversarial security tests, older/newer MCP protocol compatibility matrix, platform/device validation, sustained reliability, or harness-induced behavior changes.

This record is historical evidence **scoped to the stated revision and sessions**. It is not current project certification. The follow-up implementation adds an optional bounded worktree fingerprint for *newly created* handoffs. Older records have no fingerprint, are not retroactively verified, and remain readable.
