//! An info button that opens a short explanation in a popover anchored to it,
//! so long help does not crowd the page. The explanation is supplementary: a
//! page stays usable without opening it.
use super::prelude::*;
use nsh_base::popover;

/// The shared tooltip surface, shown by its trigger on hover or keyboard focus.
pub fn tooltip(model: Signal<Model>, content: impl IntoElement) -> nsh_base::TooltipElement {
    nsh_base::tooltip()
        .placement(Placement::Top)
        .offset(px(6.0))
        .margin(px(8.0))
        .max_w(px(scale::layout::TOAST))
        .p_3()
        .rounded(px(scale::radius::CONTAINER))
        .border_1()
        .border_color(move || palette(model).hairline)
        .bg(move || palette(model).raised)
        .shadow_lg()
        .transition()
        .duration(motion::ms(motion::INSTANT))
        .starting_style(|s| s.opacity(0.0))
        .ending_style(|s| s.opacity(0.0))
        .child(content)
}

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
                .transition_colors()
                .duration(motion::ms(motion::INSTANT))
                .hover(move |s| s.bg(palette(model).inset).text_color(palette(model).text))
                .focus_visible(move |s| s.border_1().border_color(palette(model).primary))
                .child(icon::glyph(INFO, scale::icon::MD)),
        )
        .child(
            view()
                .max_w(px(320.0))
                .p_3()
                .gap_1()
                .rounded(px(scale::radius::CONTAINER))
                .border_1()
                .border_color(move || palette(model).hairline)
                .bg(move || palette(model).raised)
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
