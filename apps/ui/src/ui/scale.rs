//! The app's geometry scales: radii, type and control sizes.
//!
//! Presentation code reads these instead of writing literal points, so one
//! change here restyles every control. Spacing and gaps use the shared
//! Tailwind scale (`.gap_2()`, `.p_3()`); these tokens cover the values that
//! scale has no step for (radii, type sizes, control heights).

/// Corner radii. Controls use [`CONTROL`]; the containers that hold them use
/// [`CONTAINER`]; small parts (badges, dots) use [`SMALL`]; intentional pills
/// and circles use [`PILL`]. Nested surfaces stay concentric: a control inside
/// a container reads as `CONTAINER - gap`.
pub mod radius {
    /// Buttons, inputs, tabs, rows.
    pub const CONTROL: f32 = 6.0;
    /// Cards, lists, popovers, menus, theme cards.
    pub const CONTAINER: f32 = 10.0;
    /// Dialogs, toasts, sheets: the floating layers.
    pub const OVERLAY: f32 = 12.0;
    /// Badges, count pills, small buttons, the tab indicator inset.
    pub const SMALL: f32 = 4.0;
    /// Intentional circles and pills only.
    pub const PILL: f32 = 999.0;
}

/// Type scale (points). The platform UI font carries interface text; monospace
/// is reserved for identifiers and aligned numbers. Body is the reference the
/// other steps relate to; the user's text size scales all of them.
pub mod text {
    /// Secondary metadata, help, captions.
    pub const CAPTION: f32 = 12.0;
    /// Body text and control labels.
    pub const BODY: f32 = 14.0;
    /// Subtitles, large control labels.
    pub const SUBTITLE: f32 = 16.0;
    /// Section and page titles.
    pub const TITLE: f32 = 20.0;
    /// The window's principal title.
    pub const DISPLAY: f32 = 22.0;
}

/// Control heights by size. One size sets height, padding, text and icon
/// together; a compact context tightens padding, not height.
pub mod control {
    /// Small: toolbars, dense repeated actions.
    pub const SM: f32 = 24.0;
    /// Medium: the default for most chrome and workflows.
    pub const MD: f32 = 32.0;
    /// Large: prominent decisions, touch-friendly frames.
    pub const LG: f32 = 40.0;
}

/// Region widths (points): how wide a column may grow before it stops helping.
pub mod layout {
    /// A form or settings column: labels, fields and their help stay scannable.
    pub const FORM: f32 = 640.0;
    /// The narrowest anchored popup (menus, selects).
    pub const POPUP: f32 = 192.0;
    /// The sidebar: wide enough for stable labels, subordinate to the page.
    pub const SIDEBAR: f32 = 232.0;
    /// Dialog widths by decision: confirm, short form, form with a rich value.
    pub const DIALOG_SM: f32 = 400.0;
    pub const DIALOG_MD: f32 = 520.0;
    pub const DIALOG_LG: f32 = 640.0;
    /// A toast's width; the stack floats 16 from the window's edges.
    pub const TOAST: f32 = 360.0;
    /// The title bar's height.
    pub const TITLE_BAR: f32 = 36.0;
    /// A trailing-edge sheet (record details beside a list).
    pub const SHEET: f32 = 440.0;
    /// A wide trailing sheet (a list with several lanes: the shortcuts).
    pub const SHEET_WIDE: f32 = 600.0;
    /// The label lane of a properties list.
    pub const LABEL_LANE: f32 = 144.0;
}

/// Icon sizes (points), one step below the text they sit beside.
pub mod icon {
    pub const SM: f32 = 14.0;
    pub const MD: f32 = 16.0;
    /// Toast and dialog header icons.
    pub const LG: f32 = 20.0;
    /// Empty-state icons.
    pub const XL: f32 = 24.0;
}

/// Disc diameters behind icons (a tinted circle that names a tone).
pub mod disc {
    /// Toasts and dialog headers.
    pub const MD: f32 = 28.0;
    /// Empty states.
    pub const LG: f32 = 48.0;
    /// Status dots.
    pub const DOT: f32 = 8.0;
}
