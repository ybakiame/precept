//! Presentation-only theme collection. Geometry, typography and data never vary by theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeId {
    /// Neutral graphite with an iris accent: the quietest, and the default.
    #[default]
    Graphite,
    Harbor,
    Ledger,
    Instrument,
}
impl ThemeId {
    pub const ALL: [Self; 4] = [Self::Graphite, Self::Harbor, Self::Ledger, Self::Instrument];
    pub fn name(self) -> &'static str {
        match self {
            Self::Graphite => "Graphite",
            Self::Instrument => "Instrument",
            Self::Ledger => "Ledger",
            Self::Harbor => "Harbor",
        }
    }
    pub fn colors(self, dark: bool, contrast: bool) -> Colors {
        if contrast {
            return Colors {
                background: 0,
                sidebar: 0,
                panel: 0,
                raised: 0,
                inset: 0,
                text: 0xffffff,
                muted: 0xffffff,
                border: 0xffffff,
                hairline: 0xffffff,
                accent: 0x00ffff,
                on_accent: 0,
                warning: 0xffff00,
                error: 0xffb0b0,
                success: 0xaaffaa,
                danger: 0xff9e9e,
                danger_foreground: 0,
            };
        }
        match (self, dark) {
            (Self::Graphite, true) => Colors {
                background: 0x111113,
                sidebar: 0x161618,
                panel: 0x1b1b1f,
                raised: 0x222227,
                inset: 0x29292f,
                text: 0xededf0,
                muted: 0xa3a3ad,
                border: 0x75757f,
                hairline: 0x2c2c33,
                accent: 0xb4bcff,
                on_accent: 0x1b1d48,
                warning: 0xf2c46d,
                error: 0xff9d9d,
                success: 0x8fdcae,
                danger: 0xff8f8f,
                danger_foreground: 0x2b0b0b,
            },
            (Self::Graphite, false) => Colors {
                background: 0xf6f6f7,
                sidebar: 0xeeeef1,
                panel: 0xffffff,
                raised: 0xffffff,
                inset: 0xe9e9ed,
                text: 0x18181c,
                muted: 0x50505a,
                border: 0x7f7f8a,
                hairline: 0xdedee3,
                accent: 0x4446c9,
                on_accent: 0xffffff,
                warning: 0x8a4a06,
                error: 0xb3261e,
                success: 0x17663a,
                danger: 0xb3261e,
                danger_foreground: 0xffffff,
            },
            (Self::Harbor, true) => Colors {
                background: 0x111a2c,
                sidebar: 0x152035,
                panel: 0x1a2740,
                raised: 0x21304c,
                inset: 0x293a58,
                text: 0xedf2fc,
                muted: 0xb3c2db,
                border: 0x7c91b4,
                hairline: 0x2a3a57,
                accent: 0xb5c8ff,
                on_accent: 0x182445,
                warning: 0xffd394,
                error: 0xffb4bd,
                success: 0x9ddfc1,
                danger: 0xff9e93,
                danger_foreground: 0x2a0a0a,
            },
            (Self::Harbor, false) => Colors {
                background: 0xf2f4f9,
                sidebar: 0xe9edf5,
                panel: 0xffffff,
                raised: 0xffffff,
                inset: 0xe3e9f3,
                text: 0x1d2943,
                muted: 0x4d5e79,
                border: 0x70819a,
                hairline: 0xd8dfeb,
                accent: 0x344e9c,
                on_accent: 0xffffff,
                warning: 0x7f4f00,
                error: 0xa82e3f,
                success: 0x236349,
                danger: 0xb02a30,
                danger_foreground: 0xffffff,
            },
            (Self::Ledger, true) => Colors {
                background: 0x1d1b18,
                sidebar: 0x221f1b,
                panel: 0x29251f,
                raised: 0x302b25,
                inset: 0x3a342c,
                text: 0xf7eee1,
                muted: 0xc7b69e,
                border: 0x9e8c72,
                hairline: 0x3b352d,
                accent: 0xedb095,
                on_accent: 0x28170f,
                warning: 0xf3cb86,
                error: 0xffb4ab,
                success: 0xabd2ac,
                danger: 0xff9e93,
                danger_foreground: 0x2a0a0a,
            },
            (Self::Ledger, false) => Colors {
                background: 0xf4f0e7,
                sidebar: 0xede7da,
                panel: 0xfffcf5,
                raised: 0xfffcf5,
                inset: 0xe8e1d3,
                text: 0x302b25,
                muted: 0x62584b,
                border: 0x8b7e6a,
                hairline: 0xe0d8c8,
                accent: 0x8e3e26,
                on_accent: 0xfffcf5,
                warning: 0x7a4c05,
                error: 0xa32f30,
                success: 0x3a5e34,
                danger: 0xb02a30,
                danger_foreground: 0xffffff,
            },
            (Self::Instrument, true) => Colors {
                background: 0x0f1415,
                sidebar: 0x131a1b,
                panel: 0x182122,
                raised: 0x1e292a,
                inset: 0x253233,
                text: 0xeaf2ee,
                muted: 0xa6bab2,
                border: 0x738d83,
                hairline: 0x263233,
                accent: 0x9ee6be,
                on_accent: 0x10231b,
                warning: 0xf3cc86,
                error: 0xffb4ab,
                success: 0x9ee6be,
                danger: 0xff9e93,
                danger_foreground: 0x2a0a0a,
            },
            (Self::Instrument, false) => Colors {
                background: 0xf1f5f2,
                sidebar: 0xe8efea,
                panel: 0xffffff,
                raised: 0xffffff,
                inset: 0xe0eae3,
                text: 0x1d2c25,
                muted: 0x425e4e,
                border: 0x667d6d,
                hairline: 0xd6e1d9,
                accent: 0x246046,
                on_accent: 0xffffff,
                warning: 0x7a4c05,
                error: 0xa32f30,
                success: 0x246046,
                danger: 0xb02a30,
                danger_foreground: 0xffffff,
            },
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colors {
    /// The page floor.
    pub background: u32,
    /// The sidebar and the title bar above it.
    pub sidebar: u32,
    /// Cards, lists and input fills.
    pub panel: u32,
    /// Everything that floats: dialogs, sheets, toasts, menus, popovers.
    pub raised: u32,
    /// Control fills and the selected row.
    pub inset: u32,
    pub text: u32,
    pub muted: u32,
    /// Boundaries the user interacts with (inputs, focus); at least 3:1.
    pub border: u32,
    /// Decorative edges and dividers; never the only boundary of a control.
    pub hairline: u32,
    pub accent: u32,
    pub on_accent: u32,
    pub warning: u32,
    pub error: u32,
    pub success: u32,
    /// Destructive action fill (a danger button, an armed destructive control).
    pub danger: u32,
    /// Foreground on `danger`; also the danger border/text on a surface.
    pub danger_foreground: u32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorMode {
    System,
    Light,
    #[default]
    Dark,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn luminance(rgb: u32) -> f64 {
        [16, 8, 0]
            .into_iter()
            .zip([0.2126, 0.7152, 0.0722])
            .map(|(shift, weight)| {
                let x = f64::from((rgb >> shift) & 255) / 255.0;
                weight
                    * if x <= 0.04045 {
                        x / 12.92
                    } else {
                        ((x + 0.055) / 1.055).powf(2.4)
                    }
            })
            .sum()
    }
    fn ratio(a: u32, b: u32) -> f64 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
    #[test]
    fn every_theme_mode_and_contrast_has_legible_semantic_pairs() {
        for theme in ThemeId::ALL {
            for dark in [false, true] {
                for hc in [false, true] {
                    let p = theme.colors(dark, hc);
                    for bg in [p.background, p.sidebar, p.panel, p.raised, p.inset] {
                        for fg in [
                            p.text, p.muted, p.accent, p.warning, p.error, p.success, p.danger,
                        ] {
                            assert!(
                                ratio(fg, bg) >= 4.5,
                                "{theme:?} dark={dark} hc={hc} text {fg:x}/{bg:x}"
                            );
                        }
                        assert!(ratio(p.border, bg) >= 3.0, "{theme:?} boundary {bg:x}");
                    }
                    assert!(ratio(p.on_accent, p.accent) >= 4.5);
                    // A hairline is visible but quieter than an interactive boundary.
                    if !hc {
                        assert!(ratio(p.hairline, p.panel) >= 1.15, "{theme:?} hairline");
                        assert!(ratio(p.hairline, p.panel) < ratio(p.border, p.panel));
                    }
                    assert!(
                        ratio(p.danger_foreground, p.danger) >= 4.5,
                        "{theme:?} dark={dark} hc={hc} danger_foreground {:x}/danger {:x}",
                        p.danger_foreground,
                        p.danger
                    );
                }
            }
        }
    }
}
