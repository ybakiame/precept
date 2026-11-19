//! The app's `Surface` and `card`: a panel with a hairline boundary and an
//! intentional inset, the container that holds controls and content. Cards do
//! not lift on hover; a region needs only a background, a boundary and spacing.
//! Line content is sanitized for display, never rendered raw.
use super::prelude::*;
use acp_ui_core::presentation::sanitize;

/// A panel builder; create it with [`surface`], give it a title and children.
pub struct Surface {
    model: Signal<Model>,
    title: Option<std::rc::Rc<dyn Fn() -> SharedString>>,
    children: Vec<AnyElement>,
}

/// An empty panel.
pub fn surface(model: Signal<Model>) -> Surface {
    Surface {
        model,
        title: None,
        children: Vec::new(),
    }
}

impl Surface {
    /// A section title identifying the region.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        let title = title.into();
        self.title = Some(std::rc::Rc::new(move || title.clone()));
        self
    }
    /// A title that stays current (a localized word).
    pub fn title_fn(mut self, title: impl Fn() -> SharedString + 'static) -> Self {
        self.title = Some(std::rc::Rc::new(title));
        self
    }
    /// Add a child (a `ui` control, text, or a layout primitive).
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_element());
        self
    }
}

impl IntoElement for Surface {
    fn into_element(self) -> AnyElement {
        let model = self.model;
        let mut body = view()
            .flex_shrink_0()
            .p_3()
            .gap_2()
            .rounded(px(scale::radius::CONTAINER))
            .border_1()
            .border_color(move || palette(model).border)
            .bg(move || palette(model).panel)
            .text_color(move || palette(model).text)
            .text_size(px(scale::text::BODY));
        if let Some(title) = self.title {
            body = body.child(
                text(move || title())
                    .role(AccessibilityRole::Header)
                    .font_weight(FontWeight::SEMIBOLD),
            );
        }
        body.children(self.children).into_element()
    }
}

/// A titled card of sanitized text lines: the app's read-only content panel.
pub fn card(
    model: Signal<Model>,
    title: impl Into<SharedString>,
    lines: Vec<String>,
) -> impl IntoElement {
    let mut panel = surface(model).title(title);
    for line in lines {
        panel = panel.child(text(sanitize(&line, 512)));
    }
    panel
}
