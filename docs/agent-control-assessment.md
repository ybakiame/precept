# agent-control design-reference adoption assessment

Reviewed 2026-11-11 for [Issue #30](https://github.com/ybakiame/agent-control-plane/issues/30).
Sibling default branch is **master**, inspected commit
[`f54be806d280b173ca6bac513134fc0e9a0f2b18`](https://github.com/ybakiame/agent-control/commit/f54be806d280b173ca6bac513134fc0e9a0f2b18).
It is a design reference only. No source, assets, crate layout, dependencies or
permissions are imported; its README claims are not validation of this product.

| Idea / inspected source | Decision | Application and reason |
| --- | --- | --- |
| Personal preferences vs project contracts: [README](https://github.com/ybakiame/agent-control/blob/f54be806d280b173ca6bac513134fc0e9a0f2b18/README.md#the-four-layers--四层架构), [model](https://github.com/ybakiame/agent-control/blob/f54be806d280b173ca6bac513134fc0e9a0f2b18/crates/core/src/model.rs) | Adopt | Distinct ownership and meaning, including project-scoped personal choices; neither becomes operator authority |
| Project-first onboarding and visible current project: [workflow](https://github.com/ybakiame/agent-control/blob/f54be806d280b173ca6bac513134fc0e9a0f2b18/README.md#projects--onboarding--项目接入) | Adapt | Explicit checkout selection and preview; no automatic `.agent-control` scaffolding, discovery or repository rewrite |
| Markdown/YAML contract files plus SQLite: [file storage](https://github.com/ybakiame/agent-control/blob/f54be806d280b173ca6bac513134fc0e9a0f2b18/crates/core/src/store/files.rs) | Adapt | Repository contracts can be Git-reviewed files; personal profiles/reviews stay local SQLite. Exact contract path/schema/parser and migrations deferred; existing docs remain valid |
| Suggestions with rationale, scope and human review: [MCP implementation](https://github.com/ybakiame/agent-control/blob/f54be806d280b173ca6bac513134fc0e9a0f2b18/crates/mcp/src/lib.rs) | Adapt | Adopt proposal/review concepts; no MCP proposal tool in J. Future operator review must bind exact content and destination and provide complete lifecycle |
| Scope/priority resolution and applicability: [engine](https://github.com/ybakiame/agent-control/blob/f54be806d280b173ca6bac513134fc0e9a0f2b18/crates/core/src/engine.rs), [code conflict checker](https://github.com/ybakiame/agent-control/blob/f54be806d280b173ca6bac513134fc0e9a0f2b18/crates/core/src/conflict.rs) | Adapt | Explainable matching/conflicts are useful; reject session-rank or numeric priority as authority over repository/security. Do not copy parsing/check execution |
| Frozen constraints and cross-harness continuity: [model](https://github.com/ybakiame/agent-control/blob/f54be806d280b173ca6bac513134fc0e9a0f2b18/crates/core/src/model.rs) | Adapt | Snapshots preserve historical versions; current sources/permissions must be rechecked, not perpetually frozen. Snapshot/bundle implementation deferred |
| Provenance and evidence views: [view inventory](https://github.com/ybakiame/agent-control/tree/f54be806d280b173ca6bac513134fc0e9a0f2b18/crates/app/src/views) | Adopt | Explanation near rules/reviews/Handoff; separate agent-reported, daemon-observed and independently verified origins; no mandatory page per concept |
| SHA-256 evidence chain / handoff bundle hashes: [model](https://github.com/ybakiame/agent-control/blob/f54be806d280b173ca6bac513134fc0e9a0f2b18/crates/core/src/model.rs) | Defer | Integrity can be useful, but needs schema/canonicalization, anchors, privacy and independent verification design. Not event truth or same-user tamper prevention |
| “Signed handoff bundles”: [README](https://github.com/ybakiame/agent-control/blob/f54be806d280b173ca6bac513134fc0e9a0f2b18/README.md#the-four-layers--四层架构) vs [model](https://github.com/ybakiame/agent-control/blob/f54be806d280b173ca6bac513134fc0e9a0f2b18/crates/core/src/model.rs) | Reject | Inspected structures use `bundle_hash`, `snapshot_hash` and SHA-256 integrity checks, not a signing/key-verification design. Use checksum/integrity wording |
| Agent `save_rule`, broad session/evidence mutations and HTTP daemon: [MCP](https://github.com/ybakiame/agent-control/blob/f54be806d280b173ca6bac513134fc0e9a0f2b18/crates/mcp/src/lib.rs) | Reject | Preserve two read-only MCP tools and local IPC; no binding agent writes, new listener, or automatic service startup |
| Separate GPUI UI, persisted JSON/user rule files, scaffold migration and portable packaging: [README](https://github.com/ybakiame/agent-control/blob/f54be806d280b173ca6bac513134fc0e9a0f2b18/README.md) | Reject | Retain NSH Rust and current runtime/storage ownership. Product packaging may be specified later without importing another stack or file-based personal profile |

“Adopt” accepts a product concept, not a claim that the sibling implementation is
secure, complete or present here. “Adapt” preserves useful intent with this repo's
boundaries. “Defer” needs a separate decision and implementation task. Decisions
are reflected in [ADR-0004](adr/0004-product-domain-trust.md),
[architecture](architecture.md) and [UI IA](product-ui.md).
