# Frozen acp-worktree-v1 contract

The Python reference is `context.worktree_fingerprint`; Rust uses the pure
`acp_core::fingerprint::FingerprintV1` accumulator and the trusted daemon Git/file
adapter. No Git/process/DB mutation enters the core. The algorithm is v1; no
normalization or new fingerprint format is introduced.

## Exact bytes

Run shell-free Git in the registered, verified repository root:

1. `git status --porcelain=v1 -z --untracked-files=all` (raw stdout).
2. `git diff --binary --no-ext-diff --no-textconv HEAD --` (raw stdout).
3. Return unavailable on failure, missing HEAD, timeout, or if status+diff exceeds
   16 MiB. Each Git child has a bounded deadline (metadata 4s, fingerprint 6s).
4. SHA-256 stream: `acp-worktree-v1\0`, status bytes, `tracked-diff\0`, diff bytes.
5. Split status on NUL; collect every record starting with `?? `, removing exactly
   those three bytes. More than 500 returns unavailable. Sort filenames as raw
   bytes, not Unicode/codepoint/localized order. This preserves the reference's
   prefix parsing even for the second record of a rename whose name starts `?? `.
6. For each sorted untracked name: append `untracked\0`, raw filename bytes,
   decimal ASCII `lstat.st_mode` with **no additional separator**. A symlink
   appends only its raw link target (never target contents). A regular file appends
   its exact bytes in 65,536-byte chunks. Other kinds/unsafe/unreadable paths fail.
7. The regular-file byte budget includes status+diff+regular contents, with exactly
   16 MiB allowed. v1 does not charge symlink-target or name/mode bytes to that
   budget. Output is lowercase 64-character hex. Persist/log only that digest.

A clean checkout's fingerprint does not include HEAD/branch directly: their drift
warnings are separate. Identical clean trees on different branches may have equal
fingerprints. This is a bounded content observation, not a worktree attestation.

## Adapter safety and portability

The registered root must match Git's canonical top-level root; untracked parents
must remain confined to that root. Unix regular-file reads use descriptor-relative
`openat`, no-follow parent/leaf flags and nonblocking leaf open; no FIFO/device
content is read. Windows validates the opened handle's final path before reading
regular contents and opens reparse points without dereferencing them. Leaf symlink
hashing reads its target string only. File metadata and later content can race;
there is no atomic repository snapshot and concurrent edits can make it unavailable
or internally inconsistent, just as in v1. Ignored files, submodule internals and
external build/device state are excluded. Repositories with `.gitmodules` or
submodule configuration conservatively return unavailable for status/fingerprint:
recursive Git status could otherwise invoke an unchecked nested filter.

Unix preserves raw non-UTF8 names/targets. Windows maps Git UTF-8 names to native
paths; unsupported encoding returns unavailable. Python Windows regular modes are
`S_IFREG|0444`, plus `0222` unless read-only, plus `0111` for .exe/.com/.bat/.cmd
(case-insensitive). Directory symlinks also receive execute bits. Windows absolute symlink target
bytes preserve Python's `\\?\` / `\\?\UNC\` spelling (Rust's standard read_link
removes it). Unix uses full native `st_mode`. Thus cross-language equality
is required **on the same host/configuration**, not an identical digest between
Windows and Unix modes. Windows symlink permission/encoding cases are labeled
NOT_RUN if the actual host cannot create them; no system/developer-mode changes.

Git output depends on its version/configuration/attributes. The adapter retains
its existing isolated Git environment (no inherited GIT_* selectors, global/system
config, fsmonitor, untracked cache, pager or lazy network fetching). Oracle fixtures
use the same isolated profile and explicit line-ending settings. This avoids
pretending that differing Git diff configurations produce identical bytes.
Configured external clean/process filters are refused before status/diff rather
than running user programs or silently hashing a different conversion. No raw
stdout/stderr, diff, private filenames or file bytes are logged. Stdout is bounded;
stderr is drained concurrently to a sink, and timeout/overflow kills and reaps the
child. Context reads never request the extra status-z/diff/content scan; valid
Handoff reads do. Health advertises on-demand capability without inspecting a root.

## Evidence

Rust oracle tests compare exact reference digests on synthetic clean, staged,
unstaged, binary, rename, non-ASCII, same-name untracked drift, mode/permission,
HEAD/branch, missing HEAD/Git and count/aggregate limits. Unix additionally covers
non-UTF8 names/targets and symlink/unsafe-parent confinement. The real service suite
compares complete Python/Rust Handoff JSON, warnings and recorded validation via
operator IPC and bound MCP, including unchanged HEAD/count with changed content.
No real profile DB or unrelated project is used by this oracle.
