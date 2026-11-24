use super::workflow::{prepare, ready, writes_on};
use crate::*;
use precept_ui_core::{i18n::Msg, operator::PersonalSyncMerge, personal_sync::*};
use tokens::palette;
use ui::{BadgeVariant, ButtonSize, ButtonVariant, badge, button, scale, surface};

fn word(model: Signal<Model>, key: Msg) -> impl Fn() -> SharedString + 'static {
    move || model.get().word(key).into()
}

fn caption(model: Signal<Model>, value: impl Fn() -> String + 'static) -> impl IntoElement {
    text(value)
        .text_size(px(scale::text::CAPTION))
        .text_color(move || palette(model).muted)
}

fn notice(model: Signal<Model>, key: Msg) -> impl IntoElement {
    view()
        .p_3()
        .rounded(px(scale::radius::CONTAINER))
        .border_1()
        .border_color(move || palette(model).hairline)
        .bg(move || palette(model).panel)
        .child(text(move || model.get().word(key)).text_size(px(scale::text::CAPTION)))
}

fn status(model: Signal<Model>) -> impl IntoElement {
    let initialized = move || {
        model
            .get()
            .sync
            .status
            .as_ref()
            .is_some_and(|s| s.initialized)
    };
    surface(model)
        .title_fn(word(model, Msg::SyncStatusTitle))
        .child(
            view()
                .gap_3()
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(dynamic(move || {
                            let m = model.get();
                            let on = initialized();
                            let key = if !m.sync.status_read {
                                Msg::SyncUnread
                            } else if on {
                                Msg::SyncInitialized
                            } else {
                                Msg::SyncNotInitialized
                            };
                            badge(model, m.word(key)).variant(if on && m.sync.status_read {
                                BadgeVariant::Success
                            } else {
                                BadgeVariant::Neutral
                            })
                        }))
                        .child(
                            button(model, "")
                                .label_fn(word(model, Msg::SyncRefresh))
                                .icon(icons::REFRESH)
                                .size(ButtonSize::Small)
                                .loading(move || model.get().sync.busy)
                                .on_press(move || super::workflow::refresh(model)),
                        ),
                )
                .child(caption(model, move || {
                    let m = model.get();
                    let Some(status) = &m.sync.status else {
                        return String::new();
                    };
                    let mut lines = Vec::new();
                    if let Some(id) = &status.device_id {
                        lines.push(format!("{}: {}", m.word(Msg::SyncDeviceId), id));
                    }
                    if let Some(id) = &status.owner_fingerprint {
                        lines.push(format!("{}: {}", m.word(Msg::SyncOwnerFingerprint), id));
                    }
                    lines.push(format!(
                        "{}: {}",
                        m.word(Msg::SyncTrackedKeys),
                        status.tracked_keys
                    ));
                    lines.push(format!(
                        "{}: {}",
                        m.word(Msg::SyncTombstones),
                        status.tombstones
                    ));
                    lines.push(format!(
                        "{}: {}",
                        m.word(Msg::SyncAppliedEntries),
                        status.applied_entries
                    ));
                    if let Some(id) = &status.pending_operation {
                        lines.push(m.word(Msg::SyncPendingOperation).replace("{id}", id));
                    }
                    lines.join("\n")
                }))
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .style_when(initialized, |s| s.hidden())
                        .child(
                            button(model, "")
                                .label_fn(word(model, Msg::SyncCreateIdentity))
                                .variant(ButtonVariant::Primary)
                                .disabled(move || !ready(model) || model.get().sync.busy)
                                .on_press(move || super::workflow::initialize(model)),
                        )
                        .child(
                            button(model, "")
                                .label_fn(word(model, Msg::SyncPair))
                                .disabled(move || !ready(model) || model.get().sync.busy)
                                .on_press(move || {
                                    model.update(|m| {
                                        m.sync.dialog = Dialog::Pair(Default::default())
                                    })
                                }),
                        ),
                )
                .child(
                    view()
                        .gap_3()
                        .style_when(move || !initialized(), |s| s.hidden())
                        .child(
                            view()
                                .flex_row()
                                .flex_wrap()
                                .gap_2()
                                .child(
                                    button(model, "")
                                        .label_fn(word(model, Msg::SyncInvitation))
                                        .icon(icons::FILE)
                                        .on_press(move || super::workflow::invitation(model)),
                                )
                                .child(
                                    button(model, "")
                                        .label_fn(word(model, Msg::SyncExport))
                                        .variant(ButtonVariant::Primary)
                                        .disabled(move || !ready(model) || model.get().sync.busy)
                                        .on_press(move || {
                                            model.update(|m| {
                                                m.sync.dialog = Dialog::Export(Default::default())
                                            })
                                        }),
                                )
                                .child(
                                    button(model, "")
                                        .label_fn(word(model, Msg::SyncImport))
                                        .disabled(move || !ready(model) || model.get().sync.busy)
                                        .on_press(move || {
                                            model.update(|m| {
                                                m.sync.dialog = Dialog::Import(Default::default())
                                            })
                                        }),
                                ),
                        )
                        .child(
                            text(move || model.get().word(Msg::SyncTrustedDevices))
                                .font_weight(FontWeight::MEDIUM),
                        )
                        .child(dynamic(move || {
                            let m = model.get();
                            let Some(status) = m.sync.status.clone() else {
                                return ui::empty(model, m.word(Msg::SyncNoDevices)).into_element();
                            };
                            if status.trusted_devices.is_empty() {
                                return ui::empty(model, m.word(Msg::SyncNoDevices)).into_element();
                            }
                            view()
                                .children(status.trusted_devices.iter().enumerate().map(
                                    |(index, device)| {
                                        let revoked = device.revoked;
                                        let id = device.invitation.device_id.clone();
                                        view()
                                            .flex_row()
                                            .flex_wrap()
                                            .items_center()
                                            .gap_2()
                                            .py_2()
                                            .style_when(move || index > 0, |s| s.border_t_1())
                                            .border_color(move || palette(model).hairline)
                                            .child(
                                                text(id.clone())
                                                    .font_family("monospace")
                                                    .text_size(px(scale::text::CAPTION))
                                                    .flex_1()
                                                    .min_w_0(),
                                            )
                                            .child(
                                                badge(
                                                    model,
                                                    m.word(if device.revoked {
                                                        Msg::SyncRevoked
                                                    } else {
                                                        Msg::SyncTrusted
                                                    }),
                                                )
                                                .variant(if device.revoked {
                                                    BadgeVariant::Warning
                                                } else {
                                                    BadgeVariant::Success
                                                }),
                                            )
                                            .child(
                                                view()
                                                    .style_when(move || revoked, |s| s.hidden())
                                                    .child(
                                                        button(model, "")
                                                            .label_fn(word(model, Msg::SyncRevoke))
                                                            .size(ButtonSize::Small)
                                                            .on_press(move || {
                                                                model.update(|m| {
                                                                    m.sync.dialog = Dialog::Confirm(
                                                                        Confirm::Revoke(id.clone()),
                                                                    )
                                                                })
                                                            }),
                                                    ),
                                            )
                                    },
                                ))
                                .into_element()
                        })),
                )
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .style_when(move || !initialized(), |s| s.hidden())
                        .child(
                            text(move || model.get().word(Msg::SyncAdvanced))
                                .text_size(px(scale::text::CAPTION))
                                .text_color(move || palette(model).muted),
                        )
                        .child(
                            button(model, "")
                                .label_fn(word(model, Msg::SyncRotate))
                                .size(ButtonSize::Small)
                                .on_press(move || {
                                    model.update(|m| {
                                        m.sync.dialog = Dialog::Confirm(Confirm::Rotate)
                                    })
                                }),
                        )
                        .child(
                            button(model, "")
                                .label_fn(word(model, Msg::SyncReset))
                                .size(ButtonSize::Small)
                                .variant(ButtonVariant::Danger)
                                .on_press(move || {
                                    model
                                        .update(|m| m.sync.dialog = Dialog::Confirm(Confirm::Reset))
                                }),
                        ),
                ),
        )
}

