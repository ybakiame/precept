# Operator UI standards

These are the current design and interaction contracts for the Precept operator app.
Follow them together with the NSH Design Guides when changing a screen. Product
authority comes from the [charter](product.md), [architecture](architecture.md) and
[security](security.md); UI state never creates authority of its own.

The visual system (themes, tokens, type, spacing, radii, elevation, icons,
component sizes, overlay anatomy, motion tokens and every screen's motion
inventory) is defined once in the [design system](../apps/ui/DESIGN.md). This file
keeps the product and interaction contracts that design must satisfy.

Screens are composed from the app's shared `apps/ui/src/ui/` layer above NSH base
components. Features reuse shared controls and tokens rather than importing base
controls or inventing local styling. Colors come from semantic tokens, geometry
from scales and motion from motion tokens. Raw hex belongs only in theme
definitions in `apps/core/src/appearance.rs`.

## Visual contract

The interface is quiet, precise and information-first: a native developer inspector
with a small, explicit set of writes, not a web dashboard.

- Each surface has one focal point and at most one primary action. Accent and
  status colors are scarce; primary action, selection and next step remain
  understandable without color.
- Surfaces step by role (floor, sidebar, panel, raised, inset); hairlines separate
  regions, and strong boundaries are reserved for controls the operator types into
  and for focus. Shadows explain floating layers only.
- Themes change color only; geometry, typography and data never vary by theme.
  Graphite (default), Harbor, Ledger and Instrument each have light, dark and
  high-contrast modes, and every text role meets 4.5:1 on every surface.
- Status and meaning always include text, never color, shape or motion alone.
- Spacing, radii, type and control sizes come from the shared scales.

## Shared controls and states

Use the shared button, icon button, navigation item, menu, radio group, field,
properties, input, tabs, badge, status dot, choice card, empty state, surface,
dialog, sheet, notification, information popup, tooltip,
Markdown input, icon and motion components. Add a variant or size to the shared
layer when a feature needs a new reusable treatment.

Every interactive control defines rest, hover, pressed, visible focus, selected,
disabled, loading, validation and destructive states as applicable. Hover is weaker
than selection and never the only cue. Press and loading do not shift layout.
Disabled and loading controls prevent duplicate submission. Destructive actions use
the danger treatment and deliberate confirmation.

Icons are original outline vectors on a 24×24 grid using `currentColor`; they
supplement labels. Icon-only controls require accessible names.

Navigation and primary actions carry static automation identifiers independent of
localized labels. Windows UI Automation exposes them as `AutomationId`; automated
lookup combines that id with the control type and treats accessible names as
localized text, not stable keys. Identifiers never contain project data, preference
values or other private runtime content.

