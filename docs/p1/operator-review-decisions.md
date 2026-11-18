# P1-M4 — Owner decision record for operator review/provenance

Status: **Gate A accepted.** This record authorizes S1 implementation of the approved
minimum persistence/typed operator API as scoped by
[Issue #55](https://github.com/ybakiame/agent-control-plane/issues/55).

Authority: [S0 contract](operator-review-contract.md) section 10,
[acceptance matrix](operator-review-acceptance-matrix.md) section 4 handoff criteria,
accepted ADR-0004/0005/0006.

Approval: repository owner `ybakiame`, 2026-11-14, confirmed in
[Issue #55 comment](https://github.com/ybakiame/agent-control-plane/issues/55#issuecomment-6868785392).
The full recommended set was presented with consequences; the owner approved all
items without amendment. This document is the recorded reference required by Gate A.

## S1-blocking decisions (approved)

| # | Decision | Approved value | Key constraint |
| --- | --- | --- | --- |
| D1 | Storage model | Replace fresh Rust-native Profile v1 DDL in one breaking commit | `PRAGMA user_version=1`; disposable fixtures only; no migration, import, restore, dual reader or compatibility shim |
| D2 | Decision history | Append-only `acp_review_decisions` table, transactionally appended for each decision and automatic same-key demotion | No historical proof/identity inferred from receipts |
| D3 | Digest-bound decisions | Require exact `proposal_digest` (SHA-256, 32 bytes) and distinct `ReviewMismatch` refusal in addition to revision CAS | Byte comparison only, not identity/attestation |
| D4 | Origin vocabulary | Reuse existing `typed_context::Origin` names | No parallel enum; deletion of the unused module is a separate future decision, not part of S1 |
| D5 | Actor attribution | Closed client-asserted `channel` label: `cli`, `desktop`, `tui` | Never authentication or permission proof |
| D7 | Deletion and history | Remove the deleted preference's decision rows in the same transaction | Keep existing identity-only receipts; no forensic SQLite erasure promise |
| D9 | Receipt ceiling | Keep 4096 committed-operation ceiling per generation; prove bounded `Capacity` refusal | No automatic expansion |
| D10 | Queue pagination | ID-ordered, at most 12 entries, disclose `commit_sequence` watermark | No cross-page snapshot stability claim |

## S2/S3-deferred decisions (approved as deferred)

| # | Decision | Approved value | Effective stage |
| --- | --- | --- | --- |
| D6 | Peer-demotion consent | Disclose at inspect only in S1; binding expected peer IDs requires its own UX cost analysis | S2 copy |
| D8 | High-risk UI scope | Deprecate + delete in S2; register and Handoff put stay CLI-only. **Superseded for registration (2026-11-18):** projects are added and removed in the GUI through the daemon's checked `project.add` / `project.remove` ([project-management plan](project-management-plan.md)); Handoff put stays CLI-only | S2 |
| D11 | TUI review parity | All five review actions required on all four native surfaces; a terminal that cannot deliver the chord is BLOCKED, never NOT_APPLICABLE; desktop-only needs an explicit owner amendment | S2 acceptance |

## Boundaries preserved

- Daemon remains sole production write owner; `acp-store` stays read-only.
- MCP keeps exactly two project-bound read-only tools; no review queue/history/provenance exposure.
- Only confirmed preferences reach agent-facing effective context.
- Profile creation/selection stays an explicit operator action.
- No Python runtime, compatibility layer, migration or downgrade path.
- No production data, external repository, model/provider/auth call, installation or release.
- S1 does not implement new queue/detail/history UI; S2 owns those.
- Parent #52 stays open until S3 full acceptance.

## Implementation reference

Gate B implementation proceeds in a single scoped Draft PR per Issue #55, based on
merged S0 `992f33ce68fbf0fdafb13dd4bc86b85d0e09bc35`. Schema, writer, IPC, CLI and
NSH call sites are updated in one coordinated breaking commit with `Request.version=2`.
All fixtures are owned synthetic; old disposable test databases are discarded by
explicit opt-in reset.