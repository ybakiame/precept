use super::actions::{available, disarm, refresh, review, submit, write_grant};
use super::detail::detail_tab;
use super::history::history_tab;
use super::queue::{loaded_list, record_line};
use super::*;
use precept_ui_core::operator::PreferenceRecord;

/// The Rules page's two reviewed scopes; the pending queue has its own destination.
pub(super) fn record_scope_tabs(model: Signal<Model>) -> impl IntoElement {
    view()
        .style_when(
            move || model.get().operator.mode == ReviewMode::Inbox,
            |s| s.hidden().invisible(),
        )
        .child(
            ui::tabs(model)
                .add_fn(
                    move || model.get().word(Msg::ReviewTabPersonal).into(),
                    move || model.get().operator.mode == ReviewMode::Personal,
                    move || open_mode(model, ReviewMode::Personal),
                )
                .add_fn(
                    move || {
                        let m = model.get();
                        match m.selected_project.as_deref() {
                            Some(name) => format!(
                                "{} · {}",
                                m.word(Msg::ReviewTabConventions),
                                sanitize(name, 32)
                            )
                            .into(),
                            None => m.word(Msg::ReviewTabConventions).into(),
                        }
                    },
                    move || model.get().operator.mode == ReviewMode::Project,
                    move || open_mode(model, ReviewMode::Project),
                ),
        )
}

/// Writes and Profile live in Runtime checks; here only a short notice when
/// changes cannot be applied, with the one-click path there.
pub(super) fn writes_notice(model: Signal<Model>) -> impl IntoElement {
    let state = computed(move || {
        let m = model.get();
        (
            m.locale,
            m.operator
                .info
                .as_ref()
                .map(|i| (i.selected, i.write_enabled)),
        )
    });
    dynamic(move || {
        let (_, info) = state.get();
        let m = model.get_untracked();
        let key = match info {
            Some((false, _)) => Msg::OperatorUnselected,
            Some((true, false)) => Msg::WritesOffNotice,
            _ => return view().into_element(),
        };
        view()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(ui::badge(model, m.word(key)).variant(ui::BadgeVariant::Warning))
            .child(
                ui::button(model, "")
                    .label_fn(move || model.get().word(Msg::OpenProfileStep).into())
                    .size(ui::ButtonSize::Small)
                    .on_press(move || shell::go_setup(model, precept_ui_core::SetupTab::Profile)),
            )
            .into_element()
    })
}

/// The page's commands: the scope switch (To review), Add preference (Personal
/// and Project tabs), and Refresh.
pub(super) fn page_toolbar(model: Signal<Model>) -> impl IntoElement {
    let set_scope = move |global: bool| {
        model.update(|m| {
            if m.operator.global != global {
                m.operator.invalidate();
                m.operator.batch.clear();
                m.operator.global = global;
                m.operator.after_id = 0;
                m.operator.queue_after_id = 0;
            }
        })
    };
    let inbox_tabs = ui::tabs(model)
        .add_fn(
            move || model.get().word(Msg::ScopePersonal).into(),
            move || model.get().operator.global,
            move || set_scope(true),
        )
        .add_fn(
            move || {
                let m = model.get();
                match m.selected_project.as_deref() {
                    Some(name) => {
                        format!("{} · {}", m.word(Msg::ScopeThisProject), sanitize(name, 32)).into()
                    }
                    None => m.word(Msg::ScopeThisProject).into(),
                }
            },
            move || !model.get().operator.global,
            move || set_scope(false),
        );
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(
            view()
                .style_when(
                    move || model.get().operator.mode != ReviewMode::Inbox,
                    |s| s.hidden().invisible(),
                )
                .child(inbox_tabs),
        )
        .child(
            view()
                .style_when(
                    move || model.get().operator.mode == ReviewMode::Inbox,
                    |s| s.hidden(),
                )
                .child(
                    ui::button(model, "")
                        .label_fn(move || {
                            let m = model.get();
                            m.word(match m.operator.mode {
                                ReviewMode::Project => Msg::AddConvention,
                                _ => Msg::AddPreference,
                            })
                            .into()
                        })
                        .icon(icons::PLUS)
                        .variant(ui::ButtonVariant::Primary)
                        .disabled(move || {
                            let m = model.get();
                            !available(model)
                                || !write_grant(model)
                                || (m.operator.mode == ReviewMode::Project
                                    && m.selected_project.is_none())
                        })
                        .on_press(move || {
                            model.update(|m| {
                                m.operator.start_draft();
                                m.operator.adding = true;
                            });
                        }),
                ),
        )
        .child(
            ui::button(model, "")
                .label_fn(move || model.get().word(Msg::ImportOpen).into())
                .disabled(move || {
                    !available(model)
                        || !write_grant(model)
                        || model.get().selected_project.is_none()
                })
                .on_press(move || import::open_import(model)),
        )
        .child(
            view()
                .style_when(
                    move || model.get().operator.mode == ReviewMode::Inbox,
                    |s| s.hidden(),
                )
                .child(ui::switch(
                    model,
                    move || model.get().word(Msg::ShowDeprecated).into(),
                    move || model.get().operator.show_deprecated,
                    || false,
                    move |show| {
                        model.update(|m| {
                            let close = !show
                                && m.operator
                                    .record()
                                    .is_some_and(|r| r.status == "deprecated");
                            m.operator.show_deprecated = show;
                            if close {
                                m.operator.selected_id = None;
                                m.operator.inspect = None;
                                m.operator.inspect_revision = 0;
                            }
                            m.operator.review = None;
                            m.operator.delete_arm = None;
                        });
                    },
                )),
        )
        .child(view().flex_1())
        .child(
            ui::button(model, "")
                .label_fn(move || model.get().word(Msg::RefreshList).into())
                .size(ui::ButtonSize::Small)
                .variant(ui::ButtonVariant::Ghost)
                .icon(icons::REFRESH)
                .disabled(move || !available(model))
                .on_press(move || {
                    model.update(|m| {
                        m.operator.page = None;
                        m.operator.queue = None;
                        m.operator.error = None;
                        // The list is read again after the Profile, even when
                        // the Profile did not change (else it stays loading).
                        m.operator.follow_up = Some(if m.operator.mode == ReviewMode::Inbox {
                            "profile.review.queue"
                        } else {
                            "profile.preferences.list"
                        });
                    });
                    refresh(model, "profile.info");
                }),
        )
}

