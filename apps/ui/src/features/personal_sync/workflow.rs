use crate::*;
use precept_ui_core::{
    i18n::Msg,
    operator::{
        PersonalSyncAbortResult, PersonalSyncApplyResult, PersonalSyncExportRequest,
        PersonalSyncOpenRequest, PersonalSyncPreview,
    },
    personal_sync::*,
};
use serde_json::json;
use std::path::{Path, PathBuf};

pub(super) fn writes_on(model: Signal<Model>) -> bool {
    model
        .get()
        .profile_info
        .as_ref()
        .is_some_and(|info| info.write_enabled)
}

pub(super) fn ready(model: Signal<Model>) -> bool {
    let m = model.get();
    m.live
        && m.profile_info
            .as_ref()
            .is_some_and(|info| info.selected && info.binding.is_some())
}

pub(super) fn fail(model: Signal<Model>, code: &'static str) {
    model.update(|m| reject(m, code));
}

fn reject(m: &mut Model, code: &'static str) {
    m.sync.busy = false;
    m.sync.error = Some(code.into());
}

fn finish(
    m: &mut Model,
    serial: u64,
    origin: &Guard,
    result: Result<serde_json::Value, &'static str>,
) -> bool {
    let current = Guard::capture(m);
    m.sync.finish(serial, origin, &current, result)
}

fn begin(model: Signal<Model>) -> (u64, Guard) {
    let mut request = (0, Guard::capture(&model.get()));
    model.update(|m| {
        let guard = Guard::capture(m);
        m.sync.invalidate(guard.clone());
        request = (m.sync.begin(), guard);
    });
    request
}

pub(super) fn refresh(model: Signal<Model>) {
    if !ready(model) {
        return;
    }
    let (serial, origin) = begin(model);
    crate::features::profile::call(
        model,
        "profile.sync.status",
        json!({}),
        move |_, m, r| match r {
            Ok(value) => {
                if finish(m, serial, &origin, Ok(value.clone())) {
                    m.sync.accept_status(value);
                }
            }
            Err(code) => {
                if finish(m, serial, &origin, Err(code)) {
                    reject(m, code);
                }
            }
        },
    );
}

fn after_change(model: Signal<Model>) {
    Owner::detached().run(|| {
        spawn(async move {
            crate::features::profile::read(model);
            refresh(model);
        });
    });
}

fn scoped_params(mut params: serde_json::Value, guard: &Guard) -> serde_json::Value {
    if let Some(scope) = guard.scope()
        && let Some(fields) = params.as_object_mut()
    {
        fields.insert("expected_binding".into(), json!(scope));
    }
    params
}

fn write(
    model: Signal<Model>,
    guard: Guard,
    method: &'static str,
    params: serde_json::Value,
    done: impl FnOnce(&mut Model, serde_json::Value) + 'static,
) {
    if !guard.matches(&model.get()) || !ready(model) {
        return;
    }
    let params = scoped_params(params, &guard);
    if writes_on(model) {
        write_now(model, guard, method, params, done);
    } else {
        crate::features::profile::enable_writes(model, move |model| {
            if guard.matches(&model.get()) {
                write_now(model, guard, method, params, done);
            }
        });
    }
}

fn write_now(
    model: Signal<Model>,
    guard: Guard,
    method: &'static str,
    params: serde_json::Value,
    done: impl FnOnce(&mut Model, serde_json::Value) + 'static,
) {
    if !guard.matches(&model.get()) {
        return;
    }
    let (serial, origin) = begin(model);
    crate::features::profile::call(model, method, params, move |_, m, r| match r {
        Ok(value) => {
            if finish(m, serial, &origin, Ok(value.clone())) {
                done(m, value);
            }
        }
        Err(code) => {
            if finish(m, serial, &origin, Err(code)) {
                reject(m, code);
            }
        }
    });
}

