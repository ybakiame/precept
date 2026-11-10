//! Semantic colors shared by shell, cards, controls, inputs and states.
use super::*;
#[derive(Clone, Copy)]
pub struct Palette {
    pub background: AdaptiveColor,
    pub panel: AdaptiveColor,
    pub inset: AdaptiveColor,
    pub text: AdaptiveColor,
    pub muted: AdaptiveColor,
    pub border: AdaptiveColor,
    pub primary: AdaptiveColor,
    pub on_primary: AdaptiveColor,
    pub warning: AdaptiveColor,
    pub error: AdaptiveColor,
}
pub fn palette(model: Signal<Model>) -> Palette {
    let m = model.get();
    let is_dark = match m.color_mode {
        acp_ui_spike_model::appearance::ColorMode::System => dark(),
        acp_ui_spike_model::appearance::ColorMode::Dark => true,
        acp_ui_spike_model::appearance::ColorMode::Light => false,
    };
    let p = m.theme.colors(is_dark, m.high_contrast);
    let c = |v| AdaptiveColor::rgb(v, v);
    Palette {
        background: c(p.background),
        panel: c(p.panel),
        inset: c(p.inset),
        text: c(p.text),
        muted: c(p.muted),
        border: c(p.border),
        primary: c(p.accent),
        on_primary: c(p.on_accent),
        warning: c(p.warning),
        error: c(p.error),
    }
}
pub fn compact() -> bool {
    TERMINAL_SURFACE.load(std::sync::atomic::Ordering::Relaxed) || window().width.0 < 900.0
}
