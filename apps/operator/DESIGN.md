# Operator app — design standards

This is the design system for the ACP operator app (`apps/operator`, NSH Rust
mode). It extends the app's established look; it does not replace it. Screens
are composed from the app's `ui` layer (`ui/src/ui/`) above the semantic
`nsh/base` components. Every color comes from a semantic token, every geometry
from a scale, every motion from a motion token. Raw hex lives only in the theme
definitions (`model/src/appearance.rs`).

Read this before adding or changing a screen. The NSH Design Guides and Coding
Guides are the general requirement; this file records how this app satisfies
them.

## Visual language

Quiet, precise, information-first. The operator console is a read-mostly
inspector with a small, explicit set of writes; the interface should feel like
a native desktop tool, not a web dashboard.

- **Hierarchy by structure first.** Size, weight, spacing and hairlines carry
  hierarchy before color or containers. A region needs a background, a hairline
  boundary and intentional spacing — not a card inside a card.
- **One focal point per surface.** Accent and strong color are a scarce budget.
  The primary action, the current selection and the next step must be obvious
  with color hidden.
- **Surfaces.** `background` is the window floor; `panel` is a card or popup;
  `inset` is a recessed control fill (default buttons, the tab indicator).
  Elevation explains stacking only: floating surfaces (dialog, menu) get a
  shadow; base regions are separated by background and hairlines, never by a
  shadow on every card.
- **Boundaries.** A 1pt hairline (`border`) on containers, a 2pt boundary on
  interactive controls so the focus ring reads. The boundary owner draws the
  hairline once.
- **Themes.** Three themes — Instrument, Ledger, Harbor — each in light and dark
  plus a high-contrast mode. Geometry, typography and data never vary by theme;
  only color does. Every surface is verified in light and dark and high contrast.

## Semantic token roles

Colors are defined once in `model/src/appearance.rs` (`Colors`, u32 hex) for all
3 themes × light/dark × high-contrast, bridged to `AdaptiveColor` by
`ui/src/tokens.rs` (`Palette`, `palette(model)`). Presentation reads the role,
never a hex value.

| Role (model → Palette) | Meaning |
| --- | --- |
| `background` | Window floor |
| `panel` | Cards, popups, dialog surface |
| `inset` | Recessed control fill: default button, tab indicator |
| `text` | Primary foreground |
| `muted` | Secondary metadata, help, unselected labels |
| `border` | Hairlines and control boundaries |
| `accent → primary` | Principal action / selection emphasis |
| `on_accent → on_primary` | Foreground on `primary` |
| `warning` | Caution that needs attention, not an error |
| `error` | Error text and failed state |
| `success` | Confirmed / healthy state |
| `danger` | Destructive action fill (danger button, armed control) |
| `danger_foreground → on_danger` | Foreground on `danger`; danger border/text |

`danger`/`danger_foreground` are the destructive tier added for the review
panel. They are distinct from `error` (a failed state) — `danger` is the fill of
an action that destroys. Both pass the WCAG test
`every_theme_mode_and_contrast_has_legible_semantic_pairs`: `danger` on
`background`/`panel`/`inset` ≥ 4.5, and `danger_foreground` on `danger` ≥ 4.5,
for every theme/mode/contrast. State washes are derived from tokens
(`tokens::mix`, `AdaptiveColor::opacity`), never new hex.

The only non-token color in presentation is the dialog scrim, `black()` at a
token opacity — the NSH Design Guides' backdrop pattern, a named neutral, not a
palette hex.

## Typography scale

The platform UI font (`sans-serif`) carries interface text; monospace is
reserved for identifiers, shortcuts and aligned numbers. Sizes live in
`ui::scale::text` (points); the user's text size scales them.

| Token | pt | Use |
| --- | --- | --- |
| `CAPTION` | 12 | Secondary metadata, help, badges |
| `BODY` | 14 | Body text and control labels (the reference) |
| `SUBTITLE` | 16 | Subtitles, large control labels, dialog title |
| `TITLE` | 20 | Section and page titles |
| `DISPLAY` | 22 | The window's principal title |

