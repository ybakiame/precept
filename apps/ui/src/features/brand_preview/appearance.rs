use super::chrome::compact;
use super::controls::{Ui, action, color, muted, set_mode};
use nsh_base::button;
use nsh_core::prelude::*;
use precept_ui_core::{
    appearance::{ColorMode, ThemeId},
    i18n::{Locale, Msg},
};

fn theme_card(ui: Signal<Ui>, theme: ThemeId) -> impl IntoElement {
    let s = ui.get();
    let p = theme.colors(
        match s.config.mode {
            ColorMode::Dark => true,
            ColorMode::Light => false,
            ColorMode::System => dark(),
        },
        false,
    );
    button()
        .w(px(176.0))
        .p_2()
        .rounded(px(8.0))
        .border_2()
        .accessibility_label(format!("{} {}", s.word(Msg::ThemeCollection), theme.name()))
        .selected(move || ui.get().config.theme == theme)
        .bg(move || color(ui.get().colors().panel))
        .text_color(move || color(ui.get().colors().text))
        .border_color(move || {
            color(if ui.get().config.theme == theme {
                ui.get().colors().accent
            } else {
                ui.get().colors().border
            })
        })
        .focus(|s| s.border_color(color(ui.get().colors().text)))
        .on_press(move || ui.update(|s| s.config.theme = theme))
        .child(
            view()
                .gap_1()
                .child(
                    view()
                        .p_2()
                        .gap_2()
                        .rounded(px(6.0))
                        .bg(color(p.background))
                        .child(text(s.word(Msg::Sample)).text_color(color(p.text)))
                        .child(
                            view().flex_row().gap_1().children(
                                [p.text, p.accent, p.success, p.warning, p.error, p.muted]
                                    .into_iter()
                                    .map(|c| {
                                        view().w(px(10.0)).h(px(10.0)).rounded(px(5.0)).bg(color(c))
                                    }),
                            ),
                        ),
                )
                .child(text(move || {
                    format!(
                        "{}{}",
                        if ui.get().config.theme == theme {
                            "> "
                        } else {
                            ""
                        },
                        theme.name()
                    )
                })),
        )
}
pub(super) fn appearance(ui: Signal<Ui>) -> impl IntoElement {
    let s = ui.get();
    view()
        .gap_3()
        .style_when(compact, |s| s.gap_2())
        .child(muted(ui, s.word(Msg::ThemeHelp)))
        .child(
            view().flex_row().flex_wrap().gap_2().children(
                ThemeId::ALL
                    .into_iter()
                    .map(move |theme| theme_card(ui, theme)),
            ),
        )
        .child(text(s.word(Msg::ColorMode)).font_weight(FontWeight::SEMIBOLD))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_1()
                .children(
                    [
                        (Msg::System, ColorMode::System),
                        (Msg::Light, ColorMode::Light),
                        (Msg::Dark, ColorMode::Dark),
                    ]
                    .into_iter()
                    .map(move |(key, mode)| {
                        action(
                            ui,
                            key,
                            move || ui.get().config.mode == mode,
                            move || set_mode(ui, mode),
                        )
                    }),
                )
                .child(action(
                    ui,
                    if s.config.contrast {
                        Msg::ContrastOn
                    } else {
                        Msg::ContrastOff
                    },
                    move || ui.get().config.contrast,
                    move || ui.update(|s| s.config.contrast = !s.config.contrast),
                )),
        )
        .child(muted(ui, s.word(Msg::SystemHelp)))
        .child(text(s.word(Msg::Language)).font_weight(FontWeight::SEMIBOLD))
        .child(
            button()
                .px_3()
                .py_1()
                .rounded(px(6.0))
                .border_2()
                .border_color(color(s.colors().border))
                .bg(color(s.colors().inset))
                .text_color(color(s.colors().text))
                .accessibility_label("English / 简体中文")
                .focus(|s| s.border_color(color(ui.get().colors().text)))
                .on_press(move || {
                    ui.update(|s| {
                        s.config.locale = if s.config.locale == Locale::En {
                            Locale::ZhCn
                        } else {
                            Locale::En
                        }
                    })
                })
                .child(text(if s.config.locale == Locale::En {
                    "English → 简体中文"
                } else {
                    "简体中文 → English"
                })),
        )
        .child(muted(ui, s.word(Msg::FontHelp)))
        .child(muted(ui, s.word(Msg::SessionOnly)))
        .child(muted(ui, s.word(Msg::PlainData)))
}
