//! Session-only appearance and explicit operator-owned connection editor.
use super::*;
use acp_ui_spike_model::{
    appearance::{ColorMode, ThemeId},
    connection::{Connection, Target},
    i18n::{Locale, Msg},
};
use components::word_action;
use nsh_core::element::input;
use tokens::palette;
pub(super) fn apply(model: Signal<Model>) {
    let mut apply = false;
    model.update(|m| apply = m.apply_connection());
    if apply {
        load(model);
    }
}
fn field(
    model: Signal<Model>,
    key: Msg,
    get: fn(&Connection) -> &String,
    set: fn(&mut Connection, String),
    wsl: bool,
) -> impl IntoElement {
    let reference = element_ref();
    effect(move || {
        let m = model.get();
        let visible = m.connection_editor
            && if wsl {
                m.connection_draft.target == Target::Wsl
            } else {
                m.connection_draft.target == Target::Local
            };
        if !visible {
            reference.blur();
        }
    });
    view()
        .gap_1()
        .style_when(
            move || {
                if wsl {
                    model.get().connection_draft.target != Target::Wsl
                } else {
                    model.get().connection_draft.target != Target::Local
                }
            },
            |s| s.hidden(),
        )
        .child(text(move || model.get().word(key)).text_size(px(12.0)))
        .child(
            input()
                .element_ref(&reference)
                .label(move || model.get().word(key))
                .h(px(32.0))
                .border_2()
                .border_color(move || palette(model).border)
                .bg(move || palette(model).panel)
                .text_color(move || palette(model).text)
                .focus(|s| s.border_color(palette(model).primary))
                .value(move || get(&model.get().connection_draft).clone())
                .on_submit(move |_: &str| apply(model))
                .on_change(move |v: &str| {
                    model.update(|m| {
                        set(&mut m.connection_draft, v.chars().take(1024).collect());
                        m.connection_invalid = false;
                    })
                }),
        )
}
pub(super) fn connection(model: Signal<Model>) -> impl IntoElement {
    view()
        .gap_2()
        .p_3()
        .rounded(px(8.0))
        .border_1()
        .border_color(move || palette(model).border)
        .bg(move || palette(model).panel)
        .child(text(move || model.get().word(Msg::Connection)).font_weight(FontWeight::SEMIBOLD))
        .child(text(move || model.get().word(Msg::DraftHelp)).text_size(px(12.0)))
        .child(
            view().flex_row().flex_wrap().gap_1().children(
                [
                    (Target::Synthetic, Msg::SyntheticTarget),
                    (Target::Local, Msg::LocalTarget),
                    (Target::Wsl, Msg::WslTarget),
                ]
                .into_iter()
                .map(move |(target, key)| {
                    word_action(
                        model,
                        key,
                        || true,
                        move || {
                            model.update(|m| {
                                m.connection_draft.target = target;
                                m.connection_invalid = false;
                            })
                        },
                    )
                }),
            ),
        )
        .child(text(move || {
            let m = model.get();
            let key = match m.connection_draft.target {
                Target::Synthetic => Msg::SyntheticTarget,
                Target::Local => Msg::LocalTarget,
                Target::Wsl => Msg::WslTarget,
            };
            format!("{}: {}", m.word(Msg::DraftTarget), m.word(key))
        }))
        .child(field(
            model,
            Msg::Runtime,
            |c| &c.runtime,
            |c, v| c.runtime = v,
            false,
        ))
        .child(field(
            model,
            Msg::Endpoint,
            |c| &c.endpoint,
            |c, v| c.endpoint = v,
            false,
        ))
        .child(field(
            model,
            Msg::Distribution,
            |c| &c.distribution,
            |c, v| c.distribution = v,
            true,
        ))
        .child(field(
            model,
            Msg::Owner,
            |c| &c.user,
            |c, v| c.user = v,
            true,
        ))
        .child(field(
            model,
            Msg::Helper,
            |c| &c.helper,
            |c, v| c.helper = v,
            true,
        ))
        .child(field(
            model,
            Msg::WslRuntime,
            |c| &c.wsl_runtime,
            |c, v| c.wsl_runtime = v,
            true,
        ))
        .child(field(
            model,
            Msg::WslEndpoint,
            |c| &c.wsl_endpoint,
            |c, v| c.wsl_endpoint = v,
            true,
        ))
        .child(
            text(move || model.get().word(Msg::WslWindowsOnly))
                .text_size(px(12.0))
                .style_when(
                    move || {
                        cfg!(target_os = "windows")
                            || model.get().connection_draft.target != Target::Wsl
                    },
                    |s| s.hidden(),
                ),
        )
        .child(
            text(move || {
                if model.get().connection_invalid {
                    model.get().word(Msg::ConnectionInvalid)
                } else {
                    ""
                }
            })
            .text_color(move || palette(model).error),
        )
        .child(word_action(
            model,
            Msg::ApplyConnection,
            move || {
                cfg!(target_os = "windows") || model.get().connection_draft.target != Target::Wsl
            },
            move || apply(model),
        ))
        .child(text(move || model.get().word(Msg::ConnectionHelp)).text_size(px(12.0)))
}
pub(super) fn appearance(model: Signal<Model>) -> impl IntoElement {
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