fn merge_label(item: &precept_ui_core::operator::PersonalSyncPreviewItem) -> Msg {
    if item.applied {
        Msg::SyncAlreadyApplied
    } else {
        match item.merge {
            PersonalSyncMerge::Accept => Msg::SyncMergeAccept,
            PersonalSyncMerge::Stale => Msg::SyncMergeStale,
            PersonalSyncMerge::Noop => Msg::SyncMergeNoop,
            PersonalSyncMerge::Conflict => Msg::SyncMergeConflict,
        }
    }
}

fn preview_row(
    model: Signal<Model>,
    item: &precept_ui_core::operator::PersonalSyncPreviewItem,
) -> AnyElement {
    let item = item.clone();
    let key = item.key.clone();
    let checkbox_key = key.clone();
    let selected_key = key.clone();
    let applied = item.applied;
    let conflict = item.merge == PersonalSyncMerge::Conflict && !item.applied;
    let value = model
        .get_untracked()
        .sync
        .preview
        .as_ref()
        .and_then(|state| {
            state
                .preview
                .bundle
                .entries
                .iter()
                .find(|entry| entry.content.key == item.key)
                .map(|entry| entry.content.value.clone())
        })
        .unwrap_or_default();
    let variant = if conflict {
        BadgeVariant::Warning
    } else if item.applied {
        BadgeVariant::Neutral
    } else {
        BadgeVariant::Info
    };
    let m = model.get_untracked();
    let merge_word = m.word(merge_label(&item));
    let change_key = key.clone();
    let mut row = view()
        .flex_row()
        .items_start()
        .gap_3()
        .py_3()
        .border_t_1()
        .border_color(move || palette(model).hairline)
        .child(if conflict {
            view().w(px(20.0)).into_element()
        } else {
            ui::checkbox(
                model,
                move || {
                    model
                        .get()
                        .word(Msg::SyncInclude)
                        .replace("{key}", &checkbox_key)
                        .into()
                },
                move || {
                    model
                        .get()
                        .sync
                        .preview
                        .as_ref()
                        .is_some_and(|state| state.selected(&selected_key))
                },
                move || applied || model.get().sync.busy,
                move |on| {
                    model.update(|m| {
                        if let Some(state) = m.sync.preview.as_mut() {
                            state.set_selected(&change_key, on);
                        }
                    })
                },
            )
            .into_element()
        })
        .child(
            view()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            text(key.clone())
                                .font_weight(FontWeight::MEDIUM)
                                .font_family("monospace"),
                        )
                        .child(badge(model, merge_word).variant(variant)),
                )
                .child(
                    text(precept_ui_core::presentation::sanitize(&value, 400))
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                ),
        );
    if conflict {
        row = row.child(conflict_choice(model, &key));
    }
    row.into_element()
}

