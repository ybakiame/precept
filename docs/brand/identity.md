# ACP identity and theme collection — P1-F

Status: **owner approved the revised base for detailed implementation**. The
owner said “可以以此版本为基础吧” and requested logic/UI refinement, a modest
icon set, Daemon status and Windows GUI access to both Windows and WSL Daemons.
Instrument / Ledger / Harbor form one theme collection. Agent Control Plane (ACP)
remains the working name; no official logo or repository/CLI/API rename is made.
[ADR-0003](../adr/0003-ui-framework-selection.md) now accepts the NSH direction in
P1-J; this identity and its recorded review remain unchanged. After implementation the owner confirmed “没有阻塞问题，
可作为本阶段基础”; the local evidence records that review separately.

## Position and voice

A local-first context and policy layer for developers operating coding agents.
Show project scope, confirmed preference provenance, observed repository facts and
historical Handoff evidence clearly. Repository facts and accepted ADRs outrank
stored preferences and previous handoffs. Use calm, direct language: Unknown,
Not run and Last successful read instead of invented assurance or activity.

## Recorded owner revisions

- Treat Instrument, Ledger and Harbor as selectable themes of one product.
- Use the supplied theme-picker reference for palette cards and separate appearance
  controls, without copying its font assets, configuration or integration behavior.
- Refine the supplied concept hierarchy: Workspaces / Agent access → Effective
  context → Handoff and preference review boundaries.
- Draw a custom title bar. Correct minimize/maximize/close centering.
- Add i18n. The revised preview supports English and Simplified Chinese.

The revised base authorized implementation. The later actual-window review accepted
the implemented five pages and connection editor as this stage's base, with no
blocking issue. Future changes and PR merge still require their own review.

## One visual language

All themes use system sans typography, a 14px body, 12px metadata, 22px headings,
8px panel corners, 6px control corners and the same 4/8/12/16px spacing scale.
Neither typeface, density, content ordering nor navigation changes with palette.
The actual console consistently uses Agent Control Plane in its header/title bar.
No provisional theme-specific wordmark is used as an official identity.

| Theme | Intent | Light background / accent | Dark background / accent |
| --- | --- | --- | --- |
| Instrument | restrained green / technical | `#F1F5F2` / `#246046` | `#111718` / `#9EE6BE` |
| Ledger | warm paper / earthy | `#F4F0E7` / `#8E3E26` | `#211F1B` / `#EDB095` |
| Harbor | cool neutral / blue | `#F2F4F9` / `#344E9C` | `#131C30` / `#B5C8FF` |

Theme cards show a type sample, semantic color dots and a textual `>` selection
marker. Appearance is independently Light / Dark / System, with a high-contrast
override. System follows the appearance reported by the current NSH runtime; it
does not modify OS settings or read another application's configuration.
The pure shared token table is in `spikes/p1-e/shared/src/appearance.rs`.

### Measured opaque token contrast

Minimum across background/panel/inset; normal text includes text, muted, accent,
warning/error/success and text on the selected accent. These measurements do not
claim a complete accessibility certification or screen-reader verification.

| Theme / mode | Minimum text ratio | Minimum essential boundary ratio |
| --- | ---: | ---: |
| Instrument light | 5.56:1 | 3.61:1 |
| Instrument dark | 6.41:1 | 3.65:1 |
| Ledger light | 4.87:1 | 3.05:1 |
| Ledger dark | 5.82:1 | 3.54:1 |
| Harbor light | 5.39:1 | 3.25:1 |
| Harbor dark | 6.24:1 | 3.52:1 |

Shared regression tests check all six palettes and the high-contrast override
against >=4.5:1 text / >=3:1 essential boundaries. Terminal meaning also uses text
and selection markers; a palette alone is not proof of monochrome accessibility.

## Concept hierarchy and product truth

- Workspaces: only allowlisted scope in live operation; the preview uses alpha/beta
  from the built-in fixture. Switching clears the exact task selection.
- Agent access: explain scoped read-only MCP and show unobserved client presence.
  No Codex/Pi/OpenCode online status, launch controls or fabricated client list.
- Effective context: distinguish metadata-only contract references from document
  bodies; confirmed records from suggestions; observed Git facts from verification.
- Handoff: exact selected task, recorded objective/next action and opaque historical
  evidence remain separate from current HEAD/fingerprint uncertainty.
- Preference review: explicitly unavailable in this read-only console. No fake
  confirm/reject buttons and no suggestion auto-approval. A future review surface
  requires the full view/confirm/reject/deprecate/delete/provenance workflow.

## Language and assets

The exhaustive typed catalog in `shared/src/i18n.rs` covers the actual console's
navigation, settings, state explanations, window controls and accessible labels.
UI language changes at runtime without changing project/task data. Project names,
preference keys/values and raw validation JSON keep their original language.
Both locale catalogs are tested for completeness. Additional languages, regional
date/number formatting and RTL are not claimed.

Use system fonts and CJK fallback; TUI uses the owner's terminal font. No font
binary, downloaded artwork or reference screenshot is committed. Window-control
icons are code-defined vectors centered in the same 14px box within 40×36px
buttons; glyph font metrics are not used. No decorative motion or gradients.

Appearance, language and explicit connection drafts are session-only. The actual
five-page Desktop/TUI consumes the same palette and catalog. The strict synthetic
`--brand-preview` root preserves the historical B0 comparison; it never opens IPC.
See [design system](../p1/ui-design-system.md) and [local evidence](../evaluation/p1-f-local-ux.md).
