# P1-M4/S0 — Review/provenance gap and four-surface acceptance matrix

Companion to the [operator review and provenance contract](operator-review-contract.md).
Status: **S0 freeze, not implementation.** Baseline merged M2/S5 main
`18f2670d294deebbc17f530c36bebecdaf8beb90`; authority is the same as that contract's
header. Markers: **implemented** (present in current source), **contract-only**
(named by an accepted ADR/frozen doc with no consumer), **proposed** (needs owner
acceptance, see contract section 10), **out of scope** (not delivered by M4).

## 1. Gap inventory

| Concept / capability | Marker | Current evidence | Gap that S1/S2 must close |
| --- | --- | --- | --- |
| Fresh Profile v1, strict reader, `user_version=1` refusal | implemented | `crates/acp-store/src/profile.rs:235`, `crates/acp-write/src/profile.rs:38` | allowlist becomes six tables / two indexes only under D1 |
| Seven typed mutations | implemented | `crates/acp-core/src/operator.rs:88`; dispatch `crates/acp-write/src/writer.rs:202-342` | `origin` on suggest; `proposal_digest`+`channel` on the four decision commands |
| Status transitions `suggested→confirmed\|rejected`, `confirmed→deprecated` | implemented | `crates/acp-write/src/writer.rs:291` | unchanged; no new transition is authorized |
| Peer demotion on confirm | implemented but **unexplained** | `crates/acp-write/src/writer.rs:296-309` | nothing records *why* a row became `deprecated`; needs `demote` decision row (D2) |
| Per-row revision + CAS | implemented | `crates/acp-write/src/writer.rs:38-47` | unchanged; digest binds on top, never replaces CAS |
| Scope isolation `global` XOR `project` | implemented | `crates/acp-store/src/profile.rs:40-43`, `crates/acp-write/src/writer.rs:280` | queue/inspect must reuse exact-scope predicate; no union |
| Transaction receipt + idempotent same-epoch replay | implemented | `crates/acp-write/src/writer.rs:96-164` | receipt stays transaction proof only; never review identity |
| `OutcomeUnknown` + retained `operation_id` | implemented | `crates/acp-write/src/writer.rs:174-189`, `crates/acp-ipc/src/operator.rs:57` | unchanged; UI/CLI retry keeps identical bytes |
| Write grant separate from read/session, per-request revocation | implemented | `crates/acp-ipc/src/write_grant.rs:37`, `crates/acp-daemon/src/profiles.rs:229` | unchanged |
| Operator bounded previews (12 rows, `next_after_id`) | implemented | `crates/acp-store/src/operator.rs:19-38` | extend with `commit_sequence` watermark; no stable multi-page snapshot |
| Handoff saved history + revision | implemented | `crates/acp-store/src/profile.rs:47-63`, `crates/acp-core/src/operator.rs:161` | saved history stays distinct from live reads; not review state |
| Historical validation statuses `PASS\|FAIL\|SKIPPED\|NOT_RUN` | implemented | `crates/acp-core/src/lib.rs:70-84` | never renewable; `Validation.extra` flatten preserves unknown keys, so it cannot be sealed |
| Free-text preference `source` | implemented, weak | `crates/acp-store/src/profile.rs:36`; immutable after insert | not provenance; needs closed `origin` beside it (D4) |
| Pending review queue | **absent / proposed** | closest is unfiltered `Pane::Preferences` rows `spikes/p1-e/nsh/src/operator.rs:559` | `profile.review.queue` over `status='suggested'`, ID-ordered, watermarked |
| Proposal content digest | **absent / proposed** | `request_digest` is idempotency-only `crates/acp-write/src/writer.rs:96` | new `preferences.proposal_digest`, bound into each decision (D3) |
| Decision history (actor/time/cause) | **absent / proposed** | no `reviewed_by`, `decided_at`, note or cause anywhere | `acp_review_decisions`, `channel` label only (D2, D5) |
| Provenance object with origin/references | **contract-only** | `crates/acp-core/src/typed_context/types.rs:105`; module has zero consumers outside `acp-core` | adopt vocabulary or justify a second one; no parallel enum (D4) |
| `ReviewState::Unreviewed/Accepted` | contract-only (repository contracts) | `crates/acp-core/src/typed_context/types.rs:64` | never persisted as a profile status; local review cannot accept a contract |
| Deprecate/delete UX | **absent / proposed** | CLI-only gate `spikes/p1-e/shared/src/i18n.rs:62`; no arm in `shared/src/operator.rs:148-223` | S2 adds both with the destructive tier (D8) |
| High-risk confirmation tier | **absent / proposed** | one generic review card; `OperatorPeerWarning` is plain text `shared/src/i18n.rs:61` | delete needs exact-ID re-entry + explicit removal disclosure |
| Diff view | **out of scope for M4** | only a hashed worktree fingerprint `crates/acp-core/src/fingerprint.rs:12` | preference "diff" is the digest of an immutable row; repository diffs stay out of scope |
| Minimal Profile controls beyond create/select/info/grant | out of scope | `crates/acp-daemon/src/profiles.rs:301-451` | no export, reset, delete-profile or backup in M4 |
| ProjectContract approval from the UI | out of scope | ADR-0004 human-review section | repository change stays a repository review |
| MCP surface | implemented, frozen | exactly two tools `crates/acp-mcp/src/main.rs:20-25` | review/provenance never exposed to agents; no third tool |
| Agent visibility of non-confirmed values | implemented as a refusal | confirmed-only filter `crates/acp-core/src/lib.rs:215` | regression must keep failing if the filter widens |
| Registration / Handoff put in UI | out of scope (S3 gate) | `native-operator-workflows.md` CLI-only list | stays CLI-only until separately authorized |
| Bounded native desktop RPC in the acceptance driver | **implemented as a gap** | unbounded `BufReader::read_line` `tools/native-acceptance/src/desktop.rs:100-106` | section 5 |