/// The scoped pages' list: records matching the status filter; a row opens the
/// record's sheet (and reads the exact record for it).
pub(super) fn records_list(model: Signal<Model>, delete_open: Signal<bool>) -> AnyElement {
    let m = model.get();
    let body = view().gap_1().flex_shrink_0();
    let Some(page) = &m.operator.page else {
        return body
            .child(ui::empty(model, m.word(Msg::RecordsLoading)))
            .into_element();
    };
    let records = std::rc::Rc::new(
        page.records
            .iter()
            .filter(|r| m.operator.show_deprecated || r.status != "deprecated")
            .cloned()
            .collect::<Vec<_>>(),
    );
    let mut body = body;
    if records.is_empty() {
        let unfiltered_empty = page.records.is_empty();
        body = body.child(
            ui::empty(
                model,
                m.word(if unfiltered_empty {
                    Msg::OperatorEmpty
                } else {
                    Msg::RecordsFilteredEmpty
                }),
            )
            .description(m.word(if unfiltered_empty {
                Msg::RecordsEmptyHelp
            } else {
                Msg::DeprecatedFilterHelp
            })),
        );
        if page.next_after_id.is_none() {
            return body.into_element();
        }
    }
    let list = loaded_list(
        model,
        records.iter().map(|r| r.id).collect(),
        {
            let records = records.clone();
            move |index| record_row(model, delete_open, &records[index])
        },
        page.next_after_id.is_some(),
        move || {
            let mut go = false;
            model.update(|m| {
                if let Some(id) = m.operator.page.as_ref().and_then(|p| p.next_after_id) {
                    m.operator.after_id = id;
                    go = true;
                }
                m.operator.review = None;
            });
            if go {
                refresh(model, "profile.preferences.list");
            }
        },
    );
    body.child(list).into_element()
}
/// One record of the scope: id, key, state and revision (pressing it opens the
/// exact record), the first line of its value, its tags and origin.
pub(super) fn record_row(
    model: Signal<Model>,
    delete_open: Signal<bool>,
    r: &PreferenceRecord,
) -> AnyElement {
    let m = model.get();
    let id = r.id;
    let variant = match r.status.as_str() {
        "confirmed" => ui::BadgeVariant::Success,
        "suggested" => ui::BadgeVariant::Warning,
        _ => ui::BadgeVariant::Neutral,
    };
    let label = format!(
        "#{} · {} · {} · r{}",
        id,
        sanitize(&r.key, 120),
        row_status(&m, &r.status),
        r.revision
    );
    view()
        .py_1()
        .child(record_line(
            model,
            r,
            label,
            &format!("#{id} · r{}", r.revision),
            move || open_record(model, delete_open, id),
            ui::badge(model, status_word(&m, &r.status))
                .variant(variant)
                .into_element(),
        ))
        .into_element()
}
pub(super) fn status_word(m: &Model, status: &str) -> String {
    match status {
        "confirmed" => m.word(Msg::StatusConfirmed).into(),
        "suggested" => m.word(Msg::StatusSuggested).into(),
        "deprecated" => m.word(Msg::StatusDeprecated).into(),
        "rejected" => m.word(Msg::StatusRejected).into(),
        other => sanitize(other, 40),
    }
}
/// The status inside a row's label (lowercase in English).
pub(super) fn row_status(m: &Model, status: &str) -> String {
    match status {
        "confirmed" => m.word(Msg::RowConfirmed).into(),
        "suggested" => m.word(Msg::RowSuggested).into(),
        "rejected" => m.word(Msg::RowRejected).into(),
        "deprecated" => m.word(Msg::RowDeprecated).into(),
        other => sanitize(other, 40),
    }
}
/// Selects a record, opens its sheet and reads the exact record for it.
pub(super) fn open_record(model: Signal<Model>, delete_open: Signal<bool>, id: i64) {
    disarm(model, delete_open);
    model.update(|m| {
        m.operator.selected_id = Some(id);
        m.operator.inspect = None;
        m.operator.inspect_revision = 0;
        m.operator.digest_expanded = false;
        m.operator.review_tab = ReviewTab::Detail;
        m.operator.review = None;
        m.operator.serial += 1;
    });
    refresh(model, "profile.review.inspect");
}