Keyboard shortcuts come from one catalog and are shown and changed in the
Shortcuts panel (`Ctrl+/`, `F1`, the sidebar's keyboard button), never printed on
pages. Overrides are presentation settings saved in the private `settings.toml`;
they never carry authority, and a binding the platform reserves or another action
uses is refused. All commands are keyboard reachable with logical Tab order, arrows within groups,
Enter/Space activation and Escape dismissal. Focus remains visible, dialogs trap
and restore focus, and pointer-only approval is forbidden.

## Navigation and scope

The first launch without a saved connection opens an independent **launch guide**
instead of the workspace shell. It presents four ordered steps: Connection,
Profile, one Workspace and optional Agent access. The first three must be observed
complete before Finish; Agent sharing may be deferred. The guide reuses the normal
setup actions and their explicit write grants, creates nothing on its own and may
be skipped at any time. Completing or skipping records presentation state only.
Runtime checks offers **Reopen launch guide…** at the first incomplete required
step.

Organize navigation around three clear operational paths plus utilities:

- **Workspace:** one persistent workspace switcher at the top of the group (or an
  explicit `Add workspace…` shortcut to Runtime checks when none exist), followed
  by **Dashboard**, **Preferences & contracts** (the repository's contracts and the
  effective confirmed rows), **Sessions**, **Memory** and **Handoff** for that active
  project. There is one workspace switcher in the app; workspace-scoped headers
  display a calm read-only scope badge rather than a second switcher.
- **Context:** **Review** holds only the pending queue (with personal vs
  current-project scope toggle) and carries its live count pill. **Rules** holds the
  reviewed records: **Personal** preferences (global scope) and **Project
  conventions** (active project scope), including hidden-by-default deprecated rows.
- **Set up:** **Runtime checks** unites **Connection** and **Profile** as two step
  tabs with live completion indicators (`✓` or `n/2`). **Workspaces**
  is a separate management page directly below it, strictly for registration and
  removal; clicking a workspace row never switches the active project.
  **Agents** follows Workspaces and keeps the launch-guide order: its first section
  shares the workspace selected above, followed by MCP and hook integrations.
  **Personal sync** manages this Profile's manual encrypted file exchange.
- **Foot:** **Diagnostics**, **Settings**, the persistent daemon status button
  (opens Runtime checks → Connection) and the keyboard shortcuts button.

On Windows, selecting a WSL distribution probes the bridge, packaged unit and daemon.
If the helper or unit is missing or incompatible, Connection offers **Install / Update
daemon in WSL**. That explicit action downloads the current musl release archive from
this repository's GitHub Release, verifies its SHA-256 against the published
`SHA256SUMS`, installs user-local binaries and the user unit, then starts it. It never
uses sudo, a package manager or a shell. The selected helper path is updated to the
user-local bridge; no Profile is created or write-enabled.

A persistent scope indicator identifies the applied daemon target and selected
project/worktree, plus the task when relevant. Personal scope is
explicit. Selecting a project never turns a personal action into a repository
write.

Desktop draws its own frame (`nsh.json` `"window"`: no system frame, a least size
of 960 × 600 so the desktop never falls back to the compact layout). The sidebar
runs from the window's top edge, its brand level with the window buttons; the page
column's top bar holds the sidebar toggle, the page title with its workspace scope
and help, the version (`v` + the product version) and the window buttons, and its
empty space moves the window. The sidebar status button's tooltip holds the target,
last read result and successful-read time, also shown on keyboard focus; page content
does not repeat them. The sidebar hides and shows from that toggle or
`Ctrl+B`, and the choice is saved; hidden, the brand mark leads the top bar. The
terminal layout has no top bar: the title stays in the page header and a wrapped
destination row replaces the sidebar. The active workspace owns one primary scroll region.
Project/target switches clear stale task and record detail and invalidate
late completions. Draft connection settings remain distinct from the applied
target.

## Main decision surfaces

### Projects and agent access

Add one existing checkout through an OS folder chooser or explicit path field. The
daemon checks the chosen path and returns a fixed reason; Check, Add and Remove are
distinct states. Add requires the appropriate Profile write grant. Removing a
project is destructive confirmation and does not claim to erase its Profile
records.

Agent sharing is a separate per-project operator switch. Configuration text can be
copied, but Precept never writes another tool's configuration automatically. Credentials
are never displayed.

Project sharing is a switch. **Agent integrations** open with **Connection by
agent**: each harness's level from its session-start hook and MCP entry (Hooks +
MCP · recommended, Hooks only, MCP only, Not connected, or Not read yet), then one
line naming the recommended setup and the add-ons. The integrations are grouped by mechanism, each
group headed by what it does and where it reaches, with one quick-setting tile per
agent: **MCP tools** (on demand; the one `precept` server entry, for every
registered project, in Claude
Code's and Codex's user configuration), **Session-start briefing**
(automatic; their session-start hooks), the **Stale-document warning** (Claude
Code's read hook: after a read, a note when committed claims about that file
fail), the **Pre-tool gate** (opt-in: the
project's gate switch, then one gate-hook tile per agent; a tile installed while
the gate is off says it asks nothing yet) and the **Repository pointer** (the
project's AGENTS.md section, whose switch opens a dialog previewing the exact text
before anything is written). Below the groups, one line says that other agents
connect themselves with the `precept-connect` skill (`precept skill`). Settings
with only two states use switches, not
button pairs. Manual setup, the configuration to paste for agents on Windows or
other MCP clients and the exact hook command, is folded under **Advanced: manual
setup**, a disclosure that is closed by default and springs open (at once under
reduced motion).

