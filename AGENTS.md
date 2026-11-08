# AGENTS.md — Agent Control Plane

## Scope

Keep the product a local-first context and policy layer, not another Agent harness. Existing repository facts and accepted ADRs outrank stored preferences or handoffs. Never infer project truth from stale memory.

## P0 architectural boundary

- `store.py`: SQLite schema, review states and scoped persistence.
- `context.py`: live, narrowly allowlisted repository facts and git revision checks.
- `mcp.py`: read-only MCP stdio, never profile or task mutation.
- `cli.py`: explicit human/operator-authorized profile and task writes.
- No network listener, no subprocess shell execution, no automatic repository rewriting, no preference auto-confirmation.

Do not add GUI/PTY/ACP dependencies to the P0 core before a measured integration spike. Proposed GPUI, GPUI Kit, and NSH choices are tracked in docs/adr.

## Validation

Run `PYTHONPATH=src python -m unittest discover -s tests -v` after code changes. Add targeted regression tests for scope isolation, stale worktrees, malformed tool requests and permission changes. Check the final diff, links, secret handling, and scope. Document unverified client or device behavior honestly.

## Security and permission

Do not write personal profile data to tracked repository files. All persistent data lives in a user-local SQLite DB. No agent-facing MCP writes; all preferences start in suggested state until explicit confirmation. Log only sanitized failures. Never store credentials, raw agent conversations, or user device identifiers in test fixtures. A handoff does not confer approval to push, publish, deploy, or modify devices.

For future user-facing profile UI, provide view/confirm/reject/deprecate/delete and provenance inspection before adding automatic suggestions.
