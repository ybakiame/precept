//! Memory: the selected project's memory, as agents keep it (environment,
//! cautions, lessons, attempts), heaviest first within each kind, with why
//! each note weighs what it does. The operator pins, edits (a correction
//! kept in history), restores, deletes or clears notes.
use crate::*;
use precept_ui_core::{
    activity::date_time,
    i18n::Msg,
    memory::{Entry, Kind, author, why},
};
use tokens::palette;
use ui::{BadgeVariant, ButtonSize, ButtonVariant, badge, button, scale, surface};

fn word(model: Signal<Model>, key: Msg) -> impl Fn() -> SharedString + 'static {
    move || model.get().word(key).into()
}

/// Reads the project's memory.
pub(crate) fn load(model: Signal<Model>, project: String) {
    model.update(|m| {
        let s = &mut m.memory;
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
        "memories.list",
        serde_json::json!({ "project": project }),
        move |_, m, r| {
            if m.memory.project.as_deref() != Some(project.as_str()) {
                return;
            }
            m.memory.busy = false;
            match r.and_then(precept_ui_core::memory::parse) {
                Ok(list) => m.memory.list = Some(list),
                Err(code) => m.memory.error = Some(code.into()),
            }
        },
    );
}

/// Keeps the page aligned with the sidebar's current project while shown.
pub(crate) fn watch(model: Signal<Model>) {
    let ready = computed(move || {
        let m = model.get();
        (
            m.memory_page && m.live && m.data.is_some(),
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
            untracked(|| load(model, project));
        }
    });
}

/// One operator call on the project's memory; the list is read again after.
fn change(
    model: Signal<Model>,
    method: &'static str,
    params: serde_json::Value,
    done: impl FnOnce(&mut Model) + 'static,
) {
    let Some(project) = model.get().memory.project.clone() else {
        return;
    };
    let mut params = params;
    params["project"] = project.clone().into();
    model.update(|m| {
        m.memory.busy = true;
        m.memory.error = None;
    });
    super::profile::call(model, method, params, move |model, m, r| {
        m.memory.busy = false;
        match r {
            Ok(_) => done(m),
            Err("Conflict") => m.memory.error = Some(m.word(Msg::MemoryTitleTaken).into()),
            Err(code) => m.memory.error = Some(code.into()),
        }
        spawn(async move { load(model, project) });
    });
}

fn set_pin(model: Signal<Model>, id: String, pinned: bool) {
    change(
        model,
        "memories.edit",
        serde_json::json!({ "id": id, "pinned": pinned }),
        |_| {},
    );
}

fn save_edit(model: Signal<Model>) {
    let m = model.get();
    let Some(id) = m.memory.edit.clone() else {
        return;
    };
    let (title, body) = (
        m.memory.title.trim().to_owned(),
        m.memory.body.trim().to_owned(),
    );
    change(
        model,
        "memories.edit",
        serde_json::json!({ "id": id, "title": title, "body": body }),
        |m| {
            m.memory.edit = None;
            ui::notify(ui::Tone::Success, m.word(Msg::ToastMemorySaved));
        },
    );
}

fn restore(model: Signal<Model>, id: String, index: usize) {
    change(
        model,
        "memories.restore",
        serde_json::json!({ "id": id, "index": index }),
        |m| ui::notify(ui::Tone::Success, m.word(Msg::ToastMemorySaved)),
    );
}

fn delete(model: Signal<Model>) {
    let Some(id) = model.get().memory.delete.clone() else {
        return;
    };
    change(
        model,
        "memories.delete",
        serde_json::json!({ "id": id }),
        |m| {
            m.memory.delete = None;
            m.memory.open = None;
            ui::notify(ui::Tone::Success, m.word(Msg::ToastMemoryDeleted));
        },
    );
}

fn clear(model: Signal<Model>) {
    change(model, "memories.clear", serde_json::json!({}), |m| {
        m.memory.clear = false;
        m.memory.open = None;
        ui::notify(ui::Tone::Success, m.word(Msg::ToastMemoryCleared));
    });
}

/// Opens Review on this project's conventions with a new draft taken from
/// the note: its title as the key, its text as the value, its kind as the
/// tag. Saving files an ordinary suggestion to confirm; the note stays.
fn propose_rule(model: Signal<Model>, e: &Entry) {
    crate::shell::go_review(model, precept_ui_core::operator::ReviewMode::Project);
    let (key, value, tag) = (
        precept_ui_core::memory::rule_key(&e.title),
        e.body.clone(),
        e.kind.tag().to_owned(),
    );
    model.update(|m| {
        m.memory.open = None;
        m.operator.start_draft();
        m.operator.key = key;
        m.operator.value = value;
        m.operator.tags = vec![tag];
        m.operator.adding = true;
    });
}

fn caption(model: Signal<Model>, t: String) -> AnyElement {
    text(t)
        .text_size(px(scale::text::CAPTION))
        .text_color(move || palette(model).muted)
        .into_element()
}

/// The entry's state badges: pinned, changed often, fading, edited by you.
fn badges(model: Signal<Model>, m: &Model, e: &Entry) -> AnyElement {
    let mut row = view().flex_row().flex_wrap().gap_1();
    for (on, msg, variant) in [
        (e.pinned, Msg::MemoryPinned, BadgeVariant::Info),
        (e.unstable, Msg::MemoryUnstable, BadgeVariant::Warning),
        (e.fading, Msg::MemoryFading, BadgeVariant::Neutral),
        (
            e.edited_by_operator,
            Msg::MemoryEditedByYou,
            BadgeVariant::Neutral,
        ),
        (
            e.proposed.is_some(),
            Msg::MemoryProposed,
            BadgeVariant::Info,
        ),
    ] {
        if on {
            row = row.child(badge(model, m.word(msg)).variant(variant));
        }
    }
    row.into_element()
}

fn byline(m: &Model, e: &Entry) -> String {
    m.word(Msg::MemoryByline)
        .replace("{by}", &author(m.locale, e.by.as_deref()))
        .replace("{time}", &date_time(e.updated_ms))
}

/// One note, the whole row pressable (it opens the detail): its title, its
/// badges, its first line, who and when, and its weight with the reasons.
/// One line of the virtualized memory list.
enum Item {
    Heading(String),
    Note(Entry),
}

fn row(model: Signal<Model>, e: &Entry) -> AnyElement {
    let m = model.get_untracked();
    let id = e.id.clone();
    let first: String = e
        .body
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(160)
        .collect();
    view()
        .role(AccessibilityRole::Button)
        .label(m.word(Msg::MemoryOpenNamed).replace("{title}", &e.title))
        .on_press(move || model.update(|m| m.memory.open = Some(id.clone())))
        .gap_1()
        .px_2()
        .py_2()
        .rounded(px(scale::radius::CONTROL))
        .bg(move || palette(model).panel)
        .hover(move |s| {
            s.bg(ui::tokens::mix(
                palette(model).panel,
                palette(model).text,
                0.06,
            ))
        })
        .focus(move |s| {
            s.ring(px(2.0))
                .ring_color(palette(model).primary.opacity(0.55))
        })
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(text(e.title.clone()).font_weight(FontWeight::SEMIBOLD))
                .child(badges(model, &m, e)),
        )
        .child(
            text(first)
                .text_size(px(scale::text::BODY))
                .text_color(move || palette(model).text),
        )
        .child(caption(
            model,
            format!("{} · {}", byline(&m, e), why(m.locale, e)),
        ))
        .into_element()
}

