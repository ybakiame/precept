//! Sessions: the selected project's work sessions, as agents recorded them
//! (goal, doing / next / done, summary). Read-only for the operator except
//! Delete; agents write them through MCP where the operator allows it.
use crate::*;
use precept_ui_core::{activity::date_time, i18n::Msg, work::WorkSession};
use tokens::palette;
use ui::{BadgeVariant, ButtonSize, ButtonVariant, badge, button, scale, surface};

fn word(model: Signal<Model>, key: Msg) -> impl Fn() -> SharedString + 'static {
    move || model.get().word(key).into()
}

/// Reads the project's sessions.
pub(crate) fn load(model: Signal<Model>, project: String) {
    model.update(|m| {
        let s = &mut m.sessions;
        if s.project.as_deref() != Some(project.as_str()) {
            s.list = None;
            s.open = None;
        }
        s.project = Some(project.clone());
        s.busy = true;
        s.error = None;
    });
    super::profile::call(
        model,
        "sessions.list",
        serde_json::json!({ "project": project }),
        move |_, m, r| {
            if m.sessions.project.as_deref() != Some(project.as_str()) {
                return;
            }
            m.sessions.busy = false;
            match r.and_then(precept_ui_core::work::parse) {
                Ok(list) => m.sessions.list = Some(list),
                Err(code) => m.sessions.error = Some(code.into()),
            }
        },
    );
}

/// Saves the session as the typed Handoff (writes are turned on first when
/// off, as Add does).
fn save_handoff(model: Signal<Model>) {
    if model.get().project_add.write_enabled == Some(true) {
        save_handoff_now(model);
    } else {
        super::profile::enable_writes(model, save_handoff_now);
    }
}
fn save_handoff_now(model: Signal<Model>) {
    let m = model.get();
    let (Some(project), Some(id)) = (m.sessions.project.clone(), m.sessions.handoff.clone()) else {
        return;
    };
    let task = m.sessions.task.trim().to_owned();
    model.update(|m| {
        m.sessions.busy = true;
        m.sessions.error = None;
    });
    super::profile::call(
        model,
        "sessions.handoff",
        serde_json::json!({ "project": project, "id": id, "task_id": task }),
        move |_, m, r| {
            m.sessions.busy = false;
            match r {
                Ok(_) => {
                    m.sessions.handoff = None;
                    ui::notify(
                        ui::Tone::Success,
                        m.word(Msg::ToastSessionHandoff).replace("{task}", &task),
                    );
                }
                Err("Conflict") => m.sessions.error = Some(m.word(Msg::SessionHandoffTaken).into()),
                Err(code) => m.sessions.error = Some(code.into()),
            }
        },
    );
}

/// The task id for the Handoff.
fn handoff_dialog(model: Signal<Model>) -> impl IntoElement {
    ui::dialog(model, "")
        .icon(
            icons::section_path(precept_ui_core::Section::Handoff),
            ui::DialogTone::Accent,
        )
        .title_fn(word(model, Msg::SessionHandoffTitle))
        .description_fn(word(model, Msg::SessionHandoffBody))
        .content_fn(move || {
            view()
                .w_full()
                .gap_2()
                .child(ui::field(
                    model,
                    word(model, Msg::SessionHandoffTask),
                    ui::input(model, "")
                        .label_fn(word(model, Msg::SessionHandoffTask))
                        .value(move || model.get().sessions.task.clone())
                        .on_change(move |v: &str| {
                            model.update(|m| m.sessions.task = v.chars().take(120).collect())
                        }),
                ))
                .child(
                    text(move || model.get().sessions.error.clone().unwrap_or_default())
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).error),
                )
                .into_element()
        })
        .cancel_fn(word(model, Msg::Cancel))
        .confirm_fn(word(model, Msg::SessionHandoffSave))
        .confirm_disabled(move || {
            let m = model.get();
            m.sessions.busy || m.sessions.task.trim().is_empty()
        })
        .open(move || model.get().sessions.handoff.is_some())
        .on_cancel(move || {
            model.update(|m| {
                m.sessions.handoff = None;
                m.sessions.error = None;
            })
        })
        .on_confirm(move || save_handoff(model))
}

fn delete(model: Signal<Model>) {
    let m = model.get();
    let (Some(project), Some(id)) = (m.sessions.project.clone(), m.sessions.delete.clone()) else {
        return;
    };
    model.update(|m| m.sessions.busy = true);
    super::profile::call(
        model,
        "sessions.delete",
        serde_json::json!({ "project": project, "id": id }),
        move |model, m, r| {
            m.sessions.busy = false;
            m.sessions.delete = None;
            match r {
                Ok(_) => {
                    m.sessions.open = None;
                    ui::notify(ui::Tone::Success, m.word(Msg::ToastSessionDeleted));
                }
                Err(code) => m.sessions.error = Some(code.into()),
            }
            spawn(async move { load(model, project) });
        },
    );
}

