//! Presentation-only theme collection. Geometry, typography and data never vary by theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeId {
    Instrument,
    Ledger,
    #[default]
    Harbor,
}
impl ThemeId {
    pub const ALL: [Self; 3] = [Self::Instrument, Self::Ledger, Self::Harbor];
    pub fn name(self) -> &'static str {
        match self {
            Self::Instrument => "Instrument",
            Self::Ledger => "Ledger",
            Self::Harbor => "Harbor",
        }
    }
    pub fn colors(self, dark: bool, contrast: bool) -> Colors {
        if contrast {
            return Colors {
                background: 0,
                panel: 0,
                inset: 0,
                text: 0xffffff,
                muted: 0xffffff,
                border: 0xffffff,
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
            (Self::Instrument, true) => Colors {
                background: 0x111718,
                panel: 0x1b2425,
                inset: 0x263334,
                text: 0xeaf2ee,
                muted: 0xa6bab2,
                border: 0x738d83,
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
                panel: 0xffffff,
                inset: 0xe0eae3,
                text: 0x1d2c25,
                muted: 0x425e4e,
                border: 0x667d6d,
                accent: 0x246046,
                on_accent: 0xffffff,
                warning: 0x805005,
                error: 0xa32f30,
                success: 0x246046,
                danger: 0xb02a30,
                danger_foreground: 0xffffff,
            },
            (Self::Ledger, false) => Colors {
                background: 0xf4f0e7,
                panel: 0xfffcf5,
                inset: 0xe8e1d3,
                text: 0x302b25,
                muted: 0x695e50,
                border: 0x8b7e6a,
                accent: 0x8e3e26,
                on_accent: 0xfffcf5,
                warning: 0x805005,
                error: 0xa32f30,
                success: 0x3e6337,
                danger: 0xb02a30,
                danger_foreground: 0xffffff,
            },
            (Self::Ledger, true) => Colors {
                background: 0x211f1b,
                panel: 0x302b25,
                inset: 0x40382e,
                text: 0xf7eee1,
                muted: 0xc7b69e,
                border: 0x9e8c72,
                accent: 0xedb095,
                on_accent: 0x28170f,
                warning: 0xf3cb86,
                error: 0xffb4ab,
                success: 0xabd2ac,
                danger: 0xff9e93,
                danger_foreground: 0x2a0a0a,
            },
            (Self::Harbor, true) => Colors {
                background: 0x131c30,
                panel: 0x202d46,
                inset: 0x2c3b56,
                text: 0xedf2fc,
                muted: 0xb3c2db,
                border: 0x7c91b4,
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
                panel: 0xffffff,
                inset: 0xe3e9f3,
                text: 0x1d2943,
                muted: 0x4d5e79,
                border: 0x70819a,
                accent: 0x344e9c,
                on_accent: 0xffffff,
                warning: 0x7f4f00,
                error: 0xa82e3f,
                success: 0x236349,
                danger: 0xb02a30,
                danger_foreground: 0xffffff,
            },
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colors {
    pub background: u32,
    pub panel: u32,
    pub inset: u32,
    pub text: u32,
    pub muted: u32,
    pub border: u32,
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
                    for bg in [p.background, p.panel, p.inset] {
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
