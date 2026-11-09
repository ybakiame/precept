# E4 historical remote branch audit

Freshly fetched against main `80fdbc57e4ec47557e9130808b401a22ad6b913f`.
This table records original remote refs; all remain retained after the owner
changed cleanup to local-only. No worktree was deleted. Original remote listings,
PR metadata, unique commit subjects and diffs remain in private E4 artifacts.

| Remote branch | Exact original tip | main-only / branch-only commits | Associated PR |
| --- | --- | --- | --- |
| `design/p1-rust-daemon-contract` | `dbc5006e03d107ea599d3d3260a4a68648af6e67` | 32 / 0 | #3 MERGED |
| `feat/p1-b-rust-acp-core` | `ae59c629d31d0902b44043a621f8e9d73d9ebbab` | 27 / 0 | #5 MERGED |
| `feat/p1-c1-readonly-sqlite` | `1608abc7fd2d79fc0b82101851df060b63337a37` | 25 / 0 | #7 MERGED |
| `feat/p1-c2a-synthetic-migration` | `538611243e5c0cd3ab138825528a5fe6cf49ee1e` | 23 / 0 | #9 MERGED |
| `feat/p1-c2b1-process-recovery` | `3d2a7973bc4b9e3a02d761b50bdbb704e3eca7ec` | 19 / 0 | #11 MERGED |
| `feat/p1-d-local-daemon` | `1a9d75f6020955cda91e419701b19a5e6372414c` | 12 / 0 | #13 MERGED |
| `feat/p1-e2-local-console` | `b4b7486aabe3ba4eac752cf527a0c7e48d04a32d` | 4 / 0 | #17 MERGED |
| `feat/p1-e3-ui-wslg-wayland` | `82571082b950d79f9ede227c10a26a98f945fc0d` | 1 / 0 | #19 MERGED |
| `spike/p1-e-ui-frameworks` | `c9a85fd7d60d3786cb48684e171602719f174d9a` | 7 / 0 | #15 MERGED |
| `feat/handoff-worktree-fingerprint` | `7c51b1a8add9caa4fe9344a5c0d08c710a7f9558` | 41 / 10 | #1 MERGED |
| `test/p0-mcp-negative-boundaries` | `431b569172f87528197b9f12678cb2792bcae489` | 40 / 5 | #2 MERGED |

The nine branch-only-zero refs are fully contained in main and all corresponding
PRs are merged. For the two divergent refs, every commit/file changed since their
merge base was inspected; divergence is merge/history topology, not missing work:

- Fingerprint branch: Python CLI/context/Store, interface/security docs and original
  operator-reported Codex/Kimi evaluation are byte-identical on baseline main.
  README/roadmap already contain its links and caveats plus later milestones.
  Store-test differences are subsequent Windows-safe connection cleanup; E4 ports
  the two content-drift cases to the exact Rust oracle and live service comparison,
  retaining legacy nullable-schema and Python write/HEAD compatibility tests.
- Negative-boundary branch: original seven-case module, evaluation and roadmap are
  byte-identical on baseline main. README includes its reference plus newer local
  console results. E4 replaces that module using the explicit migration matrix;
  no useful branch-only behavior or documentation remains unresolved.

There is no active worktree attached to these refs. Existing stale/prunable
**detached** worktree metadata is preserved; cleanup does not remove local branches,
worktrees, source files or Git history/PR records. The active E4 branch and main are
excluded. No cherry-pick, `branch -D` or force-push is used.

## Final owner-directed local cleanup

After successful clean Windows/WSL verification at
`22dd5f91e67f71b432f751e047442c9d62708cd4`, the owner directed local cleanup only,
retaining main/current work and leaving remote deletion until GitHub merge.
This supersedes the earlier optional remote-delete plan; no remote delete is pending
agent approval or executed automatically.

| Deleted local branch | Exact retained remote tip |
| --- | --- |
| `feat/handoff-worktree-fingerprint` | `7c51b1a` (full original tip above) |
| `feat/p1-e2-local-console` | `b4b7486` (full original tip above) |
| `feat/p1-e3-ui-wslg-wayland` | `8257108` (full original tip above) |

Before each deletion, local tip equaled its retained remote upstream and no active
worktree used it. Normal `git branch -d` was sufficient, including the historical
fingerprint ref whose exact upstream is retained. No forced branch deletion was
used. Main fast-forwarded safely from `d1cf45c` to `80fdbc5` using a local fetch;
only `main` and `feat/p1-e4-ux-rust-migrations` remain in the working repository.
The detached/prunable worktree metadata remains intact. Before/after local and
remote listings stay private in `artifacts/local-p1-e4/branch-audit/`.

All eleven old remote refs remain unchanged. Repository-wide automatic deletion
settings were not modified. The owner handles GitHub merge and post-merge remote
branch cleanup; this task creates a Draft PR and does not merge it.