Below project sharing, a switch lets the project's agents suggest rules (off by
default, disabled while the project is not shared); its help says suggestions need
writes on and stay inactive until confirmed in Review, where they carry the agent
origin badge. Doctor reports the switch, and warns when it is on while writes are
off.

**Sessions** (in the Workspace group) lists the selected project's work sessions
as agents recorded them: the goal, open or closed, which agent and when, and how
many items are doing, next and done. A session opens a sheet grouped by Doing,
Blocked, Next, Deferred and Done. Each item shows its stable id, milestone/task
kind, dependencies, owner and blocked reason; the summary follows. **Save as
Handoff…** (a task id prefilled from the goal; saving turns writes on for the run
first when they are off, and a taken id says so) preserves that structured item
metadata in the Handoff text. **Delete session…** stays behind a confirmation;
nothing else is editable here. The page explains that sessions are agents'
notes, not rules, and that agents write them only where agent writes are allowed.

**Memory** (in the Workspace group, after Sessions) shows the selected project's
memory: a counts line by kind with Refresh and **Clear all…** (confirmed), then
one card per kind (Environment, Cautions, Lessons, Attempts), heaviest first.
Each note shows its title, first line, who wrote it and when, its weight with
the reasons (kind, pin, confirmations, recalls, your edit, fading) and badges
for pinned, changed often lately, fading and edited by you. A note opens a sheet
with its text, a **Pinned** switch, **Edit…** (title and text; kept as your
correction), **Delete…** (confirmed) and its earlier versions, each as a diff
against the current text with **Restore**, and **Propose as a rule…**, which
opens Review on the project's conventions with a new draft from the note (its
title as a kebab-case key, its text as the value, its kind as the tag); saving
it files an ordinary suggestion to confirm, and the note stays.

