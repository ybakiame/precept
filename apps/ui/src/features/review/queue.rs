use super::actions::{available, disarm, refresh, review, write_grant};
use super::formatting::origin_label;
use super::records::status_word;
use super::*;
use precept_ui_core::operator::PreferenceRecord;
use tokens::mix;

/// The Queue tab: suggested-only rows, ascending id, bounded paging and an
/// explicit watermark delta. An empty queue is a valid state, never an error.
pub(super) fn queue_tab(model: Signal<Model>, delete_open: Signal<bool>) -> AnyElement {
    let m = model.get();
    let mut body = view().gap_2().flex_shrink_0();
    let Some(queue) = &m.operator.queue else {
        return body
            .child(ui::empty(model, m.word(Msg::ReviewQueueUnobserved)))
            .into_element();
    };
    // Even an empty later page can disclose drift; paging is not a snapshot.
    if queue.commit_sequence > m.operator.queue_watermark {
        let delta = queue.commit_sequence - m.operator.queue_watermark;
        body = body.child(
            text(
                m.word(Msg::ReviewQueueChanged)
                    .replace("{n}", &delta.to_string()),
            )
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).warning),
        );
    }
    if queue.records.is_empty() {
        return body
            .child(
                ui::empty(model, m.word(Msg::ReviewQueueEmpty))
                    .description(m.word(Msg::ReviewQueueEmptyHelp)),
            )
            .into_element();
    }
    let records = std::rc::Rc::new(queue.records.clone());
    let selected_id = m.operator.selected_id;
    let more = queue.next_after_id.is_some();
    let list = loaded_list(
        model,
        records.iter().map(|r| r.id).collect(),
        {
            let records = records.clone();
            move |index| queue_row(model, delete_open, &records[index], index, selected_id)
        },
        more,
        move || {
            disarm(model, delete_open);
            let mut go = false;
            model.update(|m| {
                if let Some(id) = m.operator.queue.as_ref().and_then(|q| q.next_after_id) {
                    m.operator.queue_after_id = id;
                    go = true;
                }
            });
            if go {
                refresh(model, "profile.review.queue");
            }
        },
    );
    body.child(list).into_element()
}
/// One pending suggestion: pressing its title inspects the exact record.
pub(super) fn queue_row(
    model: Signal<Model>,
    delete_open: Signal<bool>,
    r: &PreferenceRecord,
    index: usize,
    selected_id: Option<i64>,
) -> AnyElement {
    let m = model.get();
    let id = r.id;
    let selected = selected_id == Some(id);
    let label = format!("#{} · {} · r{}", id, sanitize(&r.key, 120), r.revision);
    let mut trailing = view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(ui::badge(model, status_word(&m, &r.status)).variant(ui::BadgeVariant::Warning));
    if selected {
        trailing =
            trailing.child(ui::badge(model, m.word(Msg::Selected)).variant(ui::BadgeVariant::Info));
    }
    // A batch arrives as a wave: a capped index stagger, instant when reduced.
    let arrive = ui::motion::arrive(((index % 12) as f32) * 50.0);
    view()
        .p_2()
        .rounded(px(ui::scale::radius::CONTROL))
        .transition_colors()
        .duration(ui::motion::ms(ui::motion::FAST))
        .ease_out()
        .bg(move || {
            if selected {
                mix(palette(model).inset, palette(model).primary, 0.10)
            } else {
                palette(model).panel
            }
        })
        .child(record_line(
            model,
            r,
            label,
            &format!("#{id} · r{}", r.revision),
            move || {
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
            },
            trailing.into_element(),
        ))
        .bind_motion(&arrive)
        .into_element()
}
/// A record's row: its checkbox, its origin as an icon in a column of its
/// own, then the title (the key, then number and revision; it wraps, and its
/// accessible name is `label`), the value's first line and the tags beneath,
/// with `trailing` (status) at the right.
pub(super) fn record_line(
    model: Signal<Model>,
    r: &PreferenceRecord,
    label: String,
    detail: &str,
    on_open: impl Fn() + 'static,
    trailing: AnyElement,
) -> AnyElement {
    let m = model.get_untracked();
    let origin = origin_label(m.locale, r.origin);
    let glyph = match r.origin {
        Origin::Operator => icons::PERSON,
        Origin::Agent => icons::AGENT,
    };
    let preview = precept_ui_core::presentation::preview_line(&r.value, 140);
    let title = view()
        .role(AccessibilityRole::Button)
        .label(label)
        .on_press(move || {
            if available(model) {
                on_open()
            }
        })
        .px_1()
        .rounded(px(ui::scale::radius::SMALL))
        .transition_colors()
        .duration(ui::motion::ms(ui::motion::FAST))
        .hover(move |s| s.bg(mix(palette(model).panel, palette(model).text, 0.06)))
        .focus(move |s| {
            s.ring(px(2.0))
                .ring_color(palette(model).primary.opacity(0.55))
        })
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_x_2()
                .child(
                    text(sanitize(&r.key, 120))
                        .font_weight(FontWeight::SEMIBOLD)
                        .min_w_0(),
                )
                .child(
                    text(detail.to_owned())
                        .text_size(px(ui::scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                ),
        );
    view()
        .flex_row()
        .items_start()
        .gap_3()
        .child(view().pt_0p5().child(pick(model, r)))
        .child(
            view()
                .w(px(ui::scale::icon::SM))
                .pt_1()
                .flex_shrink_0()
                .label(origin)
                .text_color(move || palette(model).muted)
                .child(ui::icon::glyph(glyph, ui::scale::icon::SM)),
        )
        .child(
            view()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(title)
                .child(
                    text(sanitize(&preview, 140))
                        .px_1()
                        .text_size(px(ui::scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                )
                .child(view().px_1().child(record_meta(model, r))),
        )
        .child(view().pt_0p5().flex_shrink_0().child(trailing))
        .into_element()
}
/// The batch checkbox of a suggested row (nothing for other statuses).
pub(super) fn pick(model: Signal<Model>, r: &PreferenceRecord) -> AnyElement {
    if r.status != "suggested" {
        // A checkbox's room, so every row's label starts at one edge.
        return view()
            .w(px(ui::CHECKBOX_SIZE))
            .flex_shrink_0()
            .into_element();
    }
    let id = r.id;
    ui::checkbox(
        model,
        move || {
            model
                .get()
                .word(Msg::BatchPick)
                .replace("{id}", &id.to_string())
                .into()
        },
        move || model.get().operator.batch.binary_search(&id).is_ok(),
        move || !write_grant(model) || model.get().operator.busy,
        move |_| model.update(|m| m.operator.toggle_batch(id)),
    )
    .into_element()
}
/// What to do with the picked suggestions: a note, then confirm or reject
/// them all in one write. Shown only while something is picked.
pub(super) fn batch_bar(model: Signal<Model>) -> impl IntoElement {
    let count = move || model.get().operator.batch.len();
    let words = move |key: Msg| {
        move || -> SharedString {
            model
                .get()
                .word(key)
                .replace("{n}", &count().to_string())
                .into()
        }
    };
    let ready = move || available(model) && write_grant(model) && !model.get().operator.busy;
    view()
        .style_when(move || count() == 0, |s| s.hidden())
        .gap_2()
        .p_3()
        .rounded(px(ui::scale::radius::CONTAINER))
        .border_1()
        .border_color(move || palette(model).primary)
        .bg(move || mix(palette(model).panel, palette(model).primary, 0.06))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    text(words(Msg::BatchPicked))
                        .font_weight(FontWeight::MEDIUM)
                        .flex_1(),
                )
                .child(
                    ui::button(model, "")
                        .label_fn(words(Msg::BatchConfirm))
                        .size(ui::ButtonSize::Small)
                        .variant(ui::ButtonVariant::Primary)
                        .disabled(move || !ready())
                        .on_press(move || review(model, "batch_confirm")),
                )
                .child(
                    ui::button(model, "")
                        .label_fn(words(Msg::BatchReject))
                        .size(ui::ButtonSize::Small)
                        .disabled(move || !ready())
                        .on_press(move || review(model, "batch_reject")),
                )
                .child(
                    ui::button(model, "")
                        .label_fn(move || model.get().word(Msg::BatchClear).into())
                        .size(ui::ButtonSize::Small)
                        .variant(ui::ButtonVariant::Ghost)
                        .on_press(move || model.update(|m| m.operator.batch.clear())),
                ),
        )
        .child(
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
        )
        .child(
            text(move || model.get().word(Msg::BatchHelp))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
}
/// A record's tags and origin on one wrapping line.
pub(super) fn record_meta(model: Signal<Model>, r: &PreferenceRecord) -> AnyElement {
    let m = model.get();
    let mut row = view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(ui::tag_list(model, &r.tags));
    if r.status == "confirmed" && !r.enabled {
        row = row.child(ui::badge(model, m.word(Msg::RuleOff)).variant(ui::BadgeVariant::Warning));
    }
    if r.rule.enforcement != precept_ui_core::operator::Enforcement::Advisory {
        row = row.child(
            ui::badge(model, enforcement_word(&m, r.rule.enforcement))
                .variant(ui::BadgeVariant::Info),
        );
    }
    if r.rule.priority != 0 {
        row = row.child(
            text(
                m.word(Msg::PriorityShort)
                    .replace("{n}", &r.rule.priority.to_string()),
            )
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted),
        );
    }
    if let Some(what) = precept_ui_core::presentation::applies_words(&m, &r.rule.applies_to) {
        row = row.child(
            text(what)
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        );
    }
    row.into_element()
}
pub(super) fn enforcement_word(m: &Model, e: precept_ui_core::operator::Enforcement) -> String {
    m.word(match e {
        precept_ui_core::operator::Enforcement::Advisory => Msg::EnforcementAdvisory,
        precept_ui_core::operator::Enforcement::Verify => Msg::EnforcementVerify,
        precept_ui_core::operator::Enforcement::Hard => Msg::EnforcementHard,
    })
    .into()
}
/// Rows loaded in batches: a virtualized list in the page's scroll that asks
/// for the next batch as it nears the end, and a footer that says whether more
/// exist (with the same load as a button, for keyboard and terminal).
pub(super) fn loaded_list(
    model: Signal<Model>,
    ids: Vec<i64>,
    row: impl Fn(usize) -> AnyElement + 'static,
    more: bool,
    load: impl Fn() + 'static,
) -> AnyElement {
    let load = std::rc::Rc::new(load);
    let count = ids.len();
    let footer = if more {
        let load = load.clone();
        ui::button(model, "")
            .label_fn(move || model.get().word(Msg::LoadMore).into())
            .size(ui::ButtonSize::Small)
            .variant(ui::ButtonVariant::Ghost)
            .icon(icons::REFRESH)
            .disabled(move || !available(model) || model.get().operator.busy)
            .loading(move || model.get().operator.busy)
            .on_press(move || load())
            .into_element()
    } else {
        text(model.get().word(Msg::AllLoaded))
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted)
            .into_element()
    };
    let body = view().gap_2().flex_shrink_0();
    let row = std::rc::Rc::new(row);
    let mut list = virtual_list(count, move |at: Position| {
        let row = row.clone();
        dynamic(move || {
            let index = at.index();
            if index < count {
                row(index)
            } else {
                view().into_element()
            }
        })
    })
    .keys(move || ids.clone())
    .viewport(ListViewport::Parent)
    .estimated_item_size(px(64.0))
    .separator(|| view().h(px(4.0)));
    if more {
        list = list.on_load_more(move || load());
    }
    body.child(list)
        .child(view().items_start().child(footer))
        .into_element()
}
