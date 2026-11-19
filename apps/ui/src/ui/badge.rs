//! The app's `Badge`: a short state, count or classification for rapid scanning.
//! Most badges stay neutral; semantic colors are reserved for states that truly
//! carry success, warning, destructive or informational meaning. The label text
//! always carries the state, so meaning never rests on color alone.
use super::prelude::*;

/// What a badge means.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BadgeVariant {
    /// Ordinary metadata: an inset fill, muted text.
    Neutral,
    /// Informational emphasis, the accent hue.
    Info,
    /// A confirmed or healthy state.
    Success,
    /// A caution that needs attention but is not an error.
    Warning,
    /// A destructive or failed state.
    Danger,
}

/// A badge builder; create it with [`badge`].
pub struct Badge {
    model: Signal<Model>,
    label: SharedString,
    variant: BadgeVariant,
}

/// A neutral badge with a static label.
pub fn badge(model: Signal<Model>, label: impl Into<SharedString>) -> Badge {
    Badge {
        model,
        label: label.into(),
        variant: BadgeVariant::Neutral,
    }
}

impl Badge {
    /// Its meaning (neutral, info, success, warning, danger).
    pub fn variant(mut self, variant: BadgeVariant) -> Self {
        self.variant = variant;
        self
    }
}

fn colors(model: Signal<Model>, v: BadgeVariant) -> (AdaptiveColor, AdaptiveColor) {
    let p = palette(model);
    match v {
        BadgeVariant::Neutral => (p.inset, p.muted),
        BadgeVariant::Info => (p.primary.opacity(super::tokens::tint::REST), p.primary),
        BadgeVariant::Success => (p.success.opacity(super::tokens::tint::REST), p.success),
        BadgeVariant::Warning => (p.warning.opacity(super::tokens::tint::REST), p.warning),
        BadgeVariant::Danger => (p.danger.opacity(super::tokens::tint::REST), p.danger),
    }
}

impl IntoElement for Badge {
    fn into_element(self) -> AnyElement {
        let model = self.model;
        let variant = self.variant;
        let (bg, fg) = colors(model, variant);
        view()
            .flex_shrink_0()
            .rounded(px(scale::radius::SMALL))
            .px_2()
            .py_0p5()
            .bg(bg)
            .text_color(fg)
            .child(
                text(self.label)
                    .text_size(px(scale::text::CAPTION))
                    .font_weight(FontWeight::SEMIBOLD),
            )
            .into_element()
    }
}
