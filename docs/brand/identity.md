# ACP brand exploration — P1-F / B0

Status: **PENDING owner selection**. This is a proposal, not an adopted identity.
Working product name remains **Agent Control Plane (ACP)**. Repository, executable,
CLI, protocol, license and public API names are unchanged. ADR-0003 is Proposed.

## Position and voice

A local-first context and policy layer for developers operating coding agents.
The console makes project scope, preference provenance, previous Handoff evidence
and observed local service state understandable. Its authority comes from current
repository facts and accepted ADRs, not a brand treatment or stored preferences.

Use precise, calm, direct language. Name the observation, its scope and the next
available action. Say “Last successful read” rather than “Always online”; say
“Recorded validation” rather than “Tests passing” when evidence is historical.
Use explicit Unknown / Not run / Access refused states. No invented histories,
security certification, decorative charts, marketing slogans or activity pulses.

## Three candidate directions

These are visual direction labels, **not proposed product renames**. The preview
has actual NSH Desktop/TUI rendering, two illustrative pages and synthetic text
only. It never connects to a daemon, reads a repository or opens a user database.

| | A — Instrument / 精密仪表 | B — Ledger / 工作手册 | C — Harbor / 分层工作台 |
| --- | --- | --- | --- |
| Character | Compact, exact, technical | Warm, deliberate, evidence-led | Calm, approachable, clearly grouped |
| Name sketch | `[ ACP ]`, monospaced, square frame implied by text | `ACP /`, serif heading, numbered sections | `= ACP`, two text rules suggest parallel planes |
| Background / panel | `#111718` / `#1B2425` | `#F4F0E7` / `#FFFCF5` | `#131C30` / `#202D46` |
| Text / secondary | `#EAF2EE` / `#A6BAB2` | `#302B25` / `#695E50` | `#EDF2FC` / `#B3C2DB` |
| Accent / warning | `#9EE6BE` / `#F3CC86` | `#8E3E26` / `#805005` | `#B5C8FF` / `#FFD394` |
| Essential border | `#738D83` | `#8B7E6A` | `#7C91B4` |
| Shape / grouping | 2px corners, single-column instrument panels | Square paper blocks, chapter labels | 12px corners, paired summary panels when wide |
| Typography | System sans body, mono wordmark/Git values | System serif headings, sans body, mono Git values | System sans headings/body, mono technical values |
| Dashboard | Stacked measured facts, restrained mint emphasis | Numbered repository/preference/resume sections | Side-by-side repository/preferences, continuation below |
| Handoff | Objective → action → uncertainty → recorded checks | A short evidence-led working note | Separate action, uncertainty and evidence surfaces |
| TUI | Compact rules, labels before values | Same chapters; terminal font, no serif imitation | Paired desktop panels stack; preserve labels |
| Tradeoff | Highest technical density; can feel austere | Strongest difference from generic developer tools; serif varies by OS | Easiest visual grouping; uses more space and is closer to familiar workspace UI |

All directions use a 14px desktop body and 12px metadata; Ledger explores 26px
serif titles versus 22px sans titles. Terminal typography uses the owner's terminal
font/cell metrics. Compact layout uses wrapping controls and a stacked content
column. CJK/emoji use system fallback; no font download or font binary is added.

### Measured candidate contrast

WCAG relative-luminance calculation of the opaque token pairs (sRGB). Minimum
across background/panel/inset; these are palette measurements, not a complete
accessibility audit or a claim about all final interaction states.

| Candidate | Main text | Secondary text | Accent text | Warning text | Essential border | Selected text/accent |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| A | 11.48:1 | 6.41:1 | 9.04:1 | 8.59:1 | 3.65:1 | 11.36:1 |
| B | 10.78:1 | 4.87:1 | 5.63:1 | 5.26:1 | 3.05:1 | 7.15:1 |
| C | 10.02:1 | 6.24:1 | 6.79:1 | 8.03:1 | 3.52:1 | 9.20:1 |

Normal text target is >=4.5:1; essential control boundary target is >=3:1. All
candidate preview colors meet those targets. Final light/dark/high-contrast,
hover/pressed/disabled and actual keyboard focus require the selected-system pass.

## Mark, icon and asset policy

The three text sketches are provisional name treatments, not official logos.
Do not set an application icon, rename a package or publish a new identity from
this exploration. If chosen, a small code-defined mark can be reviewed separately.
Use a consistent simple outline/rule vocabulary with visible text labels; never
rely on color or an unfamiliar icon alone. Avoid raster artwork, gradients,
decorative motion and external font services. Existing repository licenses apply
to new code; no stock assets, third-party font binaries or new artwork licenses.

## B0 review record

- Selected direction: **PENDING**.
- Owner revision notes: **PENDING**; no feedback is inferred from E4 acceptance.
- E4 base UI was accepted as a starting point only, not a brand choice.
- Review actual synthetic desktop at 1080×760 and 640×480 and terminal adaptation
  at 160×50 / 80×30. Choose A/B/C or request a concrete combination/revision.
- After selection, implement one reusable semantic system and all five pages on
  this branch. Until then, keep the normal console on its existing E4 design.

See [design-system staging](../p1/ui-design-system.md) and
[local evidence](../evaluation/p1-f-local-ux.md).
