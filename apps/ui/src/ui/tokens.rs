//! Semantic colors shared by shell, cards, controls, inputs and states.
use crate::*;
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
        acp_operator_core::appearance::ColorMode::System => dark(),
        acp_operator_core::appearance::ColorMode::Dark => true,
        acp_operator_core::appearance::ColorMode::Light => false,
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
/// Mix two semantic colors per appearance, moving `t` toward `b` (`0.0` = `a`,
/// `1.0` = `b`). Derives hover and pressed washes from tokens without
/// introducing a raw color value. NSH's `Hsla::mix(other, factor)` keeps
/// `factor` of `self`, so the weight is inverted here; passing `t` through made
/// a "7% wash" 93% text color (near-white hover fill with unreadable text).
pub fn mix(a: AdaptiveColor, b: AdaptiveColor, t: f32) -> AdaptiveColor {
    let keep = 1.0 - t.clamp(0.0, 1.0);
    AdaptiveColor::new(a.light.mix(b.light, keep), a.dark.mix(b.dark, keep))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsh_core::color::hsla;

    #[test]
    fn mix_moves_t_toward_b() {
        let dark_bg = hsla(0.6, 0.4, 0.15, 1.0);
        let light_text = hsla(0.6, 0.6, 0.9, 1.0);
        let (a, b) = (
            AdaptiveColor::new(dark_bg, dark_bg),
            AdaptiveColor::new(light_text, light_text),
        );
        let close = |x: f32, y: f32| (x - y).abs() < 1e-4;
        assert!(close(mix(a, b, 0.0).dark.l, 0.15));
        assert!(close(mix(a, b, 1.0).dark.l, 0.9));
        // A hover wash stays near the background: readable text on top of it.
        let wash = mix(a, b, 0.07);
        assert!(close(wash.dark.l, 0.15 + 0.07 * 0.75));
        assert!(close(wash.light.l, wash.dark.l));
        assert!(wash.dark.l < 0.25, "wash must stay dark under light text");
    }
}
