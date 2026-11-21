//! Checkbox: a small box that picks one item of a list for a shared action
//! (a batch decision). The box fills with the accent and shows a check when
//! picked; its label is what screen readers say.
use super::prelude::*;

const BOX: f32 = 16.0;
const CHECK: &str = "M5 12L10 17L19 7";

/// A checkbox named `label` (spoken), checked while `checked`.
pub fn checkbox(
    model: Signal<Model>,
    label: impl Fn() -> SharedString + 'static,
    checked: impl Fn() -> bool + 'static,
    disabled: impl Fn() -> bool + 'static,
    on_change: impl Fn(bool) + 'static,
) -> impl IntoElement {
    let lit = computed(checked);
    let base = nsh_base::checkbox()
        .checked(move || lit.get())
        .disabled(disabled)
        .accessibility_label(move || Some(label()))
        .on_change(move |next: &bool| on_change(*next))
        .p_1()
        .rounded(px(scale::radius::SMALL))
        .focus(move |s| {
            s.ring(px(2.0))
                .ring_color(palette(model).primary.opacity(0.55))
        })
        .disabled_style(|s| s.opacity(0.45))
        .indicator_style(move |s| {
            let p = palette(model);
            s.size(px(BOX))
                .rounded(px(scale::radius::SMALL))
                .border_1()
                .border_color(p.border)
                .bg(p.panel)
                .items_center()
                .justify_center()
                .transition_colors()
                .duration(motion::ms(motion::FAST)())
                .checked(|s| s.border_color(p.primary).bg(p.primary))
        })
        .indicator(
            view()
                .text_color(move || palette(model).on_primary)
                .opacity(move || if lit.get() { 1.0_f32 } else { 0.0_f32 })
                .transition()
                .duration(motion::ms(motion::FAST))
                .child(icon::glyph(CHECK, 12.0)),
        );
    motion::feedback(base)
}
