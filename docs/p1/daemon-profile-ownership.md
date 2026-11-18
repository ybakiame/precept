# P1-M2/S2 — daemon Profile ownership and write IPC

Contract-first implementation plan for [Issue #44](https://github.com/ybakiame/agent-control-plane/issues/44),
parent [#40](https://github.com/ybakiame/agent-control-plane/issues/40).
Base main: `be2c4a0` after S1 merge `1976d6f`.
Authority: Accepted ADRs [0004](../adr/0004-product-domain-trust.md),
[0006](../adr/0006-rust-only-runtime.md) and the retained ownership parts of
[0005](../adr/0005-local-storage-and-write-ownership.md); frozen
[Profile v1 contract](profile-v1-runtime-contract.md), especially sections 3–6.
This plan freezes the following implementation choices before product code.
It does not authorize real-profile use, release, or completion of parent M2.

## Bootstrap and selection

> **Superseded in part (2026-11-18).** `--database`, `--allow-project` and
> `--allow-root` are retired. The daemon keeps its project set and the Profile the
> operator asked to select at start in its private `--state-dir`; projects are
> added with the operator-write `project.add` after a daemon-side scope check. See
> the [project-management plan](project-management-plan.md). The selection, attachment and ownership rules below still hold.

- `--database` becomes optional. Without it, only operator health and strict
  profile management are usable (since 2026-11-18 also the operator's
  `project.list` of the daemon-owned set, all unregistered). Project/session/MCP requests cannot run.
- Startup `--database` and `profile.select` open an existing exact v1 read-only;
  missing/legacy/unsafe files fail without creation or repair. `profile.create`
  alone uses S1's exclusive fresh creator, never overwriting a target.
- Selection is single-assignment in one process. After selection, create/select
  returns `RestartRequired`; switching requires orderly restart. No hot switch,
  profile discovery, catalog, implicit grant or default path.
- Validate absolute path, every component, private parent/main-file ownership,
  mode/DACL, link/reparse safety, canonical path and native file identity. Retain
  a native file handle; compare its identity with the validated path before and
  after SQLite open/verification and before publishing selection. Bind profile
  ID/generation independently from the native file identity.
- Check the SQLite connection itself against the native guard: Unix uses the
  bundled VFS `SQLITE_FCNTL_HAS_MOVED` and current path/guard identity; Windows
  compares `SQLITE_FCNTL_WIN32_GET_HANDLE` native identity with the guard.
  Unsupported controls fail closed. These [SQLite controls](https://www.sqlite.org/c3ref/c_fcntl_begin_atomic_write.html)
  are read-only observations, not a custom VFS or schema change. Same-user hostile
  ABA races are not claimed to be OS-isolated. Replacement by a valid copy with
  identical profile metadata is refused on subsequent admissions/reads.

## Explicit project roots and control files

(Historical: the daemon's project set in `projects.json` replaced both flags; the
control-location checks below still apply to every root in it.)

- Retain `--allow-project NAME` as the independent project-name allowlist.
  Existing registered roots are pinned on selection, canonicalized and checked
  against runtime/config/executable control locations as before.
- Add daemon-only `--allow-root NAME=ABSOLUTE_PATH` for names not yet registered
  in a fresh profile. The name must also be allowlisted; roots are explicitly
  canonicalized before serving. `RegisterProject` must match this independent
  root, not merely a root supplied inside the mutation. Existing selected records
  must match any supplied root. Successful registration pins that same root.
- All configured roots are checked against private IPC/control locations and
  agent/bridge configuration before use. No control credential, lease or config
  may live under a shared project checkout. No new CLI/NSH workflow is added.

## Private existing-v1 attachment and ownership

- Add a `daemon-owner` feature on `acp-write`; only `acp-daemon` enables it as a
  normal product dependency. The selected capability opens only verified RO
  state. It cannot expose SQL, a connection or an arbitrary existing-path Writer.
- Its explicit ownership acquisition derives a stable private lock name from
  selected native file identity in the canonical profile parent, opens/validates
  that lock without links, and takes `File::try_lock`. Aliases share the lock;
  contention returns bounded `Busy`. Never unlink the lease file, force-unlock,
  infer ownership from PID/timestamps, or replace an unsafe lock.
- After acquiring the lock, revalidate parent, DB and lock identities, open the
  selected DB without CREATE, verify it fully in `BEGIN IMMEDIATE`, increment
  `owner_epoch` with checked arithmetic, and commit. No grant exists yet.
- The resulting owner retains the native guard and lease and privately invokes
  the S1 writer. File/lock checks occur outside mutation transactions. Admission
  checks happen before RW opening and immediately before BEGIN. Binding and
  complete S1 data/receipt checks remain inside the transaction.
- Failed grant publication can leave an advanced epoch but never usable write
  authority. Successful commands alone affect receipt sequence; epoch advancement
  is ownership metadata, not a domain receipt or automatic generation reset.

## Credentials, admission and revocation

- IPC wire version becomes **2**, coordinated in Rust daemon/client/MCP/session
  callers and health readers. There is no v1 shim. Credential envelope formats
  remain distinct: existing control/agent (1), session agent (2), write grant (3).
- `OperatorWrite` is a separate role with exact profile ID/generation/epoch.
  It authorizes only `profile.mutate`, never grant/control/session/read methods.
  Ordinary operator, agent and session credentials cannot mutate or obtain grants
  through any write-role path. Tokens remain in owner-only runtime files/memory,
  never in IPC result payloads, project files, SQLite, logs or fixtures.
- `profile.write.enable` requires the ordinary operator control role, explicit
  `confirm: true` and the selected profile ID/generation. Publish a fresh private
  `operator-write.cap` only after lease/epoch success. Repeated enable while active
  returns Busy, not another owner/credential. Startup/selection never enables it.
- One serialized daemon write state defines admission. The grant is rechecked
  before RW open and immediately before transaction entry. The revocation path
  closes the in-memory grant before waiting for serialized admitted work, drains
  it, removes only its still-matching private credential and then drops owner/
  lease. Established peers recheck authority on every request.
- A write admitted before revocation may commit and return its actual receipt;
  do not overwrite a committed result with a later authority-check failure.
  Requests after the closed admission point fail without mutation. Reads/session
  responses retain their existing post-computation publication check.
- Runtime-lease-protected restart rejects/removes only validated stale write
  credential files; no grant is restored. Process death releases the kernel lock.
  Re-enable reacquires/revalidates and advances epoch. Old peer/token/epoch is
  unusable; no cross-epoch receipt digest conversion is added.

## Strict wire surface and unchanged storage

All lifecycle DTOs deny unknown fields. Empty params mean exactly `{}`.
Paths are absolute UTF-8 strings bounded to 32768 bytes, with NUL rejected.
`write_enabled` reports a live grant matching the current stored binding; epoch
drift cannot be presented as a current grant. Mutation timeouts with uncertain
outcome are `OutcomeUnknown`; the caller retains the same operation ID.

| Method | Role and params | Result |
| --- | --- | --- |
| `profile.create`, `profile.select` | Operator; `{path: absolute string}` | Sanitized selected identity/info; no path echo |
| `profile.info` | Operator; `{}` | Selected boolean, profile binding and write-enabled state |
| `profile.write.enable` | Operator; `{profile_id, generation, confirm: true}` | Binding/state only; token is never returned |
| `profile.write.disable` | Operator; `{}` | Disabled after admitted work drains |
| `profile.mutate` | OperatorWrite; exact bounded S1 Request | Minimal S1 Receipt |

`profile.mutate` is one strict tagged DTO with exactly the seven existing command
variants, not arbitrary SQL or a generic callback/path API. S1's request version,
128 KiB bound, strict transitions, CAS, scope, raw JSON bytes, receipts, 4096 cap,
no-pruning rule and same-epoch exact replay remain unchanged. New IPC errors are
sanitized typed enum values; no raw payload, path or SQLite diagnostic is returned.
Repository observations for Handoff/registration occur outside transactions.
No Profile DDL change, context scan optimization, migration, GUI or Python work.

## Native acceptance plan

Use only fresh owned synthetic roots on native Windows and Linux/WSL. Implement
Rust tests, not a Python writer/test extension. Report exact clean SHAs and separate
unit faults from real independent processes. Required gates:

1. Unselected restrictions; create/select/info and strict startup; absent/old/unsafe
   refusal; single assignment/restart requirement; no startup/selection grant.
2. Opened-file identity and valid-copy replacement, symlink/junction/hardlink,
   ACL/mode, ancestor and lock substitution negatives, with unchanged sentinels.
3. Seven commands via real IPC, scope/strict transitions/CAS, unchanged complete
   state on malformed/unauthorized/stale/conflicting requests and lost-response
   same-epoch replay. Check project roots against independent launch allowlist.
4. Control/agent/session/MCP mutation/grant denial, including established peers;
   exact grant binding, post-revoke rejection and admitted-write result/drain.
5. Independent daemons with distinct runtimes competing for the same profile,
   canonical aliases, bounded Busy, process kill, lock reacquisition and advanced
   epoch with stale credentials refused. No lease-file replacement/deletion.
6. Full Rust Workspace fmt, native binary build, Clippy `-D warnings`, tests and
   current daemon/IPC/MCP/session real-process regressions. Wire synchronization
   checks only where clients consume IPC v2; no client workflow implementation.

Raw local logs remain ignored/private. Hosted Actions, models/vendors, real
profiles, production activation, macOS/mobile/niri and release are NOT_RUN.
Use `[skip ci]`; create one Draft PR with `Closes #44`, `Refs #40`; never merge.

The [native S2 evidence](../evaluation/p1-m2-s2-ownership.md) records implementation
SHAs, actual tests, development failures and NOT_RUN boundaries. S1's 5/6 scan
measurement is historical: S2 adds selected binding and root admission, and does
not implement the separately proposed snapshot optimization.