/// Keeps the page aligned with the sidebar's current project while shown.
pub(crate) fn watch(model: Signal<Model>) {
    let ready = computed(move || {
        let m = model.get();
        (
            m.sessions_page && m.live && m.data.is_some(),
            m.epoch,
            m.selected_project.clone(),
        )
    });
    effect(move || {
        let (shown, _, project) = ready.get();
        if !shown {
            return;
        }
        if let Some(project) = project {
            untracked(|| {
                load(model, project.clone());
                super::agents::load_activity(model, project);
            });
        }
    });
}

fn status_badge(model: Signal<Model>, s: &WorkSession) -> AnyElement {
    let m = model.get_untracked();
    let open = s.status == "open";
    badge(
        model,
        m.word(if open {
            Msg::SessionOpen
        } else {
            Msg::SessionClosed
        }),
    )
    .variant(if open {
        BadgeVariant::Info
    } else {
        BadgeVariant::Neutral
    })
    .into_element()
}

fn byline(m: &Model, s: &WorkSession) -> String {
    m.word(Msg::SessionUpdated)
        .replace(
            "{agent}",
            s.agent
                .as_deref()
                .unwrap_or_else(|| m.word(Msg::SessionUnnamed)),
        )
        .replace("{time}", &date_time(s.updated_ms))
}

