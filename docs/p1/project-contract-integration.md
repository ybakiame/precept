# P1-N: repository ProjectContract integration proposal

Status: **Proposed; owner boundary approval required before implementation.**
Authority: [Issue #65](https://github.com/ybakiame/agent-control-plane/issues/65),
[PR #61 re-review](https://github.com/ybakiame/agent-control-plane/pull/61),
[ADR-0004](../adr/0004-product-domain-trust.md) and
[typed-context v1](typed-context-v1.md). The review reference is the PR conversation;
the controlling updated review is dated 2026-11-16 10:47 UTC.

This is the issue's first deliverable, not a parser implementation, accepted file
format, new Profile schema or permission grant. PR #61 remains Draft, with M4
blocked on actual production enforcement. The owner intends to review and merge
the dependent work together; nothing in this proposal authorizes automatic merge.

## 1. Audit and smallest integration boundary

Audited source baseline: `95a3905ae8bcded2084ad0af22edb918d55513b5`.

| Current boundary | Finding | Proposed change after approval |
| --- | --- | --- |
| `acp-core/src/lib.rs::ContextResolver` | Checks registration, excludes inactive/foreign preferences, applies project/global and newest-confirmed-ID fallback; effective projection omits private `source` | Apply repository contracts before projecting effective preferences; retain existing unrelated-key behavior |
| `acp-core/src/typed_context` | Pure, deterministic v1 selector/conflict resolver, with no adapter consumer | Reuse this resolver; no second precedence engine or domain vocabulary |
| `acp-core/src/documents.rs::FileDocuments` | Five fixed document paths, 16 KiB bounded reads, optional bodies and content hashes; raw prose has no machine contract meaning | Keep those document references; add a separately bounded, exact-source contract adapter, without directory discovery |
| `acp-daemon/src/main.rs::Service::query` | Checks capability, allowlist and canonical root before constructing the resolver; Operator selects a project, Agent derives it from its capability | Supply the authorized binding and live contract snapshot on this same route; preserve lease checks before response publication |
| `acp-mcp/src/main.rs` | Exactly two project-bound read tools; context accepts only `include_documents` | Keep two tools; optionally supply explicit applicability labels as proposed in D3 |
| Profile v1 reader/writer | Preference values allow up to 4096 Unicode characters, including multiline text; pure typed values allow 4096 UTF-8 bytes and no controls | Do not alter Profile validation or silently discard valid unrelated preferences during translation |
| Operator exact inspection / NSH | Exact inspection retains private source and decision history; clients do not resolve authority | No changes to write commands, inspection history, UI or client resolution |

`get_handoff` remains the historical continuity tool; it neither reads a frozen
contract as current policy nor gains a mutation. Only live effective context is
integrated here. No repository observation parser, generalized policy engine,
onboarding, contract editor, cache table, session snapshot or export is included.

## 2. Owner decisions to freeze

These are recommendations, **not accepted decisions**. Approving all four rows
would freeze the concrete proposal below; alternatives require editing it first.

| Decision | Recommendation | Consequence / alternative |
| --- | --- | --- |
| D1: exact optional source and syntax | One root-relative `ACP.contracts.md`, containing exactly one versioned JSON fence as specified in section 3 | Uses the adopted repository-owned Markdown pattern and existing JSON dependency. Alternative: a separately specified YAML source/parser. Neither option creates a required directory or auto-generates files |
| D2: acceptance and failure behavior | Repository-declared per-rule `accepted` / `unreviewed`; absence means explicitly `not_configured`; unreadable/malformed/unsupported/changed source suppresses all preference output for that read | `accepted` is a repository assertion, not GitHub review verification. Unknown whole-file failure cannot identify safe keys. Alternative: fail the whole context request instead of returning inspectable metadata plus an explicit refusal status |
| D3: selector query | Add an optional `selector` object to the existing IPC context request and MCP `get_effective_context`, with `agent_id`, `path`, `language`, `task_tags` only | Labels affect applicability, never authorization. Missing fields remain unknown for populated selectors. Alternative: keep current requests unchanged; populated selectors then remain unknown and block their affected keys until a later query extension |
| D4: Profile-to-typed bridge | Preserve legacy resolution for keys absent from the contract set; use typed v1 for contract-bearing keys. An affected preference not representable in v1 blocks that key with an explicit unknown result | No text truncation, newline removal, Unicode byte-limit relaxation or Profile schema change. Alternative: separately authorize a typed-v1 revision; that is outside this minimal proposal |

Repository declarations cannot grant permissions, enable execution or confirm
personal preferences. Confirming a conflicting preference stays a valid personal
review action; the live resolver masks it when the repository contract applies.

## 3. Proposed source format and limits (D1/D2)

The only candidate is `ACP.contracts.md` directly below the already authorized
project root. No search in parents, sibling worktrees, `.git`, nested directories,
AGENTS prose, ADR prose, imports or referenced files. Existing document allowlists
are unchanged. This file is optional and never scaffolded or rewritten by ACP.

The Markdown wrapper is descriptive only. A contract block opens with the exact
unindented line `` ```acp-contracts-v1 `` and closes with the exact unindented
line `` ``` ``. CRLF and LF delimiters are accepted; block content must be strict
UTF-8 JSON. The scanner tracks ordinary Markdown fenced blocks so that a marker
quoted inside another fence is not activated. It is a bounded fence scanner,
not a Markdown instruction interpreter. Multiple contract blocks, unterminated
blocks, unsupported `acp-contracts-*` fence versions, or a present file with no
contract block are invalid. Examples live in documentation, not this live filename.

Proposed complete block payload:

```json
{
  "schema_version": 1,
  "rules": [
    {
      "id": "formatter",
      "revision": 1,
      "review": "accepted",
      "key": "formatter",
      "allowed_values": ["rustfmt"],
      "selector": {"paths": ["src/lib.rs"], "languages": ["rust"]},
      "supersedes": []
    }
  ]
}
```

All displayed record fields are required except `selector` and `supersedes`, which
default to unrestricted / empty. The selector permits only `agents`, `paths`,
`languages`, `task_tags`; scope and project are derived from the bound checkout.
Repository content cannot choose `project`, `root_id`, global/session scope,
authority, source state, provenance, historical validation or permission fields.
`supersedes` uses existing `{ "id": "formatter", "revision": 1 }` references.
Only `accepted` and `unreviewed` are valid repository review states. Accepted means
the repository explicitly declares a contract; ACP cannot authenticate its author,
verify a remote review or determine whether a dirty change was approved. Dirty
worktree bytes are current source bytes, not proof of independent acceptance.

Strict limits: 16,384 raw bytes for the entire Markdown file; at most 64 rules;
at most 64 entries per selector dimension or supersedes list; at most 64 finite
allowed values per rule. Existing typed-v1 identifier, value, revision and exact
case-sensitive path grammar applies. Unknown fields, duplicate JSON object keys,
invalid UTF-8, invalid types, trailing JSON, excessive nesting (maximum 16 JSON
containers), and unsupported schema versions invalidate the snapshot. No YAML
aliases, tags, includes, glob syntax or executable predicates are interpreted.
The byte cap bounds aggregate input and serialization; additions must also respect
the existing IPC frame limit without truncating safety fields.

An empty `rules` list is a valid explicit empty source. Stable IDs/revisions and
supersession obey typed v1; there is no implicit latest-contract winner. Syntax
and semantic shape/selector invalidity invalidate the whole source before use.
Well-formed competing accepted rules remain an unresolved conflict for their key,
not a parser error. Unreviewed rules remain inactive and never block a key merely
by being pending; malformed records still invalidate their containing source.

## 4. Live identity, freshness and failure table

The daemon checks project/capability/root before adapter access. The adapter takes
that authorized root, not a caller path. Reject a symlink/reparse source, directory,
non-regular file or path outside that root; distinguish a genuinely absent entry
from a dangling link, permission error or failed stat. Perform bounded reads even
if the file grows after metadata inspection. No traversal, link following to another
checkout, persistent cache, database copy or automatic fallback parser.

Source identity is the fixed repository-relative path plus SHA-256 of the exact
whole-file bytes, including prose and line endings. The daemon supplies fresh
project binding and a request-local opaque root label to typed v1; that label is
not a persistent root generation, a native file-identity proof or an exposed path.
Only this authorized project is registered in that resolution input. Source version
is `sha256:<lowercase digest>`; digest is also retained in `SourceRef.digest`.
HEAD alone is insufficient: dirty edits with unchanged HEAD must change identity.
Origin is `daemon_observed` for the read, not independently verified contract review.

Before returning the assembled context, recheck authorized root and source
presence/type/content digest using another bounded read. An observed change refuses
the source for this call without retrying or publishing a previous contract result.
This detects the specified read/assembly interval; a same-user edit after the final
check is outside the cooperative guarantee. No atomic repository/SQLite snapshot
or hostile same-user filesystem sandbox is claimed. A later call rereads current
bytes; no last-known-good or old HEAD fallback is permitted.

| Source state in this read | Agent result |
| --- | --- |
| Absent at initial and final checks | `not_configured`; retain current preference behavior, explicitly disclose that no structured source was configured |
| Valid unchanged empty source | Ready, no contracts; retain current preference behavior |
| Valid unchanged source | Apply typed rules to affected keys, publish sanitized effective collection |
| Malformed, unsupported, oversized, unreadable, wrong type or escaping source | `invalid_source` / `source_unavailable`; publish no effective preferences or contracts; allow unrelated repository metadata to remain inspectable |
| Presence, bytes or authorized-root identity changes during assembly | `source_changed` or existing root refusal; publish no effective preference/contract result from the stale read |
| Applicable contracts disagree / required selector query missing | Partial resolution; suppress the affected key, preserve independently resolved keys |

Absence is not proof that a repository has no rules: ordinary AGENTS/ADRs/config
remain authoritative but unparsed. Removing the optional source between *separate*
reads is a current repository change, not detectable historical staleness without
a cache. The resulting `not_configured` must remain visible. D2 explicitly asks
the owner to accept this optional-source policy rather than quietly treating a
failed source read as absence.

## 5. Resolution adapter, selectors and safe projection

Construct repository rules as typed `Contract` / `Repository` / bound project
scope. Adapt only genuinely confirmed, correctly scoped personal candidates.
Do not attach private preference source, review actor, rationale, decision records,
or historical evidence to the resolver input used for agent projection. Synthetic
adapter reference labels express local record identity, not provenance verification.

Use the frozen pure resolver for each contract-bearing key, supplying all relevant
confirmed project/global candidates so its legacy fallback, compatible
intersection, uncertainty and conflict behavior remain authoritative. The adapter
must not reimplement precedence. Compatible preferences remain eligible; disjoint
preferences are masked; equal-authority conflicting contracts have no winner.
Unknown applicability blocks only its key. More-specific selectors are not stronger
authority. Unreviewed, suggested, rejected, deprecated and foreign-project data
cannot enter effective output. No session constraints/policy boundaries or inferred
repository observations are manufactured for this task.

Profile text may be valid yet unrepresentable by typed v1. Preserve exact bytes:
do not trim, replace, truncate or hash values into a new comparison language. For
an affected key, refuse its effective result if a relevant candidate cannot be
represented, and report a bounded unknown status without echoing that value.
Unrelated keys continue to use existing exact legacy fallback. Tests must cover
multiline values, multibyte values exceeding 4096 bytes, and control-bearing keys.
This proposal does not weaken typed validation or tighten the Profile schema.

With D3, both existing context request DTOs gain an optional `selector` object:
`agent_id`, `path`, `language`, `task_tags`. The MCP tool schema and validation
forward exactly these labels to the same daemon route. No `project`, root,
session, permission or source override is allowed in that object. Top-level Agent
project override remains denied. Unknown fields and malformed populated queries
fail the request; they never become an unrestricted query. Missing labels are
unknown, while supplied empty task tags mean known no tags. AND/OR and path case
rules stay exactly typed v1 on Windows and Linux. Agent IDs are applicability
labels, not authenticated identities; D3 does not make selectors a security gate.

Proposed response keeps the existing safe `preferences`, repository and document
fields. `preferences` contains only surviving personal values, never synthesized
contract rows. Add:

- `contract_resolution`: bounded status plus repository contract keys requiring
  review/query/refresh; no excluded personal values or private rule/source IDs.
- `effective_contracts`: resolved keys, compatible `allowed_values`, repository
  rule IDs and `{path, sha256}` from the current source. Unresolved keys are absent.
  Values reflect the typed effective intersection, including a compatible personal
  narrowing; these are derived context, never capabilities.

Do not serialize `EffectiveContextV1` wholesale: its explanations include excluded
personal content and provenance. Raw parser/SQLite diagnostics and absolute paths
never enter responses or logs. `include_documents` continues to control raw bodies
of the existing five document references; it neither disables contract enforcement
nor implicitly exports the new Markdown body. Repository contract values in the
approved effective projection are intentionally shared context.

## 6. Implementation and acceptance after owner approval

Use one dependent Draft PR based on PR #61's branch, allowing joint review. Keep
the contract-first commit separate from implementation and test/evidence commits.

1. Freeze D1–D4 here after owner confirmation; implement bounded parser/adapter
   and pure regression tests without Profile DDL or writer changes.
2. Integrate the single production daemon context route and D3 DTO/MCP schema;
   test actual responses through a separately launched daemon and MCP bridge.
3. Commit clean source, run affected per-OS gates, record exact code SHA, commands,
   outcomes and limits; update #61's blocked disposition only from actual evidence.
   Owner milestone closure remains a separate review decision.

| Gate | Required evidence after implementation |
| --- | --- |
| Parser / bounded source | Strict syntax/version/unknown/duplicate fields, nesting/count/byte caps, CRLF/LF, ordinary-fence examples, absent/empty/malformed/unreadable sources, symlink/reparse refusal, deterministic order |
| Pure bridge | Conflicting confirmed project/global, compatible global fallback, inactive exclusion, distinct projects, duplicate revisions/supersession, contract intersections, unknown/known/nonmatching and malformed selectors; valid Profile text outside typed bounds |
| Independent daemon + IPC + MCP | Actual masking and safe effective contract output on both routes; unrelated fallback preserved; no private source/provenance/decisions/inactive/foreign sentinels; exactly two readonly tools, method/field/project override denial |
| Live adversarial process checks | Dirty same-HEAD edit, source add/delete/rename/replacement and root drift between controlled read/admission checkpoints; invalid source never becomes empty; session revoke/publication refusal remains effective |
| Native Windows and Linux/WSL | Targeted resolver/process tests, workspace fmt/Clippy with warnings denied/tests and source checks on one clean code SHA; no GUI input or unrelated screenshots for this backend-only change |

Apply the owner's updated regression cadence from the 2026-11-16 re-review and
Issue #65. PR #61's clean `19942b6e8ba58e990e60095bf6440d572cda6123`
four-surface UI cohort remains historical evidence for that UI, **not proof for a
later source SHA**. Reuse it by exact attribution. Repeat full native UI matrices
only for a major UI milestone or material shared NSH behavior change. No NSH
change is proposed here. Same-physical-machine Windows/WSL exception remains
limited to M4; independent human/physical-host acceptance is not inferred.

## 7. Current evidence and remaining work

This proposal changes documentation only. Architecture audit on Linux/WSL used
unchanged product sources from `95a3905ae8bcded2084ad0af22edb918d55513b5`:

- `cargo test -p acp-core --test typed_context --test read_only --locked --offline`:
  PASS, 18 typed-context and 12 read-only tests. These are existing component tests,
  not #65 integration tests; proposal documentation was uncommitted during this run.
- `node tools/check_source.mjs`: PASS, 241 source paths / 34 archives at initial
  proposal validation; local links, workflow targets and SDK pins checked.
- `git diff --check`: PASS for the documentation delta.

Their success does not establish parser or production IPC/MCP enforcement. Implementation,
freshness fault injection and native Windows integration for #65 remain NOT_RUN
until the boundary is approved and implemented. No hosted Actions/model calls,
real Profile/auth/external checkout access, system configuration or production
activation. PR #61, #60 and #52 remain open; no milestone is declared complete.