At most three or four sizes and two weights per screen. Secondary text uses
`muted`, not a smaller size alone. Nothing below 12pt for reading.

## Spacing scale

Spacing uses the shared Tailwind scale (`.gap_2()`, `.p_3()`, `.px_2()`); one
step is 4pt. Relationships, not literals:

| Relationship | Step | pt |
| --- | --- | --- |
| Parts of one control | `gap-1` | 4 |
| Closely related controls | `gap-2` | 8 |
| One content group | `gap-3` | 12 |
| Groups in a section / panel padding | `gap-4` / `p-4` | 16 |
| Separate sections | `gap-6` | 24 |
| Empty-state breathing room | `p-6` / `gap-8` | 24 / 32 |

Padding belongs to the component; gaps belong to the parent. No doubled insets.
Peers share alignment spines; repeated rows keep fixed lanes. Literal points are
avoided except audited geometry (a hairline, a window inset).

## Radii tokens

`ui::scale::radius`. A finite scale, concentric nesting (`inner ≈ outer − gap`):

| Token | pt | Use |
| --- | --- | --- |
| `SMALL` | 3 | Badges, small tags, small buttons |
| `CONTROL` | 6 | Buttons, inputs, tabs, popups, the tab indicator |
| `CONTAINER` | 8 | Cards, panels, dialogs |
| `PILL` | 999 | Intentional circles and pills only |

A radius governs the whole visible surface; children and selection fills stay
inside the corners. `PILL` is never an accident on a rectangle.

## Density

- **Desktop / Web (pointer):** compact controls. Buttons share the control
  heights `ui::scale::control` (SM 24 / MD 30 / LG 38); inputs match. One `size`
  sets height, padding, text, icon and radius together. `compact()` (TUI or
  width < 900) tightens padding, not height.
- **TUI (whole cells):** keyboard-first on the character grid. Layout snaps to
  whole cells; no fractional spacing, radii or shadows. Terminal colors and
  attributes (bold, dim, reverse) carry state — never color alone. Labels replace
  glyphs that may be missing; the base `Tabs` draws the selected tab bold on a
  band with no indicator. Icons are hidden in a terminal (`ui::icon`).

## Icon policy

One family: original outline vectors on a 24×24 grid, stroke 1.6, round caps and
joins, `currentColor` (`ui::icon`). Sizes from `ui::scale::icon` (SM 14 / MD 16),
one step below the text they sit beside. Icons supplement labels; an icon-only
control always has an accessible name. Filled/colored icons communicate a state,
not decoration. Hidden in a terminal, where the label carries the meaning.

## Interaction states

Every interactive control is designed for each state, from tokens, consistently
across the app (`ui::button` is the reference):

