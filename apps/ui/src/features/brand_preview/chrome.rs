use super::controls::{Ui, color};
use nsh_base::button;
use nsh_core::{desktop::WindowMode, element::desktop::current_window, prelude::*};
use precept_ui_core::i18n::Msg;

pub(super) fn compact() -> bool {
    crate::ui::tokens::compact()
}
pub(super) fn stack_cards() -> bool {
    window().width.0 < 580.0
}

// Paths are centered in a 24×24 viewBox. No font glyph metrics or baseline offset.
fn caption(
    ui: Signal<Ui>,
    key: Msg,
    path: &'static str,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    let markup = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path d='{path}' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'/></svg>"
    );
    button()
        .accessibility_label(ui.get().word(key))
        .w(px(40.0))
        .h(px(36.0))
        .p_0()
        .flex_row()
        .items_center()
        .justify_center()
        .border_2()
        .bg(move || color(ui.get().colors().panel))
        .text_color(move || color(ui.get().colors().text))
        .border_color(move || color(ui.get().colors().panel))
        .hover(|s| s.bg(color(ui.get().colors().inset)))
        .focus(|s| s.border_color(color(ui.get().colors().accent)))
        .on_press(callback)
        .child(
            svg(nsh_core::element::ImageSrc::svg_markup(&markup))
                .w(px(14.0))
                .h(px(14.0)),
        )
}
pub(super) fn titlebar(ui: Signal<Ui>) -> impl IntoElement {
    let minimize = current_window();
    let maximize = current_window();
    let close = current_window();
    let maximized = maximize
        .state()
        .is_some_and(|s| s.mode == WindowMode::Maximized);
    view()
        .flex_row()
        .h(px(36.0))
        .items_center()
        .bg(move || color(ui.get().colors().panel))
        .text_color(move || color(ui.get().colors().text))
        .child(
            view()
                .flex_1()
                .h_full()
                .px_3()
                .justify_center()
                .window_drag(true)
                .child(
                    text(format!("Precept / {}", ui.get().word(Msg::Preview))).text_size(px(12.0)),
                ),
        )
        .child(caption(ui, Msg::Minimize, "M5 12H19", move || {
            minimize.minimize()
        }))
        .child(caption(
            ui,
            if maximized {
                Msg::Restore
            } else {
                Msg::Maximize
            },
            if maximized {
                "M8 5H19V16M5 8H16V19H5Z"
            } else {
                "M5 5H19V19H5Z"
            },
            move || {
                maximize.set_mode(
                    if maximize
                        .state()
                        .is_some_and(|s| s.mode == WindowMode::Maximized)
                    {
                        WindowMode::Normal
                    } else {
                        WindowMode::Maximized
                    },
                );
            },
        ))
        .child(caption(ui, Msg::Close, "M6 6L18 18M18 6L6 18", move || {
            close.close()
        }))
}