The Agents page is setup only. Agent activity is on the **Sessions** page: when an
MCP client started `precept-mcp`, what it read (by count), suggestions and work
session calls, who asked (the client's self-reported name, or how it arrived) and
each outcome, including refused stale credentials. A headline says when the
project was last read, or that a client started but never called a tool; a line
says whether the evidence chain is intact (with how many entries) or broken at
which entry. It persists across daemon restarts. A session's sheet lists the calls
made in that session. Refresh is explicit.

### Personal sync

**Personal sync** is Profile-scoped. Its default page has two persistent forms:
**Export encrypted preferences** and **Import and merge preferences**, followed by
the import comparison and the latest operation outcome. Devices may initialize
independently; the default password-file workflow never requires an invitation,
matching owner identity, key rotation or identity reset. Advanced recipient-key
pairing and recovery remain available through the native CLI.

Export takes a masked password (12 characters minimum), a password generator,
an explicit show/hide action and a new `.age` output path. The user saves the
password separately. Import takes an operator-selected `.age` file and a masked
password entered directly in the app, then **Decrypt and preview**. The app reads
only bounded ciphertext; the daemon decrypts it. Ordinary Downloads permissions
are accepted for ciphertext, while private state and CLI password files retain
their owner-only checks. File selection is bound to the originating Profile,
connection, page and picker generation. No existing output is overwritten.

The first explicit export or import may initialize a device's daemon-owned sync
identity through OperatorWrite. It never creates or changes the selected Profile.
An existing identity, its trust list and local preferences are preserved. Password
imports may cross independent owners, but successful decryption and an enclosed
signature are not claims of sender identity. The preview explains that the
operator is authorizing this file's exact contents, without granting persistent
trust to its source. Known revoked senders and changed keys remain refused.

The bounded comparison shows incoming contents, unchanged and older entries,
explicit deletions and conflicts. Already-applied entries stay excluded.
Non-conflicting updates start selected; every conflict requires an explicit
Accept remote, Keep local or Manual merge choice. Manual content is inspected
before Apply. Partial selection is named by key and count. Absence is never
shown as deletion; only an explicit tombstone is described as a delete. Version
clocks survive a round trip so independent edits can merge in both directions.

Apply names the selected keys and their conflict choices before one Profile-
scoped OperatorWrite request. Exact operation ids and destination versions remain
in the guarded request and CLI templates rather than the ordinary form. Passwords remain transient,
are masked in native accessibility, and are never persisted in settings, templates,
logs or the app's clipboard. Form passwords are cleared on success, page exit or
Profile/connection change. Preview/retry secrets live only for that guarded
operation and are cleared on its terminal outcome. An uncertain apply retains
its exact encrypted file, local versions, selections, commit, operation id and
password for deliberate identical retry. New file imports are disabled until it
settles or is explicitly aborted. Changed inputs require a fresh preview. Abort
reports whether a receipt settled or a proven-uncommitted operation was discarded.
Writes are restored only for a remembered startup Profile after explicit operator
enablement and an exact profile_id/generation match. An identity mismatch stays
read-only and is explained in Profile and Doctor. Disabling writes or forgetting
startup clears restoration.

### Preferences and review

Adding a preference is two explicit steps: save it as suggested, then inspect and
confirm the exact new record. The add dialog also takes the rule's options: where it
applies (languages, paths, agents, task tags; optional), its priority and its
declared enforcement. Record rows show them; a confirmed
rule's sheet has a **Delivered to agents** switch that takes it out of what agents
receive without a new review. A preference carries one to ten tags (lowercase
letters of any script, digits and inner hyphens, up to 32 characters) and an origin
that is exactly `operator` (typed by the operator) or `agent` (an agent's suggestion
the operator records). Tags and origin belong to the reviewed proposal: changing
either is a new suggestion. Origin is the operator's claim, not verified authorship.

A suggestion's sheet has **Edit…**: the add dialog opens with the suggestion's
content, and saving prepares one write that files the edit as a new suggestion
(origin `operator`) and closes the old one as replaced; the new one then opens to
be confirmed. Above the decisions, an optional **Note** (at most 500 characters)
is kept with the next confirm, reject, retire or edit and shown in the record's
history, with "replaced by #N" for edits. A confirmed rule that is on has
**Edit…** too ("Edit rule #N", **Save the new version**): the new version takes
effect at once and the old record keeps the earlier text. Agents cannot edit: an
agent's correction is a suggestion. When a suggestion would replace a confirmed
rule of its key, its sheet starts with **Updates #N** and the change: the rule's
text against the suggestion's in a unified diff (changed words marked, long lines
wrapped; plain − / + lines in the terminal), plus changed tags or options.

