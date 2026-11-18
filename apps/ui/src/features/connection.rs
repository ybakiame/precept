//! The explicit operator-owned connection editor.
use crate::*;
use acp_operator_core::{
    connection::{Connection, Target},
    i18n::Msg,
};
use nsh_core::element::input;
use shell::word_action;
use tokens::palette;
pub(crate) fn apply(model: Signal<Model>) {
    let mut apply = false;
    model.update(|m| apply = m.apply_connection());
    if apply {
        // Leaving WSL closes its bridge (never the WSL daemon); off the UI thread.
        #[cfg(feature = "ipc")]
        if model.get().connection.target != Target::Wsl {
            spawn(async {
                let _ = blocking(acp_operator_client::bridge::close_active).await;
            });
        }
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
pub(crate) fn connection(model: Signal<Model>) -> impl IntoElement {
    view()
        .gap_2()
        .p_3()
        .rounded(px(8.0))
        .border_1()
        .border_color(move || palette(model).border)
        .bg(move || palette(model).panel)
        .child(text(move || model.get().word(Msg::Connection)).font_weight(FontWeight::SEMIBOLD))
        .child(text(move || model.get().word(Msg::DraftHelp)).text_size(px(12.0)))
        .child(word_action(
            model,
            Msg::ProfileOperator,
            || true,
            move || review::open(model),
        ))
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
