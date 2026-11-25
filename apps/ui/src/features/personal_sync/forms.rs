use super::workflow;
use crate::*;
use precept_ui_core::{
    i18n::Msg,
    personal_sync::{Dialog, FormAction, Guard, State},
};
use ui::{ButtonVariant, button, field, input, surface};

fn word(model: Signal<Model>, key: Msg) -> impl Fn() -> SharedString + 'static {
    move || model.get().word(key).into()
}

fn busy(model: Signal<Model>) -> bool {
    let m = model.get();
    m.sync.busy
        || m.sync.apply.as_ref().is_some_and(|apply| apply.uncertain)
        || !workflow::ready(model)
}

fn file_field(
    model: Signal<Model>,
    label: Msg,
    value: impl Fn() -> String + 'static,
    set: fn(&mut State, String),
    save: bool,
) -> impl IntoElement {
    field(
        model,
        word(model, label),
        view()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                view().flex_1().min_w_0().child(
                    input(model, "")
                        .label_fn(word(model, label))
                        .value(value)
                        .disabled(move || busy(model))
                        .on_change(move |value| model.update(|m| set(&mut m.sync, value.into()))),
                ),
            )
            .child(
                button(model, "")
                    .label_fn(word(
                        model,
                        if save {
                            Msg::SyncChooseOutput
                        } else {
                            Msg::SyncChooseFile
                        },
                    ))
                    .disabled(move || busy(model))
                    .on_press(move || pick(model, label, set, save)),
            ),
    )
}

fn pick(model: Signal<Model>, label: Msg, set: fn(&mut State, String), save: bool) {
    let m = model.get();
    let title = m.word(label).to_string();
    let guard = Guard::capture(&m);
    model.update(|m| m.sync.dialog_generation = m.sync.dialog_generation.wrapping_add(1));
    let generation = model.get().sync.dialog_generation;
    spawn(async move {
        let result = if save {
            let mut dialog = nsh_core::file_dialog::SaveDialog::default();
            dialog.title = Some(title);
            dialog.file_name = Some("precept-preferences.age".into());
            pick_save_path(dialog).await
        } else {
            pick_paths(nsh_core::file_dialog::OpenDialog::files().title(title))
                .await
                .map(|paths| paths.and_then(|paths| paths.first().cloned()))
        };
        model.update(|m| {
            if !m.sync_open || !m.sync.dialog_is_current(&guard, generation, &Dialog::None) {
                return;
            }
            match result {
                Ok(Some(path)) => set(&mut m.sync, path.to_string_lossy().into_owned()),
                Ok(None) => {}
                Err(_) => m.sync.error = Some("dialog".into()),
            }
        });
    });
}

pub(super) fn export_form(model: Signal<Model>) -> impl IntoElement {
    surface(model)
        .title_fn(word(model, Msg::SyncExportTitle))
        .child(
            view()
                .gap_3()
                .child(text(move || model.get().word(Msg::SyncExportHelp)))
                .child(field(
                    model,
                    word(model, Msg::SyncExportPassword),
                    input(model, "")
                        .label_fn(word(model, Msg::SyncExportPassword))
                        .masked(move || !model.get().sync.export.show_password)
                        .max_length(1024)
                        .value(move || model.get().sync.export.passphrase.as_str().to_owned())
                        .disabled(move || busy(model))
                        .on_change(move |value| {
                            model.update(|m| m.sync.export.passphrase = value.to_owned().into())
                        }),
                ))
                .child(text(move || model.get().word(Msg::SyncPasswordHelp)))
                .child(file_field(
                    model,
                    Msg::SyncOutputFile,
                    move || model.get().sync.export.output_path.clone(),
                    |state, value| state.export.output_path = value,
                    true,
                ))
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            button(model, "")
                                .label_fn(word(model, Msg::SyncExportConfirm))
                                .variant(ButtonVariant::Primary)
                                .loading(move || {
                                    let m = model.get();
                                    m.sync.busy && m.sync.form_action == Some(FormAction::Export)
                                })
                                .disabled(move || {
                                    let m = model.get();
                                    busy(model)
                                        || m.sync.export.passphrase.chars().count() < 12
                                        || m.sync.export.passphrase.len() > 1024
                                        || m.sync.export.output_path.trim().is_empty()
                                })
                                .on_press(move || workflow::export(model)),
                        )
                        .child(
                            button(model, "")
                                .label_fn(word(model, Msg::SyncGeneratePassword))
                                .disabled(move || busy(model))
                                .on_press(
                                    move || match precept_ui_client::sync_files::operation_id() {
                                        Ok(password) => model.update(|m| {
                                            m.sync.export.passphrase = password.into();
                                            m.sync.export.show_password = true;
                                        }),
                                        Err(code) => workflow::fail(model, code),
                                    },
                                ),
                        )
                        .child(
                            button(model, "")
                                .label_fn(move || {
                                    let m = model.get();
                                    m.word(if m.sync.export.show_password {
                                        Msg::SyncHidePassword
                                    } else {
                                        Msg::SyncShowPassword
                                    })
                                    .into()
                                })
                                .disabled(move || busy(model))
                                .on_press(move || {
                                    model.update(|m| {
                                        m.sync.export.show_password = !m.sync.export.show_password
                                    })
                                }),
                        ),
                ),
        )
}

pub(super) fn import_form(model: Signal<Model>) -> impl IntoElement {
    surface(model)
        .title_fn(word(model, Msg::SyncImportTitle))
        .child(
            view()
                .gap_3()
                .child(text(move || model.get().word(Msg::SyncImportHelp)))
                .child(file_field(
                    model,
                    Msg::SyncEnvelopeFile,
                    move || model.get().sync.import.envelope_path.clone(),
                    |state, value| {
                        state.discard_preview();
                        state.import.envelope_path = value;
                    },
                    false,
                ))
                .child(field(
                    model,
                    word(model, Msg::SyncImportPassword),
                    input(model, "")
                        .label_fn(word(model, Msg::SyncImportPassword))
                        .password()
                        .max_length(1024)
                        .value(move || model.get().sync.import.passphrase.as_str().to_owned())
                        .disabled(move || busy(model))
                        .on_change(move |value| {
                            model.update(|m| {
                                m.sync.discard_preview();
                                m.sync.import.passphrase = value.to_owned().into();
                            })
                        }),
                ))
                .child(
                    button(model, "")
                        .label_fn(word(model, Msg::SyncPreview))
                        .variant(ButtonVariant::Primary)
                        .loading(move || {
                            let m = model.get();
                            m.sync.busy && m.sync.form_action == Some(FormAction::Preview)
                        })
                        .disabled(move || {
                            let m = model.get();
                            busy(model)
                                || m.sync.import.envelope_path.trim().is_empty()
                                || m.sync.import.passphrase.is_empty()
                        })
                        .on_press(move || workflow::preview(model)),
                ),
        )
}
