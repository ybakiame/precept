//! The app's `Empty`: an empty state that explains what is missing and the next
//! action, rather than leaving a blank region. It offers an optional glyph, a
//! description and an action slot (a `ui` button), centered with breathing room.
use super::prelude::*;

/// An empty-state builder; create it with [`empty`].
pub struct Empty {
    model: Signal<Model>,
    title: SharedString,
    description: Option<SharedString>,
    icon: Option<&'static str>,
    action: Option<AnyElement>,
}

/// An empty state with a title.
pub fn empty(model: Signal<Model>, title: impl Into<SharedString>) -> Empty {
    Empty {
        model,
        title: title.into(),
        description: None,
        icon: None,
        action: None,
    }
}

impl Empty {
    /// A sentence saying what is empty and how to recover.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
    /// A leading outline glyph from the app's icon family.
    pub fn icon(mut self, path: &'static str) -> Self {
        self.icon = Some(path);
        self
    }
    /// The next action, a `ui` control (never a hand-built pressable view).
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_element());
        self
    }
}

impl IntoElement for Empty {
    fn into_element(self) -> AnyElement {
        let model = self.model;
        let mut body = view()
            .flex_1()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .text_center()
            .gap_2()
            .p_8();
        if let Some(path) = self.icon {
            // The icon in a tinted disc: the empty state's one focal shape.
            body = body.child(
                view()
                    .size(px(scale::disc::LG))
                    .mb_2()
                    .items_center()
                    .justify_center()
                    .rounded(px(scale::radius::PILL))
                    .bg(move || palette(model).primary.opacity(super::tokens::tint::REST))
                    .text_color(move || palette(model).primary)
                    .child(icon::glyph(path, scale::icon::XL)),
            );
        }
        body = body.child(
            text(self.title)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(scale::text::SUBTITLE))
                .text_color(move || palette(model).text),
        );
        if let Some(description) = self.description {
            body = body.child(
                text(description)
                    .text_size(px(scale::text::BODY))
                    .text_color(move || palette(model).muted),
            );
        }
        if let Some(action) = self.action {
            body = body.child(view().mt_2().child(action));
        }
        body.into_element()
    }
}