## 2. Four-surface acceptance matrix

All four surfaces must be driven by real OS input against a real owned daemon and a
real owned synthetic Profile. Every cell below is **NOT_RUN in S0**, because the
review behavior does not exist yet; recording the requirement is the deliverable.

| Boundary | Windows Desktop (real HWND) | WSLg native Wayland | Windows ConPTY | Linux/WSL PTY |
| --- | --- | --- | --- | --- |
| Queue page and watermark rendered from live daemon read | required: real `alt` chord + rendered revision/digest evidence | required | required (TUI over ConPTY) | required (TUI over PTY) |
| Inspect exact record + digest + provenance label | required | required | required | required |
| Confirm with digest binding; peer-demotion disclosure | required | required | NOT_APPLICABLE (desktop-only panel) | NOT_APPLICABLE |
| Reject | required | required | NOT_APPLICABLE | NOT_APPLICABLE |
| Deprecate (proposed `alt+shift+w`) | required | required | NOT_APPLICABLE | NOT_APPLICABLE |
| Delete with destructive tier (proposed `alt+shift+q`) | required | required | NOT_APPLICABLE | NOT_APPLICABLE |
| History view shows decision rows; deleted leaves none | required | required | NOT_APPLICABLE | NOT_APPLICABLE |
| Stale revision / changed scope / revoked grant refusal | required | required | required | required |
| `OutcomeUnknown` then identical same-epoch replay, receipt equal, no second write | required | required | required | required |
| Agent/MCP denial of queue, decisions, provenance | required once on the desktop surface with a live client | required once | NOT_RUN without a real client | NOT_RUN without a real client |

Rules that define PASS, independent of environment:

- Headless runs, the Web build, X11/XWayland and Xvfb do **not** substitute for
  native acceptance. niri and mobile are out of scope for M4.
- A window is locked by its unique nonce title plus owning process identity
  (`tools/native_window.ps1:32-49`); the driver never force-focuses and never sends
  input to a window it does not own. `ctrl+1` stays the user's screenshot key.
- CLI-read data is not GUI-rendered data. A rendered assertion needs the marker from
  the owning process' own probe output, as in the S5 Alt+Shift+H saved-revision probe
  ([S5 evidence](../evaluation/p1-m2-s5-rust-only-integrated-acceptance.md)).
- Real OS input, component test, model test and fault injection are reported as
  separate evidence classes; a unit test never becomes a native PASS, and a fake
  worker test never becomes a window PASS.
- Fixtures are one-shot owned synthetic roots only: fresh temp Profile per case,
  independent IPC endpoint, `--allow-project`/`--allow-root` pinned to the fixture
  checkout. No real ACP database, no external development repo, no credentials, no
  model or provider call, no installation, and no authentication/runtime directory
  is deleted.
- Environment restoration is part of the gate: Windows fixtures restore the caller
  process environment (especially `PATH` and Git configuration) and only select an
  already-installed English layout for the owned window thread; nothing is installed
  and no persistent system setting is modified.

## 3. Presentation copy obligations

| State | Required copy behavior |
| --- | --- |
| empty queue | "no pending suggestions", never an error and never "unavailable" |
| unavailable daemon / no write grant | names which one is missing; controls stay disabled, never fake-active |
| not-yet-implemented control | labelled CLI-only or not available, per the existing `OperatorCliGate` pattern |
| stale page after new commits | states the watermark delta; never silently reorders |
| digest mismatch refusal | "content differs from what was reviewed; re-inspect", never "write failed" |
| deleted preference | states that content and decision history are gone and receipts remain identity-only |
| historical Handoff validation | keeps the existing "recorded evidence, not current verification" wording |
| both locales | English and zh-CN shipped together; `sanitize()` bounds stay applied |

