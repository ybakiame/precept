//! Preferences: confirmed effective preferences with filter and paging.
use crate::*;
use acp_operator_core::i18n::Msg;
use acp_operator_core::{PAGE_SIZE, presentation::sanitize};
use shell::*;
use tokens::palette;

/// Where an effective preference comes from, in the operator's words; the
/// private source is not part of live context, so only scope is shown.
fn origin(m: &Model, scope: &str, source: &str) -> String {
    let scope = match scope {
        "global" => m.word(Msg::ScopePersonal).to_owned(),
        "project" => m.word(Msg::ScopeThisProject).to_owned(),
        other => sanitize(other, 64),
    };
    if source.is_empty() {
        scope
    } else {
        format!("{scope} / {}", sanitize(source, 80))
    }
}
pub(crate) fn preferences(model: Signal<Model>) -> impl IntoElement {
    let current = model.get();
    let rows = current.rows();
    let page = current.page;
    let mut body = view()
        .gap_2()
        .child(
            text(move || model.get().word(Msg::PreferencesHeading))
                .font_weight(FontWeight::SEMIBOLD),
        )
        .child(
            text(move || model.get().word(Msg::PreferencesSources))
                .text_size(px(12.0))
                .text_color(move || palette(model).muted),
        );
    // Managing records is the review workspace, fixed to one scope.
    if current.live {
        body = body.child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(word_action(
                    model,
                    Msg::ManageProjectPreferences,
                    move || model.get().selected_project.is_some(),
                    move || {
                        review::open_mode(model, acp_operator_core::operator::ReviewMode::Project)
                    },
                ))
                .child(word_action(
                    model,
                    Msg::ManagePersonalPreferences,
                    || true,
                    move || {
                        review::open_mode(model, acp_operator_core::operator::ReviewMode::Personal)
                    },
                )),
        );
    }
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
    if let Some(row) = current.preference_detail() {
        body = body
            .child(card(
                model,
                current.word(Msg::SelectedPreference),
                vec![
                    sanitize(&row.key, 120),
                    current.detail_chunk(),
                    format!(
                        "{} {}/{} · {}",
                        current.word(Msg::ValueDetail),
                        current.detail_page + 1,
                        current.detail_pages(),
                        origin(&current, &row.scope, &row.source)
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
                    .child(control(model, Msg::CopyDetail, move || {
                        set_clipboard(model.get().detail_chunk());
                        model.update(|m| m.copied = true);
                    }))
                    .child(control(model, Msg::CloseDetail, move || {
                        model.update(|m| m.selected_preference = None)
                    })),
            );
    }
    for (offset, row) in rows
        .iter()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .enumerate()
    {
        let index = page * PAGE_SIZE + offset;
        body = body.child(
            view()
                .p_2()
                .gap_1()
                .rounded(px(6.0))
                .bg(move || palette(model).panel)
                .border_1()
                .border_color(move || palette(model).border)
                .child(
                    view()
                        .flex_row()
                        .gap_2()
                        .child(
                            text(sanitize(&row.key, 64))
                                .font_weight(FontWeight::SEMIBOLD)
                                .flex_1(),
                        )
                        .child(action(
                            model,
                            format!("{} {}", current.word(Msg::Details), offset + 1),
                            || true,
                            move || model.update(|m| m.select_preference(index)),
                        )),
                )
                .child(text(sanitize(&row.value, 160)))
                .child(
                    text(origin(&current, &row.scope, &row.source))
                        .text_size(px(12.0))
                        .text_color(move || palette(model).muted),
                ),
        );
    }
    body
}
