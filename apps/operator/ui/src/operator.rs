//! Minimal native operator panel; all business transitions live in shared::Model.
use super::*;
use acp_operator_model::{
    connection::Target,
    i18n::Msg,
    operator::{Pane, Review, Scope},
};
use components::{action, card, word_action};
use nsh_core::element::input;
use tokens::palette;

pub(super) fn open(model: Signal<Model>) {
    model.update(|m| {
        m.operator.open = true;
        m.sessions = false;
        m.settings = false;
        m.connection_editor = false;
        m.operator.review = None;
    });
    refresh(model, "profile.info");
}
fn available(model: Signal<Model>) -> bool {
    let m = model.get();
    m.operator.open
        && m.connection.target == Target::Local
        && m.connection.validate()
        && !m.operator.busy
        && !m.operator.outcome_unknown
}
fn refresh(model: Signal<Model>, method: &str) {
    let mut request = None;
    model.update(|m| request = m.operator_read(method));
    if let Some(request) = request {
        send(model, request);
    }
}
fn send(model: Signal<Model>, request: Review) {
    let connection = model.get().connection;
    spawn(async move {
        #[cfg(feature = "ipc")]
        let result = {
            let request = request.clone();
            blocking(move || {
                acp_operator_model::native::Inputs::from_connection(&connection)?
                    .operator_call(&request)
            })
            .await
            .unwrap_or(Err("Unavailable"))
        };
        #[cfg(not(feature = "ipc"))]
        let result = {
            let _ = connection;
            Err("Unauthorized")
        };
        model.update(|m| {
            m.complete_operator(&request, result);
        });
        // Explicit Refresh updates effective context after inspection. Do not
        // destroy retained uncertain request identity through an automatic read.
    });
}
fn review(model: Signal<Model>, action: &str) {
    if !available(model) {
        return;
    }
    #[cfg(feature = "ipc")]
    let id = acp_operator_model::operator::operation_id();
    #[cfg(not(feature = "ipc"))]
    let id: Result<String, &str> = Err("Unauthorized");
    if let Ok(id) = id {
        model.update(|m| {
            m.operator_review(action, &id);
        });
    }
}
fn submit(model: Signal<Model>) {
    let mut request = None;
    model.update(|m| request = m.operator_confirm());
    if let Some(request) = request {
        send(model, request);
    }
}
fn field(
    model: Signal<Model>,
    key: Msg,
    get: fn(&acp_operator_model::operator::Operator) -> &str,
    set: fn(&mut acp_operator_model::operator::Operator, String),
    max: usize,
) -> impl IntoElement {
    let reference = element_ref();
    let shortcut = match key {
        Msg::ProfilePath => "alt+p",
        Msg::OperatorKey => "alt+k",
        Msg::OperatorValue => "alt+w",
        Msg::OperatorSource => "alt+f",
        _ => "alt+t",
    };
    on_key_with(
        shortcut,
        KeyOptions {
            when: nsh_core::keymap::When::Always,
            ..Default::default()
        },
        move |_| {
            if model.get().operator.open {
                model.update(|m| {
                    m.operator.pane = match key {
                        Msg::ProfilePath => Pane::Profile,
                        Msg::OperatorTask => Pane::Handoff,
                        _ => Pane::Preferences,
                    };
                    m.operator.review = None;
                });
                reference.focus();
            }
        },
    );
    effect(move || {
        if !model.get().operator.open {
            reference.blur();
        }
    });
    view()
        .gap_1()
        .child(text(move || model.get().word(key)))
        .child(
            input()
                .element_ref(&reference)
                .label(move || model.get().word(key))
                .value(move || get(&model.get().operator).to_owned())
                .border_2()
                .border_color(move || palette(model).border)
                .bg(move || palette(model).panel)
                .text_color(move || palette(model).text)
                .focus(|s| s.border_color(palette(model).primary))
                .disabled(move || model.get().operator.busy || model.get().operator.outcome_unknown)
                .on_change(move |v: &str| {
                    model.update(|m| {
                        m.operator.review = None;
                        m.operator.serial += 1;
                        set(&mut m.operator, v.chars().take(max + 1).collect());
                    })
                }),
        )
}
/// Preserve all record characters for review; make control/bidi content visible.
fn literal(s: &str) -> Vec<String> {
    let escaped: String = s
        .chars()
        .flat_map(|c| {
            if c.is_control() || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}') {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect();
    escaped
        .chars()
        .collect::<Vec<_>>()
        .chunks(120)
        .map(|c| c.iter().collect())
        .collect()
}
pub(super) fn panel(model: Signal<Model>) -> impl IntoElement {
    let global = KeyOptions {
        when: nsh_core::keymap::When::Always,
        ..Default::default()
    };
    for (key, action) in [
        ("alt+shift+u", "create"),
        ("alt+shift+m", "select"),
        ("alt+shift+e", "enable"),
        ("alt+shift+x", "disable"),
        ("alt+shift+s", "suggest"),
        ("alt+shift+y", "confirm"),
        ("alt+shift+n", "reject"),
    ] {
        on_key_with(key, global, move |_| {
            if model.get().operator.open {
                review(model, action);
            }
        });
    }
    on_key_with("alt+shift+v", global, move |_| {
        if model.get().operator.open {
            submit(model);
        }
    });
    on_key_with("alt+shift+r", global, move |_| {
        if model.get().operator.open {
            refresh(model, "profile.info");
        }
    });
    on_key_with("alt+shift+l", global, move |_| {
        if model.get().operator.open {
            model.update(|m| m.operator.pane = Pane::Preferences);
            refresh(model, "profile.preferences.list");
        }
    });
    // Windows' native Tab chain visits text fields; expose the existing exact
    // saved-Handoff read without depending on a button's platform traversal.
    on_key_with("alt+shift+h", global, move |_| {
        let m = model.get();
        if m.operator.pane == Pane::Handoff && m.selected_project.is_some() && available(model) {
            refresh(model, "profile.handoff.get");
        }
    });
    on_key_with("alt+shift+z", global, move |_| {
        if model.get().operator.open {
            let mut request = None;
            model.update(|m| request = m.operator_retry());
            if let Some(r) = request {
                send(model, r);
            }
        }
    });
    on_key_with("alt+shift+i", global, move |_| {
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
                    m.operator.serial += 1;
                    m.operator.review = None;
                }
            });
        }
    });
    let summary = computed(move || {
        let m = model.get();
        (m.locale, m.operator)
    });
    let rows = computed(move || {
        let m = model.get();
        (
            m.locale,
            m.operator.page,
            m.operator.selected_id,
            m.operator.info,
            m.operator.review,
            m.operator.serial,
        )
    });
    effect(move || {
        #[cfg(not(target_arch = "wasm32"))]
        if std::env::var_os("ACP_UI_PROBE").is_some() {
            let m = model.get();
            let o = &m.operator;
            if o.open {
                eprintln!(
                    "ACP_NSH_OPERATOR selected={} grant={} busy={} review={} unknown={} rows={} record_revision={} receipt={} error={} handoff_revision={}",
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
                        .map_or(0, |r| r.revision)
                );
            }
        }
    });
    view()
        .on_layout(move |_: &Bounds<Points>| {
            #[cfg(not(target_arch = "wasm32"))]
            if model.get().operator.open && std::env::var_os("ACP_UI_PROBE").is_some() {
                eprintln!("ACP_NSH_LAYOUT_READY elapsed={:.6}", probe_elapsed());
            }
        })
        .gap_3()
        .flex_shrink_0()
        .child(text(move || model.get().word(Msg::OperatorHelp)))
        .child(text(move || model.get().word(Msg::OperatorKeys)))
        .child(
            view().flex_row().flex_wrap().gap_2().children(
                [
                    (Pane::Profile, Msg::ProfileOperator),
                    (Pane::Preferences, Msg::OperatorPreferences),
                    (Pane::Handoff, Msg::OperatorSavedHandoff),
                ]
                .into_iter()
                .map(move |(pane, key)| {
                    word_action(
                        model,
                        key,
                        move || !model.get().operator.busy,
                        move || {
                            model.update(|m| {
                                m.operator.pane = pane;
                                m.operator.review = None;
                                m.operator.serial += 1;
                            })
                        },
                    )
                }),
            ),
        )
        .child(word_action(
            model,
            Msg::OperatorRefresh,
            move || available(model),
            move || refresh(model, "profile.info"),
        ))
        .child(dynamic(move || {
            let (_, o) = summary.get();
            let m = model.get_untracked();
            let mut lines = Vec::new();
            if let Some(info) = &o.info {
                lines.push(
                    m.word(if info.selected {
                        Msg::OperatorSelected
                    } else {
                        Msg::OperatorUnselected
                    })
                    .into(),
                );
                lines.push(
                    m.word(if info.write_enabled {
                        Msg::OperatorGrantOn
                    } else {
                        Msg::OperatorGrantOff
                    })
                    .into(),
                );
                if let Some(b) = &info.binding {
                    lines.push(format!(
                        "Profile {} · generation {} · epoch {}",
                        b.profile_id, b.generation, b.owner_epoch
                    ));
                }
            }
            if o.busy {
                lines.push(m.word(Msg::OperatorBusy).into());
            }
            if let Some(e) = o.error {
                lines.push(format!("{}: {e}", m.word(Msg::ProfileOperator)));
            }
            if let Some(r) = o.receipt {
                lines.push(format!(
                    "{} {} · commit {}",
                    m.word(Msg::OperatorLastReceipt),
                    r.operation_id,
                    r.commit_sequence
                ));
            }
            if let Some(p) = o.pending.and_then(|r| r.mutation) {
                lines.push(format!(
                    "Operation {} · epoch {}",
                    p.operation_id, p.binding.owner_epoch
                ));
            }
            card(model, m.word(Msg::ProfileOperator), lines)
        }))
        .child(
            view()
                .gap_2()
                .style_when(
                    move || {
                        model.get().operator.pane != Pane::Profile
                            || model.get().operator.review.is_some()
                            || model.get().operator.outcome_unknown
                    },
                    |s| s.hidden(),
                )
                .child(field(
                    model,
                    Msg::ProfilePath,
                    |o| &o.path,
                    |o, v| o.path = v,
                    32768,
                ))
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(word_action(
                            model,
                            Msg::ProfileCreate,
                            move || {
                                available(model)
                                    && model.get().operator.info.is_some_and(|i| !i.selected)
                            },
                            move || review(model, "create"),
                        ))
                        .child(word_action(
                            model,
                            Msg::ProfileSelect,
                            move || {
                                available(model)
                                    && model.get().operator.info.is_some_and(|i| !i.selected)
                            },
                            move || review(model, "select"),
                        ))
                        .child(word_action(
                            model,
                            Msg::WriteEnable,
                            move || {
                                available(model)
                                    && model
                                        .get()
                                        .operator
                                        .info
                                        .is_some_and(|i| i.selected && !i.write_enabled)
                            },
                            move || review(model, "enable"),
                        ))
                        .child(word_action(
                            model,
                            Msg::WriteDisable,
                            move || {
                                available(model)
                                    && model.get().operator.info.is_some_and(|i| i.write_enabled)
                            },
                            move || review(model, "disable"),
                        )),
                ),
        )
        .child(
            view()
                .gap_2()
                .style_when(
                    move || {
                        model.get().operator.pane != Pane::Preferences
                            || model.get().operator.review.is_some()
                            || model.get().operator.outcome_unknown
                    },
                    |s| s.hidden(),
                )
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(word_action(
                            model,
                            Msg::OperatorGlobal,
                            move || available(model),
                            move || {
                                model.update(|m| {
                                    m.operator.invalidate();
                                    m.operator.global = true;
                                    m.operator.after_id = 0;
                                })
                            },
                        ))
                        .child(word_action(
                            model,
                            Msg::OperatorProject,
                            move || available(model) && model.get().selected_project.is_some(),
                            move || {
                                model.update(|m| {
                                    m.operator.invalidate();
                                    m.operator.global = false;
                                    m.operator.after_id = 0;
                                })
                            },
                        ))
                        .child(text(move || {
                            let m = model.get();
                            if m.operator.global {
                                m.word(Msg::OperatorGlobal).into()
                            } else {
                                format!(
                                    "{}: {}",
                                    m.word(Msg::OperatorProject),
                                    m.selected_project.as_deref().unwrap_or("—")
                                )
                            }
                        })),
                )
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(word_action(
                            model,
                            Msg::OperatorList,
                            move || {
                                available(model)
                                    && model
                                        .get()
                                        .operator
                                        .info
                                        .as_ref()
                                        .is_some_and(|i| i.selected)
                            },
                            move || refresh(model, "profile.preferences.list"),
                        ))
                        .child(word_action(
                            model,
                            Msg::OperatorFirst,
                            move || available(model) && model.get().operator.after_id > 0,
                            move || {
                                model.update(|m| {
                                    m.operator.after_id = 0;
                                    m.operator.review = None;
                                    m.operator.serial += 1;
                                });
                                refresh(model, "profile.preferences.list");
                            },
                        ))
                        .child(word_action(
                            model,
                            Msg::OperatorNext,
                            move || {
                                available(model)
                                    && model
                                        .get()
                                        .operator
                                        .page
                                        .as_ref()
                                        .is_some_and(|p| p.next_after_id.is_some())
                            },
                            move || {
                                model.update(|m| {
                                    if let Some(id) =
                                        m.operator.page.as_ref().and_then(|p| p.next_after_id)
                                    {
                                        m.operator.after_id = id;
                                    }
                                    m.operator.review = None;
                                });
                                refresh(model, "profile.preferences.list");
                            },
                        )),
                )
                .child(dynamic(move || {
                    let (locale, page, selected, info, _, serial) = rows.get();
                    let mut body = view().gap_2().flex_shrink_0();
                    let Some(page) = page else {
                        return body;
                    };
                    if page.records.is_empty() {
                        body = body.child(text(acp_operator_model::i18n::tr(
                            locale,
                            Msg::OperatorEmpty,
                        )));
                    }
                    for r in page.records {
                        let id = r.id;
                        let record = r.clone();
                        let record2 = r.clone();
                        body = body.child(action(
                            model,
                            format!("#{} · {} · {} · r{}", id, r.key, r.status, r.revision),
                            move || available(model),
                            move || {
                                model.update(|m| {
                                    m.operator.selected_id = Some(id);
                                    m.operator.review = None;
                                    m.operator.serial += 1;
                                })
                            },
                        ));
                        if selected == Some(id) {
                            let mut lines = vec![format!(
                                "Scope: {}",
                                match &r.scope {
                                    Scope::Global => "global".into(),
                                    Scope::Project { name } => format!("project {name}"),
                                }
                            )];
                            for (label, value) in
                                [("key", &r.key), ("value", &r.value), ("source", &r.source)]
                            {
                                lines.push(format!("{label}:"));
                                lines.extend(literal(value));
                            }
                            body = body.child(card(
                                model,
                                acp_operator_model::i18n::tr(locale, Msg::OperatorReview),
                                lines,
                            ));
                            let write = info.as_ref().is_some_and(|i| i.write_enabled)
                                && r.status == "suggested";
                            body = body.child(
                                view()
                                    .flex_row()
                                    .flex_wrap()
                                    .gap_2()
                                    .child(word_action(
                                        model,
                                        Msg::OperatorConfirm,
                                        move || available(model) && write,
                                        move || {
                                            let m = model.get();
                                            if m.operator.serial == serial
                                                && m.operator.record() == Some(&record)
                                            {
                                                review(model, "confirm");
                                            }
                                        },
                                    ))
                                    .child(word_action(
                                        model,
                                        Msg::OperatorReject,
                                        move || available(model) && write,
                                        move || {
                                            let m = model.get();
                                            if m.operator.serial == serial
                                                && m.operator.record() == Some(&record2)
                                            {
                                                review(model, "reject");
                                            }
                                        },
                                    )),
                            );
                        }
                    }
                    body
                }))
                .child(field(
                    model,
                    Msg::OperatorKey,
                    |o| &o.key,
                    |o, v| o.key = v,
                    120,
                ))
                .child(field(
                    model,
                    Msg::OperatorValue,
                    |o| &o.value,
                    |o, v| o.value = v,
                    4096,
                ))
                .child(field(
                    model,
                    Msg::OperatorSource,
                    |o| &o.source,
                    |o, v| o.source = v,
                    512,
                ))
                .child(word_action(
                    model,
                    Msg::OperatorSuggest,
                    move || {
                        available(model)
                            && model.get().operator.info.is_some_and(|i| i.write_enabled)
                    },
                    move || review(model, "suggest"),
                )),
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
                    |s| s.hidden(),
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
        .child(dynamic(move || {
            let m = model.get();
            let mut body = view().gap_2().flex_shrink_0();
            if let Some(review) = &m.operator.review {
                let mut lines = vec![
                    m.word(Msg::OperatorPending).into(),
                    review.method.clone(),
                    m.word(Msg::OperatorPeerWarning).into(),
                ];
                if let Some(record) = &review.record {
                    lines.extend(literal(&serde_json::to_string(record).unwrap_or_default()));
                }
                if let Some(mutation) = &review.mutation {
                    lines.extend(literal(
                        &serde_json::to_string(mutation).unwrap_or_default(),
                    ));
                } else {
                    lines.extend(literal(&review.params.to_string()));
                }
                body = body
                    .child(card(model, m.word(Msg::OperatorReview), lines))
                    .child(word_action(
                        model,
                        Msg::OperatorCommit,
                        move || available(model),
                        move || submit(model),
                    ))
                    .child(word_action(
                        model,
                        Msg::OperatorCancel,
                        || true,
                        move || model.update(|m| m.operator.review = None),
                    ));
            }
            if m.operator.outcome_unknown {
                body = body
                    .child(text(m.word(Msg::OperatorUnknown)))
                    .child(word_action(
                        model,
                        Msg::OperatorRetry,
                        move || {
                            let m = model.get();
                            !m.operator.busy
                                && m.operator
                                    .pending
                                    .as_ref()
                                    .is_some_and(|p| m.operator_current(p))
                        },
                        move || {
                            let mut request = None;
                            model.update(|m| request = m.operator_retry());
                            if let Some(r) = request {
                                send(model, r);
                            }
                        },
                    ))
                    .child(word_action(
                        model,
                        Msg::OperatorAbandon,
                        move || !model.get().operator.busy,
                        move || {
                            model.update(|m| {
                                m.operator.outcome_unknown = false;
                                m.operator.review = None;
                                m.operator.invalidate();
                                m.operator.error = Some("OutcomeUnknown".into());
                            })
                        },
                    ));
            }
            body.child(text(m.word(Msg::OperatorCliGate)))
        }))
}
