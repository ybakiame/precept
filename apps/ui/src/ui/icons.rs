//! Small original outline vectors; labels always carry the same meaning in TUI.
use crate::*;
use acp_ui_core::Section;
/// The outline path of a section's destination.
pub(crate) fn section_path(section: Section) -> &'static str {
    match section {
        Section::Dashboard => "M3 3H10V10H3ZM14 3H21V10H14ZM3 14H10V21H3ZM14 14H21V21H14Z",
        Section::Projects => "M3 6H10L12 9H21V20H3Z",
        Section::Preferences => "M4 6H20M4 12H20M4 18H20M8 3V9M16 9V15M10 15V21",
        Section::Handoff => "M4 4H14V20H4ZM9 9H21M17 5L21 9L17 13",
        Section::Diagnostics => "M3 12H7L10 5L14 19L17 12H21",
    }
}
/// Review: an inbox of suggestions awaiting a decision.
pub(crate) const REVIEW: &str = "M3 13H8L10 16H14L16 13H21M5 6H19L21 13V19H3V6Z";
/// Sessions: an agent process in a terminal.
pub(crate) const SESSIONS: &str = "M4 5H20V19H4ZM8 10L11 12.5L8 15M13 15H16";
/// Connection: a link between the app and its daemon.
pub(crate) const CONNECTION: &str =
    "M9 15L15 9M10 6L12 4A4 4 0 0 1 20 12L18 14M14 18L12 20A4 4 0 0 1 4 12L6 10";
/// Settings: sliders.
/// A person's card: the Profile.
pub(crate) const PROFILE: &str =
    "M4 5H20V19H4ZM9 10A2 2 0 1 0 9.01 10M6.5 16C7 14 11 14 11.5 16M14 10H18M14 14H17";
/// A plug: agents connecting.
pub(crate) const AGENTS: &str = "M9 3V8M15 3V8M6 8H18V12A6 6 0 0 1 6 12ZM12 18V21";
pub(crate) const SETTINGS: &str = "M4 7H14M18 7H20M4 17H8M12 17H20M16 5V9M10 15V19";
/// Re-read.
pub(crate) const REFRESH: &str = "M20 12A8 8 0 1 1 12 4M20 4V8H16";
/// The ACP mark: three stacked planes; context travels down to the agent.
pub(crate) const BRAND: &str = "M12 3L21 7.5L12 12L3 7.5ZM3 12L12 16.5L21 12M3 16.5L12 21L21 16.5";
/// Personal scope: one person.
pub(crate) const PERSON: &str = "M12 12A4 4 0 1 0 12 4A4 4 0 1 0 12 12M4 21C5 16 19 16 20 21";
/// Copy text.
pub(crate) const COPY: &str = "M8 8H19V19H8ZM5 15V5H15";
/// Add something.
pub(crate) const PLUS: &str = "M12 5V19M5 12H19";
/// Done, confirmed.
pub(crate) const CHECK: &str = "M5 12.5L10 17.5L19 7";
/// Needs attention.
pub(crate) const ALERT: &str = "M12 4L21.5 20H2.5ZM12 10V14M12 17V17.01";
/// Failed or refused.
pub(crate) const ERROR: &str = "M12 21A9 9 0 1 0 12 3A9 9 0 1 0 12 21M9 9L15 15M15 9L9 15";
/// More explanation.
pub(crate) const INFO: &str = "M12 21A9 9 0 1 0 12 3A9 9 0 1 0 12 21M12 11V16M12 8V8.01";
/// Dismiss.
pub(crate) const CLOSE: &str = "M6 6L18 18M18 6L6 18";
/// Destructive removal.
#[allow(dead_code)]
pub(crate) const TRASH: &str = "M4 7H20M9 7V4H15V7M6 7L7 20H17L18 7M10 11V16M14 11V16";
/// Writes off.
#[allow(dead_code)]
pub(crate) const LOCK: &str = "M6 11H18V20H6ZM8 11V8A4 4 0 0 1 16 8V11";
/// Writes on.
#[allow(dead_code)]
pub(crate) const UNLOCK: &str = "M6 11H18V20H6ZM8 11V8A4 4 0 0 1 15.5 6";
/// Filter.
#[allow(dead_code)]
pub(crate) const SEARCH: &str = "M11 18A7 7 0 1 0 11 4A7 7 0 1 0 11 18M16 16L20 20";
#[allow(dead_code)]
pub(crate) fn section(section: Section) -> impl IntoElement {
    let path = section_path(section);
    let markup = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path d='{path}' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'/></svg>"
    );
    view()
        .style_when(
            || TERMINAL_SURFACE.load(std::sync::atomic::Ordering::Relaxed),
            |s| s.hidden(),
        )
        .child(
            svg(nsh_core::element::ImageSrc::svg_markup(&markup))
                .w(px(16.0))
                .h(px(16.0)),
        )
}
