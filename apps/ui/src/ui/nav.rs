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