/// One session: its goal (pressing it opens the detail), who and when, and
/// how much is doing / next / done.
fn row(model: Signal<Model>, index: usize, s: &WorkSession) -> AnyElement {
    let m = model.get_untracked();
    let id = s.id.clone();
    let counts = m
        .word(Msg::SessionCounts)
        .replace("{doing}", &s.doing.len().to_string())
        .replace("{todo}", &s.todo.len().to_string())
        .replace("{done}", &s.done.len().to_string());
    view()
        .gap_1()
        .py_2()
        .style_when(move || index > 0, |s| s.border_t_1())
        .border_color(move || palette(model).hairline)
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    view().flex_1().min_w_0().child(
                        button(model, s.goal.clone())
                            .variant(ButtonVariant::Ghost)
                            .on_press(move || model.update(|m| m.sessions.open = Some(id.clone()))),
                    ),
                )
                .child(status_badge(model, s)),
        )
        .child(
            text(format!("{} · {counts}", byline(&m, s)))
                .text_size(px(scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
        .into_element()
}

fn section(model: Signal<Model>, title: Msg, items: &[String]) -> AnyElement {
    if items.is_empty() {
        return view().into_element();
    }
    let m = model.get_untracked();
    view()
        .gap_1()
        .child(text(m.word(title)).font_weight(FontWeight::SEMIBOLD))
        .children(items.iter().map(|i| {
            text(format!("• {i}"))
                .text_size(px(scale::text::BODY))
                .into_element()
        }))
        .into_element()
}

/// The open session's detail, in a sheet.
fn detail(model: Signal<Model>) -> impl IntoElement {
    ui::sheet(model)
        .title_fn(move || {
            model
                .get()
                .sessions
                .opened()
                .map(|s| s.goal.clone())
                .unwrap_or_default()
                .into()
        })
        .close_fn(move || model.get().word(Msg::CloseDetail).into())
        .open(move || {
            let m = model.get();
            m.sessions_page && m.sessions.opened().is_some()
        })
        .on_close(move || model.update(|m| m.sessions.open = None))
        .body(move || {
            dynamic(move || {
                let m = model.get();
                let Some(s) = m.sessions.opened().cloned() else {
                    return view().into_element();
                };
                let mut body = view()
                    .gap_4()
                    .child(
                        view()
                            .flex_row()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .child(status_badge(model, &s))
                            .child(
                                text(format!(
                                    "{} · {}",
                                    byline(&m, &s),
                                    m.word(Msg::SessionStarted)
                                        .replace("{time}", &date_time(s.started_ms))
                                ))
                                .flex_1()
                                .min_w(px(160.0))
                                .text_size(px(scale::text::CAPTION))
                                .text_color(move || palette(model).muted),
                            ),
                    )
                    .child(section(model, Msg::SessionDoing, &s.doing))
                    .child(section(model, Msg::SessionTodo, &s.todo))
                    .child(section(model, Msg::SessionDone, &s.done));
                if let Some(sum) = &s.summary {
                    body = body.child(section(
                        model,
                        Msg::SessionSummary,
                        std::slice::from_ref(sum),
                    ));
                }
                // The calls this session made, from the project's evidence.
                let calls: Vec<precept_ui_core::activity::Event> = m
                    .agents
                    .activity
                    .page
                    .as_ref()
                    .map(|p| {
                        p.events
                            .iter()
                            .filter(|e| e.session.as_deref() == Some(s.id.as_str()))
                            .take(30)
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default();
                if !calls.is_empty() {
                    let mut list = view().gap_1().child(
                        text(m.word(Msg::SessionEvidence)).font_weight(FontWeight::SEMIBOLD),
                    );
                    for e in &calls {
                        list = list.child(super::agents::activity_row(model, &m, e));
                    }
                    body = body.child(list);
                }
                let id = s.id.clone();
                let handoff_id = s.id.clone();
                let goal = s.goal.clone();
                body.child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            button(model, "")
                                .label_fn(word(model, Msg::SessionHandoff))
                                .variant(ButtonVariant::Primary)
                                .size(ButtonSize::Small)
                                .disabled(move || model.get().sessions.busy)
                                .on_press(move || {
                                    model.update(|m| {
                                        m.sessions.task = precept_ui_core::work::task_id(&goal);
                                        m.sessions.error = None;
                                        m.sessions.handoff = Some(handoff_id.clone());
                                    })
                                }),
                        )
                        .child(
                            button(model, "")
                                .label_fn(word(model, Msg::SessionDelete))
                                .variant(ButtonVariant::Danger)
                                .size(ButtonSize::Small)
                                .disabled(move || model.get().sessions.busy)
                                .on_press(move || {
                                    model.update(|m| m.sessions.delete = Some(id.clone()))
                                }),
                        ),
                )
                .into_element()
            })
            .into_element()
        })
}

/// The deletion confirmation.
fn delete_dialog(model: Signal<Model>) -> impl IntoElement {
    ui::alert_dialog(model, "")
        .icon(icons::TRASH, ui::DialogTone::Danger)
        .title_fn(word(model, Msg::SessionDeleteTitle))
        .description_fn(word(model, Msg::SessionDeleteBody))
        .cancel_fn(word(model, Msg::Cancel))
        .confirm_fn(word(model, Msg::SessionDeleteConfirm))
        .confirm_disabled(move || model.get().sessions.busy)
        .open(move || model.get().sessions.delete.is_some())
        .on_cancel(move || model.update(|m| m.sessions.delete = None))
        .on_confirm(move || delete(model))
}

/// The Sessions page (mounted by the shell, outside the per-change rebuild).
pub(crate) fn page(model: Signal<Model>) -> impl IntoElement {
    let state = computed(move || {
        let m = model.get();
        (m.sessions.list.clone(), m.sessions.error.clone(), m.locale)
    });
    view()
        .gap_6()
        .max_w(px(scale::layout::FORM))
        .child(
            surface(model)
                .child(
                    view()
                        .flex_row()
                        .items_start()
                        .gap_3()
                        .child(
                            text(word(model, Msg::SessionsIntro))
                                .flex_1()
                                .text_size(px(scale::text::CAPTION))
                                .text_color(move || palette(model).muted),
                        )
                        .child(
                            button(model, "")
                                .label_fn(word(model, Msg::RefreshList))
                                .size(ButtonSize::Small)
                                .variant(ButtonVariant::Ghost)
                                .icon(ui::icons::RESET)
                                .loading(move || model.get().sessions.busy)
                                .on_press(move || {
                                    if let Some(p) = model.get().selected_project.clone() {
                                        load(model, p.clone());
                                        super::agents::load_activity(model, p);
                                    }
                                }),
                        ),
                )
                .child(dynamic(move || {
                    let (list, error, _) = state.get();
                    let m = model.get_untracked();
                    if let Some(code) = error {
                        return text(code)
                            .text_size(px(scale::text::CAPTION))
                            .text_color(move || palette(model).error)
                            .into_element();
                    }
                    let Some(list) = list else {
                        return view().into_element();
                    };
                    if list.is_empty() {
                        return ui::empty(model, m.word(Msg::SessionsEmpty))
                            .description(m.word(Msg::SessionsEmptyHelp))
                            .icon(ui::icons::AGENTS)
                            .into_element();
                    }
                    view()
                        .children(list.iter().enumerate().map(|(i, s)| row(model, i, s)))
                        .into_element()
                })),
        )
        .child(super::agents::activity(model))
        .child(detail(model))
        // After the sheet, so the dialogs stack above it.
        .child(delete_dialog(model))
        .child(handoff_dialog(model))
}
