//! Preferences: the project's confirmed effective preferences, narrowed by
//! tag filters and the filter text above them. On Desktop one virtualized
//! list in the page's scroll (no pages); in a terminal, whose surface does
//! not draw a list in its parent's scroll, pages of rows with Previous and
//! Next (keyboard-first, whole cells).
use crate::*;
use acp_ui_core::i18n::Msg;
use acp_ui_core::presentation::sanitize;
use acp_ui_core::{Model, Preference};
use shell::*;
use std::rc::Rc;
use tokens::palette;

/// Where an effective preference applies, in the operator's words.
fn scope_word(m: &Model, scope: &str) -> String {
    match scope {
        "global" => m.word(Msg::ScopePersonal).to_owned(),
        "project" => m.word(Msg::ScopeThisProject).to_owned(),
        other => sanitize(other, 64),
    }
}

/// The tag filters: All, then every tag in use with its count. Several
/// selected tags narrow to the rows carrying all of them.
fn tag_filters(model: Signal<Model>, current: &Model) -> AnyElement {
    let counts = current.tag_counts();
    let colors = ui::button(model, "")
        .label_fn(move || model.get().word(Msg::TagColorsOpen).into())
        .size(ui::ButtonSize::Small)
        .variant(ui::ButtonVariant::Ghost)
        .icon(ui::icons::TAG)
        .on_press(move || features::tags::open(model));
    if counts.is_empty() {
        return view().flex_row().child(colors).into_element();
    }
    let mut row = view().flex_row().flex_wrap().items_center().gap_1().child(
        ui::button(model, "")
            .label_fn(move || model.get().word(Msg::TagsAll).into())
            .size(ui::ButtonSize::Small)
            .variant(ui::ButtonVariant::Ghost)
            .selected(move || model.get().tag_filter.is_empty())
            .on_press(move || model.update(|m| m.clear_tags())),
    );
    for (name, count) in counts {
        let (is, toggle) = (name.clone(), name.clone());
        row = row.child(ui::tag_toggle(
            model,
            &name,
            count,
            move || model.get().tag_filter.contains(&is),
            move || model.update(|m| m.toggle_tag(&toggle)),
        ));
    }
    let several = current.tag_filter.len() > 1;
    view()
        .gap_1()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    text(current.word(Msg::TagFilter))
                        .flex_1()
                        .text_size(px(ui::scale::text::CAPTION))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(move || palette(model).muted),
                )
                .child(colors),
        )
        .child(row)
        .child(if several {
            text(current.word(Msg::TagFilterHelp))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted)
                .into_element()
        } else {
            view().into_element()
        })
        .into_element()
}

/// One confirmed row: key and Details, the value's first line, then where it
/// applies and its tags.
fn row(model: Signal<Model>, r: &Preference, index: usize) -> AnyElement {
    let m = model.get();
    let key = sanitize(&r.key, 64);
    let spoken = format!("{} · {key}", m.word(Msg::Details));
    view()
        .p_3()
        .gap_1()
        .rounded(px(ui::scale::radius::CONTAINER))
        .bg(move || palette(model).panel)
        .border_1()
        .border_color(move || palette(model).hairline)
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    text(key)
                        .font_weight(FontWeight::SEMIBOLD)
                        .flex_1()
                        .min_w_0(),
                )
                .child(
                    ui::button(model, "")
                        .label_fn(move || model.get().word(Msg::Details).into())
                        .spoken(move || spoken.clone().into())
                        .size(ui::ButtonSize::Small)
                        .variant(ui::ButtonVariant::Ghost)
                        .on_press(move || model.update(|m| m.select_preference(index))),
                ),
        )
        .child(text(sanitize(&r.value, 160)))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    text(scope_word(&m, &r.scope))
                        .text_size(px(ui::scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                )
                .child(ui::tag_list(model, &r.tags)),
        )
        .into_element()
}

