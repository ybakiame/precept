//! Small original outline vectors; labels always carry the same meaning in TUI.
use crate::*;
use acp_operator_core::Section;
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
pub(crate) const SETTINGS: &str = "M4 7H14M18 7H20M4 17H8M12 17H20M16 5V9M10 15V19";
/// Re-read.
pub(crate) const REFRESH: &str = "M20 12A8 8 0 1 1 12 4M20 4V8H16";
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
