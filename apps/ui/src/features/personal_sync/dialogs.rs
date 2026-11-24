use super::workflow::writes_on;
use super::workflow::{confirm, export, pair, preview};
use crate::*;
use precept_ui_core::{i18n::Msg, personal_sync::*};
use tokens::palette;
use ui::{DialogSize, DialogTone, alert_dialog, button, dialog, field, input, radio_group, scale};

fn word(model: Signal<Model>, key: Msg) -> impl Fn() -> SharedString + 'static {
    move || model.get().word(key).into()
}

fn file_field(
    model: Signal<Model>,
    label: Msg,
    value: impl Fn() -> String + 'static,
    set: fn(&mut State, String),
    save: bool,
) -> impl IntoElement {
    let choose_label = if save {
        Msg::SyncChooseOutput
    } else {
        Msg::SyncChooseFile
    };
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
                        .on_change(move |value| model.update(|m| set(&mut m.sync, value.into()))),
                ),
            )
            .child(
                button(model, "")
                    .label_fn(word(model, choose_label))
                    .on_press(move || choose_file(model, label, set, save)),
            ),
    )
}

async fn pick(
    model: Signal<Model>,
    title: String,
    set: fn(&mut State, String),
    save: bool,
    guard: Guard,
    generation: u64,
    expected: Dialog,
) {
    let result = if save {
        let mut dialog = nsh_core::file_dialog::SaveDialog::default();
        dialog.title = Some(title);
        dialog.file_name = Some("precept-sync.json".into());
        pick_save_path(dialog).await
    } else {
        let dialog = nsh_core::file_dialog::OpenDialog::files().title(title);
        pick_paths(dialog)
            .await
            .map(|paths| paths.and_then(|paths| paths.first().cloned()))
    };
    match result {
        Ok(Some(path)) => {
            let path = path.to_string_lossy().to_string();
            model.update(|m| {
                if m.sync.dialog_is_current(&guard, generation, &expected) {
                    set(&mut m.sync, path);
                }
            });
        }
        Ok(None) => {}
        Err(_) => model.update(|m| {
            if m.sync.dialog_is_current(&guard, generation, &expected) {
                m.sync.error = Some("dialog".into());
            }
        }),
    }
}

fn choose_file(model: Signal<Model>, label: Msg, set: fn(&mut State, String), save: bool) {
    let m = model.get();
    let title = m.word(label).to_string();
    let guard = Guard::capture(&m);
    let generation = m.sync.dialog_generation;
    let expected = m.sync.dialog.clone();
    spawn(async move { pick(model, title, set, save, guard, generation, expected).await });
}

fn invitation(model: Signal<Model>) -> impl IntoElement {
    dialog(model, "")
        .size(DialogSize::Large)
        .icon(icons::FILE, DialogTone::Accent)
        .title_fn(word(model, Msg::SyncInvitation))
        .description_fn(word(model, Msg::SyncInvitationHelp))
        .content_fn(move || {
            dynamic(move || {
                let m = model.get();
                let Some(invitation) = &m.sync.invitation else {
                    return view().into_element();
                };
                let Ok(file) = precept_ui_client::sync_files::InvitationFile::new(
                    invitation.invitation.clone(),
                ) else {
                    return view().into_element();
                };
                view()
                    .gap_3()
                    .child(
                        text(format!(
                            "{}: {}",
                            m.word(Msg::SyncOwnerFingerprint),
                            invitation.invitation.owner_fingerprint
                        ))
                        .font_family("monospace")
                        .text_size(px(scale::text::CAPTION)),
                    )
                    .child(
                        text(format!(
                            "{}: {}",
                            m.word(Msg::SyncInvitationFingerprintInput),
                            invitation.fingerprint
                        ))
                        .font_family("monospace")
                        .text_size(px(scale::text::CAPTION)),
                    )
                    .child(
                        view()
                            .flex_row()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                button(model, "")
                                    .label_fn(word(model, Msg::SyncCopyInvitation))
                                    .icon(icons::COPY)
                                    .on_press(move || {
                                        if let Ok(value) = file.value() {
                                            set_clipboard(
                                                serde_json::to_string(&value).unwrap_or_default(),
                                            );
                                            ui::notify(
                                                ui::Tone::Success,
                                                model.get().word(Msg::SyncCopy),
                                            );
                                        }
                                    }),
                            )
                            .child(
                                button(model, "")
                                    .label_fn(word(model, Msg::SyncSaveInvitation))
                                    .on_press(move || save_invitation(model)),
                            ),
                    )
                    .into_element()
            })
        })
        .cancel_fn(word(model, Msg::SyncClose))
        .confirm_fn(word(model, Msg::SyncDone))
        .open(move || model.get().sync.dialog == Dialog::Invitation)
        .on_cancel(move || model.update(|m| m.sync.close_dialog()))
        .on_confirm(move || model.update(|m| m.sync.close_dialog()))
}