/// The selected record beside the list: its exact detail and decision history,
/// and its decisions (each prepares, then the Apply dialog commits it).
/// A confirmed rule's switch: off keeps the reviewed record but agents stop
/// receiving it. Applied at once (revision-checked), no new review.
pub(super) fn delivered_switch(model: Signal<Model>) -> impl IntoElement {
    let record = move || model.get().operator.record().cloned();
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .p_3()
        .rounded(px(ui::scale::radius::CONTAINER))
        .bg(move || palette(model).inset)
        .style_when(
            move || record().is_none_or(|r| r.status != "confirmed"),
            |s| s.hidden(),
        )
        .child(
            view()
                .flex_1()
                .gap_0p5()
                .child(
                    text(move || model.get().word(Msg::DeliveredSwitch))
                        .font_weight(FontWeight::MEDIUM),
                )
                .child(
                    text(move || model.get().word(Msg::DeliveredHelp))
                        .text_size(px(ui::scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                ),
        )
        .child(ui::switch(
            model,
            move || model.get().word(Msg::DeliveredSwitch).into(),
            move || record().is_some_and(|r| r.enabled),
            move || {
                let m = model.get();
                m.operator.busy || !m.operator_inspected() || !write_grant(model)
            },
            move |on| {
                review(model, if on { "switch_on" } else { "switch_off" });
                submit(model);
            },
        ))
}
/// The note the next decision on this record keeps (outside the rebuilt
/// detail, so typing keeps its focus).
pub(super) fn decision_note(model: Signal<Model>) -> impl IntoElement {
    view()
        .style_when(
            move || {
                model
                    .get()
                    .operator
                    .record()
                    .is_none_or(|r| !matches!(r.status.as_str(), "suggested" | "confirmed"))
            },
            |s| s.hidden(),
        )
        .gap_1()
        .child(ui::field(
            model,
            move || model.get().word(Msg::DecisionNote).into(),
            ui::input(model, "")
                .label_fn(move || model.get().word(Msg::DecisionNote).into())
                .value(move || model.get().operator.note.clone())
                .disabled(move || model.get().operator.busy || model.get().operator.outcome_unknown)
                .on_change(move |v: &str| {
                    model.update(|m| {
                        m.operator.review = None;
                        m.operator.note = v.chars().take(600).collect();
                    })
                }),
        ))
        .child(
            text(move || model.get().word(Msg::DecisionNoteHelp))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
}
pub(super) fn record_sheet(model: Signal<Model>, delete_open: Signal<bool>) -> impl IntoElement {
    ui::sheet(model)
        .title_fn(move || {
            let m = model.get();
            m.operator
                .record()
                .map(|r| format!("#{} · {}", r.id, sanitize(&r.key, 80)))
                .unwrap_or_default()
                .into()
        })
        .close_fn(move || model.get().word(Msg::CloseDetail).into())
        .open(move || {
            let m = model.get();
            m.operator.open && m.operator.selected_id.is_some() && m.operator.record().is_some()
        })
        .on_close(move || {
            disarm(model, delete_open);
            model.update(|m| {
                m.operator.selected_id = None;
                m.operator.inspect = None;
                m.operator.inspect_revision = 0;
                m.operator.review = None;
            })
        })
        .body(move || {
            view()
                .gap_4()
                .child(delivered_switch(model))
                .child(decision_note(model))
                .child(dynamic(move || detail_tab(model, delete_open)))
                .child(dynamic(move || history_tab(model)))
                .into_element()
        })
}
