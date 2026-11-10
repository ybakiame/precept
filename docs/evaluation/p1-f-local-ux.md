# P1-F local brand/UI evidence

Status: **B0 exploration; owner selection PENDING**. No final brand acceptance or
complete five-page redesign is claimed. Start: latest main merge #21,
`0ba227c077c2d4fd0176adc4fbb43730a998c17b`; branch `feat/p1-f-brand-ui`.
No GitHub Actions, niri or mobile tests. ADR-0003 remains Proposed.

## Scope and decision gate

Three independent visual directions use synthetic-only actual NSH Desktop/TUI
previews. Default console behavior and production backend are unchanged. Preview
arguments reject IPC location, credentials, project and WSL helper options; the
preview does not initialize native inputs or access a user database/repository.
Illustrative recorded checks are labeled historical; current verification is
explicitly Unknown/NOT_RUN. No fabricated measurement or live connection.

Candidate identity and tradeoffs: [identity](../brand/identity.md).
Implementation staging: [design system](../p1/ui-design-system.md).
Owner-selected direction/revisions: **PENDING**. Stop after proposals/base preview
components until the owner chooses; full local acceptance and Draft PR follow later.

## Private evidence

All raw synthetic screenshots, terminal frames, logs, measurements and host data
stay under gitignored `artifacts/local-p1-f/`. Native capture uses only the owned
HWND via PrintWindow, never a desktop crop/fallback. WSLg must observe actual
GdkWaylandDisplay and map/paint; X11/Xvfb is not a substitute.

Local run results, exact preview source revision and screenshot locations will be
recorded after execution. The final selected-system Windows/WSLg/PTy/ConPTY, 10k,
fingerprint, MCP security, parity/recovery and Codex matrix is **PENDING**, not
inferred from E4's passing evidence. Native clipboard, full IME/screen reader and
macOS remain NOT_RUN unless explicitly executed on an available host.

## Owner-authorized historical branch cleanup

After fresh fetch and checking PR head tips/merged state and worktree use, twelve
old remote branches were atomically deleted with exact-tip lease guards. The
eleven tips listed in the [E4 audit](../p1/e4-branch-audit.md) were unchanged; the
additional E4 branch tip was `5e23b5ad39011528c03b6acd43463a67fddc54e5` (merged #21).
The two divergent historical refs match the completed E4 content/migration audit;
the other ten are ancestors of current main. No unresolved unique work was lost.

Remote now has only main; the new design branch has not been pushed at this stage.
Local has main plus `feat/p1-f-brand-ui`. Old local E4 was deleted with normal
`git branch -d` after verifying containment in main. Detached/prunable worktree
metadata is intact. No force-push of main or repository-wide setting change.
Private exact before/after refs and command results: `branch-audit/` in the P1-F
artifact directory. GitHub PR review records remain intact.