fn save_invitation(model: Signal<Model>) {
    let Some(invitation) = model.get().sync.invitation.clone() else {
        return;
    };
    let title = model.get().word(Msg::SyncSaveInvitation).to_string();
    spawn(async move {
        let mut dialog = nsh_core::file_dialog::SaveDialog::default();
        dialog.title = Some(title);
        dialog.file_name = Some("precept-invitation.json".into());
        if let Ok(Some(path)) = pick_save_path(dialog).await {
            let result = precept_ui_client::sync_files::InvitationFile::new(invitation.invitation)
                .and_then(|file| file.value())
                .and_then(|value| precept_ui_client::sync_files::write_new(&path, &value));
            model.update(|m| match result {
                Ok(()) => ui::notify(ui::Tone::Success, m.word(Msg::SyncSaved)),
                Err(code) => m.sync.error = Some(code.into()),
            });
        }
    });
}

fn pair_draft(model: Signal<Model>) -> PairDraft {
    match model.get().sync.dialog {
        Dialog::Pair(draft) => draft,
        _ => PairDraft::default(),
    }
}

fn export_draft(model: Signal<Model>) -> ExportDraft {
    match model.get().sync.dialog {
        Dialog::Export(draft) => draft,
        _ => ExportDraft::default(),
    }
}

fn import_draft(model: Signal<Model>) -> ImportDraft {
    match model.get().sync.dialog {
        Dialog::Import(draft) => draft,
        _ => ImportDraft::default(),
    }
}

fn pair_dialog(model: Signal<Model>) -> impl IntoElement {
    dialog(model, "")
        .size(DialogSize::Large)
        .title_fn(word(model, Msg::SyncPair))
        .description_fn(word(model, Msg::SyncPairHelp))
        .content_fn(move || {
            view()
                .gap_3()
                .child(file_field(
                    model,
                    Msg::SyncInvitationFile,
                    move || pair_draft(model).path,
                    |state, value| {
                        if let Dialog::Pair(draft) = &mut state.dialog {
                            draft.path = value;
                        }
                    },
                    false,
                ))
                .child(field(
                    model,
                    word(model, Msg::SyncOwnerFingerprintInput),
                    input(model, "")
                        .label_fn(word(model, Msg::SyncOwnerFingerprintInput))
                        .max_length(32)
                        .value(move || pair_draft(model).owner_fingerprint)
                        .on_change(move |value| {
                            model.update(|m| {
                                if let Dialog::Pair(draft) = &mut m.sync.dialog {
                                    draft.owner_fingerprint = value.into();
                                }
                            })
                        }),
                ))
                .child(field(
                    model,
                    word(model, Msg::SyncInvitationFingerprintInput),
                    input(model, "")
                        .label_fn(word(model, Msg::SyncInvitationFingerprintInput))
                        .max_length(32)
                        .value(move || pair_draft(model).invitation_fingerprint)
                        .on_change(move |value| {
                            model.update(|m| {
                                if let Dialog::Pair(draft) = &mut m.sync.dialog {
                                    draft.invitation_fingerprint = value.into();
                                }
                            })
                        }),
                ))
                .into_element()
        })
        .cancel_fn(word(model, Msg::Cancel))
        .confirm_fn(word(model, Msg::SyncTrust))
        .confirm_disabled(move || {
            let Dialog::Pair(draft) = model.get().sync.dialog else {
                return true;
            };
            draft.path.trim().is_empty()
                || draft.owner_fingerprint.trim().len() != 32
                || draft.invitation_fingerprint.trim().len() != 32
                || model.get().sync.busy
        })
        .open(move || matches!(model.get().sync.dialog, Dialog::Pair(_)))
        .on_cancel(move || model.update(|m| m.sync.close_dialog()))
        .on_confirm(move || pair(model))
}

