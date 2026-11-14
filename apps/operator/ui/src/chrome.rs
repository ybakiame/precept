//! Custom native window chrome shared by the actual console.
use super::*;
use acp_operator_model::i18n::Msg;
use nsh_core::{
    desktop::{ResizeEdge, WindowMode},
    element::desktop::current_window,
};
use tokens::palette;
// Paths are centered in a 24×24 viewBox. No font glyph metrics or baseline offset.
fn caption(
    model: Signal<Model>,
    key: Msg,
    path: &'static str,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    let markup = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path d='{path}' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'/></svg>"
    );
    button()
        .accessibility_label(move || Some(model.get().word(key).into()))
        .w(px(40.0))
        .h(px(36.0))
        .p_0()
        .flex_row()
        .items_center()
        .justify_center()
        .border_2()
        .bg(move || palette(model).panel)
        .text_color(move || palette(model).text)
        .border_color(move || palette(model).panel)
        .hover(|s| s.bg(palette(model).inset))
        .focus(|s| s.border_color(palette(model).primary))
        .on_press(callback)
        .child(
            svg(nsh_core::element::ImageSrc::svg_markup(&markup))
                .w(px(14.0))
                .h(px(14.0)),
        )
}
pub(super) fn titlebar(model: Signal<Model>) -> impl IntoElement {
    let minimize = current_window();
    let close = current_window();
    view()
        .flex_row()
        .h(px(36.0))
        .items_center()
        .bg(move || palette(model).panel)
        .text_color(move || palette(model).text)
        .child(
            view()
                .flex_1()
                .h_full()
                .px_3()
                .justify_center()
                .window_drag(true)
                .child(text("Agent Control Plane").text_size(px(12.0))),
        )
        .child(caption(model, Msg::Minimize, "M5 12H19", move || {
            minimize.minimize()
        }))
        .child(dynamic(move || {
            let maximize = current_window();
            let maximized = maximize
                .state()
                .is_some_and(|s| s.mode == WindowMode::Maximized);
            caption(
                model,
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
            )
        }))
        .child(caption(
            model,
            Msg::Close,
            "M6 6L18 18M18 6L6 18",
            move || close.close(),
        ))
}
pub(super) fn grip(model: Signal<Model>) -> impl IntoElement {
    view().h(px(14.0)).flex_row().justify_end().child(
        view()
            .w(px(18.0))
            .h(px(14.0))
            .window_resize(ResizeEdge::BottomRight)
            .accessible(true)
            .label(move || model.get().word(Msg::Resize))
            .child(
                text("◢")
                    .text_size(px(12.0))
                    .text_color(move || palette(model).muted),
            ),
    )
}
