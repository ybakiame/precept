# P1-L implementation plan — Issue #34

> **Historical record — not a current runbook.** Original dates, source revisions
> and outcomes below are preserved. S4 retired the executable prototype and probes;
> use [Rust-only development](rust-only-development.md) for current commands.

Status: L0–L2 implemented and locally validated in one PR candidate, pending Draft
review and owner merge. The [v1 contract](typed-context-v1.md) and pure resolver are
covered by the [local evidence](../evaluation/p1-l-typed-context-resolver.md).
The original planning checklist below is retained as the implementation sequence.

Authority: [Issue #34](https://github.com/ybakiame/agent-control-plane/issues/34),
[Accepted ADR-0004](../adr/0004-product-domain-trust.md) and [AGENTS.md](../../AGENTS.md).
Start: clean main `e9b56c53c26fca1e77e49c70c2aa1afb68e720e2`, after PR #33 and the
owner's ADR acceptance. Working branch: `feat/p1-l-typed-context-resolver`.

## Architecture review and compatibility boundary

Read the current [product](../product.md), [architecture](../architecture.md),
[interfaces](../interfaces.md), [security](../security.md),
[roadmap](../roadmap.md), ADR-0004, core implementation/tests and local Rust CI
contract before writing this plan.

| Current implementation | Consequence for P1-L |
| --- | --- |
| `crates/acp-core/src/lib.rs` defines the legacy preference/Handoff types, adapter traits and `ContextResolver` | Add the versioned model alongside these types; preserve their public and serialized behavior |
| `resolve_preferences()` checks registration, confirmed review state and scope; sorts by key, project before global, then descending ID | Retain this exact legacy behavior, including the newest-confirmed-ID exception; do not generalize it to contract authority |
| `context()` and `handoff()` return existing JSON, including fixed warnings and unchanged validation provenance | Do not insert explanation fields or new statuses into either MCP response |
| Repository inspection arrives through adapter traits; existing `documents`/`fingerprint` modules have filesystem duties | The new resolver must be a separate pure module, never call those adapters or I/O helpers |
| `ReadOnlyPermissions` validates tool request shape; daemon/IPC own capabilities and lease admission/publication | A rule match or selected constraint is an explanation result, never an authorization decision |
| `read_only.rs`, the shared preference fixture and `contract_reader` provide legacy behavior checks | Keep them intact and verify Python/Rust and SQLite parity after implementation |

Prefer additive `acp_core::typed_context` with types and resolution split into
submodules if necessary. It takes explicit value inputs and returns a versioned
explanation. It accepts a registered binding supplied by the caller; selectors
cannot discover, choose or rebind roots. No wiring into daemon/MCP is required to
deliver the pure v1 API. Reusing it inside legacy resolution is optional and must
not change the oracle responses; leaving the legacy projection unchanged is valid.

## L0 — freeze the contract before coding

Create `docs/p1/typed-context-v1.md`. Complete these decisions, serialized examples
and a decision table before implementing the API:

1. Define stable rule identity/revision and typed `RuleKind`, scope, authority,
   source/provenance reference, source version/state, review state, selectors,
   applicability, resolution outcome, conflicts and next review action. Define
   versioned `EffectiveContextV1` as an explanation DTO, not a changed wire payload.
2. Define the explicit input binding/query, rules, repository observations and
   policy-boundary facts. No credential, lease or session lifecycle operation is
   part of the resolver. Source and review labels describe caller-supplied facts;
   deserializing an acceptance label does not independently verify its authority.
3. Specify exact project/agent/path/language/task-tag matching. AND across
   populated dimensions, OR within each, absent unrestricted, and missing query
   data unknown. Freeze truth tables for combinations of false/unknown/invalid,
   including an explicitly empty list versus an absent dimension.
4. Specify portable repository-relative path grammar and case semantics. Cover
   traversal, absolute/drive/UNC paths, separators, empty components and malformed
   syntax; reject unsupported glob/regex semantics rather than widening a match.
   Filesystem containment/symlink evidence remains an upstream adapter concern.
5. Define the bounded representation of rule values and compatibility/narrowing.
   Do not infer semantic compatibility from arbitrary prose or executable
   predicates. Unsupported comparisons must retain an explicit unknown/conflict
   reason; session constraints cannot weaken repository requirements or policy.
6. Freeze precedence: accepted contracts and observed repository facts outrank
   personal/Handoff claims; code-versus-contract disagreement becomes drift for
   human resolution. Applicable confirmed project preferences mask global choices
   of the same key; same-scope newest confirmed ID remains the compatibility
   exception. Equal-authority incompatible rules have no effective winner.
7. Specify exclusions and uncertainty independently: suggested/rejected/deprecated
   preference, unaccepted contract, stale/unavailable/unknown source, invalid
   selector/schema and missing query evidence. An uncertain higher-scope rule must
   not silently become a matching override. Specify affected-action handling while
   allowing inspection of unrelated context.
8. Define duplicate identity/revision handling and a total stable ordering for
   selected/masked/excluded/unknown rules, reasons, conflicts and references.
   Normalize order-insensitive selector values; input permutations must produce
   equal typed output. Do not depend on hash-map iteration, time or fresh IDs.
9. Preserve origin and historical validation exactly: agent-reported,
   daemon-observed and independently verified remain distinct. Source digests are
   comparison values, not signatures, fresh PASS or permissions.

Examples must include project/global fallback, a rejected preference, an exact-path
match and unknown query, incompatible accepted contracts, repository drift, a
session attempt to weaken a contract/policy, and stale/unavailable sources. Every
example should map to a later test case.

These are design items to settle in the v1 contract, not unresolved changes to the
accepted ADR. When L0 fits ADR-0004, implementation proceeds directly to L1 in the
same PR without another approval round-trip. If a necessary choice would contradict
the ADR or remains genuinely ambiguous, report the specific decision needed before
implementing that behavior.

## L1 — implementation slices and focused tests

| Slice | Planned files | Completion gate |
| --- | --- | --- |
| Types and validation | New `crates/acp-core/src/typed_context/` module; additive module export in `lib.rs` | Types/serialization match frozen L0 examples; malformed selectors/source/schema retain explicit failure categories |
| Applicability | Pure selector validation/matching functions | Table-driven AND/OR, absent/missing/empty, exact-path, malformed and no-match cases; unregistered/cross-project binding cannot apply |
| Resolution and explanation | Pure deterministic resolution functions | Review/source gates, preference compatibility tie-break, contract authority, session narrowing, unknowns and unresolved conflict references agree with the decision table |
| Adversarial and compatibility coverage | New focused core integration tests; existing `read_only.rs` and shared fixtures retained | Input permutation/duplicate cases, stable reference ordering, source-version/origin retention, stale/drift and no fabricated winner; legacy projections and both parity suites unchanged |

Use existing `serde`, `serde_json` and standard ordered collections. No new runtime,
fuzz/property-test or domain dependency is presently justified. Test permutations
and adversarial tables using ordinary Rust tests. Runtime safety validation uses
explicit branches/results; reserve assertions for test expectations.

## Local validation and evidence

During each slice, run affected `acp-core` tests and formatting. After the complete
code change, run the repository-required Python command and the final Rust/parity
contract once on a recorded clean source. Repeat only affected checks after fixes;
do not repeatedly rerun the entire platform matrix without a new reason.

The implementation report `docs/evaluation/p1-l-typed-context-resolver.md` must
record exact source SHA/dirty state, host OS/toolchain, command, outcome and limits.
The following were the planned commands. Actual results, platform argv and clean
source provenance are now recorded in the linked L2 evidence report:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo build --locked --example contract_reader
cargo build --locked -p acp-store --example sqlite_reader
PYTHONPATH=src python -m unittest discover -s tests -v
PYTHONPATH=src python tools/check_p1_parity.py
PYTHONPATH=src python tools/check_sqlite_parity.py
cargo run --locked -p acp-rehearsal --example synthetic_rehearsal -- --synthetic-only
cargo build --workspace --bins --examples --locked
PYTHONPATH=src python tools/test_local_service.py -v
PYTHONPATH=src python tools/test_agent_session.py -v
PYTHONPATH=src python tools/test_session_context.py -v
cargo test --locked -p acp-rehearsal --features recovery-tests --test process_recovery -- --nocapture
git diff --check
```

Validate locally on Windows and Linux/WSL using synthetic fixtures and the owned
Windows test checkout, without normal user databases or external development
repositories. In PowerShell set `PYTHONPATH=src`; use the two parity readers at
`target/debug/examples/contract_reader.exe` and `sqlite_reader.exe` via `--reader`.
When synthetic rehearsal invokes Python on Windows, use the established local
`ACP_TEST_PYTHON` setting. Record any missing prerequisites or blocked commands
explicitly; do not install or reconfigure a system merely to manufacture a PASS.

The pure-core scope needs no GUI interaction, compositor or real-agent/model turn.
Keep hosted Actions, niri, mobile and unrun platforms NOT_RUN. Existing P1-K model
and platform evidence remains historical at its original revision. No authentication
directory or API key is needed for this task.

## Delivery and final review

- Deliver the frozen v1 contract, additive pure resolver, focused tests and exact
  local evaluation report. Update roadmap completion only after those gates pass;
  M–R remain gated briefs.
- Inspect the full diff for wire/schema/dependency drift, accidental source/path
  leakage and secret handling. Keep SQLite records/schema, Python write ownership,
  two MCP tools, legacy response fields, capability checks and revocation intact.
- Use `[skip ci]` for pushed commits. Create a Draft PR for the completed work and
  leave merging to the owner. Acceptance claims come from the linked L2 report;
  the planning checklist alone is not evidence.