fn conflict_choice(model: Signal<Model>, key: &str) -> AnyElement {
    let key = key.to_owned();
    let group_key = key.clone();
    let manual = key.clone();
    let choices = [
        (ConflictChoice::Skip, Msg::SyncConflictSkip),
        (ConflictChoice::Accept, Msg::SyncConflictAccept),
        (ConflictChoice::KeepLocal, Msg::SyncConflictKeepLocal),
        (ConflictChoice::Manual, Msg::SyncConflictManual),
    ];
    let mut group = ui::radio_group(model, move || {
        model
            .get()
            .word(Msg::SyncMergeConflict)
            .replace("{key}", &group_key)
            .into()
    });
    for (choice, label) in choices {
        let key = key.clone();
        let checked_key = key.clone();
        group = group.option_fn(
            move || model.get().word(label).into(),
            move || {
                model
                    .get()
                    .sync
                    .preview
                    .as_ref()
                    .is_some_and(|state| state.choice(&checked_key) == choice)
            },
            move || {
                model.update(|m| {
                    if let Some(state) = m.sync.preview.as_mut() {
                        state.set_choice(&key, choice);
                    }
                })
            },
        );
    }
    let group: AnyElement = group.into_element();
    let chosen_key = manual.clone();
    let value_key = manual.clone();
    let value_change_key = manual.clone();
    let tags_key = manual.clone();
    let tags_change_key = manual.clone();
    let enabled_key = manual.clone();
    let enabled_change_key = manual;
    view()
        .w(px(240.0))
        .gap_2()
        .child(group)
        .child(
            view()
                .gap_2()
                .style_when(
                    move || {
                        model
                            .get()
                            .sync
                            .preview
                            .as_ref()
                            .is_none_or(|state| state.choice(&chosen_key) != ConflictChoice::Manual)
                    },
                    |s| s.hidden(),
                )
                .child(
                    ui::input(model, "")
                        .label_fn(word(model, Msg::SyncManualValue))
                        .multiline(4)
                        .max_length(4096)
                        .value(move || {
                            model
                                .get()
                                .sync
                                .preview
                                .as_ref()
                                .map_or_else(String::new, |state| {
                                    state.manual_draft(&value_key).value
                                })
                        })
                        .on_change(move |value| {
                            model.update(|m| {
                                if let Some(state) = m.sync.preview.as_mut() {
                                    let mut draft = state.manual_draft(&value_change_key);
                                    draft.value = value.into();
                                    state.set_manual(&value_change_key, draft);
                                }
                            })
                        }),
                )
                .child(
                    ui::input(model, "")
                        .label_fn(word(model, Msg::SyncManualTags))
                        .max_length(256)
                        .value(move || {
                            model
                                .get()
                                .sync
                                .preview
                                .as_ref()
                                .map_or_else(String::new, |state| {
                                    state.manual_draft(&tags_key).tags
                                })
                        })
                        .on_change(move |value| {
                            model.update(|m| {
                                if let Some(state) = m.sync.preview.as_mut() {
                                    let mut draft = state.manual_draft(&tags_change_key);
                                    draft.tags = value.into();
                                    state.set_manual(&tags_change_key, draft);
                                }
                            })
                        }),
                )
                .child(ui::switch(
                    model,
                    word(model, Msg::SyncManualEnabled),
                    move || {
                        model
                            .get()
                            .sync
                            .preview
                            .as_ref()
                            .is_some_and(|state| state.manual_draft(&enabled_key).enabled)
                    },
                    move || model.get().sync.busy,
                    move |on| {
                        model.update(|m| {
                            if let Some(state) = m.sync.preview.as_mut() {
                                let mut draft = state.manual_draft(&enabled_change_key);
                                draft.enabled = on;
                                state.set_manual(&enabled_change_key, draft);
                            }
                        })
                    },
                )),
        )
        .into_element()
}

