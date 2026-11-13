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
| Bounded native desktop RPC in the acceptance driver | **implemented (harness only)** | correlated, cancelable `tools/native-acceptance/src/worker.rs:222`, `:437`, `:514` | none in S0; re-verify the driver at each later tested SHA (section 6) |

## 2. Four-surface acceptance matrix

All four surfaces must be driven by real OS input against a real owned daemon and a
real owned synthetic Profile. Every cell below is **NOT_RUN in S0**, because the
review behavior does not exist yet; recording the requirement is the deliverable.

| Boundary | Windows Desktop (real HWND) | WSLg native Wayland | Windows ConPTY | Linux/WSL PTY |
| --- | --- | --- | --- | --- |
| Queue page and watermark rendered from live daemon read | required: real `alt` chord + rendered revision/digest evidence | required | required (TUI over ConPTY) | required (TUI over PTY) |
| Inspect exact record + digest + provenance label | required | required | required | required |
| Confirm with digest binding; peer-demotion disclosure | required | required | required: same shared action, real chord over ConPTY | required: same shared action, real chord over PTY |
| Reject | required | required | required | required |
| Deprecate (proposed `alt+shift+w`) | required | required | required | required |
| Delete with destructive tier (proposed `alt+shift+q`) | required | required | required | required |
| History view shows decision rows; deleted leaves none | required | required | required | required |
| Stale revision / changed scope / revoked grant refusal | required | required | required | required |
| `OutcomeUnknown` then identical same-epoch replay, receipt equal, no second write | required | required | required | required |
| Agent/MCP denial of queue, decisions, provenance | required once on the desktop surface with a live client | required once | NOT_RUN without a real client | NOT_RUN without a real client |