## 4. Handoff criteria

| Stage | Must be true before the next stage starts |
| --- | --- |
| **M4/S1 — persistence and IPC** | D1-D5, D7, D9, D10 accepted by the owner; DDL replacement in one breaking commit with regenerated fixtures and updated `inspect()` allowlist; writer/daemon/CLI/NSH bumped together with `Request.version` 2 and no v1 mutation reader; storage + real-process IPC tests green on both native hosts |
| **M4/S2 — review UX** | S1 merged and its evidence committed; queue/detail/history panes and the destructive tier implemented in the shared model only (no TUI-specific rule); driver chord set extended only by an explicitly authorized commit; component plus native surface checks green |
| **M4/S3 — full acceptance** | S2 merged; the whole section 2 matrix executed on clean committed SHAs on real Windows Desktop, WSLg Wayland, ConPTY and Linux PTY with per-surface PASS/FAIL/BLOCKED/NOT_RUN, binary hashes, and the section 5 driver gate green on both hosts |

S0 authorizes neither S1 nor any product code change. This Draft PR existing is not
permission to start implementation.

## 5. Bounded native desktop RPC (S0 Deliverable B)

`Desktop::rpc` writes one JSON line and then blocks in
`BufReader<ChildStdout>::read_line` with no per-call bound
(`tools/native-acceptance/src/desktop.rs:100-106`). The only deadlines live in the
outer loops — 30 s window appearance (`:81`), 900 s foreground readiness (`:108`) —
so one blocked worker response wedges the run instead of failing it. This is
reachable for real: `Owned-Window` enumerates top-level windows and calls
`GetWindowText`, which sends `WM_GETTEXT` to the owning thread and waits, so a GUI
thread that stopped pumping blocks the synchronous PowerShell worker indefinitely
(`tools/native_window.ps1:32-49`, `:11`).

Required properties, harness-only (product IPC, daemon, UI and permissions are
untouched):

1. Every request carries an explicit per-request deadline; the outstanding request is
   tracked with a monotonic sequence number and exactly one response may settle it.
2. A hung worker, a malformed line, an oversized line, worker EOF, an unsolicited
   response and a second response to one request all fail closed and mark the session
   unusable, so a late response can never be mistaken for a later action's ack.
3. Timeout kills and reaps only the owned worker and the owned GUI child; the reader
   thread is unblocked through the child's own stdout EOF and joined at teardown
   instead of detached.
4. Finite foreground waiting stays distinct from per-request deadlines;
   `foreground_required` remains an explicit state, not a stalled pipe.
5. Nonce, exact PID and WSLg host-process ownership plus the frontmost requirement
   and English-layout restoration semantics are preserved unchanged.
6. A standalone Rust fake-worker binary (no interpreter dependency reintroduced)
   proves finite failure and cleanup for healthy, silent, EOF, malformed, oversized,
   late and duplicate replies, on both hosts and without any real window. These tests
   prove driver boundedness only; they never certify a native GUI PASS.

## 6. Gates and evidence honesty

- Commit order is contract first, then the harness change; product DDL, writer,
  daemon, core, IPC, CLI and UI code stay untouched by this PR, and no new MCP method
  is added.
- Harness gates on the clean committed SHA:
  `cargo fmt --manifest-path tools/native-acceptance/Cargo.toml -- --check`,
  `cargo clippy --manifest-path tools/native-acceptance/Cargo.toml --all-targets --locked -- -D warnings`,
  `cargo test --manifest-path tools/native-acceptance/Cargo.toml --locked`, plus the
  targeted fake-worker cases. Because only `tools/native-acceptance` and `docs/`
  change, the full workspace/NSH suites are reported with a precise unchanged-product
  rationale and the current S5 regression linkage, not silently skipped.
- Full gates, when run, use `tools/local_verify.sh all NEW_REPORT_DIR FULL_MERGED_BASE_SHA`
  and `tools/windows_local_verify.ps1 -Suite All -ReportDir NEW_REPORT_DIR -DiffBase FULL_MERGED_BASE_SHA`.
  Both already require a clean tree and a 40-character ancestor baseline, and both run
  `git diff --check BASE HEAD`: a clean working tree is never a substitute for that
  diff gate.
- Evidence goes to `docs/evaluation/` with exact source SHA, diff base, commands,
  per-host PASS/FAIL/BLOCKED/NOT_RUN and binary hashes. Raw logs and screenshots stay
  in the ignored local artifact root. `node tools/check_source.mjs` runs locally
  because every commit carries `[skip ci]`, so hosted Actions stay NOT_RUN — that is a
  statement about CI, not a test result.
- S5's native acceptance PASS belongs to its tested SHA
  (`ddc12ac8c17215014e04cc65ed802819b1bc8376`); a new harness revision does not
  recertify the product and is not presented as a fresh product PASS.
