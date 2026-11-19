//! The app's `NavItem`: one sidebar destination. A ghost button that fills
//! the sidebar's width, with its icon in a fixed lane and one persistent
//! selected fill (never a leading bar). Keyboard, focus ring and press
//! feedback are the button's.
use super::prelude::*;
use super::{ButtonVariant, button};

/// A destination: its icon path, a label that stays current, whether it is the
/// current one and what choosing it does.
pub fn nav_item(
    model: Signal<Model>,
    icon_path: &'static str,
    label: impl Fn() -> SharedString + 'static,
    selected: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    button(model, "")
        .label_fn(label)
        .variant(ButtonVariant::Ghost)
        .icon(icon_path)
        .full_width()
        .selected(selected)
        .on_press(on_press)
}

/// A numbered set-up step: the visible label is the step's name, with a
/// success check in the trailing lane once it is done (`✓` in a terminal,
/// where icons are hidden); `spoken` stays one stable name ("Step 2: Profile").
/// The check grows in when the step completes.
pub fn step_item(
    model: Signal<Model>,
    icon_path: &'static str,
    label: impl Fn() -> SharedString + 'static,
    spoken: impl Fn() -> SharedString + 'static,
    selected: impl Fn() -> bool + 'static,
    done: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    let done = computed(done);
    let check = dynamic(move || {
        if !done.get() {
            return view().into_element();
        }
        if terminal() {
            return text("✓")
                .text_color(move || palette(model).success)
                .into_element();
        }
        let pop = motion::pop();
        view()
            .text_color(move || palette(model).success)
            .bind_motion(&pop)
            .child(icon::glyph(super::icons::CHECK, scale::icon::SM))
            .into_element()
    });
    button(model, "")
        .label_fn(label)
        .spoken(spoken)
        .variant(ButtonVariant::Ghost)
        .icon(icon_path)
        .full_width()
        .selected(selected)
        .trailing(check)
        .on_press(on_press)
}