/// The open note's detail, in a sheet.
fn detail(model: Signal<Model>) -> impl IntoElement {
    ui::sheet(model)
        .title_fn(move || {
            model
                .get()
                .memory
                .opened()
                .map(|e| e.title.clone())
                .unwrap_or_default()
                .into()
        })
        .close_fn(move || model.get().word(Msg::CloseDetail).into())
        .open(move || {
            let m = model.get();
            m.memory_page && m.memory.opened().is_some()
        })
        .on_close(move || model.update(|m| m.memory.open = None))
        .body(move || {
            // Rebuilt when the open note or the language changes only.
            let shown = computed(move || {
                let m = model.get();
                (m.memory.opened().cloned(), m.locale)
            });
            dynamic(move || {
                let _ = shown.get();
                let m = model.get_untracked();
                let Some(e) = m.memory.opened().cloned() else {
                    return view().into_element();
                };
                let pin_id = e.id.clone();
                let edit = e.clone();
                let delete_id = e.id.clone();
                let propose = e.clone();
                let mut body = view()
                    .gap_4()
                    .child(
                        view()
                            .gap_1()
                            .child(badges(model, &m, &e))
                            .child(caption(
                                model,
                                format!("{} · {}", m.word(e.kind.heading()), byline(&m, &e)),
                            ))
                            .child(caption(model, why(m.locale, &e))),
                    )
                    .child(ui::markdown_view(model, e.body.clone()))
                    .child(
                        view()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(
                                text(word(model, Msg::MemoryPin))
                                    .flex_1()
                                    .text_size(px(scale::text::BODY)),
                            )
                            .child(ui::switch(
                                model,
                                word(model, Msg::MemoryPin),
                                move || model.get().memory.opened().is_some_and(|e| e.pinned),
                                move || model.get().memory.busy,
                                move |on| set_pin(model, pin_id.clone(), on),
                            )),
                    )
                    .child(
                        view()
                            .flex_row()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                button(model, "")
                                    .label_fn(word(model, Msg::MemoryEdit))
                                    .size(ButtonSize::Small)
                                    .disabled(move || model.get().memory.busy)
                                    .on_press(move || {
                                        let e = edit.clone();
                                        model.update(|m| {
                                            m.memory.title = e.title.clone();
                                            m.memory.body = e.body.clone();
                                            m.memory.error = None;
                                            m.memory.edit = Some(e.id.clone());
                                        })
                                    }),
                            )
                            .child(
                                button(model, "")
                                    .label_fn(word(model, Msg::MemoryPropose))
                                    .size(ButtonSize::Small)
                                    .disabled(move || model.get().memory.busy)
                                    .on_press(move || propose_rule(model, &propose)),
                            )
                            .child(
                                button(model, "")
                                    .label_fn(word(model, Msg::MemoryDelete))
                                    .variant(ButtonVariant::Danger)
                                    .size(ButtonSize::Small)
                                    .disabled(move || model.get().memory.busy)
                                    .on_press(move || {
                                        model.update(|m| m.memory.delete = Some(delete_id.clone()))
                                    }),
                            ),
                    );
                if !e.history.is_empty() {
                    let mut list = view()
                        .gap_3()
                        .child(text(m.word(Msg::MemoryHistory)).font_weight(FontWeight::SEMIBOLD));
                    for (i, h) in e.history.iter().enumerate() {
                        let id = e.id.clone();
                        let spoken = m
                            .word(Msg::MemoryRestoreNamed)
                            .replace("{n}", &(i + 1).to_string());
                        list = list.child(
                            view()
                                .gap_1()
                                .child(caption(
                                    model,
                                    m.word(Msg::MemoryEarlier)
                                        .replace("{by}", &author(m.locale, h.by.as_deref()))
                                        .replace("{time}", &date_time(h.until_ms)),
                                ))
                                .child(ui::text_diff(model, h.body.clone(), e.body.clone()))
                                .child(
                                    view().flex_row().child(
                                        button(model, "")
                                            .label_fn(word(model, Msg::MemoryRestore))
                                            .spoken(move || spoken.clone().into())
                                            .size(ButtonSize::Small)
                                            .variant(ButtonVariant::Ghost)
                                            .icon(ui::icons::UNDO)
                                            .disabled(move || model.get().memory.busy)
                                            .on_press(move || restore(model, id.clone(), i)),
                                    ),
                                ),
                        );
                    }
                    body = body.child(list);
                }
                body.into_element()
            })
            .into_element()
        })
}