| State | Treatment |
| --- | --- |
| Rest | Clear affordance, no noise |
| Hover | A quiet wash (`mix(inset, text, 0.07)`; primary/danger at 0.88 opacity), weaker than selected/pressed; never the only cue; no size/layout change |
| Pressed | Whole control scales to 0.97 on the soft settle and deepens the wash (`mix` 0.13 / opacity 0.78); stronger than hover |
| Focus visible | The 2pt boundary takes the focus color (`primary` on default, `text` on primary/danger). This NSH revision exposes `.focus()` (the app's established ring); a typed `focus-visible` is not available, so the ring shows on focus |
| Selected | Persistent fill (the tab indicator) + stronger foreground, distinct from hover; announced to screen readers |
| Disabled | Opacity 0.45, no hover/press response, announced dimmed |
| Loading | Dims, disables to prevent a duplicate submit, shows a spinner beside the label; keeps its size |
| Validation | The field boundary takes `danger`; an error message sits beside the field |
| Danger | `danger` fill, `on_danger` foreground; a serious irreversible action confirms through `AlertDialog` |

Status is never color alone: a badge or state always carries text. Commands are
reachable by keyboard (Tab order, arrows in groups, Enter/Space activate, Escape
closes) with visible focus, and every control has a role and name.

## Platform targets

- **Desktop + Web: one family.** macOS, Windows, Linux and the web in the desktop
  idiom share interaction, design and sizes. Pointer and keyboard first;
  persistent navigation; dialogs centered, Escape to dismiss; the layout holds
  from the minimum window size up.
- **TUI: cells.** Keyboard-first, whole cells, 16 colors and monochrome, text
  labels, terminal mouse a bonus. The same semantics and tokens; presentation
  adapts (no indicator, no icons, no shadows).
- **Mobile** is not a current target of this app; if added, it follows iOS /
  Android sizes and navigation rather than shrinking the desktop.

The `ui` layer keeps a control's value, events and accessibility meaning across
targets while presentation adapts. Validate in each target's appearances and
input modes, including terminal cells.

## The `ui` layer

`ui/src/ui/`, one file per control, built on `nsh/base`. Each takes `variant`,
`size`, states and slots — never colors, radii, paddings or fonts. Features
compose `ui`; they do not import `nsh/base` controls or style controls
themselves. A look a feature needs becomes a new variant or size here first.

- `button` — `Button` (Default / Primary / Danger; Small / Medium / Large;
  disabled / loading; optional icon; hover/press/focus motion).
- `input` — `Input` (text field; value, on_change, on_submit, placeholder,
  disabled, invalid, element_ref, max_length, medium/large).
- `tabs` — `Tabs` (base WAI-ARIA tablist keyboard pattern; one sliding selection
  indicator that honors reduced motion natively).
- `badge` — `Badge` (Neutral / Info / Success / Warning / Danger).
- `empty` — `Empty` (title, description, icon, action slot).
- `surface` — `Surface` / `card` (titled panel of sanitized lines).
- `alert_dialog` — `AlertDialog` (native modal layer: focus trap, Escape/back
  dismissal; fading scrim, growing panel, default Cancel + danger commitment).
- `icon`, `scale`, `motion` — the shared family, geometry and motion tokens.

`components.rs`'s `action`, `word_action`, `card` and the task/filter fields
delegate to this layer; existing screens keep their behavior.

**Show it.** `ui::gallery` is a debug-only surface showing every control in its
variants, sizes and states, in light and dark. It is mounted only when
`ACP_UI_GALLERY` is set (never in normal navigation) and is covered by a headless
test. Review new components there before features use them.

## Motion tokens

`ui/src/ui/motion.rs`, beside the theme. One small set, used everywhere; every
helper honors `reduce_motion()` (durations collapse to 0, scales rest at 1).

| Token | Value | For |
| --- | --- | --- |
| `INSTANT` | 120ms | A hover wash, a color change |
| `FAST` | 180ms | A press, a control's state feedback |
| `BASE` | 240ms | Moving into place: a selection sliding, a glyph swap |
| `SLOW` | 360ms | Reveals: a dialog or popup opening |
| `SLOWER` | 500ms | Unhurried entrances: a list's wave |
| `SETTLE` | `cubic-bezier(0.22,1,0.36,1)` | Soft settle for presses (ease-out family) |
| `PRESS` / `ICON_PRESS` | 0.97 / 0.95 | Press scale |
| `ENTER` | 0.97 | Popup entrance scale |
| `RISE` / `LABEL_RISE` | 8 / 4pt | Popup / label entrance rise |

Tools, lightest first: a **transition** for a state style (hover wash, press
scale, focus ring — `motion::feedback`); a **motion** for a view appearing
(`motion::grow` for a popup, `motion::scrim` for a backdrop); the base **Tabs
indicator** (a native layout spring) for a sliding selection; **LayoutAnimation**
for a user-driven layout change; a **Shader** only for something live. Animate
opacity and transform, not layout. Create a motion once in a component body.
Leaving is faster than entering. `BASE`, `SLOWER`, `ICON_PRESS`, `LABEL_RISE`
complete the vocabulary for the review panel (Commit B/C).

## Motion inventory — review panel (Queue / Detail / History)

Written before the screen code (Commit A). Each state change names its tool and
tokens; reduced motion makes every line instant while keeping the state
understandable. Verify each line in the running app before the work is done.

| Change | Motion tool | Tokens | Reduced motion |
| --- | --- | --- | --- |
| Tab switch (Queue/Detail/History) | Base `Tabs` sliding indicator (native layout spring) | `SLOW`-class (~362ms native), ease-in-out | Indicator jumps at once (base honors it) |
| Queue arrival (new suggestions) | `motion` enter, capped index stagger | `SLOWER`, rise `RISE` 8pt + fade, ~50ms stagger capped at first 10–12 rows | Instant, no wave |
| Prepare feedback (arm a mutation) | Transition on the review panel + button state; panel `motion::grow` | `FAST` ease-out; grow `SLOW`, `ENTER` 0.97, `RISE` 8pt | Instant; panel at rest |
| Apply feedback (commit receipt) | Glyph/label swap + success badge fade | `BASE` ease-in-out; badge fade `FAST` | Instant swap |
| Destructive arm (`AlertDialog` open) | `motion::grow` panel + `motion::scrim` | `SLOW` ease-out, `ENTER` 0.97, `RISE` 8pt; scrim `FAST` | Lands at rest, no grow |
| Error toast | `motion` slide from its edge + fade | `BASE` ease-out, slide 8pt | Instant appear |
| Button hover / press / focus | Transition (`motion::feedback`) | `FAST`, `SETTLE`; press `PRESS` 0.97 | Instant; no scale |
| Queue row selection | Transition colors | `FAST` | Instant |
| Loading (prepare/apply in flight) | `animate_spin` spinner | Continuous | Rests (still drawn) |
| A value updating many times a second | No motion | — | Dense data cell stays put |

## Review safety and copy

Queue absence means **not inspected**, not an empty queue. Every received page,
including an empty later page, displays its commit watermark and any observed
delta; pages never claim to share one snapshot.

Decision preparation requires a current exact inspect. When a records-list
shortcut has no current inspect, it only opens Detail and reads the record;
the operator invokes the decision again after seeing its fields and peer impact.
Changing selection clears the previous inspect and digest expansion; History
never displays a different selected ID. A delete commitment also requires the
dialog to remain open, the current write grant, and the exact retyped ID.

The records list is only a selector: Detail is the sole immutable-content
inspector and owns the one confirm/reject/deprecate/delete action group. The old
inline list inspector is removed rather than stacked above Detail. Receipts use
one animated acknowledgement row, not another copy in the Profile summary.
The active workspace owns one scroll region. Review content uses its own region
instead of nesting inside the general shell scroll; the native dialog is a sibling
of that region, never an offscreen child at the bottom of the scrolled document.
Explicit tab changes, new inspect/queue results and newly prepared/uncertain
operations reveal the tab header or preview from measured layout on the next
frame, not the document's end. Superseded navigation cannot move the viewport.
Keyboard focus stays unchanged; retype edits, field focus and cancellation do
not request that scrolling.
An explicit field shortcut first switches/disarms its pane, then requests focus
on the next frame only while that pane is still active and editable. Hidden
native Windows fields cannot take focus before their visibility update.

The destructive dialog keeps one native modal for each open interval. Only the
boolean open state changes its structure; localized labels, validation and retype
text update their own bindings without remounting or restarting entrance motion.
Its root fills the native modal layer. Initial focus is requested from the native
`on_open` callback, after the input is mounted and modal focus scope is active,
not from an earlier boolean-state effect. Probe-only lifecycle/size markers carry
no proposal content.
Escape dismisses only the active destructive tier and clears its retype/preview;
the review workspace stays open. A later delete starts with an empty field.

Detail preserves immutable key/value/source text in bounded escaped lines.
Source/origin remain claims, the digest proves only byte equality, and preference
confirmation never approves a repository ProjectContract. History's channel is
client-asserted; receipts identify transactions, not reviewers or human approval.
English and zh-CN carry these distinctions together. Capacity refusal never
suggests deleting records to recover the generation's receipt ceiling.

## Verification

Behavior is covered by headless tests (`nsh_testing::Headless`) and the
real-process IPC integration test; the gallery test mounts every control and
exercises the destructive dialog's open and Escape dismissal. Headless tests
prove behavior, not the look: inspect the running app (and the gallery) in light
and dark, and on the TUI, before calling a screen done. Record any platform
verification the environment cannot perform.