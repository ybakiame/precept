//! Session-only appearance settings.
use crate::*;
use acp_operator_core::{
    appearance::{ColorMode, ThemeId},
    i18n::{Locale, Msg},
};
use shell::word_action;
use tokens::palette;
pub(crate) fn appearance(model: Signal<Model>) -> impl IntoElement {
    view()
        .gap_2()
        .p_3()
        .rounded(px(8.0))
        .border_1()
        .border_color(move || palette(model).border)
        .bg(move || palette(model).panel)
        .child(
            text(move || model.get().word(Msg::ThemeCollection)).font_weight(FontWeight::SEMIBOLD),
        )
        .child(text(move || model.get().word(Msg::ThemeHelp)).text_size(px(12.0)))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .children(ThemeId::ALL.into_iter().map(move |theme| {
                    let p = theme.colors(false, false);
                    view()
                        .w(px(160.0))
                        .p_2()
                        .gap_1()
                        .border_2()
                        .rounded(px(8.0))
                        .border_color(move || {
                            if model.get().theme == theme {
                                palette(model).primary
                            } else {
                                palette(model).border
                            }
                        })
                        .child(
                            view()
                                .p_2()
                                .bg(AdaptiveColor::rgb(p.background, p.background))
                                .child(
                                    text("Aa  Context / 上下文")
                                        .text_color(AdaptiveColor::rgb(p.text, p.text)),
                                ),
                        )
                        .child(
                            button()
                                .accessibility_label(theme.name())
                                .px_2()
                                .py_1()
                                .rounded(px(6.0))
                                .border_2()
                                .border_color(move || palette(model).border)
                                .bg(move || palette(model).inset)
                                .text_color(move || palette(model).text)
                                .selected(move || model.get().theme == theme)
                                .focus(|s| s.border_color(palette(model).text))
                                .on_press(move || model.update(|m| m.theme = theme))
                                .child(text(move || {
                                    format!(
                                        "{}{}",
                                        if model.get().theme == theme { "> " } else { "" },
                                        theme.name()
                                    )
                                })),
                        )
                })),
        )
        .child(text(move || model.get().word(Msg::ColorMode)).font_weight(FontWeight::SEMIBOLD))
        .child(
            view().flex_row().flex_wrap().gap_1().children(
                [
                    (ColorMode::System, Msg::System),
                    (ColorMode::Light, Msg::Light),
                    (ColorMode::Dark, Msg::Dark),
                ]
                .into_iter()
                .map(move |(mode, key)| {
                    word_action(
                        model,
                        key,
                        || true,
                        move || {
                            model.update(|m| {
                                m.color_mode = mode;
                                m.dark = mode != ColorMode::Light;
                            });
                            set_color_scheme(match mode {
                                ColorMode::System => Appearance::System,
                                ColorMode::Light => Appearance::Light,
                                ColorMode::Dark => Appearance::Dark,
                            });
                        },
                    )
                }),
            ),
        )
        .child(text(move || {
            let m = model.get();
            format!(
                "> {} · {}",
                m.word(match m.color_mode {
                    ColorMode::System => Msg::System,
                    ColorMode::Light => Msg::Light,
                    ColorMode::Dark => Msg::Dark,
                }),
                m.word(if m.high_contrast {
                    Msg::ContrastOn
                } else {
                    Msg::ContrastOff
                })
            )
        }))
        .child(word_action(
            model,
            Msg::HighContrast,
            || true,
            move || toggle_contrast(model),
        ))
        .child(word_action(
            model,
            Msg::Language,
            || true,
            move || {
                model.update(|m| {
                    m.locale = if m.locale == Locale::En {
                        Locale::ZhCn
                    } else {
                        Locale::En
                    }
                })
            },
        ))
        .child(text(move || model.get().word(Msg::SystemHelp)).text_size(px(12.0)))
        .child(text(move || model.get().word(Msg::SessionOnly)).text_size(px(12.0)))
        .child(text(move || model.get().word(Msg::FontHelp)).text_size(px(12.0)))
        .child(text(move || model.get().word(Msg::PlainData)).text_size(px(12.0)))
}
