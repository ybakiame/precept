# ADR-0004: product domain, ownership and evidence trust

- Status: Accepted — owner approved 2026-11-11; implementation remains sequenced by roadmap issues
- Date: 2026-11-11
- Authority: [Issue #30](https://github.com/ybakiame/agent-control-plane/issues/30)
- Baseline: main `14798f2809b557a16bda16729f2cdeed7f24bbb0`

## Context

The Rust core/daemon, MCP/ACP and NSH experiments establish useful boundaries,
but the product needs a common vocabulary before adding rules, review UI or durable
sessions. The sibling [assessment](../agent-control-assessment.md) supplies design
ideas, not source code or authority. The current preference/Handoff schema cannot
be silently reinterpreted as a general rule engine or authenticated evidence log.

## Decision: domain objects

| Concept | Meaning / identity | Owner and lifetime |
| --- | --- | --- |
| PersonalPreference | A reviewed habit/environment choice; stable local ID plus revision, key/value, scope and review state | User-local SQLite; global or one project; never a repository contract |
| ProjectContract | Repository engineering requirement/convention with source path, source version/digest, kind and applicability | Repository owner/review process; live source is authoritative over cached copies |
| SessionConstraintSnapshot | Immutable task-scoped resolved rule references/versions plus explicit operator additions, project/worktree, task and agent binding | Daemon-managed session generation; historical after sources or permissions change |
| EffectiveContext | Derived applicable context, selected and excluded rules, reasons, conflicts, unknowns and source versions | Resolver result for project/task/agent at a particular read; not an authority store |
| Suggestion | Proposed exact content/version, target kind/scope, rationale and attributed source | Local pending record; no binding effect |
| ReviewDecision | Human decision on exact proposal digest/version and destination, with rationale/time and local actor attribution | Local state; actor label is not cryptographic identity |
| Provenance | Source kind, author/reporter, scope, version/digest, derivation and review references | Travels with the described claim; source labels alone do not verify it |
| EvidenceRecord | Bounded claim, origin class, observer/verifier, method, subject revision/platform, result and limits | Local minimal evidence; verification adds a linked record, never rewrites the original claim |
| Handoff | Objective, completed work, next action, project/task/agent references, constraint snapshot reference, revision/worktree and validation/evidence references | Historical continuity; no authority, credential or automatic execution |

These are target logical concepts, **not implemented Rust structs or a DB schema**.
Current PersonalPreference scopes remain `global`/`project`; existing Handoff has
no frozen constraint snapshot. No J migration or new wire field is implied.

## Rule kinds, scopes and applicability

Kinds distinguish **preference** (user habit), **contract** (repository requirement)
and **constraint** (operator task-specific narrowing). Scope describes where a rule
applies, not how much authority it has. A project-scoped preference stays personal;
a session constraint cannot weaken a project contract. Descriptive observations
(e.g. an installed version) are evidence, not new normative rules.

Target selectors use an explicitly registered project and optional agent ID,
repository-relative paths, language identifiers and task tags. No selector can
choose an unregistered root, follow an escaping symlink or grant filesystem access.
Agent identity is an applicability label, not authentication; authorization comes
from daemon capabilities. Matching is deterministic: AND across populated selector
dimensions, OR within a dimension; absent dimensions impose no restriction.
Missing query data for a populated dimension means **unknown applicability**, not
an automatic match. Unknown/invalid selector syntax is reported and never silently
widens a rule. Exact path-pattern grammar/case rules require versioned format
review and traversal/ambiguity tests before implementation; J enables no globs or
new document discovery. Arbitrary substring matches on prompt text are excluded.

## Deterministic resolution and conflict handling

1. **Gate authority.** Validate operator/agent role, project binding, current policy
   and lease before disclosing context or allowing a session operation. Rules,
   prompts, snapshots and approvals cannot extend a capability or revive revocation.
2. **Read current sources.** Recheck registration/root, repository revision and
   available bounded worktree/source digests. Load repository contracts and their
   acceptance status; unreviewed/imported/inferred rules remain proposals. Do not
   treat every Markdown paragraph or an untrusted repository instruction as permission.
3. **Filter review and applicability.** Include confirmed personal choices and
   applicable reviewed contracts; retain reasons for excluded/unknown entries in
   the operator explanation. Pending/rejected/deprecated personal values stay out
   of agent-facing effective context. Sensitive review history is not exposed by
   implication through MCP.
4. **Respect repository authority.** Current code/config/tests establish observed
   state; accepted ADRs and repository rules establish intended requirements. Both
   outrank stored preferences/Handoff. If code and a contract disagree, report
   RuleVsRepository drift and require human resolution for the affected decision;
   do not automatically rewrite code or discard an accepted ADR.
5. **Resolve personal fallback.** For the same preference key, an applicable
   confirmed project choice masks a global one. Preserve current same-scope newest
   confirmed ID tie-break for compatibility. Future revisions must explicitly
   supersede earlier ones; incompatible distinct rules of equal authority produce
   a conflict, not a winner chosen by timestamp or arbitrary numeric priority.
6. **Apply session narrowing.** Add explicit task constraints only where compatible
   with authorization and repository contracts. Selectors alone never permit an
   override. Conflicting contracts require repository-defined delegation or review;
   a more specific path/agent match by itself is not higher authority.
7. **Freeze explanation, not permission.** A future snapshot records the exact
   selected sources and unresolved issues. At resume/read, compare it to current
   sources, review state and policy. Changed or unavailable sources cause stale/
   unknown warnings and fresh review of affected work. Live MCP remains live;
   snapshot creation will not convert it into an old frozen read endpoint.

Conflict categories: RuleVsRule, RuleVsRepository, PreferenceVsContract,
SessionVsPolicy/Contract, StaleSource/Snapshot, UnknownApplicability and
InvalidSource/Schema. The result identifies affected rules/sources, reason and
next review action. Determinism includes an explicit **unresolved** outcome: no
fabricated “effective winner” for a contradictory requirement. Unaffected context
can still be inspected; the unresolved action cannot be reported as approved.

Examples: a personal formatter preference loses to the project's formatter
contract; a task prompt requesting unrestricted execution cannot change a scoped
read capability; a snapshot requiring an old compiler while current config and ADR
require a newer one is stale/conflicted, not a command to downgrade the project.

## Human review and storage

Preferences follow `suggested → confirmed | rejected`; confirmed records may be
`deprecated` or deleted. Changing confirmed content/scope needs a new reviewed
revision. Generalized suggestions will retain an explicit decision reference;
rejection never makes a rule effective. Project suggestions need a reviewed
repository change before becoming contracts. A local “approve” cannot bypass the
project review process. No automatic conversion of behavior or chat to user intent.

Before automatic suggestions, a future operator UI must provide view, confirm,
reject, deprecate, delete and provenance inspection. Deletion must account for
privacy/retention; referential history may retain a minimal tombstone but must not
secretly preserve deleted preference content. SQLite backups/WAL are not forensic
secure deletion. Current CLI deletion behavior remains unchanged.

Adopt Git-friendly Markdown with versioned metadata/YAML as an **optional future
project-contract storage pattern** alongside existing AGENTS/docs/ADRs. Exact
location/format is deferred, with no new parser or scaffold in J. Repository-owned
contract files may be committed through normal review; personal profiles, proposals,
runtime evidence, credentials and private device data never belong in those files.
All application-owned durable state stays user-local SQLite; ephemeral credentials
stay in private runtime files. SQLite contract indexes are caches, not competing
sources of truth. See [architecture ownership](../architecture.md).

Before any migration: separate DB/contract format versions, preserve legacy IDs,
scopes/review states and unknown provenance, reject unknown future versions, and
prove consistent backup, recovery and old/new reader behavior on synthetic data.
No production Rust writer, contract auto-export or migration is accepted here.

## Evidence and Handoff integrity

| Origin class | What it establishes | What it does not establish |
| --- | --- | --- |
| Agent-reported | A particular agent/channel reported a bounded claim | The action happened, files changed, tests ran, or rules were followed |
| Daemon-observed | The daemon observed a specific protocol/process event, authorized read or counter at a bound generation | Agent-internal behavior, test correctness or unrelated filesystem changes |
| Independently verified | A separately identified checker examined/reran a specific claim using a recorded method and source/platform | Universal truth, complete coverage, unchanged future state or same-user tamper resistance |

Result and origin are separate dimensions. A `PASS` with agent-reported origin is
still only a claim; daemon ingestion does not upgrade it. A verifier that merely
repeats agent output is not independent. Record method, exact source/environment,
verifier identity as locally attributed, limits and links to original observations.
No cryptographically authenticated reporter is claimed. Legacy opaque validation
JSON stays historical/unclassified unless supported by explicit external evidence.

Current wire validation statuses are `PASS`, `FAIL`, `SKIPPED`, `NOT_RUN`.
Evaluation reports may also use `BLOCKED`/`UNSUPPORTED`; they are report outcomes,
not newly accepted serialized Handoff statuses. No enum migration is part of J.

Current SHA-256 worktree fingerprints are bounded drift detectors, not signatures
or full attestations. Future bundle checksums verify specified serialized content
only. A hash chain can detect inconsistencies relative to an expected head, but a
same-user writer may recompute it; a truncated or rewritten chain needs an external
trusted anchor to detect reliably. Hashes neither authenticate a reporter nor prove
an event occurred. No evidence chain or signature implementation is adopted in J.

The receiver must recheck project/root identity, current HEAD/branch/worktree,
source versions and contracts, current permissions and pending conflicts, and rerun
relevant validation. Missing/incompatible metadata is unknown, not matching.
A matching hash does not renew old PASS or authorize push/publish/deploy/device
changes. No credentials or executable instructions are transferred as authority.

## Protocol and privacy boundaries

- MCP stays project-bound/read-only: the existing two tools only, no `save_rule`,
  proposal API, session writes or file mutation. A narrowly scoped proposal-only
  extension needs its own future ADR and cannot confirm a proposal.
- ACP stays optional, operator-configured, single native session with explicit
  Start/Stop, per-request permission choices and separate context opt-in. No shell,
  automatic login/retry, Windows→WSL execution or OS sandbox is implied.
- Preserve [P1-I](../p1/session-context-bridge.md): project/agent/generation lease,
  monotonic expiry, request and response-admission checks, established-peer denial
  after revoke; Stop, valid prompt Cancel, lifecycle failures and restart end access.
  Cancel requires Stop/fresh opted-in Start to re-enable sharing; delivered data
  and already-admitted responses cannot be recalled. Static format-1 keys retain
  their earlier behavior.
- Local-first: no hidden telemetry, cloud sync, raw conversation ingestion,
  credential scraping, third-party memory dependency or automatic model call.
  Persist minimal sanitized records; public evidence excludes tokens, raw logs,
  device identifiers and private paths. Same-user filesystem access remains outside
  the cooperative transport guarantee.

## Alternatives and consequences

Reject the sibling's scope-rank/priority model as sufficient authority, MCP direct
rule writes, auto-scaffolding and treating hashes as signatures. Defer proposal-only
MCP, evidence chains, portable bundles and structured-contract parsers until separately
specified. This costs explicit review and may leave conflicts unresolved; it keeps
policy changes, user intent and factual evidence distinguishable and testable.

Acceptance requires targeted regressions for selectors/scope isolation, stale
worktrees/sources, malformed requests, permission/review changes and evidence-origin
preservation in the relevant later task. J changes documentation only; implementation
briefs follow in the [roadmap](../roadmap.md), with cross-agent evaluation reserved
for [Issue #31](https://github.com/ybakiame/agent-control-plane/issues/31).