fn preview(model: Signal<Model>) -> impl IntoElement {
    let reveal = ui::motion::reveal();
    let has_preview = computed(move || model.get().sync.preview.is_some());
    let rows = computed(move || {
        let m = model.get();
        let Some(state) = &m.sync.preview else {
            return Vec::new();
        };
        state
            .preview
            .items
            .iter()
            .map(|item| {
                let value = state
                    .preview
                    .bundle
                    .entries
                    .iter()
                    .find(|entry| entry.content.key == item.key)
                    .map_or_else(String::new, |entry| entry.content.value.clone());
                (item.clone(), value)
            })
            .collect::<Vec<_>>()
    });
    surface(model)
        .title_fn(word(model, Msg::SyncPreviewTitle))
        .child(
            view()
                .bind_motion(&reveal)
                .gap_3()
                .child(dynamic(move || {
                    if !has_preview.get() {
                        return ui::empty(model, model.get_untracked().word(Msg::SyncNoPreview))
                            .into_element();
                    }
                    text(move || {
                        let m = model.get();
                        m.word(Msg::SyncSelectedCount).replace(
                            "{n}",
                            &m.sync
                                .preview
                                .as_ref()
                                .map_or(0, |state| state.selected_count())
                                .to_string(),
                        )
                    })
                    .font_weight(FontWeight::MEDIUM)
                    .into_element()
                }))
                .child(dynamic(move || {
                    view()
                        .children(
                            rows.get()
                                .into_iter()
                                .map(|(item, _)| preview_row(model, &item)),
                        )
                        .into_element()
                }))
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            button(model, "")
                                .label_fn(word(model, Msg::SyncApply))
                                .variant(ButtonVariant::Primary)
                                .disabled(move || {
                                    model
                                        .get()
                                        .sync
                                        .preview
                                        .as_ref()
                                        .is_none_or(|state| state.selections().is_none())
                                })
                                .on_press(move || prepare(model)),
                        )
                        .child(
                            button(model, "")
                                .label_fn(word(model, Msg::Cancel))
                                .on_press(move || model.update(|m| m.sync.discard_preview())),
                        ),
                ),
        )
}

