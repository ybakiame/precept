//! The Profile: one per daemon, shared by every project (application-wide).
//!
//! Set up once from a card shown where it is needed (Dashboard, Workspaces,
//! Connection) while none is selected: the daemon suggests a path in its own
//! private state folder; "Select at start" is on by default; nothing is created
//! or selected before the operator's click. Once selected, the Connection page
//! shows it with the startup choice and this run's writes. Writes can also be
//! turned on in place by the action that needs them (Workspaces). The daemon
//! checks every request again.
use crate::*;
use acp_operator_core::i18n::Msg;
use tokens::palette;
use ui::{BadgeVariant, ButtonSize, ButtonVariant, badge, button, field, input, scale, surface};

fn word(model: Signal<Model>, key: Msg) -> impl Fn() -> SharedString + 'static {
    move || model.get().word(key).into()
}
fn caption(model: Signal<Model>, f: impl Fn(&Model) -> Option<Msg> + 'static) -> impl IntoElement {
    text(move || {
        let m = model.get();
        f(&m).map(|k| m.word(k)).unwrap_or_default()
    })
    .text_size(px(scale::text::CAPTION))
    .text_color(move || palette(model).muted)
}

/// A daemon request through the current connection, off the UI thread.
pub(crate) fn call(
    model: Signal<Model>,
    method: &'static str,
    params: serde_json::Value,
    done: impl FnOnce(Signal<Model>, &mut Model, Result<serde_json::Value, &'static str>) + 'static,
) {
    #[cfg(feature = "ipc")]
    {
        let inputs = acp_operator_client::native::Inputs::from_connection(&model.get().connection);
        Owner::detached().run(|| {
            spawn(async move {
                let result = blocking(move || inputs.and_then(|i| i.project_call(method, params)))
                    .await
                    .unwrap_or(Err("Unavailable"));
                model.update(|m| done(model, m, result));
            });
        });
    }
    #[cfg(not(feature = "ipc"))]
    {
        let _ = (method, params);
        model.update(|m| done(model, m, Err("Unavailable")));
    }
}

/// Re-reads the Profile state everyone shows (Review summary, Workspaces gate).
pub(crate) fn read(model: Signal<Model>) {
    if !model.get_untracked().live {
        return;
    }
    call(model, "profile.info", serde_json::json!({}), |_, m, r| {
        let Ok(value) = r else { return };
        let Ok(info) = serde_json::from_value::<acp_operator_core::operator::ProfileInfo>(value)
        else {
            return;
        };
        m.project_add.write_enabled = Some(info.write_enabled);
        m.project_add.profile_selected = Some(info.selected);
        m.profile_setup.suggest(info.suggested_path.as_deref());
        *m.profile_info = Some(info);
    });
}

/// Selected, written state as last read (`None`: not read yet).
fn selected(m: &Model) -> Option<bool> {
    m.profile_info.as_ref().map(|i| i.selected)
}
fn needs_setup(m: &Model) -> bool {
    m.live && selected(m) == Some(false)
}

/// Re-reads when a page that depends on the Profile becomes visible on a live
/// connection, and after every new snapshot read there (a read clears the
/// shared Profile state). Only these are tracked; the read itself is untracked.
pub(crate) fn watch(model: Signal<Model>) {
    let visible = computed(move || {
        let m = model.get();
        let shown = m.live
            && matches!(
                shell::current(&m),
                shell::Destination::Connection
                    | shell::Destination::Profile
                    | shell::Destination::Section(
                        acp_operator_core::Section::Dashboard
                            | acp_operator_core::Section::Projects
                    )
            );
        (shown, m.epoch)
    });
    effect(move || {
        if visible.get().0 {
            untracked(|| read(model));
        }
    });
}

fn settle(m: &mut Model, r: Result<serde_json::Value, &'static str>) -> bool {
    m.profile_setup.busy = false;
    match r {
        Ok(_) => {
            m.profile_setup.error = None;
            true
        }
        Err(code) => {
            m.profile_setup.error = Some(code.into());
            false
        }
    }
}
/// After create/select: the snapshot changes (projects become served); the
/// new read also re-reads the Profile (see `watch`).
fn after_selection(
    model: Signal<Model>,
    m: &mut Model,
    r: Result<serde_json::Value, &'static str>,
) {
    settle(m, r);
    spawn(async move { load(model) });
}
/// After a write or startup change: only the Profile state changes.
fn after_change(model: Signal<Model>, m: &mut Model, r: Result<serde_json::Value, &'static str>) {
    settle(m, r);
    spawn(async move { read(model) });
}

fn create_or_select(model: Signal<Model>, method: &'static str) {
    let Some(params) = model.get().profile_setup.params() else {
        return;
    };
    model.update(|m| {
        m.profile_setup.busy = true;
        m.profile_setup.error = None;
    });
    call(model, method, params, after_selection);
}

/// Turns on writes for the selected Profile (this daemon run), then `then`.
pub(crate) fn enable_writes(model: Signal<Model>, then: impl FnOnce(Signal<Model>) + 'static) {
    let binding = model
        .get()
        .profile_info
        .as_ref()
        .and_then(|i| i.binding.clone());
    let Some(b) = binding else {
        model.update(|m| m.profile_setup.error = Some("Unavailable".into()));
        return;
    };
    model.update(|m| m.profile_setup.busy = true);
    call(
        model,
        "profile.write.enable",
        serde_json::json!({"profile_id":b.profile_id,"generation":b.generation,"confirm":true}),
        move |model, m, r| {
            m.profile_setup.busy = false;
            let ok = r.is_ok();
            if let Err(code) = r {
                m.profile_setup.error = Some(code.into());
                m.project_add.error = Some(code.into());
            } else {
                m.project_add.write_enabled = Some(true);
                if let Some(i) = m.profile_info.as_mut() {
                    i.write_enabled = true;
                }
            }
            spawn(async move {
                read(model);
                if ok {
                    then(model);
                }
            });
        },
    );
}

fn disable_writes(model: Signal<Model>) {
    model.update(|m| m.profile_setup.busy = true);
    call(
        model,
        "profile.write.disable",
        serde_json::json!({}),
        after_change,
    );
}
fn forget(model: Signal<Model>) {
    model.update(|m| m.profile_setup.busy = true);
    call(
        model,
        "profile.startup.forget",
        serde_json::json!({}),
        after_change,
    );
}

fn error_note(model: Signal<Model>) -> impl IntoElement {
    text(move || {
        let m = model.get();
        match m.profile_setup.error.as_deref() {
            None => String::new(),
            Some("Conflict") => m.word(Msg::ProfileExists).into(),
            Some("NotFound" | "UnsafeRuntime" | "InvalidRequest") => {
                m.word(Msg::ProfileRefused).into()
            }
            Some(code) => code.to_string(),
        }
    })
    .text_size(px(scale::text::CAPTION))
    .text_color(move || palette(model).error)
}

/// A badge whose words and tone follow the model (rebuilt only when they change).
fn state_badge(model: Signal<Model>, f: fn(&Model) -> (Msg, bool)) -> impl IntoElement {
    let state = computed(move || {
        let m = model.get();
        let (key, good) = f(&m);
        (m.word(key).to_owned(), good)
    });
    dynamic(move || {
        let (label, good) = state.get();
        badge(model, label).variant(if good {
            BadgeVariant::Success
        } else {
            BadgeVariant::Neutral
        })
    })
}

/// The first-run card: one explicit create (or select) of the Profile.
pub(crate) fn setup(model: Signal<Model>) -> impl IntoElement {
    // A file already at the suggested path: offer to use it first.
    let existing = move || {
        let m = model.get();
        m.profile_info.as_ref().is_some_and(|i| {
            i.suggested_exists && i.suggested_path.as_deref() == Some(m.profile_setup.path.trim())
        })
    };
    surface(model).title_fn(word(model, Msg::SetupTitle)).child(
        view()
            .gap_4()
            .child(text(move || model.get().word(Msg::SetupBody)).text_size(px(scale::text::BODY)))
            .child(
                field(
                    model,
                    word(model, Msg::ProfileFile),
                    input(model, "")
                        .label_fn(word(model, Msg::ProfileFile))
                        .value(move || model.get().profile_setup.path.clone())
                        .on_change(move |v: &str| {
                            model.update(|m| m.profile_setup.set_path(v.into()))
                        }),
                )
                .help(word(model, Msg::ProfileFileHelp)),
            )
            .child(
                view().items_start().child(
                    button(model, "")
                        .label_fn(word(model, Msg::SelectAtStart))
                        .size(ButtonSize::Small)
                        .selected(move || model.get().profile_setup.remember)
                        .on_press(move || {
                            model.update(|m| m.profile_setup.remember = !m.profile_setup.remember)
                        }),
                ),
            )
            .child(caption(model, |m| {
                m.profile_info
                    .as_ref()
                    .and_then(|i| i.startup_failure.as_deref())
                    .map(acp_operator_core::projects::refusal)
            }))
            .child(error_note(model))
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        button(model, "")
                            .label_fn(move || {
                                model
                                    .get()
                                    .word(if existing() {
                                        Msg::UseThisProfile
                                    } else {
                                        Msg::CreateProfileNow
                                    })
                                    .into()
                            })
                            .variant(ButtonVariant::Primary)
                            .disabled(move || model.get().profile_setup.params().is_none())
                            .loading(move || model.get().profile_setup.busy)
                            .on_press(move || {
                                create_or_select(
                                    model,
                                    if existing() {
                                        "profile.select"
                                    } else {
                                        "profile.create"
                                    },
                                )
                            }),
                    )
                    .child(
                        view().style_when(existing, |s| s.hidden()).child(
                            button(model, "")
                                .label_fn(word(model, Msg::UseExistingProfile))
                                .disabled(move || model.get().profile_setup.params().is_none())
                                .on_press(move || create_or_select(model, "profile.select")),
                        ),
                    ),
            ),
    )
}

