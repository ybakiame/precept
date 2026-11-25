use super::workflow::writes_on;
use crate::*;
use precept_ui_core::{i18n::Msg, personal_sync::*};
use tokens::palette;
use ui::{DialogSize, dialog};

fn word(model: Signal<Model>, key: Msg) -> impl Fn() -> SharedString + 'static {
    move || model.get().word(key).into()
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
                    .children(apply.submission.selections.iter().map(|selection| {
                        let m = model.get();
                        let choice = match selection.resolution {
                            precept_ui_core::operator::PersonalSyncResolution::Accept => {
                                Msg::SyncConflictAccept
                            }
                            precept_ui_core::operator::PersonalSyncResolution::KeepLocal => {
                                Msg::SyncConflictKeepLocal
                            }
                            precept_ui_core::operator::PersonalSyncResolution::Manual(_) => {
                                Msg::SyncConflictManual
                            }
                        };
                        text(format!("{} · {}", selection.key, m.word(choice)))
                    }))
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

pub(super) fn all(model: Signal<Model>) -> impl IntoElement {
    apply_dialog(model)
}