/// The edit: title and text.
fn edit_dialog(model: Signal<Model>) -> impl IntoElement {
    ui::dialog(model, "")
        .icon(icons::MEMORY, ui::DialogTone::Accent)
        .title_fn(word(model, Msg::MemoryEditTitle))
        .description_fn(word(model, Msg::MemoryEditBody))
        .content_fn(move || {
            view()
                .w_full()
                .gap_3()
                .child(ui::field(
                    model,
                    word(model, Msg::MemoryTitleField),
                    ui::input(model, "")
                        .label_fn(word(model, Msg::MemoryTitleField))
                        .value(move || model.get().memory.title.clone())
                        .on_change(move |v: &str| {
                            model.update(|m| m.memory.title = v.chars().take(120).collect())
                        }),
                ))
                .child(ui::field(
                    model,
                    word(model, Msg::MemoryBodyField),
                    ui::input(model, "")
                        .label_fn(word(model, Msg::MemoryBodyField))
                        .multiline(8)
                        .value(move || model.get().memory.body.clone())
                        .on_change(move |v: &str| {
                            model.update(|m| m.memory.body = v.chars().take(2000).collect())
                        }),
                ))
                .child(
                    text(move || model.get().memory.error.clone().unwrap_or_default())
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).error),
                )
                .into_element()
        })
        .cancel_fn(word(model, Msg::Cancel))
        .confirm_fn(word(model, Msg::MemorySave))
        .confirm_disabled(move || {
            let m = model.get();
            m.memory.busy || m.memory.title.trim().is_empty() || m.memory.body.trim().is_empty()
        })
        .open(move || model.get().memory.edit.is_some())
        .on_cancel(move || {
            model.update(|m| {
                m.memory.edit = None;
                m.memory.error = None;
            })
        })
        .on_confirm(move || save_edit(model))
}