pub(super) fn export(model: Signal<Model>) {
    model.update(|m| m.sync.form_action = Some(FormAction::Export));
    let guard = Guard::capture(&model.get());
    let draft = model.get().sync.export.clone();
    let output = PathBuf::from(&draft.output_path);
    if precept_ui_client::sync_files::ensure_absent(&output).is_err() {
        fail(model, "Conflict");
        return;
    }
    if draft.passphrase.chars().count() < 12 || draft.passphrase.len() > 1024 {
        fail(model, "password");
        return;
    }
    let request = PersonalSyncExportRequest {
        recipient: None,
        passphrase: Some(draft.passphrase.as_str().to_owned()),
        keys: None,
    };
    let Ok(params) = serde_json::to_value(request) else {
        return;
    };
    write(
        model,
        guard,
        "profile.sync.export",
        params,
        move |m, value| {
            let written = precept_ui_client::sync_files::write_envelope(&output, &value);
            match written {
                Ok(()) => {
                    m.sync.export.passphrase = Default::default();
                    ui::notify(ui::Tone::Success, m.word(Msg::SyncSaved));
                    after_change(model);
                }
                Err(code) => {
                    m.sync.busy = false;
                    m.sync.error = Some(code.into());
                }
            }
        },
    );
}

pub(super) fn preview(model: Signal<Model>) {
    model.update(|m| m.sync.form_action = Some(FormAction::Preview));
    if model
        .get()
        .sync
        .status
        .as_ref()
        .is_some_and(|status| status.initialized)
    {
        preview_now(model);
    } else {
        let guard = Guard::capture(&model.get());
        write(model, guard, "profile.sync.init", json!({}), move |_, _| {
            preview_now(model)
        });
    }
}

fn preview_now(model: Signal<Model>) {
    let draft = model.get().sync.import.clone();
    let envelope =
        match precept_ui_client::sync_files::read_envelope(Path::new(&draft.envelope_path)) {
            Ok(value) => value,
            Err(code) => {
                fail(model, code);
                return;
            }
        };
    if draft.passphrase.is_empty() {
        fail(model, "password");
        return;
    }
    let passphrase = Some(draft.passphrase.as_str().to_owned());
    let request = PersonalSyncOpenRequest {
        envelope: envelope.clone(),
        passphrase: passphrase.clone(),
    };
    let Ok(params) = serde_json::to_value(request) else {
        return;
    };
    let guard = Guard::capture(&model.get());
    let operation_id =
        precept_ui_client::sync_files::operation_id().unwrap_or_else(|_| "desktop-import".into());
    if !ready(model) {
        return;
    }
    let params = scoped_params(params, &guard);
    let (serial, origin) = begin(model);
    crate::features::profile::call(
        model,
        "profile.sync.preview",
        params,
        move |_, m, r| match r {
            Ok(value) => {
                if finish(m, serial, &origin, Ok(json!({}))) && m.sync_open {
                    match serde_json::from_value::<PersonalSyncPreview>(value) {
                        Ok(preview) => {
                            m.sync.preview = Some(PreviewState::new(
                                preview,
                                envelope,
                                passphrase,
                                guard,
                                operation_id,
                            ));
                            m.sync.apply = None;
                            m.sync.import.passphrase = Default::default();
                        }
                        Err(_) => reject(m, "InvalidRequest"),
                    }
                }
            }
            Err(code) => {
                if finish(m, serial, &origin, Err(code)) {
                    reject(m, code);
                }
            }
        },
    );
}

pub(super) fn prepare(model: Signal<Model>) {
    let mut apply = None;
    model.update(|m| apply = m.sync.preview.as_ref().and_then(PreviewState::prepare));
    if let Some(apply) = apply {
        model.update(|m| {
            m.sync.apply = Some(apply);
            m.sync.dialog = Dialog::Apply;
        });
    } else {
        fail(model, "InvalidRequest");
    }
}

