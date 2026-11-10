//! Small original outline vectors; labels always carry the same meaning in TUI.
use super::*;
use acp_ui_spike_model::Section;
pub(super) fn section(section: Section) -> impl IntoElement {
    let path = match section {
        Section::Dashboard => "M3 3H10V10H3ZM14 3H21V10H14ZM3 14H10V21H3ZM14 14H21V21H14Z",
        Section::Projects => "M3 6H10L12 9H21V20H3Z",
        Section::Preferences => "M4 6H20M4 12H20M4 18H20M8 3V9M16 9V15M10 15V21",
        Section::Handoff => "M4 4H14V20H4ZM9 9H21M17 5L21 9L17 13",
        Section::Diagnostics => "M3 12H7L10 5L14 19L17 12H21",
    };
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
