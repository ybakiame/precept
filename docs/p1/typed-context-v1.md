# Typed context v1 — frozen resolution contract (P1-L / L0)

Status: v1 implementation contract, frozen before L1. Authority:
[Issue #34](https://github.com/ybakiame/agent-control-plane/issues/34) and
[Accepted ADR-0004](../adr/0004-product-domain-trust.md). This is an additive pure
Rust API, **not** a new MCP/IPC response, persisted schema or authorization engine.

## API, types and ownership

`acp_core::typed_context::resolve(&ResolutionInputV1)` returns
`Result<EffectiveContextV1, InputError>`. Both DTOs have numeric `schema_version: 1`.
Unknown top-level versions, invalid query values and unregistered/ambiguous project
bindings fail with sanitized typed errors. Per-rule invalidity stays in explanation.
Serde rejects unknown fields, enum variants and incorrectly typed data; unsupported
per-rule schema versions have an explicit invalid-rule outcome.

The caller supplies **values**, after daemon authorization and current-source reads.
No filesystem, SQLite, Git, network, clock, process, MCP, session or UI operation is
available to this module. The caller remains responsible for live root/symlink and
lease validation. Labels supplied by the caller are not independently authenticated.

| Type / field | Exact v1 meaning |
| --- | --- |
| `ProjectBinding { project, root_id }` | Opaque registration identity and root-generation label, not a path or credential. Query binding must exactly occur in `registered_projects`; a project name cannot have two root IDs |
| `Query` | Binding plus optional `agent_id`, `path`, `language`, `task_tags`, `session_id`; no prompt or arbitrary root input |
| `RuleRef { id, revision }` | Stable nonempty local identity, positive revision; never random output identity |
| `Rule` | Schema version, reference, kind, scope, authority, review, selector, key, `allowed_values`, optional legacy preference ID, explicit superseded references, source |
| `RuleKind` | `preference`, `contract`, `constraint` |
| `Scope` | Tagged `global`, `project { binding }`, `session { binding, session_id }`. Scope selects applicability; it is not a rank |
| `Authority` | `personal`, `repository`, `session_narrowing`; must agree with kind and valid scope |
| `ReviewState` | `suggested`, `confirmed`, `rejected`, `deprecated`, `unreviewed`, `accepted`. Preferences/constraints need confirmed; contracts need accepted |
| `SourceRef { id, version, digest }` | Required nonempty identity/version and optional opaque comparison digest; no signature claim |
| `Source` | Reference, state and provenance; versions are retained even when stale |
| `SourceState` | `fresh`, `stale`, `unavailable`, `unknown`, `malformed`, `unsupported_schema` |
| `Provenance` | Origin, reference labels and historical validation claims. Origins: `agent_reported`, `daemon_observed`, `independently_verified`, `legacy_unknown` |
| `HistoricalValidation` | Reference, original origin/result/revision; results remain PASS/FAIL/SKIPPED/NOT_RUN. Resolution never reruns or renews them |
| `RepositoryFact` | Bound key and one exact observed value, plus source; descriptive, never a normative rule |
| `PolicyBoundary` | Bound key and finite allowed values, plus source; already-supplied descriptive policy limits for checking session narrowing, never a grant |
| `RuleExplanation` | Normalized rule, applicability, outcome and sorted reasons |
| `EffectiveValue` | Key, finite compatible values, selected rule/source references; no `approved`/capability field |
| `Conflict` | Category, resolved-by-authority or unresolved state, key, affected rule/source references and next human review action |

`EffectiveContextV1` contains the binding, explanations, effective values, conflicts,
and bound repository facts/policy boundaries. Explanations include rejected/pending
content **for operator inspection only**. They must not be silently attached to an
agent response. Only the effective collection is selected context; even that
collection is not permission to perform an action. Foreign project/root and other
known-session records are omitted entirely, including their source references.

## Normalization, values and revisions

Identifiers/keys/labels are nonempty, trimmed, control-free UTF-8, at most 1024
bytes. Values are nonempty, control-free UTF-8 strings at most 4096 bytes. A rule
or policy finite set has 1–64 unique values after normalization. A preference has
exactly one value and a positive `legacy_preference_id`; other kinds have no such
ID. Sets have exact opaque categorical meaning, **not natural-language semantics**.
There are no executable predicates, string-substring tests, wildcard values,
numeric ranges or inferred semantic compatibility. An absent external translation
is not a parsed contract; parsers/adapters remain future work.

Contract compatibility is nonempty set intersection. Session constraints must be
subsets of each relevant contract and supplied policy boundary, then intersect with
other compatible constraints. A wider or disjoint session value set is a conflict,
not automatically intersected into an apparently valid request. A compatible
preference may narrow the remaining choices; an incompatible preference is masked.
Multiple accepted contracts with empty total intersection remain unresolved even
if some pairs overlap. Facts disagreeing with contracts/constraints produce drift;
a preference disagreeing with a current fact is masked, without turning that fact
into a normative rule.

Identical normalized repeated records deduplicate. Divergent records with the same
rule reference are invalid; their variants remain explainable in stable order and
cannot win. Multiple active revisions of one stable identity are unresolved unless
a valid applicable fresh reviewed revision explicitly lists the older reference in
`supersedes`. Supersession must retain kind, key, scope, authority and source ID;
references must use the same ID and strictly older revisions. An invalid/ineligible
new revision cannot suppress an eligible older one. No implicit latest revision or
contract timestamp/priority winner exists.

## Selector and query grammar

`Selector` has optional lists `projects`, `agents`, `paths`, `languages`, `task_tags`.
Null/absent means unrestricted; an explicit empty list is invalid. Projects are
exact registered `ProjectBinding`s, never new roots. All list values deduplicate
and sort. AND across populated dimensions; OR within a dimension. Task tags match
by a nonempty intersection with supplied query tags; supplied `[]` means known no
tags, unlike missing/null. Scope and selector must both match.

Paths use exact **case-sensitive** repository-relative `/` spelling on every host.
Reject empty/absolute/trailing-slash paths, empty/`.`/`..` components, backslashes,
colon/drive/UNC syntax, controls, and `* ? [ ] { }` pattern syntax. No filesystem
normalization, case folding, glob/regex or symlink traversal happens here. Agent,
language and tag identifiers also compare exactly, without prompt interpretation.
Malformed populated query fields fail the entire call, rather than becoming absent.

| Dimension / combined state | Applicability |
| --- | --- |
| Absent dimension | Match |
| Populated dimension, known matching query | Match |
| Populated dimension, known nonmatching query | NoMatch |
| Populated dimension, missing query | Unknown |
| Invalid selector, including an unregistered project entry | Invalid, even if another dimension does not match |
| Valid dimensions with any NoMatch | NoMatch, even with another Unknown |
| Otherwise any Unknown | Unknown |
| Otherwise | Match |

Missing session query for a session-scoped rule is Unknown; another known session
is out of scope. A global/project scope never gains session authority through a
selector. Invalid selectors never become unrestricted selectors.

## Resolution and conflict decision table

All eligible rules for a key are resolved together. Unknown/invalid relevant
reviewed rules or observations conservatively block that key's effective value;
no-match and unreviewed/excluded rules do not block it. Unrelated keys remain
inspectable. Masking and uncertainty are distinct outcomes.

| Condition | Outcome / category / next action |
| --- | --- |
| Preference suggested/rejected/deprecated or contract unaccepted | Excluded / review reason; no effective value from that record |
| Valid selector NoMatch | Excluded / selector mismatch |
| Missing query for reviewed rule | Unknown / UnknownApplicability / SupplyQuery |
| Stale source | Unknown / StaleSourceSnapshot / RefreshSource |
| Unavailable/unknown source | Unknown / SourceUnavailable or SourceUnknown / RefreshSource |
| Malformed source, unsupported schema, invalid rule/selector/identity | Excluded / InvalidSourceSchema / RepairInput |
| Applicable confirmed project preference and global same key | Global masked / ProjectPreference |
| Same scope/key confirmed preferences | Highest legacy ID wins; earlier IDs masked. A colliding highest ID with incompatible values is unresolved |
| Personal preference incompatible with accepted contract | Preference masked; PreferenceVsContract resolved by authority; InspectPreference |
| Equal-authority incompatible rules or unsuperseded competing revisions | Conflicted / RuleVsRule / ReviewRules; no winner or effective value for the key |
| Contract/constraint vs current repository observation | Conflicted / RuleVsRepository / InspectRepository; no automatic code/ADR change |
| Session constraint wider than contract/policy | Conflicted / SessionVsContract or SessionVsPolicy / ReviewConstraint; no effective value for the key |
| Compatible contracts/constraints and preference | Selected; effective intersection with exact source/rule references |

An observation with only agent-reported/legacy-unknown origin cannot be promoted
into observed repository/policy authority: retain it, mark unknown source and request
inspection. Already-attributed independently verified input is preserved as a claim
of that origin, not reverified by this resolver. All unresolved conflicts block the
affected key, including otherwise valid rules, so no conflicting action appears as
an effective winner. Resolved-by-authority conflicts retain the losing reference.

## Determinism and serialization

All v1 structs deny unknown fields. Enums serialize as snake_case (historical check
results retain uppercase). Scope uses a `kind` tag. Rule lists sort by key, reference,
then normalized full record; references sort by ID/revision; source references sort
by ID/version/digest. Selector/value/provenance-reference/historical-claim lists
sort and deduplicate using their typed lexicographic order. Facts/policy boundaries
and conflicts also sort/deduplicate by their full typed records (conflicts: key,
category, status, rule refs, source refs, next action). Reasons use declared enum
order. Effective values sort by key. Enum ordering is declaration order in v1.
There is no clock, hash-map order, database arrival order or generated output ID.
Identical normalized inputs yield byte-identical `serde_json` output.

## Representative serialized examples

A confirmed project preference (input fragment; all references are synthetic):

```json
{
  "schema_version": 1,
  "reference": {"id": "preference-7", "revision": 1},
  "kind": "preference",
  "scope": {"kind": "project", "binding": {"project": "alpha", "root_id": "root-alpha-v1"}},
  "authority": "personal",
  "review": "confirmed",
  "selector": {"projects": null, "agents": null, "paths": ["src/lib.rs"], "languages": ["rust"], "task_tags": null},
  "key": "formatter",
  "allowed_values": ["rustfmt"],
  "legacy_preference_id": 7,
  "supersedes": [],
  "source": {
    "reference": {"id": "local-review-7", "version": "1", "digest": null},
    "state": "fresh",
    "provenance": {"origin": "agent_reported", "references": ["operator-confirmation-7"], "historical_validation": []}
  }
}
```

With matching bound query path/language, that rule is selected. With missing query
language it is unknown and produces no effective formatter. With a reviewed
repository contract requiring `cargo-fmt`, the preference is masked and this
conflict fragment is retained:

```json
{
  "key": "formatter",
  "category": "preference_vs_contract",
  "status": "resolved_by_authority",
  "rules": [{"id": "contract-fmt", "revision": 2}, {"id": "preference-7", "revision": 1}],
  "sources": [{"id": "AGENTS-contract", "version": "2", "digest": null}, {"id": "local-review-7", "version": "1", "digest": null}],
  "next_action": "inspect_preference"
}
```

If two accepted contracts require disjoint values, category becomes `rule_vs_rule`,
status is `unresolved`, both references remain and the effective formatter entry is
absent. Session `allowed_values: ["read", "write"]` against supplied policy
`["read"]` similarly yields unresolved `session_vs_policy`; the result never emits
a permission grant. A stale version stays attached to its rule with `source.state:
"stale"`; its historical PASS and origin are unchanged, not current validation.

Tests in `crates/acp-core/tests/typed_context.rs` exercise these fragments, the
empty/no-match/unknown table, masking, conflicts, drift, invalid schemas and stable
permutations. The legacy fixture and Python parity remain separate unchanged oracles.