pub(super) fn apply(model: Signal<Model>, retry: bool) {
    let Some(apply) = model.get().sync.apply.clone() else {
        return;
    };
    if (!retry && apply.uncertain) || (retry && !apply.retryable(&model.get())) {
        return;
    }
    let guard = apply.guard.clone();
    if !guard.matches(&model.get()) {
        return;
    }
    let operation_id = apply.submission.operation_id.clone();
    let params = scoped_params(apply.params(), &guard);
    if writes_on(model) {
        apply_now(model, guard, params, operation_id);
    } else {
        crate::features::profile::enable_writes(model, move |model| {
            if guard.matches(&model.get()) {
                apply_now(model, guard, params, operation_id);
            }
        });
    }
}

fn outcome_unknown(m: &mut Model) {
    m.sync.error = Some("OutcomeUnknown".into());
    if let Some(state) = m.sync.apply.as_mut() {
        state.error = Some("OutcomeUnknown".into());
        state.uncertain = true;
    }
}

fn apply_now(model: Signal<Model>, guard: Guard, params: serde_json::Value, operation_id: String) {
    if !guard.matches(&model.get()) {
        return;
    }
    let (serial, origin) = begin(model);
    crate::features::profile::call(
        model,
        "profile.sync.apply",
        params,
        move |_, m, r| match r {
            Ok(value) if finish(m, serial, &origin, Ok(value.clone())) => {
                match serde_json::from_value::<PersonalSyncApplyResult>(value) {
                    Ok(result) if result.operation_id == operation_id => {
                        if let Some(state) = m.sync.apply.as_mut() {
                            state.succeed(result.clone());
                        }
                        m.sync.discard_preview();
                        ui::notify(
                            ui::Tone::Success,
                            m.word(Msg::SyncApplySucceeded)
                                .replace("{applied}", &result.applied.to_string())
                                .replace("{changed}", &result.changed.to_string()),
                        );
                        m.sync.close_dialog();
                        after_change(model);
                    }
                    Ok(_) => outcome_unknown(m),
                    Err(_) => outcome_unknown(m),
                }
            }
            Ok(_) => {}
            Err(code) => {
                if finish(m, serial, &origin, Err(code)) {
                    m.sync.error = Some(code.into());
                    let uncertain = matches!(code, "OutcomeUnknown" | "Unavailable" | "Timeout");
                    if let Some(state) = m.sync.apply.as_mut() {
                        if uncertain {
                            state.error = Some(code.into());
                            state.uncertain = true;
                        } else {
                            state.refuse(code);
                            m.sync.discard_preview();
                        }
                    }
                }
            }
        },
    );
}

pub(super) fn abort(model: Signal<Model>) {
    let Some(apply) = model.get().sync.apply.clone() else {
        return;
    };
    let guard = apply.guard.clone();
    let operation_id = apply.submission.operation_id.clone();
    write(
        model,
        guard,
        "profile.sync.abort",
        json!({"operation_id": operation_id.clone()}),
        move |m, value| {
            let Ok(abort) = serde_json::from_value::<PersonalSyncAbortResult>(value) else {
                reject(m, "InvalidRequest");
                return;
            };
            if abort.operation_id != operation_id {
                reject(m, "InvalidRequest");
                return;
            }
            if let Some(state) = m.sync.apply.as_mut() {
                state.settle_abort(abort);
            }
            m.sync.discard_preview();
            ui::notify(ui::Tone::Success, m.word(Msg::SyncAbortSucceeded));
            m.sync.close_dialog();
            after_change(model);
        },
    );
}

pub(crate) fn watch(model: Signal<Model>) {
    let key = computed(move || {
        let m = model.get();
        let binding = m
            .profile_info
            .as_ref()
            .and_then(|info| info.binding.as_ref())
            .map(|binding| (binding.profile_id.clone(), binding.generation.clone()));
        (m.live, m.sync_open, binding, m.profile_epoch)
    });
    effect(move || {
        if key.get().1 {
            untracked(|| refresh(model));
        } else {
            model.update(|m| {
                m.sync.export.passphrase = Default::default();
                m.sync.export.show_password = false;
                m.sync.import.passphrase = Default::default();
                m.sync.dialog_generation = m.sync.dialog_generation.wrapping_add(1);
            });
        }
    });
}