Suggested rows (in To review and in a scope's list) have a checkbox. While any are
picked, a bar above the list shows the count, an optional note and **Confirm N…**
/ **Reject N…** / **Clear**; the prepared operation lists every picked record (id,
key, digest prefix) before one write decides them all, each as its own decision.
If any picked record changed since it was listed, nothing is written. Switching
tab or scope clears the selection. Possible duplicates in the loaded queue are
marked without reordering records or selecting them implicitly.

Detail names the record's source and affected files, then shows conservative
duplicate/similar pointers from the same scope. Exact content duplicates are
distinguished from same-key and same-value records; the preview is bounded and
never replaces exact inspection. Project-scoped Detail also shows retained gate
denial count and last denial for the key, explicitly scoped to retained evidence;
global records say that no single project history applies.

**Import rule files…** in Review reads the selected project's fixed AGENTS.md,
CLAUDE.md, Cursor, Continue and Copilot inventory. Each candidate shows a
checkbox, source-kind badge, relative path, title, description (or first body
line), key and project scope. New ones start picked; ones already in Precept are
marked and cannot be picked. Skipped entries name the source and fixed reason,
with bounded detail for duplicates, unsupported metadata or applicability, and
budget exhaustion. The dialog says that reads are non-recursive, links are not
followed, source files are unchanged and unsupported glob/manual activation is
not guessed. Literal source paths are shown as Precept path filters, not as a
claim of identical source-tool auto-attachment. Saving prepares one write that files the picks as suggestions; they
are confirmed like any other. Loading, empty, partial-skip and over-batch states
use the existing dialog motion and do not introduce a separate import workflow.

The Preferences & contracts page starts with the repository's effective contracts
(read-only; changed in the repository), then one virtualized list of the confirmed
effective rows, with no pages. Repository rules (committed `.precept/rules` files) are listed with their file,
the current check result (checks pass n/m, check failing with the file, or checks
could not run), declared enforcement and guard count, and for guarded rules whether
the gate is on; uncommitted and invalid files
are named below them. In Rules → Project conventions, a confirmed record offers
**Write to repository…**: a dialog previews the exact file and path and writes it;
when the file exists and differs it shows the change as a diff and **Update file**
keeps the checks, guards and allowed values written in the file. The operator
reviews and commits it. A row's **Details** opens a dialog that renders the rule's
Markdown, with where it applies and its tags, **Copy text** and **Close** (the
terminal keeps the paged inline detail). Review rows put the origin icon in its own
column, then the title (key, number and revision; it wraps, and its accessible name
is the full record label), the value's first line and the tags beneath it, with the
status on the right. Tag filters above it narrow to rows carrying every selected tag; the filter
text matches key, value, scope or tag, and a leading `#` matches tags only. Review
lists (the queue and a scope's records) use segmented scope selectors and align
list footers to the content's leading edge. They load further batches as the list nears its
end, with a visible Load more control for keyboard and terminal use. The queue
loads when To review opens, like the record lists, and the page's Refresh reads it
again; a Profile change since it loaded is named ("The Profile changed N time(s)…").

**Check rules** runs the Operator-only, read-only lint report for the selected
repository. It shows loading, waiting-for-project, connection-error, clean,
not-configured, issue and unavailable states; issue rows name the
repository-relative file, stable code, severity and line/column, with the daemon's
fixed message and any related rule file. The report can be copied; copy output is
bounded and explicitly discloses count- or size-based omissions.
A missing current path is described as a warning that future files may still match,
not proof that a rule can never apply. **Templates…** opens Rust, Git, security and
testing starting points in a keyboard-navigable tab group: the operator selects
one, reads the exact target path and text, and copies it for repository review.
Precept never writes a template, and the dialog says so. Both actions use the
existing button, badge, tabs, dialog, scroll and toast motion; all status is carried
by text as well as color, and the whole flow is keyboard operable.

The Dashboard keeps one line per fact: the effective context (what it promises and
does not is behind its info button, in the operator's language), the daemon's
target, state and last read (transport, versions and timing are in Diagnostics),
each agent's connection level, the task to resume and the suggestions waiting. Above
these facts, one **Recommended next step** card chooses from observed state:
incomplete setup, waiting suggestions, unshared agent access, an empty preference
set, or the normal Handoff/Review route. It never infers success from unread data
and every action opens the existing guarded workflow. The
Handoff page shows the task (objective, next action), **Before you continue** (the
daemon's resume warnings in the operator's language and a short worktree
fingerprint) and **Recorded checks** (each check's result and name, never raw
records). Below it, **Saved handoffs** lists the project's handoffs (task,
objective, branch, when saved), newest first; pressing one opens it. Diagnostics lists the repository's documents as path, state and size.

Each tag has one color wherever it appears, kept per tag in the private
`settings.toml` (never in the Profile, never sent to agents). A tag without a
chosen color shows a color generated from its name as CSS `oklch()`; a tag first
used by a suggestion keeps that generated color. The operator changes a color in the
Tag colors sheet with the color picker. A color is a dot and a light tint behind
theme text; the tag's name carries its meaning. Queue absence means not yet read, not an empty queue. The
sidebar's count of suggestions waiting is the daemon's exact count for the personal
scope and the selected project; an unknown or refused count is hidden, never shown
as zero.
Pages expose their observed watermark and do not pretend to share one snapshot.

The records list selects; Detail is the immutable-content inspector and owns the
confirm/reject/deprecate/delete actions. Personal and project lists hide deprecated
records by default; a **Show deprecated** switch reveals them for inspection and
deletion. Show exact value, scope, project, tags,
origin, revision, proposal digest and peer effects before a decision. Stale or missing
inspection only triggers a fresh read, never an automatic mutation.

Delete requires a separate destructive tier and retyping the exact preference ID.
The dialog shows that ID beside a **Copy ID** command so the operator does not have
to find it elsewhere. Escape cancels consent. Profile, grant, target, project, task, scope, selection,
record content or watermark drift invalidates prepared work and delayed replies. The daemon
independently rechecks authority at mutation admission.

An uncertain commit retains only the exact same-epoch request for deliberate replay.
Changing its bytes, authority or scope discards replay authority. Receipts prove
transactions, not reviewer identity; channel and origin labels are not
authentication or independent verification.

### Handoff and evidence

Handoff resume uses exact task lookup and presents objective, next action, current
versus recorded HEAD/branch/worktree/source versions and checks to rerun.
Acknowledging receipt is neither permission approval nor independent verification.

Evidence displays result, origin, source revision/platform, reporter,
observer/verifier, method and limitations together. Agent-reported,
daemon-observed and independently verified claims must not share an unexplained
success badge. Historical PASS remains labeled historical.

### Diagnostics (Doctor)

Diagnostics opens with two equal-width cards, **Daemon** and **Diagnostics**, side by
side above **Doctor** (stacked in the compact layout). Doctor has one row per link an
agent needs to receive the selected workspace's context, in the order a fix usually
has to happen (daemon,
Profile and writes, workspace registration and agent sharing, preferences
delivered and waiting, repository rules (invalid files, failing checks, files not
committed) and the pre-tool gate (switch, gate hooks; reported as set up, never as
enforcement achieved), each harness's session-start hook, connection by agent (OK when one agent has
hooks + MCP), the AGENTS.md section,
agent reads since the daemon started). Each row shows what was observed and one of
OK, Look, Problem, Off or Not read; a value that was not read is never shown as a
pass. A row that can be fixed has **Fix…**, which opens the page where the change
is made; Doctor itself writes nothing. **Check again** re-reads; **Copy report**
copies the rows as text without paths, preference values, commands or
credentials. The raw daemon and evidence lines stay below.

## Overlays and feedback

Use a sheet for record detail/history beside a list, a dialog for a short focused
form or Apply decision, and an alert dialog for destructive confirmation. Dialog
width follows the decision and its content always fits inside the panel at every
window size. Notifications float in the window's overlay layer and never take,
move or resize page layout. Avoid
stacked panels for one job. Notifications report outcomes but never replace an
inline uncertain outcome or a required decision. Longer page explanations belong
behind an information control rather than obscuring the primary workflow.

Native modals remain mounted for one open interval; label and validation updates do
not restart entrance motion. Initial focus is requested after the input and modal
focus scope exist. Hidden controls cannot receive focus.

## Motion

Use shared motion tokens and helpers, and keep each screen's motion inventory in
the [design system](../apps/ui/DESIGN.md#9-motion) current. Animate opacity and transform rather than
layout where possible; state feedback is lighter than entrances, and exits are
faster than entrances. Do not animate frequently updating data or routine list
refreshes as decorative entrances.

Every motion honors reduced-motion settings: durations collapse, scale/translation
rests and state remains understandable. Spinner motion rests while its loading
label remains. Motion never confirms a mutation, focus, connection or rendered
result by itself.

## Platform and honest states

The operator app is a desktop app (Windows, and Linux under Wayland). There is no
terminal or web interface; the `precept` CLI covers headless use. Mobile is not
a target.

Keep review (`Suggested`, `Confirmed`, `Rejected`, `Deprecated`), resolution
(`Applies`, `Masked`, `Conflict`, `Unknown`), connection, lease and evidence origin
as separate textual dimensions. Empty, unavailable, refused and not implemented
are different states. Never fabricate agent presence, infer health from an old
read, turn unknown into zero or relabel an unknown fingerprint as clean.

Reuse shared theme, localization and content bounds. Project data remains in its
original language. Validate affected behavior with shared headless tests and real
IPC integration, then inspect the running desktop app for visual and input
behavior. Headless success is not proof of a mapped window, physical keyboard,
IME, screen reader or platform support.