fn outcome(model: Signal<Model>) -> impl IntoElement {
    dynamic(move || {
        let m = model.get();
        let Some(apply) = &m.sync.apply else {
            return view().into_element();
        };
        let mut body = view().gap_2();
        if apply.uncertain {
            body = body
                .child(text(m.word(Msg::SyncUncertain)).text_color(move || palette(model).warning))
                .child(
                    button(model, "")
                        .label_fn(word(model, Msg::SyncRetry))
                        .variant(ButtonVariant::Primary)
                        .disabled(move || model.get().sync.busy)
                        .on_press(move || super::workflow::apply(model, true)),
                )
                .child(
                    button(model, "")
                        .label_fn(word(model, Msg::SyncAbort))
                        .disabled(move || model.get().sync.busy)
                        .on_press(move || super::workflow::abort(model)),
                );
        } else if let Some(result) = &apply.result {
            body = body.child(
                text(
                    m.word(Msg::SyncApplySucceeded)
                        .replace("{applied}", &result.applied.to_string())
                        .replace("{changed}", &result.changed.to_string()),
                )
                .text_color(move || palette(model).success),
            );
        } else if let Some(abort) = &apply.abort {
            let message = if abort.aborted {
                m.word(Msg::SyncAbortDiscarded).to_string()
            } else {
                m.word(Msg::SyncAbortSettled).replace(
                    "{n}",
                    &abort
                        .settled_commit
                        .map_or_else(|| "unknown".into(), |n| n.to_string()),
                )
            };
            body = body.child(text(message).text_color(move || palette(model).success));
        } else if let Some(error) = &apply.error {
            body = body.child(
                text(m.word(Msg::SyncApplyRefused).replace(
                    "{error}",
                    &precept_ui_core::presentation::sanitize(error, 64),
                ))
                .text_color(move || palette(model).error),
            );
        }
        body.into_element()
    })
}

pub(crate) fn page(model: Signal<Model>) -> impl IntoElement {
    view()
        .max_w(px(scale::layout::CONTENT))
        .gap_4()
        .child(caption(model, move || {
            model.get().word(Msg::SyncIntro).into()
        }))
        .child(
            view()
                .style_when(move || model.get().live, |s| s.hidden())
                .child(notice(model, Msg::SyncConnectFirst)),
        )
        .child(
            view()
                .style_when(move || model.get().live, |s| s.hidden())
                .child(notice(model, Msg::SyncNotLive)),
        )
        .child(
            view()
                .style_when(
                    move || {
                        model
                            .get()
                            .profile_info
                            .as_ref()
                            .is_some_and(|info| info.selected)
                    },
                    |s| s.hidden(),
                )
                .child(notice(model, Msg::SyncNoProfile)),
        )
        .child(
            view()
                .gap_2()
                .style_when(move || writes_on(model), |s| s.hidden())
                .child(notice(model, Msg::SyncWritesOff))
                .child(
                    button(model, "")
                        .label_fn(word(model, Msg::SyncEnableWrites))
                        .size(ButtonSize::Small)
                        .disabled(move || !ready(model) || model.get().profile_setup.busy)
                        .loading(move || model.get().profile_setup.busy)
                        .on_press(move || crate::features::profile::enable_writes(model, |_| {})),
                ),
        )
        .child(dynamic(move || {
            let m = model.get();
            if let Some(error) = &m.sync.error {
                return view()
                    .child(
                        text(m.word(Msg::SyncError).replace(
                            "{error}",
                            &precept_ui_core::presentation::sanitize(error, 64),
                        ))
                        .text_color(move || palette(model).error),
                    )
                    .into_element();
            }
            view().into_element()
        }))
        .child(status(model))
        .child(preview(model))
        .child(outcome(model))
}

pub(crate) fn dialogs(model: Signal<Model>) -> impl IntoElement {
    super::dialogs::all(model)
}
