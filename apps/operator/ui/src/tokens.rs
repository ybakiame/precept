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
    pub success: AdaptiveColor,
    pub danger: AdaptiveColor,
    pub on_danger: AdaptiveColor,
}
pub fn palette(model: Signal<Model>) -> Palette {
    let m = model.get();
    let is_dark = match m.color_mode {
        acp_operator_model::appearance::ColorMode::System => dark(),
        acp_operator_model::appearance::ColorMode::Dark => true,
        acp_operator_model::appearance::ColorMode::Light => false,
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
        success: c(p.success),
        danger: c(p.danger),
        on_danger: c(p.danger_foreground),
    }
}
pub fn compact() -> bool {
    TERMINAL_SURFACE.load(std::sync::atomic::Ordering::Relaxed) || window().width.0 < 900.0
}
/// Mix two semantic colors per appearance (`t` toward `b`). Derives hover and
/// pressed washes from tokens without introducing a raw color value.
pub fn mix(a: AdaptiveColor, b: AdaptiveColor, t: f32) -> AdaptiveColor {
    AdaptiveColor::new(a.light.mix(b.light, t), a.dark.mix(b.dark, t))
}