fn export_dialog(model: Signal<Model>) -> impl IntoElement {
    dialog(model, "")
        .size(DialogSize::Large)
        .title_fn(word(model, Msg::SyncExport))
        .description_fn(word(model, Msg::SyncExportHelp))
        .content_fn(move || {
            view()
                .gap_3()
                .child(
                    radio_group(model, word(model, Msg::SyncExport))
                        .option_fn(
                            move || model.get().word(Msg::SyncExportModeRecipient).into(),
                            move || export_draft(model).mode == ExportMode::Recipient,
                            move || {
                                model.update(|m| {
                                    if let Dialog::Export(draft) = &mut m.sync.dialog {
                                        draft.mode = ExportMode::Recipient;
                                    }
                                })
                            },
                        )
                        .option_fn(
                            move || model.get().word(Msg::SyncExportModePassphrase).into(),
                            move || export_draft(model).mode == ExportMode::Passphrase,
                            move || {
                                model.update(|m| {
                                    if let Dialog::Export(draft) = &mut m.sync.dialog {
                                        draft.mode = ExportMode::Passphrase;
                                    }
                                })
                            },
                        ),
                )
                .child(
                    view()
                        .style_when(
                            move || export_draft(model).mode != ExportMode::Recipient,
                            |s| s.hidden(),
                        )
                        .child(field(
                            model,
                            word(model, Msg::SyncRecipient),
                            input(model, "")
                                .label_fn(word(model, Msg::SyncRecipient))
                                .max_length(32)
                                .value(move || export_draft(model).recipient)
                                .on_change(move |value| {
                                    model.update(|m| {
                                        if let Dialog::Export(draft) = &mut m.sync.dialog {
                                            draft.recipient = value.into();
                                        }
                                    })
                                }),
                        )),
                )
                .child(
                    view()
                        .style_when(
                            move || export_draft(model).mode != ExportMode::Passphrase,
                            |s| s.hidden(),
                        )
                        .child(file_field(
                            model,
                            Msg::SyncPassphraseFile,
                            move || export_draft(model).passphrase_path,
                            |state, value| {
                                if let Dialog::Export(draft) = &mut state.dialog {
                                    draft.passphrase_path = value;
                                }
                            },
                            false,
                        ))
                        .child(text(move || model.get().word(Msg::SyncPassphraseHelp))),
                )
                .child(file_field(
                    model,
                    Msg::SyncOutputFile,
                    move || export_draft(model).output_path,
                    |state, value| {
                        if let Dialog::Export(draft) = &mut state.dialog {
                            draft.output_path = value;
                        }
                    },
                    true,
                ))
                .into_element()
        })
        .cancel_fn(word(model, Msg::Cancel))
        .confirm_fn(word(model, Msg::SyncExportConfirm))
        .confirm_disabled(move || {
            let Dialog::Export(draft) = model.get().sync.dialog else {
                return true;
            };
            draft.output_path.trim().is_empty()
                || match draft.mode {
                    ExportMode::Recipient => draft.recipient.trim().is_empty(),
                    ExportMode::Passphrase => draft.passphrase_path.trim().is_empty(),
                }
                || model.get().sync.busy
        })
        .open(move || matches!(model.get().sync.dialog, Dialog::Export(_)))
        .on_cancel(move || model.update(|m| m.sync.close_dialog()))
        .on_confirm(move || export(model))
}

fn import_dialog(model: Signal<Model>) -> impl IntoElement {
    dialog(model, "")
        .size(DialogSize::Large)
        .title_fn(word(model, Msg::SyncImport))
        .description_fn(word(model, Msg::SyncImportHelp))
        .content_fn(move || {
            view()
                .gap_3()
                .child(file_field(
                    model,
                    Msg::SyncEnvelopeFile,
                    move || import_draft(model).envelope_path,
                    |state, value| {
                        if let Dialog::Import(draft) = &mut state.dialog {
                            draft.envelope_path = value;
                        }
                    },
                    false,
                ))
                .child(file_field(
                    model,
                    Msg::SyncPassphraseFile,
                    move || import_draft(model).passphrase_path,
                    |state, value| {
                        if let Dialog::Import(draft) = &mut state.dialog {
                            draft.passphrase_path = value;
                        }
                    },
                    false,
                ))
                .child(text(move || model.get().word(Msg::SyncPassphraseHelp)))
                .into_element()
        })
        .cancel_fn(word(model, Msg::Cancel))
        .confirm_fn(word(model, Msg::SyncPreview))
        .confirm_disabled(move || {
            let Dialog::Import(draft) = model.get().sync.dialog else {
                return true;
            };
            draft.envelope_path.trim().is_empty() || model.get().sync.busy
        })
        .open(move || matches!(model.get().sync.dialog, Dialog::Import(_)))
        .on_cancel(move || model.update(|m| m.sync.close_dialog()))
        .on_confirm(move || preview(model))
}