The five review actions are one shared-model flow, so they are required on the TUI
surfaces as well as the desktop surfaces. A terminal that cannot deliver the chord is
a **BLOCKED** finding for that surface with the observed key event recorded, never a
reason to mark the row `NOT_APPLICABLE`. Desktop-only review actions would contradict
the parent #52 shared Desktop/TUI goal and require the owner decision in
[contract section 10](operator-review-contract.md#10-owner-decisions-required-before-s1-and-s2)
(D11) plus a contract amendment first.

Rules that define PASS, independent of environment:

- Both hosts measure the same source: `.gitattributes` pins Unix LF, so a native
  Windows checkout is byte-equal to the Linux/WSL tree and each host's own
  `git status --porcelain` must be empty on the tested revision. A file copy or an
  end-of-line-normalized checkout is not byte identity; when Git disagrees between
  hosts about cleanliness, stop and fix the attributes rather than relabel the run.

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
| **M4/S2 — review UX** | S1 merged and its evidence committed; D6, D8 and D11 accepted by the owner; queue/detail/history panes and the destructive tier implemented in the shared model only (no TUI-specific rule); driver chord set extended only by an explicitly authorized commit; component plus native surface checks green on all four surfaces |
| **M4/S3 — full acceptance** | S2 merged; the whole section 2 matrix executed on clean committed SHAs on real Windows Desktop, WSLg Wayland, ConPTY and Linux PTY with per-surface PASS/FAIL/BLOCKED/NOT_RUN, binary hashes, and the section 5 driver gate green on both hosts |

S0 authorizes neither S1 nor any product code change. This Draft PR existing is not
permission to start implementation.

## 5. Bounded native desktop RPC (S0 Deliverable B)

Before this slice, `Desktop::rpc` wrote one JSON line and then blocked in
`BufReader<ChildStdout>::read_line` with no per-call bound; the only deadlines lived in
the outer loops — 30 s window appearance, 900 s foreground readiness — so one stalled
worker response wedged the run instead of failing it. The reachability is real:
`Owned-Window` enumerates top-level windows and calls `GetWindowText`, which sends
`WM_GETTEXT` to the owning thread and waits, so a GUI thread that stopped pumping blocks
the synchronous PowerShell worker indefinitely (`tools/native_window.ps1:32-49`, `:11`).

The pipe now runs through `WorkerRpc` (`tools/native-acceptance/src/worker.rs`). This is
harness-only: product IPC, daemon, UI and permissions are untouched.

1. Every request carries a monotonic `rpc_id` on the wire (`worker.rs:239`) and the
   reply must name it (`:594`). Absent, non-numeric, mismatched, duplicated or
   unsolicited correlation fails closed and marks the session unusable, so an older
   reply can never settle a newer action — including when it arrives only after that
   newer request was written (`stale-after-next` mode, tested without any sleep).
2. One deadline covers the write and the reply (`worker.rs:222`). The write is a job on
   the writer thread (`:437`) that the caller can cancel; a worker that stops consuming
   stdin is driven until the pipe refuses bytes and each attempt still returns inside its
   own bound instead of blocking on a full pipe.
3. Both IO threads are cooperative: an explicit stop flag ends the reader (`:514`) and
   the writer, teardown joins them (`:427`) and reports `reader_joined` plus
   `writer_joined` rather than assuming them. Neither thread may block past one poll —
   Unix sets `O_NONBLOCK` on both ends, Windows sets `PIPE_NOWAIT` only on the write end
   and polls the read end with `PeekNamedPipe`, because `PIPE_NOWAIT` is not honored for
   the read end of an anonymous pipe and a held stdout would otherwise leak the reader.
   `DESKTOP_IO_MODE` records the mode actually achieved. Only the child this harness
   spawned is terminated and reaped; an unjoined thread is a reported failure, never a
   PASS. `hold-stdout` (a grandchild the harness never owns keeps the pipe open) and
   `crash` (abnormal exit) are the negative cases; the Windows case is what produced the
   `PeekNamedPipe` strategy.
4. Finite foreground waiting stays distinct from per-request deadlines;
   `foreground_required` remains an explicit state, not a stalled pipe.
5. Nonce, exact PID and WSLg host-process ownership, the frontmost requirement and
   English-layout restoration semantics are unchanged. Request and reply sizes stay
   bounded (`:24`, `:26`); a reply is never echoed into evidence, only its byte length.
6. The owned standalone Rust fake-worker binary (no interpreter dependency reintroduced)
   proves the above for healthy, silent, EOF, crash, malformed, oversized, late,
   duplicate, stale, unsolicited, shapeless, wrong-nonce, refusal, no-stdin-read and
   hold-stdout behaviour in `tests/desktop_worker.rs`, on both hosts, without any real
   window. These tests prove driver boundedness only; they never certify a native GUI
   PASS, and the real-window runs in the evidence report stay a separate evidence class.

## 6. Gates and evidence honesty

- Commit order is contract first, then the harness change; product DDL, writer,
  daemon, core, IPC, CLI and UI code stay untouched by this PR, and no new MCP method
  is added.
- Harness gates on the clean committed SHA:
  `cargo fmt --manifest-path tools/native-acceptance/Cargo.toml -- --check`,
  `cargo clippy --manifest-path tools/native-acceptance/Cargo.toml --all-targets --locked -- -D warnings`,
  `cargo build --manifest-path tools/native-acceptance/Cargo.toml --locked` before any
  native run (a running executable blocks its own replacement on Windows), then
  `cargo test --manifest-path tools/native-acceptance/Cargo.toml --locked` plus the
  targeted fake-worker cases. S0 also ran the full workspace/shared/NSH suites on both
  hosts rather than relying on the unchanged-product argument; a future slice may report
  the rationale plus current regression linkage instead, but never silently skips.
- Full gates use `tools/local_verify.sh all NEW_REPORT_DIR FULL_MERGED_BASE_SHA`
  and `tools/windows_local_verify.ps1 -Suite All -ReportDir NEW_REPORT_DIR -DiffBase FULL_MERGED_BASE_SHA`.
  Both require a clean tree and a 40-character ancestor baseline, and both run
  `git diff --check BASE HEAD`: a clean working tree is never a substitute for that
  diff gate. Build the product binaries first and the driver before a native run, and
  keep every helper script, bundle, manifest and report inside the repository's ignored
  artifact tree rather than beside the checkout.
- Evidence goes to `docs/evaluation/` with exact source SHA, diff base, commands,
  per-host PASS/FAIL/BLOCKED/NOT_RUN and binary hashes. Raw logs and screenshots stay
  in the ignored local artifact root. `node tools/check_source.mjs` runs locally
  because every commit carries `[skip ci]`, so hosted Actions stay NOT_RUN — that is a
  statement about CI, not a test result.
- S5's native acceptance PASS belongs to its tested SHA
  (`ddc12ac8c17215014e04cc65ed802819b1bc8376`); a new harness revision does not
  recertify the product and is not presented as a fresh product PASS.