/// Shown where a page needs the Profile and none is selected yet.
pub(crate) fn setup_when_needed(model: Signal<Model>) -> impl IntoElement {
    view()
        .max_w(px(scale::layout::FORM))
        .style_when(move || !needs_setup(&model.get()), |s| s.hidden())
        .child(setup(model))
}

/// The Connection page's Profile: setup while none is selected, otherwise the
/// startup choice and this run's writes.
pub(crate) fn section(model: Signal<Model>) -> impl IntoElement {
    let writes_on = move || {
        model
            .get()
            .profile_info
            .as_ref()
            .is_some_and(|i| i.write_enabled)
    };
    let current = surface(model)
        .title_fn(word(model, Msg::ProfileSection))
        .child(
            view()
                .gap_3()
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(state_badge(model, |_| (Msg::OperatorSelected, true)))
                        .child(caption(model, |m| {
                            Some(if m.profile_info.as_ref().is_some_and(|i| i.remembered) {
                                Msg::SelectedAtStart
                            } else {
                                Msg::NotSelectedAtStart
                            })
                        }))
                        .child(
                            view()
                                .style_when(
                                    move || {
                                        !model
                                            .get()
                                            .profile_info
                                            .as_ref()
                                            .is_some_and(|i| i.startup_profile)
                                    },
                                    |s| s.hidden(),
                                )
                                .child(
                                    button(model, "")
                                        .label_fn(word(model, Msg::ForgetStartup))
                                        .size(ButtonSize::Small)
                                        .on_press(move || forget(model)),
                                ),
                        ),
                )
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(state_badge(model, |m| {
                            let on = m.profile_info.as_ref().is_some_and(|i| i.write_enabled);
                            (if on { Msg::WritesOn } else { Msg::WritesOff }, on)
                        }))
                        .child(
                            view().style_when(writes_on, |s| s.hidden()).child(
                                button(model, "")
                                    .label_fn(word(model, Msg::EnableWrites))
                                    .size(ButtonSize::Small)
                                    .loading(move || model.get().profile_setup.busy)
                                    .on_press(move || enable_writes(model, |_| {})),
                            ),
                        )
                        .child(
                            view()
                                .style_when(move || !writes_on(), |s| s.hidden())
                                .child(
                                    button(model, "")
                                        .label_fn(word(model, Msg::DisableWrites))
                                        .size(ButtonSize::Small)
                                        .loading(move || model.get().profile_setup.busy)
                                        .on_press(move || disable_writes(model)),
                                ),
                        ),
                )
                .child(caption(model, |_| Some(Msg::WritesHelp)))
                .child(error_note(model)),
        );
    view()
        .max_w(px(scale::layout::FORM))
        .style_when(move || !model.get().live, |s| s.hidden())
        .child(
            view()
                .style_when(
                    move || selected(&model.get()) != Some(false),
                    |s| s.hidden(),
                )
                .child(setup(model)),
        )
        .child(
            view()
                .style_when(move || selected(&model.get()) != Some(true), |s| s.hidden())
                .child(current),
        )
}