fn apply_dialog(model: Signal<Model>) -> impl IntoElement {
    dialog(model, "")
        .size(DialogSize::Large)
        .title_fn(word(model, Msg::SyncApplyTitle))
        .description_fn(word(model, Msg::SyncApplyHelp))
        .content_fn(move || {
            dynamic(move || {
                let Some(apply) = model.get().sync.apply.clone() else {
                    return view().into_element();
                };
                view()
                    .gap_2()
                    .child(
                        text(format!(
                            "{}: {}",
                            model.get().word(Msg::SyncOperationId),
                            apply.submission.operation_id
                        ))
                        .font_family("monospace")
                        .text_size(px(scale::text::CAPTION)),
                    )
                    .child(
                        text(format!(
                            "{}: {}",
                            model.get().word(Msg::SyncSender),
                            apply.submission.sender_device_id
                        ))
                        .font_family("monospace")
                        .text_size(px(scale::text::CAPTION)),
                    )
                    .child(text(
                        model
                            .get()
                            .word(Msg::SyncExpectedCommit)
                            .replace("{n}", &apply.submission.expected_commit.to_string()),
                    ))
                    .child(text(model.get().word(Msg::SyncSelectedCount).replace(
                        "{n}",
                        &apply.submission.selections.len().to_string(),
                    )))
                    .child(dynamic(move || {
                        let m = model.get();
                        let Some(apply) = &m.sync.apply else {
                            return view().into_element();
                        };
                        if apply.uncertain {
                            return view()
                                .child(
                                    text(m.word(Msg::SyncUncertain))
                                        .text_color(move || palette(model).warning),
                                )
                                .into_element();
                        }
                        if let Some(result) = &apply.result {
                            return view()
                                .child(text(
                                    m.word(Msg::SyncApplySucceeded)
                                        .replace("{applied}", &result.applied.to_string())
                                        .replace("{changed}", &result.changed.to_string()),
                                ))
                                .into_element();
                        }
                        if let Some(error) = &apply.error {
                            return view()
                                .child(
                                    text(m.word(Msg::SyncApplyRefused).replace(
                                        "{error}",
                                        &precept_ui_core::presentation::sanitize(error, 64),
                                    ))
                                    .text_color(move || palette(model).error),
                                )
                                .into_element();
                        }
                        view().into_element()
                    }))
                    .into_element()
            })
        })
        .cancel_fn(word(model, Msg::Cancel))
        .confirm_fn(move || {
            let m = model.get();
            let key = match &m.sync.apply {
                Some(apply) if apply.result.is_some() => Msg::SyncDone,
                Some(apply) if apply.uncertain => Msg::SyncRetry,
                Some(apply) if apply.error.is_some() => Msg::SyncDone,
                _ if writes_on(model) => Msg::SyncApplyConfirm,
                _ => Msg::SyncEnableAndApply,
            };
            m.word(key).into()
        })
        .confirm_disabled(move || model.get().sync.busy)
        .open(move || model.get().sync.dialog == Dialog::Apply)
        .on_cancel(move || model.update(|m| m.sync.close_dialog()))
        .on_confirm(move || {
            let m = model.get();
            match &m.sync.apply {
                Some(apply) if apply.result.is_some() => model.update(|m| m.sync.close_dialog()),
                Some(apply) if apply.uncertain => super::workflow::apply(model, true),
                Some(apply) if apply.error.is_some() => model.update(|m| m.sync.close_dialog()),
                _ => super::workflow::apply(model, false),
            }
        })
}

fn confirm_dialog(model: Signal<Model>) -> impl IntoElement {
    alert_dialog(model, "")
        .icon(icons::ALERT, DialogTone::Danger)
        .title_fn(move || {
            let m = model.get();
            match &m.sync.dialog {
                Dialog::Confirm(Confirm::Rotate) => m.word(Msg::SyncRotateTitle).into(),
                Dialog::Confirm(Confirm::Reset) => m.word(Msg::SyncResetTitle).into(),
                Dialog::Confirm(Confirm::Revoke(id)) => {
                    m.word(Msg::SyncRevokeTitle).replace("{device}", id).into()
                }
                _ => String::new().into(),
            }
        })
        .description_fn(move || {
            let m = model.get();
            match &m.sync.dialog {
                Dialog::Confirm(Confirm::Rotate) => m.word(Msg::SyncRotateBody).into(),
                Dialog::Confirm(Confirm::Reset) => m.word(Msg::SyncResetBody).into(),
                Dialog::Confirm(Confirm::Revoke(_)) => m.word(Msg::SyncRevokeBody).into(),
                _ => String::new().into(),
            }
        })
        .cancel_fn(word(model, Msg::Cancel))
        .confirm_fn(word(model, Msg::SyncConfirm))
        .confirm_disabled(move || model.get().sync.busy)
        .open(move || matches!(model.get().sync.dialog, Dialog::Confirm(_)))
        .on_cancel(move || model.update(|m| m.sync.close_dialog()))
        .on_confirm(move || confirm(model))
}

pub(super) fn all(model: Signal<Model>) -> impl IntoElement {
    view()
        .child(invitation(model))
        .child(pair_dialog(model))
        .child(export_dialog(model))
        .child(import_dialog(model))
        .child(apply_dialog(model))
        .child(confirm_dialog(model))
}
