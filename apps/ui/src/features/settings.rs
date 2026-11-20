//! Settings: appearance and language (this session), and the keyboard reference.
//! Composed only from the `ui` layer; theme swatches show each theme's own
//! colors as data.
use crate::*;
use precept_ui_core::{
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

/// One theme: a card previewing the theme's own floor, text, accent and
/// status colors (data, in the current light or dark mode), and its name.
fn theme_choice(model: Signal<Model>, theme: ThemeId) -> impl IntoElement {
    let preview = dynamic(move || {
        let m = model.get();
        let dark = match m.color_mode {
            ColorMode::System => dark(),
            ColorMode::Dark => true,
            ColorMode::Light => false,
        };
        let p = theme.colors(dark, false);
        let c = |v: u32| AdaptiveColor::rgb(v, v);
        let dot = move |v: u32| {
            view()
                .size(px(scale::disc::DOT))
                .rounded(px(scale::radius::PILL))
                .bg(c(v))
        };
        view()
            .w_full()
            .h(px(64.0))
            .p_2()
            .gap_2()
            .justify_between()
            .bg(c(p.background))
            .child(
                text("Aa")
                    .font_family("monospace")
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(scale::text::BODY))
                    .text_color(c(p.text)),
            )
            .child(
                view()
                    .flex_row()
                    .gap_1p5()
                    .child(dot(p.accent))
                    .child(dot(p.success))
                    .child(dot(p.warning))
                    .child(dot(p.error))
                    .child(dot(p.muted)),
            )
            .into_element()
    });
    ui::choice_card(
        model,
        theme.name(),
        preview,
        move || model.get().theme == theme,
        move || model.update(|m| m.theme = theme),
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
                    view().flex_row().flex_wrap().gap_2().children(
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
                .child(note(model, Msg::KeysSettingsHelp))
                .child(
                    view().items_start().child(
                        button(model, "")
                            .label_fn(word(model, Msg::KeysOpenPanel))
                            .icon(ui::icons::KEYBOARD)
                            .on_press(move || crate::shortcuts::open(model)),
                    ),
                ),
        )
}
