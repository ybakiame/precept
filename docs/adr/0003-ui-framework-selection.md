# ADR-0003: NSH Rust for Desktop and TUI

- Status: **Accepted** — product framework direction; not production/platform acceptance
- Proposed: 2026-11-09; accepted direction: 2026-11-11
- Authority: owner direction and [Issue #30](https://github.com/ybakiame/agent-control-plane/issues/30)
- Supersedes the framework comparison proposal in this ADR and the UI-selection portion of ADR-0001

## Context and decision

The measured P1-E comparison led to NSH Desktop/TUI implementation and successive
E2–I local verification. The owner has now selected the existing stack for formal
productization. **Use NSH Rust as the single primary UI framework for Desktop and
TUI**, with one framework-neutral typed presentation model and Rust domain logic
behind the existing daemon/local IPC boundary. Do not introduce GPUI/GPUI Kit,
a hybrid renderer, Electron, React or a new runtime into the product application.

Keep `apps/operator/model` and `apps/operator/ui` (formerly `spikes/p1-e/shared`
and `spikes/p1-e/nsh`) as the implemented baseline;
production packaging/location can be specified later. Historical Kit/hybrid
benchmark directories remain experimental evidence, not competing product paths.
J does not remove dependencies, move workspaces or redesign screens. Desktop/TUI
are two surfaces of the same product; neither queries SQLite or reimplements
resolution/review rules. Optional session controls still require native operator
authority, and Windows→WSL remains read-only.

## Rationale and evidence

The [P1-E comparison](../p1/ui-spike-evaluation.md) measured alternatives rather
than choosing from upstream platform claims. Later NSH work established the shared
five-page model, explicit connection handling, bounded 10k-row presentation,
keyboard/terminal behavior and an owner-reviewed theme family. The
[P1-F local evidence](../evaluation/p1-f-local-ux.md) records Instrument/Ledger/
Harbor, English/Simplified Chinese, light/dark/high-contrast and reviewed windows.
[P1-I local evidence](../evaluation/p1-i-context-bridge.md) records subsequent
Windows/ConPTY and native WSLg regressions and preserves earlier failed attempts.
These provide a practical baseline for continued work without another UI experiment.

The decision is an owner-selected product direction using scoped evidence, **not**
a claim that every gate in the earlier comparative proposal passed. That proposal
required three-OS interaction/accessibility, clean-machine packaging, repeated
startup/RSS and browser authentication evaluation before accepting a universal
framework winner. J narrows the product to existing Desktop/TUI and accepts NSH
with the following gaps retained as future release gates, rather than implying
those broader criteria were met. No aggregate comparative score is asserted.

## Counterevidence and platform limits

| Target / behavior | Evidence-backed status | Remaining limit |
| --- | --- | --- |
| Windows Desktop / ConPTY | Local E2–I native windows, input/state/10k and deterministic session checks | Not proof of every device, full IME/AT or real-model GUI behavior |
| Native WSLg / Linux TUI | Local Wayland display observations and PTY interaction/regression evidence | Not general Linux compositor or physical GPU acceptance; niri excluded |
| macOS | Historical P1-E CI native build/window smoke at its recorded SHA | Latest product/theme/session behavior and packaging not revalidated; no current macOS support claim |
| English / Simplified Chinese | Shared catalogs, switching and scoped CJK paste evidence | Full IME composition, regional formatting and RTL not verified |
| Accessibility | Token contrast, visible focus and scoped keyboard checks | Screen-reader/assistive-technology and complete accessibility certification NOT_RUN |
| Mobile / browser | Local offline Web smoke at its recorded S2 review SHA; mobile remains historical experiment/typecheck evidence | Outside current product targets; no mobile release or authenticated browser UI |
| Packaging/performance | Local release/probe results at source-specific SHAs | Clean-machine distribution and repeated production startup/RSS budgets remain open |

### Current NSH source pin

The product now pins immutable upstream revision
[`b7f31a08478be262fd7b76ec856036fd5b8ffdb0`](https://github.com/longbridge/nsh/commit/b7f31a08478be262fd7b76ec856036fd5b8ffdb0).
The fix and its parent both declare workspace version **0.5.0**. This is the
owner-selected upstream desktop keyboard/focus revision, **not** the official
stable **0.5.1** tag; no local SDK fork or cached-source patch is maintained.
Active Cargo dependencies, offline renderer packaging and its workflow checkout
must agree on this exact revision. The repository source verifier checks that
agreement before the Web job builds.

The [S2 source decision and native acceptance report](../evaluation/p1-m4-s2-native-review.md#nsh-source-decision)
records why this revision was selected, source-specific local results and remaining
limits. Those author-provided checks do not establish hosted CI, release or S3
independent-host acceptance.

**Historical/pre-S2 baseline:** `87c7e8da85d2f15a56107baa6f0fabb419b508b3`
was the pre-S2 development pin; release 0.4.0 lacked TUI. Earlier reports retain that
revision and their original scope; it is not the current product dependency.
GTK platform dependencies and Rust rebuilds remain maintenance/release costs.
Kit's dense virtualized table remains useful counterevidence; the hybrid adds
focus/theme/lifecycle and renderer cost without a current product need.
Framework changes require a new measured ADR, not a second renderer added
preemptively.

## Consequences

Preserve the [visual identity](../brand/identity.md) and existing
[typed console architecture](../p1/ui-console-architecture.md). The planned
[product information architecture](../product-ui.md) organizes workflows without
adding pages/controls in J. Full preference review, onboarding and context
explanations require separately reviewed operator APIs and tests before UI work.
A framework decision grants no MCP write authority, session auto-start or production
security claim. Original reports retain their historical “Proposed” status language.
