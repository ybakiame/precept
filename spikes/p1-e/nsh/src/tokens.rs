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
    let color = AdaptiveColor::rgb;
    if model.get().high_contrast {
        return Palette {
            background: color(0, 0),
            panel: color(0, 0),
            inset: color(0, 0),
            text: color(0xffffff, 0xffffff),
            muted: color(0xffffff, 0xffffff),
            border: color(0xffffff, 0xffffff),
            primary: color(0x00ffff, 0x00ffff),
            on_primary: color(0, 0),
            warning: color(0xffff00, 0xffff00),
            error: color(0xffa0a0, 0xffa0a0),
        };
    }
    Palette {
        background: color(0xf1f5f9, 0x020617),
        panel: color(0xffffff, 0x0f172a),
        inset: color(0xf8fafc, 0x172033),
        text: color(0x0f172a, 0xe2e8f0),
        muted: color(0x475569, 0xa8b6ca),
        border: color(0x64748b, 0x64748b),
        primary: color(0x1d4ed8, 0x60a5fa),
        on_primary: color(0xffffff, 0x071426),
        warning: color(0x92400e, 0xfbbf24),
        error: color(0xb91c1c, 0xfca5a5),
    }
}
pub fn compact() -> bool {
    TERMINAL_SURFACE.load(std::sync::atomic::Ordering::Relaxed) || window().width.0 < 900.0
}
