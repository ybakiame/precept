//! The app's choice card: one option of a small visible set shown by its own
//! preview (a theme). Built on the semantic `nsh/base` button (role, keyboard,
//! selected state), with the preview above the option's name and a check that
//! grows in on the chosen card. Selection is a `primary` edge and the check,
//! never color alone.
use super::prelude::*;
use std::rc::Rc;

/// A card's width: four fit a form column's surface in one row.
const CARD: f32 = 136.0;

/// A card that chooses `label`; `preview` is drawn as given (data colors).
pub fn choice_card(
    model: Signal<Model>,
    label: impl Into<SharedString>,
    preview: impl IntoElement,
    selected: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    let label: SharedString = label.into();
    let selected: Rc<dyn Fn() -> bool> = Rc::new(selected);
    let (edge, a11y, check) = (selected.clone(), selected.clone(), selected.clone());
    let spoken = label.clone();
    let checked = computed(move || check());
    let card = base_button()
        .accessibility_label(move || Some(spoken.clone()))
        .selected(move || a11y())
        .on_press(on_press)
        .w(px(CARD))
        .p_1p5()
        .gap_1p5()
        .rounded(px(scale::radius::CONTAINER))
        .border_1()
        .bg(move || palette(model).panel)
        .border_color(move || {
            let p = palette(model);
            if edge() { p.primary } else { p.hairline }
        })
        .hover(move |s| s.bg(mix(palette(model).panel, palette(model).text, 0.04)))
        .active(move |s| s.scale(motion::press()))
        .focus(move |s| {
            s.ring(px(2.0))
                .ring_color(palette(model).primary.opacity(0.55))
        });
    let card = motion::feedback(card);
    card.child(
        view()
            .w_full()
            .rounded(px(scale::radius::CONTROL))
            .overflow_hidden()
            .child(preview),
    )
    .child(
        view()
            .flex_row()
            .items_center()
            .gap_1()
            .px_1()
            .child(
                text(label)
                    .flex_1()
                    .min_w_0()
                    .text_size(px(scale::text::CAPTION))
                    .text_color(move || palette(model).text),
            )
            .child(dynamic(move || {
                if !checked.get() {
                    return view().into_element();
                }
                if terminal() {
                    return text("✓").into_element();
                }
                let pop = motion::pop();
                view()
                    .text_color(move || palette(model).primary)
                    .bind_motion(&pop)
                    .child(icon::glyph(super::icons::CHECK, scale::icon::SM))
                    .into_element()
            })),
    )
}
