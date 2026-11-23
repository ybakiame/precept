# Precept Operator

The Precept Operator is the NSH desktop app for the local Precept daemon. It uses
the same operator authority and scoped data as the native CLI; it never opens a
Profile directly or creates a second write owner.

Navigation is grouped into **Workspace** (Dashboard, Preferences & contracts,
Agents, Sessions, Memory and Handoff for the selected project), **Review**
(suggestions, personal preferences and project conventions) and **Set up**
(Runtime checks: Connection, Profile and Workspaces), with Diagnostics and
Settings at the navigation foot. See the [UI standards](../docs/product-ui.md) for interaction,
scope and presentation contracts, and the [design system](ui/DESIGN.md) for the
visual language, components and motion.

## Review keyboard flow

These are the default keys; every shortcut can be changed in the Shortcuts panel
(`Ctrl+/` or `F1`), and changes are kept in the private `settings.toml`.

Alt+O opens the review workspace; Alt+Left/Right switches Queue, Detail and History.
Load the scoped suggested queue, select a row and inspect the exact revision,
proposal fields, digest and peer-demotion disclosure before deciding.

Alt+Shift+Y/N prepares confirm/reject, W prepares deprecate, Q opens the separate
delete tier, and V applies a prepared operation. If inspection is missing or stale,
a decision chord only refreshes Detail; invoke it again after reviewing the result
to prepare. It never applies automatically.

Delete requires retyping the exact preference ID in its focused input before V.
Escape cancels consent. Profile, grant, target, project, task, scope, selection,
source or watermark drift invalidates review and delayed replies. The daemon
independently rechecks authority before every mutation.

An uncertain commit retains only the exact same-epoch request for deliberate
Alt+Shift+Z replay. Changing its authority or scope discards replay authority.
Receipts prove transactions, not reviewer identity; history channel labels and
origin claims are not authentication or independent verification.

## Platform boundaries

The Windows Desktop can operate a Windows daemon over a current-user named pipe,
or a daemon inside an explicitly selected WSL2 distribution through `precept-bridge`.
The local Connection check recognizes `%LOCALAPPDATA%\Precept-state\runtime` and can
start the `preceptd.exe` beside the Desktop only after an explicit Start action. The
bridge carries typed requests only; it does not open SQLite, expose a network listener
or grant authority. Closing the UI or leaving the WSL target closes the helper without
stopping the daemon or changing Profile state.

Desktop behavior and platform support require actual native validation; headless
behavior is not a substitute.
