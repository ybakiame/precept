use super::actions::{
    available, commit_delete, delete_armed, error_msg, open_delete, refresh, review, send, submit,
};
use super::delete::delete_dialog;
use super::dialogs::{add_dialog, apply_dialog, unknown_banner};
use super::formatting::{field, literal};
use super::queue::{batch_bar, queue_tab};
use super::records::{page_toolbar, record_scope_tabs, record_sheet, records_list, writes_notice};
use super::*;

pub(crate) fn panel(model: Signal<Model>) -> impl IntoElement {
    let global = KeyOptions {
        when: nsh_core::keymap::When::Always,
        ..Default::default()
    };
    // The destructive tier's presentation state; the armed identity itself lives
    // in the shared model so every view enforces one guard.
    let delete_open = signal(false);
    let retype_ref = element_ref();
    let bind = |id: &'static str, handler: Box<dyn Fn()>| {
        crate::shortcuts::bind(model, id, global, handler)
    };
    for (key, action) in [
        ("review.suggest", "suggest"),
        ("review.confirm", "confirm"),
        ("review.reject", "reject"),
        ("review.deprecate", "deprecate"),
    ] {
        bind(
            key,
            Box::new(move || {
                if model.get().operator.open {
                    review(model, action);
                }
            }),
        );
    }
    bind(
        "review.delete",
        Box::new(move || {
            if model.get().operator.open {
                open_delete(model, delete_open);
            }
        }),
    );
    bind(
        "review.apply",
        Box::new(move || {
            if !model.get().operator.open {
                return;
            }
            // An open, exactly armed delete tier prepares then applies in one chord;
            // a blind or stale retype prepares nothing and applies nothing.
            if delete_open.get() && model.get().operator.delete_arm.is_some() {
                commit_delete(model, delete_open);
                if model.get().operator.review.is_some() {
                    submit(model);
                }
                return;
            }
            submit(model);
        }),
    );
    bind(
        "review.profile",
        Box::new(move || {
            if model.get().operator.open {
                refresh(model, "profile.info");
            }
        }),
    );
    bind(
        "review.records",
        Box::new(move || {
            if model.get().operator.open {
                model.update(|m| m.operator.pane = Pane::Preferences);
                refresh(model, "profile.preferences.list");
            }
        }),
    );
    // Windows' native Tab chain visits text fields; expose the existing exact
    // saved-Handoff read without depending on a button's platform traversal.
    bind(
        "review.handoff",
        Box::new(move || {
            let m = model.get();
            if m.operator.pane == Pane::Handoff && m.selected_project.is_some() && available(model)
            {
                refresh(model, "profile.handoff.get");
            }
        }),
    );
    bind(
        "review.retry",
        Box::new(move || {
            if model.get().operator.open {
                let mut request = None;
                model.update(|m| request = m.operator_retry());
                if let Some(r) = request {
                    send(model, r);
                }
            }
        }),
    );
    bind(
        "review.next",
        Box::new(move || {
            if model.get().operator.open && available(model) {
                model.update(|m| {
                    if let Some(page) = &m.operator.page {
                        let next = m
                            .operator
                            .selected_id
                            .and_then(|id| page.records.iter().position(|r| r.id == id))
                            .map(|n| (n + 1) % page.records.len().max(1))
                            .unwrap_or(0);
                        m.operator.selected_id = page.records.get(next).map(|r| r.id);
                        m.operator.inspect = None;
                        m.operator.inspect_revision = 0;
                        m.operator.digest_expanded = false;
                        m.operator.serial += 1;
                        m.operator.review = None;
                        // A selection change never carries an armed delete forward.
                        m.operator.delete_arm = None;
                    }
                });
            }
        }),
    );
    effect(move || {
        if model.get().operator.delete_arm.is_none() {
            delete_open.set(false);
        }
    });
    effect(move || {
        if std::env::var_os("PRECEPT_UI_PROBE").is_some() {
            let m = model.get();
            let o = &m.operator;
            if o.open {
                eprintln!(
                    "PRECEPT_NSH_OPERATOR selected={} grant={} busy={} review={} unknown={} rows={} record_revision={} receipt={} error={} handoff_revision={} review_tab={:?} queue={} inspect={} delete_armed={} digest_expanded={} inspection_current={} inspected_id={} inspected_revision={} peer_total={} delete_retype_chars={} delete_ready={}",
                    o.info.as_ref().is_some_and(|i| i.selected),
                    o.info.as_ref().is_some_and(|i| i.write_enabled),
                    o.busy,
                    o.review.is_some(),
                    o.outcome_unknown,
                    o.page.as_ref().map_or(0, |p| p.records.len()),
                    o.record().map_or(0, |r| r.revision),
                    o.receipt.as_ref().map_or(0, |r| r.commit_sequence),
                    o.error.as_deref().unwrap_or("none"),
                    o.handoff
                        .as_ref()
                        .and_then(|h| h.record.as_ref())
                        .map_or(0, |r| r.revision),
                    o.review_tab,
                    o.queue.as_ref().map_or(0, |q| q.records.len()),
                    o.inspect.as_ref().map_or(0, |i| i.decisions.len()),
                    o.delete_arm.is_some(),
                    o.digest_expanded,
                    m.operator_inspected(),
                    o.inspect.as_ref().map_or(0, |i| i.record.id),
                    o.inspect.as_ref().map_or(0, |i| i.record.revision),
                    o.inspect.as_ref().map_or(0, |i| i.peer_demotion_total),
                    o.delete_arm
                        .as_ref()
                        .map_or(0, |a| a.retype.chars().count()),
                    delete_armed(model),
                );
                eprintln!(
                    "PRECEPT_NSH_REVIEW global={} selected_id={} queue_after={} queue_first={} queue_last={} queue_sequence={} queue_baseline={} queue_stale={} inspected_status={} inspected_origin={:?} history={}",
                    o.global,
                    o.selected_id.unwrap_or(0),
                    o.queue_after_id,
                    o.queue
                        .as_ref()
                        .and_then(|q| q.records.first())
                        .map_or(0, |r| r.id),
                    o.queue
                        .as_ref()
                        .and_then(|q| q.records.last())
                        .map_or(0, |r| r.id),
                    o.queue.as_ref().map_or(0, |q| q.commit_sequence),
                    o.queue_watermark,
                    o.queue
                        .as_ref()
                        .is_some_and(|q| q.commit_sequence > o.queue_watermark),
                    o.inspect
                        .as_ref()
                        .map_or("none", |i| i.record.status.as_str()),
                    o.inspect.as_ref().map(|i| i.record.origin),
                    o.inspect.as_ref().map_or(0, |i| i.decisions.len()),
                );
            }
        }
    });
    // Display-none removes layout; visibility also removes native Win32 focus
    // stops. A concealed pane must not leave its retained fields/actions tabbable.
    let offline_view = dynamic(move || {
        let m = model.get();
        if matches!(m.connection.target, Target::Local | Target::Wsl) {
            return view().into_element();
        }
        ui::surface(model)
            .child(
                ui::empty(model, m.word(Msg::ReviewOfflineTitle))
                    .icon(icons::REVIEW)
                    .description(m.word(Msg::ReviewOfflineBody))
                    .action(
                        ui::button(model, "")
                            .label_fn(move || model.get().word(Msg::GoToRuntimeChecks).into())
                            .variant(ui::ButtonVariant::Primary)
                            .on_press(move || {
                                shell::go_setup(model, precept_ui_core::SetupTab::Connection)
                            }),
                    ),
            )
            .into_element()
    });
    let live_body = view()
        .gap_4()
        .style_when(
            move || !matches!(model.get().connection.target, Target::Local | Target::Wsl),
            |s| s.hidden().invisible(),
        )
        .child(record_scope_tabs(model))
        .child(writes_notice(model))
        .child(page_toolbar(model))
        .child(batch_bar(model))
        .child(
            view()
                .style_when(
                    move || model.get().operator.pane != Pane::Preferences,
                    |s| s.hidden().invisible(),
                )
                .child({
                    // What the queue or record list shows: it is rebuilt only
                    // when one of these changes, never by an unrelated update.
                    let shown = computed(move || {
                        let m = model.get();
                        (
                            m.operator.mode,
                            m.selected_project.is_none(),
                            m.operator.queue.clone(),
                            m.operator.queue_watermark,
                            m.operator.page.clone(),
                            m.operator.show_deprecated,
                            m.operator.selected_id,
                            m.locale,
                        )
                    });
                    dynamic(move || {
                        let _ = shown.get();
                        let m = model.get_untracked();
                        let mode = m.operator.mode;
                        if mode == ReviewMode::Project && m.selected_project.is_none() {
                            return ui::surface(model)
                                .child(
                                    ui::empty(model, m.word(Msg::ReviewNeedsProjectTitle))
                                        .icon(icons::section_path(
                                            precept_ui_core::Section::Projects,
                                        ))
                                        .description(m.word(Msg::ReviewNeedsProjectBody))
                                        .action(
                                            ui::button(model, "")
                                                .label_fn(move || {
                                                    model.get().word(Msg::GoToWorkspaces).into()
                                                })
                                                .variant(ui::ButtonVariant::Primary)
                                                .on_press(move || {
                                                    shell::go_setup(
                                                        model,
                                                        precept_ui_core::SetupTab::Workspaces,
                                                    )
                                                }),
                                        ),
                                )
                                .into_element();
                        }
                        untracked(|| {
                            if mode == ReviewMode::Inbox {
                                queue_tab(model, delete_open)
                            } else {
                                records_list(model, delete_open)
                            }
                        })
                    })
                }),
        )
        .child(
            view()
                .gap_2()
                .style_when(
                    move || {
                        model.get().operator.pane != Pane::Handoff
                            || model.get().operator.review.is_some()
                            || model.get().operator.outcome_unknown
                    },
                    |s| s.hidden().invisible(),
                )
                .child(field(
                    model,
                    Msg::OperatorTask,
                    |o| &o.task,
                    |o, v| {
                        o.task = v;
                        o.handoff = None;
                    },
                    120,
                ))
                .child(word_action(
                    model,
                    Msg::OperatorHandoff,
                    move || available(model) && model.get().selected_project.is_some(),
                    move || refresh(model, "profile.handoff.get"),
                ))
                .child(dynamic(move || {
                    let m = model.get();
                    let mut lines = vec![m.word(Msg::OperatorHistorical).into()];
                    let mut total = 0;
                    if let Some(h) = &m.operator.handoff {
                        let all = literal(&serde_json::to_string(&h.record).unwrap_or_default());
                        total = all.len().div_ceil(24);
                        lines.push(format!("{} / {}", m.operator.handoff_page + 1, total));
                        lines.extend(all.into_iter().skip(m.operator.handoff_page * 24).take(24));
                    }
                    view()
                        .gap_2()
                        .child(card(model, m.word(Msg::Handoff), lines))
                        .child(word_action(
                            model,
                            Msg::PreviousPage,
                            move || model.get().operator.handoff_page > 0,
                            move || {
                                model.update(|m| {
                                    m.operator.handoff_page =
                                        m.operator.handoff_page.saturating_sub(1)
                                })
                            },
                        ))
                        .child(word_action(
                            model,
                            Msg::NextPage,
                            move || model.get().operator.handoff_page + 1 < total,
                            move || model.update(|m| m.operator.handoff_page += 1),
                        ))
                })),
        )
        .child(unknown_banner(model));
    let content = view()
        .on_layout(move |_: &Bounds<Points>| {
            if model.get().operator.open && std::env::var_os("PRECEPT_UI_PROBE").is_some() {
                eprintln!("PRECEPT_NSH_LAYOUT_READY elapsed={:.6}", probe_elapsed());
            }
        })
        .gap_4()
        .flex_shrink_0()
        .max_w(px(ui::scale::layout::CONTENT))
        .child(offline_view)
        .child(live_body);
    // Toasts report what happened (applied receipts, refusals); the page shows
    // the resulting state itself.
    let feedback = computed(move || {
        let m = model.get();
        (
            m.operator.receipt.as_ref().map(|r| r.commit_sequence),
            m.operator.error.clone(),
            m.operator.outcome_unknown,
            // A deleted record leaves identity-only proof, never content.
            m.operator
                .receipt
                .as_ref()
                .is_some_and(|r| r.affected.iter().any(|a| a.deleted)),
        )
    });
    let reported = signal((None, None::<String>));
    effect(move || {
        let (receipt, error, unknown, deleted) = feedback.get();
        let (last_receipt, last_error) = untracked(|| reported.get());
        let m = untracked(|| model.get());
        if receipt.is_some() && receipt != last_receipt {
            ui::notify(
                ui::Tone::Success,
                format!(
                    "{} · commit {}",
                    m.word(if deleted {
                        Msg::ReviewDeleted
                    } else {
                        Msg::ReviewApplied
                    }),
                    receipt.unwrap_or_default()
                ),
            );
        }
        if let Some(e) = &error
            && error != last_error
            && !unknown
        {
            let wsl = m.connection.target == precept_ui_core::connection::Target::Wsl;
            ui::notify(
                ui::Tone::Error,
                format!("{e}: {}", tr(m.locale, error_msg(e, wsl))),
            );
        }
        reported.set((receipt, error));
    });
    // The list loads itself when the page is entered, its scope changes, a
    // change was applied (a receipt never leaves a stale page), a snapshot read
    // completed, or the Profile observation changed (writes turned on in
    // Profile change the binding: an old observation would refuse the list as
    // a stale generation). On first mount only when nothing was read yet.
    let entry = computed(move || {
        let m = model.get();
        let o = &m.operator;
        (
            o.open,
            o.mode,
            o.global,
            o.receipt.as_ref().map(|r| r.commit_sequence),
            m.epoch,
            m.phase == precept_ui_core::Phase::Loading,
            o.info
                .as_ref()
                .map(|i| (i.selected, i.write_enabled, i.binding.clone())),
        )
    });
    let last_entry = signal(None);
    let pending = signal(false);
    effect(move || {
        let next = entry.get();
        let previous = untracked(|| last_entry.get());
        last_entry.set(Some(next.clone()));
        let first_unread = previous.is_none() && untracked(|| model.get().operator.info.is_none());
        if next.0 && (first_unread || previous.is_some_and(|p| p != next)) {
            pending.set(true);
        }
    });
    let stage = computed(move || {
        let m = model.get();
        let o = &m.operator;
        // Never while a snapshot read is loading: its completion changes the
        // read guard, so a reply sent now would be discarded (and stay busy).
        let idle = o.open
            && o.pane == Pane::Preferences
            && m.phase != precept_ui_core::Phase::Loading
            && !o.busy
            && !o.outcome_unknown
            && o.review.is_none();
        (idle, o.info.as_ref().map(|i| i.selected), o.error.is_none())
    });
    effect(move || {
        let (idle, selected, clean) = stage.get();
        if !idle {
            return;
        }
        untracked(|| {
            let method = match selected {
                // Observe the Profile first (no retry loop over an error).
                None if clean => "profile.info",
                Some(true) if pending.get() => {
                    if model.get().operator.mode == ReviewMode::Inbox {
                        "profile.review.queue"
                    } else {
                        "profile.preferences.list"
                    }
                }
                _ => return,
            };
            let mut request = None;
            model.update(|m| request = m.operator_read(method));
            if let Some(request) = request {
                if method != "profile.info" {
                    pending.set(false);
                }
                send(model, request);
            }
        });
    });
    // The native modals belong to the visible workspace root, never to the
    // clipped/scrolled document that may place its insertion point off-screen.
    view()
        .flex_1()
        .style_when(
            move || !model.get().operator.open,
            |s| s.hidden().invisible(),
        )
        .child(scroll().flex_1().child(content))
        .child(record_sheet(model, delete_open))
        .child(apply_dialog(model, false))
        .child(apply_dialog(model, true))
        .child(add_dialog(model))
        .child(import::import_dialog(model))
        .child(delete_dialog(model, delete_open, retype_ref))
}
