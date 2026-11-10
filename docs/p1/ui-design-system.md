# P1-F shared design system — revised B0 staging

Owner direction: **one operator product, a selectable theme collection**, informed
by the supplied concept hierarchy and theme settings references. The revised
actual Desktop/TUI preview is awaiting owner review before final five-page
integration. The normal E4 console is still the existing default entry point.

## Anatomy and reusable foundation

`shared/src/appearance.rs` defines framework-independent ThemeId, ColorMode and
semantic opaque Colors. Instrument, Ledger and Harbor each have Light/Dark pairs;
all use the same spacing/type/radius/layout. NSH preview components consume these
tokens for surfaces, text, essential borders, selection, focus, warning and sample
swatches. System appearance resolves through NSH's actual runtime report. High
contrast overrides the semantic colors. No OS/terminal configuration is edited.

Desktop has a custom title bar, ACP/navigation row, explicit synthetic/live source
state, scrollable page content and shortcut help. The preview supports Overview,
Handoff and Appearance as B0 samples, not a new sixth production page. Appearance
contains palette cards, System/Light/Dark, high contrast and language. Its layout
and content are identical across themes. Theme changes never reset selected scope.

Overview follows Workspaces / Agent access → Effective context → Resume context →
Preference review boundary. Two summary panels remain side by side at 640px; at
smaller allocations they stack. Only real typed fixture fields populate context;
there are no invented agent states or completed repository checks.

| Token | Desktop | Compact / TUI adaptation |
| --- | --- | --- |
| Gutters | 16px | 8px; native terminal cell metrics |
| Spacing | 4 / 8 / 12 / 16px | 4 / 8px |
| Type | 22px heading / 14px body / 12px metadata | terminal font; CJK width-aware |
| Panel / control radius | 8px / 6px | same semantics; terminal renderer's own borders |
| Title bar / caption target | 36px / 40×36px | absent in TUI |
| Caption vector | centered 14×14px, 24×24 viewBox | absent in TUI |
| Theme card | fixed 176px width, wraps | sample text, palette, explicit `>` selection |
| Data bounds | 512 characters per rendered line; six disclosed evidence entries | same bound |

The title bar uses `WindowFrameStyle::None`, a separate native drag region and
NSH window requests for minimize/maximize-restore/close. Caption buttons use
explicit flex centering and SVG paths; their geometric center is independent of
font baseline. A bottom-right grip initiates native resize. Windows snap flyouts
and all eight explicit resize edges are not yet claimed.

## i18n foundation

`shared/src/i18n.rs` has a typed, exhaustive English/Simplified Chinese catalog.
Translate interface labels and explanations at render time. Never translate or
rewrite user/project values, preference content, task IDs or raw recorded JSON.
Locale/theme state is independent of the shared typed snapshot and query scope.
The preview's native and terminal language switches preserve the active workspace
and task; a workspace switch still clears the selected task. No profile/DB write.

Runtime preview keys: Alt+1 overview, Alt+4 Handoff, Alt+0 Appearance, Alt+A/B/C
theme, Ctrl/Cmd+L language, Esc overview. TUI Ctrl+Q restores terminal state.
These B0 controls do not replace normal console Alt+1–5, filter/task/reconnect,
Tab/Esc, paging or explicit copy semantics.

Contrast tests check both modes of every theme plus high contrast. All statuses
carry text; unknown never means clean, recorded success never means current
success. No decorative motion is introduced. Full IME/screen readers, additional
locales and RTL are NOT_RUN / out of the current two-locale implementation.

## After revised B0 review

- Integrate the approved shared components/tokens/catalog into Dashboard, Projects,
  Preferences, Handoff and Diagnostics, preserving bounded provenance/detail/copy.
- Preserve stable task/filter inputs, request epochs, cancellation, denied auth,
  restart/reconnect and exact project/task validation.
- Translate all five-page interface explanations while keeping protocol and raw
  evidence byte semantics unchanged. Test both locales at wide/compact sizes.
- Verify complete focus order, hover/pressed/disabled states and high contrast;
  preserve the 10k actual-window and terminal workload regression matrix.
- Keep the read-only backend boundary, ADR Proposed and mobile/niri deferred.

Owner feedback and actual measured evidence are in
[the evaluation](../evaluation/p1-f-local-ux.md). Palette intent, language and asset
policy are in [identity](../brand/identity.md).