fn delete_dialog(model: Signal<Model>) -> impl IntoElement {
    ui::alert_dialog(model, "")
        .icon(icons::TRASH, ui::DialogTone::Danger)
        .title_fn(word(model, Msg::MemoryDeleteTitle))
        .description_fn(word(model, Msg::MemoryDeleteBody))
        .cancel_fn(word(model, Msg::Cancel))
        .confirm_fn(word(model, Msg::MemoryDeleteConfirm))
        .confirm_disabled(move || model.get().memory.busy)
        .open(move || model.get().memory.delete.is_some())
        .on_cancel(move || model.update(|m| m.memory.delete = None))
        .on_confirm(move || delete(model))
}

fn clear_dialog(model: Signal<Model>) -> impl IntoElement {
    ui::alert_dialog(model, "")
        .icon(icons::TRASH, ui::DialogTone::Danger)
        .title_fn(word(model, Msg::MemoryClearTitle))
        .description_fn(word(model, Msg::MemoryClearBody))
        .cancel_fn(word(model, Msg::Cancel))
        .confirm_fn(word(model, Msg::MemoryClearConfirm))
        .confirm_disabled(move || model.get().memory.busy)
        .open(move || model.get().memory.clear)
        .on_cancel(move || model.update(|m| m.memory.clear = false))
        .on_confirm(move || clear(model))
}

/// The Memory page (mounted by the shell, outside the per-change rebuild).
pub(crate) fn page(model: Signal<Model>) -> impl IntoElement {
    let state = computed(move || {
        let m = model.get();
        (m.memory.list.clone(), m.memory.error.clone(), m.locale)
    });
    view()
        .gap_6()
        .max_w(px(scale::layout::CONTENT))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    text(move || {
                        let m = model.get();
                        let list = m.memory.list.clone().unwrap_or_default();
                        Kind::ALL
                            .iter()
                            .map(|k| {
                                format!(
                                    "{} {}",
                                    m.word(k.heading()),
                                    list.iter().filter(|e| e.kind == *k).count()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(" · ")
                    })
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
                        .loading(move || model.get().memory.busy)
                        .on_press(move || {
                            if let Some(p) = model.get().selected_project.clone() {
                                load(model, p);
                            }
                        }),
                )
                .child(
                    button(model, "")
                        .label_fn(word(model, Msg::MemoryClear))
                        .size(ButtonSize::Small)
                        .variant(ButtonVariant::Ghost)
                        .icon(ui::icons::TRASH)
                        .disabled(move || {
                            let m = model.get();
                            m.memory.busy || m.memory.list.as_ref().is_none_or(Vec::is_empty)
                        })
                        .on_press(move || model.update(|m| m.memory.clear = true)),
                ),
        )
        .child(dynamic(move || {
            let (list, error, _) = state.get();
            let m = model.get_untracked();
            if let Some(code) = error
                && m.memory.edit.is_none()
            {
                return text(code)
                    .text_size(px(scale::text::CAPTION))
                    .text_color(move || palette(model).error)
                    .into_element();
            }
            let Some(list) = list else {
                return view().into_element();
            };
            if list.is_empty() {
                return surface(model)
                    .child(
                        ui::empty(model, m.word(Msg::MemoryEmpty))
                            .description(m.word(Msg::MemoryEmptyHelp))
                            .icon(icons::MEMORY),
                    )
                    .into_element();
            }
            // One virtualized list (up to 300 notes): each kind's heading,
            // then its notes, heaviest first; only visible rows are built.
            let mut items: Vec<Item> = Vec::new();
            for kind in Kind::ALL {
                let entries: Vec<&Entry> = list.iter().filter(|e| e.kind == kind).collect();
                if entries.is_empty() {
                    continue;
                }
                items.push(Item::Heading(format!(
                    "{} · {}",
                    m.word(kind.heading()),
                    entries.len()
                )));
                items.extend(entries.into_iter().map(|e| Item::Note(e.clone())));
            }
            let items = std::rc::Rc::new(items);
            let count = items.len();
            virtual_list(count, move |at: Position| {
                let items = items.clone();
                dynamic(move || match items.get(at.index()) {
                    Some(Item::Heading(title)) => view()
                        .pt_4()
                        .pb_1()
                        .child(text(title.clone()).font_weight(FontWeight::SEMIBOLD))
                        .into_element(),
                    Some(Item::Note(e)) => row(model, e),
                    None => view().into_element(),
                })
            })
            .viewport(ListViewport::Parent)
            .estimated_item_size(px(72.0))
            .into_element()
        }))
        .child(detail(model))
        // After the sheet, so the dialogs stack above it.
        .child(edit_dialog(model))
        .child(delete_dialog(model))
        .child(clear_dialog(model))
}
