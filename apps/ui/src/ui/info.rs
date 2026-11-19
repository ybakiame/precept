//! An info button that opens a short explanation in a popover anchored to it,
//! so long help does not crowd the page. The explanation is supplementary: a
//! page stays usable without opening it. In a terminal the button is a "(?)"
//! label; the popover is the same.
use super::prelude::*;
use nsh_base::popover;

const INFO: &str = "M12 21A9 9 0 1 0 12 3A9 9 0 1 0 12 21M12 11V16M12 8V8.01";

/// An info button named `label` (what it explains) showing `body`.
pub fn info(
    model: Signal<Model>,
    label: impl Fn() -> SharedString + 'static,
    body: impl Fn() -> SharedString + 'static,
) -> impl IntoElement {
    let label = std::rc::Rc::new(label);
    let spoken = label.clone();
    popover()
        .offset(px(6.0))
        .margin(px(8.0))
        .trigger(
            view()
                .accessible(true)
                .role(AccessibilityRole::Button)
                .label(move || spoken())
                .focusable(true)
                .size(px(scale::control::SM))
                .items_center()
                .justify_center()
                .rounded(px(scale::radius::CONTROL))
                .text_color(move || palette(model).muted)
                .hover(move |s| s.bg(palette(model).inset))
                .focus_visible(move |s| s.border_1().border_color(palette(model).primary))
                .child(icon::glyph(INFO, scale::icon::MD))
                .child(
                    text("(?)")
                        .style_when(move || !terminal(), |s| s.hidden())
                        .text_size(px(scale::text::CAPTION)),
                ),
        )
        .child(
            view()
                .max_w(px(320.0))
                .p_3()
                .gap_1()
                .rounded(px(scale::radius::CONTAINER))
                .border_1()
                .border_color(move || palette(model).border)
                .bg(move || palette(model).panel)
                .shadow_lg()
                .child(
                    text(move || label())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(scale::text::BODY)),
                )
                .child(
                    text(body)
                        .text_size(px(scale::text::BODY))
                        .text_color(move || palette(model).muted),
                ),
        )
}