pub(crate) fn preferences(model: Signal<Model>) -> impl IntoElement {
    let current = model.get();
    let rows: Rc<Vec<Preference>> = Rc::new(current.rows().into_iter().cloned().collect());
    let mut body = view().gap_3().child(
        view()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                text(move || model.get().word(Msg::PreferencesHeading))
                    .font_weight(FontWeight::SEMIBOLD)
                    .flex_1(),
            )
            .child(
                text(
                    current
                        .word(Msg::ConfirmedCount)
                        .replace("{count}", &rows.len().to_string()),
                )
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
            ),
    );
    // Managing or adding records navigates cleanly to the unified Review hub.
    if current.live {
        body = body.child(
            view().items_start().child(
                ui::button(model, "")
                    .label_fn(move || model.get().word(Msg::OpenPreferencesManager).into())
                    .size(ui::ButtonSize::Small)
                    .disabled(move || model.get().selected_project.is_none())
                    .on_press(move || {
                        shell::go_review(model, acp_ui_core::operator::ReviewMode::Project)
                    }),
            ),
        );
    }
    body = body.child(tag_filters(model, &current));
    if rows.is_empty() {
        return body.child(card(
            model,
            current.word(Msg::NoMatching),
            vec![
                current.word(Msg::EmptyPreferences).into(),
                current.word(Msg::FilterRecovery).into(),
            ],
        ));
    }
    if let Some(selected) = current.preference_detail() {
        body = body
            .child(card(
                model,
                current.word(Msg::SelectedPreference),
                vec![
                    sanitize(&selected.key, 120),
                    current.detail_chunk(),
                    format!(
                        "{} {}/{} · {} · {}",
                        current.word(Msg::ValueDetail),
                        current.detail_page + 1,
                        current.detail_pages(),
                        scope_word(&current, &selected.scope),
                        acp_ui_core::presentation::hashtags(&selected.tags)
                    ),
                ],
            ))
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_1()
                    .child(word_action(
                        model,
                        Msg::EarlierValue,
                        move || model.get().detail_page > 0,
                        move || model.update(|m| m.detail_by(-1)),
                    ))
                    .child(word_action(
                        model,
                        Msg::LaterValue,
                        move || model.get().detail_page + 1 < model.get().detail_pages(),
                        move || model.update(|m| m.detail_by(1)),
                    ))
                    .child(ui::copy_button(
                        model,
                        move || model.get().word(Msg::CopyDetail).into(),
                        move || model.get().word(Msg::CopiedShort).into(),
                        move || {
                            set_clipboard(model.get().detail_chunk());
                            model.update(|m| m.copied = true);
                            true
                        },
                    ))
                    .child(control(model, Msg::CloseDetail, move || {
                        model.update(|m| m.selected_preference = None)
                    })),
            );
    }
    if ui::terminal() {
        let page = current.page;
        let mut list = view().gap_2();
        for (offset, r) in rows
            .iter()
            .enumerate()
            .skip(page * acp_ui_core::PAGE_SIZE)
            .take(acp_ui_core::PAGE_SIZE)
        {
            list = list.child(row(model, r, offset));
        }
        return body.child(list).child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(word_action(
                    model,
                    Msg::PreviousPage,
                    move || model.get().page > 0,
                    move || model.update(|m| m.page_by(-1)),
                ))
                .child(word_action(
                    model,
                    Msg::NextPage,
                    move || {
                        (model.get().page + 1) * acp_ui_core::PAGE_SIZE < model.get().rows().len()
                    },
                    move || model.update(|m| m.page_by(1)),
                ))
                .child(text(current.page_label())),
        );
    }
    let count = rows.len();
    body.child(
        virtual_list(count, move |at: Position| {
            let rows = rows.clone();
            dynamic(move || {
                let index = at.index();
                match rows.get(index) {
                    Some(r) => row(model, r, index),
                    None => view().into_element(),
                }
            })
        })
        .viewport(ListViewport::Parent)
        .estimated_item_size(px(104.0))
        .separator(|| view().h(px(8.0))),
    )
}
