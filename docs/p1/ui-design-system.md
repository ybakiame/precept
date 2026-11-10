# P1-F design system — proposal and staging

Status: **B0, PENDING brand selection**. Candidate palettes live only in the
synthetic `nsh/src/brand_preview.rs` preview. They are not three production themes.
The existing E4 `tokens.rs` and five-page console remain the normal application.
See [candidate identity](../brand/identity.md).

## Shared anatomy and meanings

Keep identity/project scope, page navigation, current read state and historical
last-read time separate. Desktop at 1080×760 may use a sidebar and paired summary
panels; 640×480 and terminals stack controls and content without losing scope or
provenance. TUI at 160×50 / 80×30 shares semantics, shortcuts and state, not pixels.

The selected system will define background, panel, inset, primary/secondary text,
essential border, accent/on-accent, success, warning, error and focus. Use one
reusable mapping for normal/light/dark/high contrast; no per-page hardcoded colors.
Every status also has text. A passed historical check is never a current success
indicator. Unknown Git/fingerprint is not Clean. Confirmed preference scope/source
must remain visible even when values truncate.

## Candidate layout and type

Preview spacing is 4/8/12/16px, 16px wide gutters, 8px compact gutters, and a 196px
wide identity/navigation column. A and B use stacked sections; C pairs repository
and preferences on wide desktop. Corners are A 2px / B 0px / C 12px. System sans body
is 14px with 12px metadata; titles A/C 22px, B system serif 26px. Monospace is limited
to compact name sketches and technical values. No font is installed or embedded.
Terminal rendering uses cell metrics and its configured font.

The preview has a bounded Dashboard/Handoff example and explicit candidate/page
buttons, visible borders/focus, Alt+A/B/C for candidates and Alt+1/4 for pages.
These exploration keys do not replace the normal console's Alt+1–5, Ctrl/Cmd+F/K/R,
Tab/Shift+Tab, Esc, paging, explicit copy or TUI Ctrl+Q semantics.

## After owner selection — not implemented yet

- Consolidate shared cards/rows, badges, inputs, buttons, pagination, disclosure,
  loading/empty/error/denied/unknown and lost-connection surfaces.
- Map hover, pressed, focused, disabled and selected states with fixed bounds and
  visible focus. Measure all applicable text/control pairs in each theme.
- Preserve request-epoch/scope/cancel/reload behavior and bounded data rendering.
- Keep reduced-motion behavior static: no decorative animation or essential motion.
  Document any platform preference that cannot actually be observed.
- Define a TUI monochrome fallback using labels, selection markers and rules;
  do not claim a palette alone proves monochrome accessibility.
- Refine Dashboard, Projects, Preferences, Handoff and Diagnostics after selection;
  do not add backend data, write actions or task enumeration.

Candidate pair calculations meet >=4.5:1 text / >=3:1 essential boundaries as
recorded in the identity document. Final focus order, full themes, monochrome,
IME, screen readers and selected-design performance acceptance are **PENDING** or
**NOT_RUN**, never inferred from palette calculations or E4's earlier results.
