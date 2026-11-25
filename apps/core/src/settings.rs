//! The operator's settings kept between launches, as `settings.toml` in the
//! private state directory beside the saved connection: appearance, language,
//! shortcut overrides and tag colors. Presentation only: never data, a path, a
//! credential or authority. Unknown fields refuse the whole file (the app
//! then starts from defaults); unknown values within a known field are
//! dropped one by one.
use crate::{
    Model,
    appearance::{ColorMode, ThemeId},
    i18n::Locale,
    keys::Keymap,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub format: u32,
    #[serde(default)]
    pub appearance: Appearance,
    /// Action id → keystroke, only where the operator changed it.
    #[serde(default)]
    pub shortcuts: BTreeMap<String, String>,
    /// Tag name → color, only for tags whose color was chosen or pinned.
    #[serde(default)]
    pub tag_colors: BTreeMap<String, String>,
    /// The operator completed or skipped the first-launch guide.
    #[serde(default)]
    pub onboarding_seen: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Appearance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_contrast: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidebar_collapsed: Option<bool>,
}

/// What launch arguments set explicitly; those win over saved settings.
#[derive(Clone, Copy, Debug, Default)]
pub struct Explicit {
    pub theme: bool,
    pub mode: bool,
    pub language: bool,
}

impl Settings {
    pub const FORMAT: u32 = 1;

    /// The settings a model holds now.
    pub fn of(m: &Model) -> Self {
        Self {
            format: Self::FORMAT,
            appearance: Appearance {
                theme: Some(m.theme.id().into()),
                mode: Some(m.color_mode.id().into()),
                high_contrast: Some(m.high_contrast),
                language: Some(
                    match m.locale {
                        Locale::En => "en",
                        Locale::ZhCn => "zh-CN",
                    }
                    .into(),
                ),
                sidebar_collapsed: Some(m.sidebar_collapsed),
            },
            shortcuts: m.keymap.overrides().clone(),
            tag_colors: m.tag_colors.clone(),
            onboarding_seen: m.onboarding_seen,
        }
    }

    /// Applies what is known and valid; launch arguments in `explicit` win.
    /// A different format applies nothing.
    pub fn apply(&self, m: &mut Model, explicit: Explicit) {
        if self.format != Self::FORMAT {
            return;
        }
        let a = &self.appearance;
        if !explicit.theme
            && let Some(theme) = a.theme.as_deref().and_then(ThemeId::from_id)
        {
            m.theme = theme;
        }
        if !explicit.mode
            && let Some(mode) = a.mode.as_deref().and_then(ColorMode::from_id)
        {
            m.color_mode = mode;
        }
        if let Some(contrast) = a.high_contrast {
            m.high_contrast = contrast;
        }
        if let Some(collapsed) = a.sidebar_collapsed {
            m.sidebar_collapsed = collapsed;
        }
        if !explicit.language
            && let Some(locale) = a.language.as_deref().and_then(Locale::parse)
        {
            m.locale = locale;
        }
        m.keymap = Keymap::from_overrides(self.shortcuts.clone());
        m.tag_colors = crate::tag_colors::sanitize(self.tag_colors.clone());
        m.onboarding_seen = self.onboarding_seen;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_round_trips_and_explicit_arguments_win() {
        let mut m = Model::loading();
        m.theme = ThemeId::Ledger;
        m.color_mode = ColorMode::Light;
        m.high_contrast = true;
        m.locale = Locale::ZhCn;
        m.keymap.set("reconnect", "mod+e");
        m.set_tag_color("git", "oklch(60% 0.2 30)");
        m.onboarding_seen = true;
        let saved = Settings::of(&m);

        let mut fresh = Model::loading();
        saved.apply(&mut fresh, Explicit::default());
        assert_eq!(fresh.theme, ThemeId::Ledger);
        assert_eq!(fresh.color_mode, ColorMode::Light);
        assert!(fresh.high_contrast);
        assert_eq!(fresh.locale, Locale::ZhCn);
        assert_eq!(fresh.keymap.primary("reconnect").as_deref(), Some("mod+e"));
        assert_eq!(fresh.tag_color("git"), "oklch(60% 0.2 30)");
        assert!(fresh.onboarding_seen);

        let mut flagged = Model::loading();
        let theme = flagged.theme;
        saved.apply(
            &mut flagged,
            Explicit {
                theme: true,
                mode: false,
                language: true,
            },
        );
        assert_eq!(flagged.theme, theme);
        assert_eq!(flagged.locale, Locale::En);
        assert_eq!(flagged.color_mode, ColorMode::Light);
    }

    #[test]
    fn unknown_values_are_dropped_and_other_formats_apply_nothing() {
        let mut s = Settings::of(&Model::loading());
        s.appearance.theme = Some("neon".into());
        s.shortcuts.insert("nope".into(), "mod+x".into());
        let mut m = Model::loading();
        let theme = m.theme;
        s.apply(&mut m, Explicit::default());
        assert_eq!(m.theme, theme);
        assert!(m.keymap.overrides().is_empty());
        s.format = 2;
        s.appearance.mode = Some("light".into());
        let mut other = Model::loading();
        s.apply(&mut other, Explicit::default());
        assert_eq!(other.color_mode, ColorMode::default());
    }
}
