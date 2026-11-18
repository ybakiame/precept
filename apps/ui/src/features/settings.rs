//! Settings: appearance and language (this session), and the keyboard reference.
//! Composed only from the `ui` layer; theme swatches show each theme's own
//! colors as data.
use crate::*;
use acp_operator_core::{
    appearance::{ColorMode, ThemeId},
    i18n::{Locale, Msg},
};
use tokens::palette;
use ui::{ButtonSize, button, field, scale, surface, tabs};

fn note(model: Signal<Model>, key: Msg) -> impl IntoElement {
    text(move || model.get().word(key))
        .text_size(px(scale::text::CAPTION))
        .text_color(move || palette(model).muted)
}
fn word(model: Signal<Model>, key: Msg) -> impl Fn() -> SharedString + 'static {
    move || model.get().word(key).into()
}
fn set_mode(model: Signal<Model>, mode: ColorMode) {
    model.update(|m| {
        m.color_mode = mode;
        m.dark = mode != ColorMode::Light;
    });
    set_color_scheme(match mode {
        ColorMode::System => Appearance::System,
        ColorMode::Light => Appearance::Light,
        ColorMode::Dark => Appearance::Dark,
    });
}

/// One theme: a swatch of its own background and text, and its choice.
fn theme_choice(model: Signal<Model>, theme: ThemeId) -> impl IntoElement {
    let p = theme.colors(false, false);
    view()
        .w(px(scale::layout::POPUP))
        .gap_2()
        .child(
            view()
                .p_3()
                .rounded(px(scale::radius::CONTROL))
                .border_1()
                .border_color(move || palette(model).border)
                .bg(AdaptiveColor::rgb(p.background, p.background))
                .child(text("Aa  Context / 上下文").text_color(AdaptiveColor::rgb(p.text, p.text))),
        )
        .child(
            button(model, theme.name())
                .full_width()
                .selected(move || model.get().theme == theme)
                .on_press(move || model.update(|m| m.theme = theme)),
        )
}

pub(crate) fn appearance(model: Signal<Model>) -> impl IntoElement {
    let modes = tabs(model)
        .add_fn(
            word(model, Msg::System),
            move || model.get().color_mode == ColorMode::System,
            move || set_mode(model, ColorMode::System),
        )
        .add_fn(
            word(model, Msg::Light),
            move || model.get().color_mode == ColorMode::Light,
            move || set_mode(model, ColorMode::Light),
        )
        .add_fn(
            word(model, Msg::Dark),
            move || model.get().color_mode == ColorMode::Dark,
            move || set_mode(model, ColorMode::Dark),
        );
    // Each language names itself, as language pickers do.
    let languages = tabs(model)
        .add(
            "English",
            move || model.get().locale == Locale::En,
            move || model.update(|m| m.locale = Locale::En),
        )
        .add(
            "简体中文",
            move || model.get().locale == Locale::ZhCn,
            move || model.update(|m| m.locale = Locale::ZhCn),
        );
    view()
        .gap_6()
        .max_w(px(scale::layout::FORM))
        .child(
            surface(model)
                .title_fn(word(model, Msg::ThemeCollection))
                .child(note(model, Msg::ThemeHelp))
                .child(
                    view().flex_row().flex_wrap().gap_3().children(
                        ThemeId::ALL
                            .into_iter()
                            .map(move |t| theme_choice(model, t)),
                    ),
                ),
        )
        .child(
            view()
                .gap_4()
                .child(
                    field(model, word(model, Msg::ColorMode), modes)
                        .help(word(model, Msg::SystemHelp)),
                )
                .child(field(
                    model,
                    word(model, Msg::HighContrast),
                    view().items_start().child(
                        button(model, "")
                            .label_fn(move || {
                                let m = model.get();
                                m.word(if m.high_contrast {
                                    Msg::ContrastOn
                                } else {
                                    Msg::ContrastOff
                                })
                                .into()
                            })
                            .size(ButtonSize::Small)
                            .selected(move || model.get().high_contrast)
                            .on_press(move || toggle_contrast(model)),
                    ),
                ))
                .child(
                    field(model, word(model, Msg::Language), languages)
                        .help(word(model, Msg::PlainData)),
                )
                .child(note(model, Msg::SessionOnly))
                .child(note(model, Msg::FontHelp)),
        )
        .child(
            surface(model)
                .title_fn(word(model, Msg::Keyboard))
                .child(note(model, Msg::ConsoleHelp))
                .child(note(model, Msg::OperatorKeys)),
        )
}
