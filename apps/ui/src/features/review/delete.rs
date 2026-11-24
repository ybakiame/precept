use super::actions::{commit_delete, delete_armed, disarm, retype_mismatch};
use super::formatting::scope_label;
use super::*;

pub(super) fn delete_dialog(
    model: Signal<Model>,
    delete_open: Signal<bool>,
    retype_ref: ElementRef,
) -> impl IntoElement {
    ui::alert_dialog(model, "")
        .icon(icons::TRASH, ui::DialogTone::Danger)
        .title_fn(move || {
            let m = model.get();
            match m.operator.delete_arm.as_ref().zip(m.operator.record()) {
                Some((a, r)) => tr(m.locale, Msg::ReviewDeleteTitle)
                    .replace("{scope}", &scope_label(m.locale, &r.scope))
                    .replace("{id}", &a.id.to_string())
                    .into(),
                None => String::new().into(),
            }
        })
        .description_fn(move || model.get().word(Msg::ReviewDeleteBody).into())
        .cancel_fn(move || model.get().word(Msg::Cancel).into())
        .confirm_fn(move || model.get().word(Msg::ReviewDeleteConfirm).into())
        .confirm_disabled(move || !delete_armed(model))
        .open(move || {
            delete_open.get() && model.with(|m| m.operator.open && m.operator.delete_arm.is_some())
        })
        .initial_focus(&retype_ref)
        .on_cancel(move || disarm(model, delete_open))
        .on_confirm(move || commit_delete(model, delete_open))
        .content_fn(move || {
            view()
                .gap_3()
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            text(move || {
                                let m = model.get();
                                let id = m.operator.delete_arm.as_ref().map_or(0, |a| a.id);
                                m.word(Msg::ReviewDeleteId).replace("{id}", &id.to_string())
                            })
                            .flex_1()
                            .font_family("monospace")
                            .text_size(px(ui::scale::text::BODY)),
                        )
                        .child(
                            ui::copy_button(
                                model,
                                move || model.get().word(Msg::ReviewCopyId).into(),
                                move || model.get().word(Msg::CopiedShort).into(),
                                move || {
                                    let id = model
                                        .get()
                                        .operator
                                        .delete_arm
                                        .as_ref()
                                        .map_or(0, |a| a.id);
                                    if id <= 0 {
                                        return false;
                                    }
                                    set_clipboard(id.to_string());
                                    true
                                },
                            )
                            .size(ui::ButtonSize::Small),
                        ),
                )
                .child(ui::field(
                    model,
                    move || model.get().word(Msg::ReviewDeleteRetype).into(),
                    ui::input(model, "")
                        .label_fn(move || model.get().word(Msg::ReviewDeleteRetype).into())
                        .element_ref(&retype_ref)
                        .value(move || {
                            model
                                .get()
                                .operator
                                .delete_arm
                                .as_ref()
                                .map(|a| a.retype.clone())
                                .unwrap_or_default()
                        })
                        .placeholder(model.get().word(Msg::ReviewDeleteRetypeHint))
                        .max_length(20)
                        .invalid(move || retype_mismatch(model))
                        .on_change(move |v| {
                            model.update(|m| {
                                if let Some(a) = &mut m.operator.delete_arm {
                                    a.retype = v.chars().take(20).collect();
                                }
                            })
                        })
                        .on_submit(move |_| commit_delete(model, delete_open)),
                ))
                .child(
                    text(move || {
                        let m = model.get();
                        if delete_armed(model) {
                            m.word(Msg::ReviewDeleteArmed)
                        } else if retype_mismatch(model) {
                            m.word(Msg::ReviewDeleteMismatch)
                        } else {
                            m.word(Msg::ReviewDeleteRetypeHint)
                        }
                    })
                    .text_size(px(ui::scale::text::CAPTION))
                    .text_color(move || {
                        if delete_armed(model) {
                            palette(model).success
                        } else if retype_mismatch(model) {
                            palette(model).danger
                        } else {
                            palette(model).muted
                        }
                    }),
                )
                .into_element()
        })
}
