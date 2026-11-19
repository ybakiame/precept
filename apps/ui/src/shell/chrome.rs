//! Custom native window chrome shared by the actual console.
use crate::*;
use acp_ui_core::i18n::Msg;
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
    destructive: bool,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    let markup = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path d='{path}' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'/></svg>"
    );
    button()
        .accessibility_label(move || Some(model.get().word(key).into()))
        .w(px(46.0))
        .h(px(ui::scale::layout::TITLE_BAR))
        .p_0()
        .flex_row()
        .items_center()
        .justify_center()
        .border_1()
        .bg(move || palette(model).background.opacity(0.0))
        .text_color(move || palette(model).muted)
        .border_color(move || palette(model).background.opacity(0.0))
        .transition_colors()
        .duration(ui::motion::ms(ui::motion::INSTANT))
        .hover(move |s| {
            if destructive {
                s.bg(palette(model).danger)
                    .text_color(palette(model).on_danger)
            } else {
                s.bg(palette(model).inset).text_color(palette(model).text)
            }
        })
        .focus(|s| s.border_color(palette(model).primary))
        .on_press(callback)
        .child(
            svg(nsh_core::element::ImageSrc::svg_markup(&markup))
                .w(px(14.0))
                .h(px(14.0)),
        )
}
pub(crate) fn titlebar(model: Signal<Model>) -> impl IntoElement {
    let minimize = current_window();
    let close = current_window();
    // The sidebar's color runs up to the window's top edge above the sidebar,
    // the page's above the page: no full-width strip in the system's colors.
    view()
        .flex_row()
        .h(px(ui::scale::layout::TITLE_BAR))
        .items_center()
        .bg(move || palette(model).background)
        .text_color(move || palette(model).text)
        .child(
            view()
                .w(px(ui::scale::layout::SIDEBAR))
                .h_full()
                .flex_shrink_0()
                .bg(move || palette(model).sidebar)
                .border_r_1()
                .border_color(move || palette(model).hairline)
                .window_drag(true)
                .style_when(ui::tokens::compact, |s| s.hidden()),
        )
        .child(
            view()
                .flex_1()
                .h_full()
                .px_3()
                .justify_center()
                .window_drag(true)
                .child(
                    text("Agent Control Plane")
                        .text_size(px(ui::scale::text::CAPTION))
                        .text_color(move || palette(model).muted)
                        .style_when(move || !ui::tokens::compact(), |s| s.hidden()),
                ),
        )
        .child(caption(
            model,
            Msg::Minimize,
            "M5 12H19",
            false,
            move || minimize.minimize(),
        ))
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
                false,
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
            true,
            move || close.close(),
        ))
}
pub(crate) fn grip(model: Signal<Model>) -> impl IntoElement {
    view().flex_row().justify_end().child(
        view()
            .w(px(18.0))
            .h(px(14.0))
            .window_resize(ResizeEdge::BottomRight)
            .accessible(true)
            .label(move || model.get().word(Msg::Resize))
            .child(
                text("◢")
                    .text_size(px(ui::scale::text::CAPTION))
                    .text_color(move || palette(model).muted),
            ),
    )
}
