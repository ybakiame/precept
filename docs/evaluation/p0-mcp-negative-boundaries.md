# P0 negative tests: unconfirmed preferences and project isolation

**Historical P0 record:** the seven Python checks below were superseded in E4 by
Rust core + actual daemon/IPC/MCP subprocess tests. See the [migration matrix](../p1/rust-test-migration.md)
and [local E4 evidence](p1-e4-local-ux.md). The old module was removed only after
Windows/WSL parity; Python runtime compatibility tests remain. No Actions run is
part of E4. The original P0 trial scope below is preserved for provenance.

**Scope:** Local P0 application-level MCP behavior. This is not a same-user filesystem sandbox or authorization boundary for a coding agent that can use shell/file tools directly.

## Automated regression

From the Agent Control Plane repository:

```bash
python -m pip install --no-deps -e .
python -m unittest discover -s tests -v
```

The historical `tests/test_security_boundaries.py` created two **temporary Git repositories and a temporary SQLite state directory**. It never writes to a real Lanpath checkout or the operator's existing preference database.

The seven negative checks cover:
1. Only confirmed preferences are exposed; `suggested`, `rejected` and `deprecated` values are absent.
2. Project-level preferences override globals by key, while another project's preferences/documents are excluded.
3. `get_effective_context` and `get_handoff` reject injected `project` / `project_id` parameters.
4. Equal Handoff `task_id` values in two projects cannot be confused.
5. A task that exists only in another project is inaccessible.
6. Unregistered MCP project scopes and unsupported write tools are rejected.
7. A real local stdio subprocess returns the correct project content and no unrelated project or pending-preference markers.

## Optional real-harness validation

After the automated tests pass, optionally repeat against Codex and Kimi Code using a **separate test database and test projects**, not the actual Lanpath profile or its dirty worktree.

1. Create two temporary directories, `alpha` and `beta`, each with a distinct `AGENTS.md`. Initialize both as Git repositories.
2. Choose one temporary state path. Prefix the following commands with `AGENT_CP_HOME=/absolute/path/to/temp-state` so no production profile is modified.
3. Register `alpha` and `beta` using `acpctl project add`. Suggest one unique preference for each project and one pending global preference. Confirm only project-specific preferences.
4. Configure the MCP process named `acp-negative-alpha` with command `acpctl`, arguments `mcp --project alpha` and environment `AGENT_CP_HOME=/absolute/path/to/temp-state`; restart each harness so it actually loads that process.
5. Ask each harness to call **only** `acp-negative-alpha.get_effective_context(include_documents=true)` and report the project ID, returned document marker, and confirmed preferences.
6. Attempt to call `get_effective_context` with an additional `project=beta` argument. A compliant tool call should return a tool error; a client may reject the extra parameter before the server sees it.
7. Ask for a beta-only Handoff ID via `acp-negative-alpha.get_handoff`. It must return a not-found error. A same-named alpha task, if present, must yield alpha's task only.
8. Confirm that **no beta-only marker or pending/rejected preference marker appears in the MCP tool response**.

**Expected:** The alpha-scoped server returns only alpha-specific and confirmed global context. It never accepts a caller-selected project or a write operation. An agent's free-form text is not proof of the raw tool response; retain sanitized tool traces where supported.

**Do not overclaim:** A model might know beta data from its conversation, host filesystem, another MCP server or prior session. This test does not prove the agent cannot access beta in general; it tests only what the **alpha-scoped MCP tool returns**.

## Evidence and acceptance

Record harness versions, configured command/environment, actual MCP tool call/response, exact expected markers, and check status (`PASS`, `FAIL`, `BLOCKED`, `NOT_RUN`). Do not copy raw private documents or personal SQLite databases into the repository. Mark independently unobserved steps as unverified.

**Go/no-go for P1:** Automated negative tests and at least one real-harness isolation trial should pass before introducing a GUI or unattended context injection. Full OS-level process isolation and adversarial security remain separate design work.
