//! Preferences: the project's confirmed effective preferences, narrowed by
//! tag filters and the filter text above them, in one virtualized list in the
//! page's scroll.
use crate::*;
use precept_ui_core::i18n::Msg;
use precept_ui_core::presentation::sanitize;
use precept_ui_core::{Model, Preference};
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

/// The open details: a preference or a repository rule, as title,
/// description and Markdown.
fn open_detail(m: &Model) -> Option<(String, String, String)> {
    if let Some(r) = m.preference_detail() {
        return Some((
            sanitize(&r.key, 120),
            format!(
                "{} · {}",
                scope_word(m, &r.scope),
                precept_ui_core::presentation::hashtags(&r.tags)
            ),
            r.value.clone(),
        ));
    }
    let r = m.rule_detail()?;
    let mut text = r.text.clone();
    if !r.allowed_values.is_empty() {
        text = format!("{text}\n\n`{}`", r.allowed_values.join("` | `"));
    }
    Some((
        sanitize(r.title.as_deref().unwrap_or(&r.key), 120),
        format!(
            "{} · {}",
            sanitize(&r.file, 120),
            precept_ui_core::presentation::hashtags(&r.tags)
        ),
        text,
    ))
}

/// The selected rule in a dialog: its Markdown rendered, where it lives and
/// its tags; Copy puts the text on the clipboard. Mounted by the shell.
pub(crate) fn detail_dialog(model: Signal<Model>) -> impl IntoElement {
    let selected = move || open_detail(&model.get());
    ui::dialog(model, "")
        .size(ui::DialogSize::Large)
        .title_fn(move || selected().map(|d| d.0).unwrap_or_default().into())
        .description_fn(move || selected().map(|d| d.1).unwrap_or_default().into())
        .content_fn(move || {
            let value = selected().map(|d| d.2).unwrap_or_default();
            scroll()
                .max_h(px(440.0))
                .w_full()
                .child(ui::markdown_view(model, value))
                .into_element()
        })
        .cancel_fn(move || model.get().word(Msg::CloseDialog).into())
        .confirm_fn(move || model.get().word(Msg::CopyRule).into())
        .open(move || selected().is_some())
        .on_cancel(move || {
            model.update(|m| {
                m.selected_preference = None;
                m.selected_rule = None;
            })
        })
        .on_confirm(move || {
            if let Some(d) = selected() {
                drop(set_clipboard(d.2));
                ui::notify(ui::Tone::Success, model.get().word(Msg::CopiedShort));
            }
        })
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
        .child(text(sanitize(
            &precept_ui_core::presentation::preview_line(&r.value, 160),
            170,
        )))
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
                .child(ui::tag_list(model, &r.tags))
                .child(match r.enforcement.as_deref() {
                    Some(e @ ("verify" | "hard")) => ui::badge(
                        model,
                        m.word(if e == "hard" {
                            Msg::EnforcementHard
                        } else {
                            Msg::EnforcementVerify
                        }),
                    )
                    .variant(ui::BadgeVariant::Info)
                    .into_element(),
                    _ => view().into_element(),
                })
                .child(
                    text(
                        r.applies_to
                            .as_ref()
                            .and_then(|a| precept_ui_core::presentation::applies_words(&m, a))
                            .unwrap_or_default(),
                    )
                    .text_size(px(ui::scale::text::CAPTION))
                    .text_color(move || palette(model).muted),
                ),
        )
        .into_element()
}

/// What a rule's checks found now, as a badge (`None` when it has none).
fn check_badge(model: Signal<Model>, m: &Model, checks: &serde_json::Value) -> AnyElement {
    let n = |k: &str| {
        checks
            .get(k)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
    };
    let (label, variant) = match checks.get("status").and_then(serde_json::Value::as_str) {
        Some("verified") => (
            m.word(Msg::CheckVerified)
                .replace("{passed}", &n("passed").to_string())
                .replace("{total}", &n("total").to_string()),
            ui::BadgeVariant::Success,
        ),
        Some("failing") => (
            m.word(Msg::CheckFailing).replace(
                "{file}",
                &sanitize(
                    checks
                        .pointer("/failure/file")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("?"),
                    80,
                ),
            ),
            ui::BadgeVariant::Danger,
        ),
        Some("unavailable") => (
            m.word(Msg::CheckUnavailable).to_owned(),
            ui::BadgeVariant::Warning,
        ),
        _ => return view().into_element(),
    };
    ui::badge(model, label).variant(variant).into_element()
}

/// One committed repository rule: key and Details, its first line, then its
/// file, check result, enforcement and tags.
fn rule_row(model: Signal<Model>, r: &precept_ui_core::RepositoryRule, index: usize) -> AnyElement {
    let m = model.get();
    let key = sanitize(&r.key, 64);
    // A rule with a title is named by it; its key follows, quietly.
    let name = r
        .title
        .as_deref()
        .map_or_else(|| key.clone(), |t| sanitize(t, 80));
    let preview = match &r.title {
        Some(t) => precept_ui_core::presentation::preview_after_title(&r.text, t, 160),
        None => precept_ui_core::presentation::preview_line(&r.text, 160),
    };
    let spoken = format!("{} · {key}", m.word(Msg::Details));
    let mut meta = view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(
            text(sanitize(&r.file, 120))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
        .child(check_badge(model, &m, &r.checks))
        .child(ui::tag_list(model, &r.tags));
    if let Some(e @ ("verify" | "hard")) = r.enforcement.as_deref() {
        meta = meta.child(
            ui::badge(
                model,
                m.word(if e == "hard" {
                    Msg::EnforcementHard
                } else {
                    Msg::EnforcementVerify
                }),
            )
            .variant(ui::BadgeVariant::Info),
        );
    }
    if r.guards > 0 {
        meta = meta.child(
            text(
                m.word(Msg::RuleGuards)
                    .replace("{count}", &r.guards.to_string()),
            )
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted),
        );
        // Whether the gate is switched on, never a claim that it enforced.
        let gate_on = m.project().is_some_and(|p| {
            m.data
                .as_ref()
                .and_then(|d| d.directory.iter().find(|e| e.name == p.name))
                .is_some_and(|e| e.gate)
        });
        meta = meta.child(
            ui::badge(
                model,
                m.word(if gate_on {
                    Msg::RuleGateOn
                } else {
                    Msg::RuleGateOff
                }),
            )
            .variant(if gate_on {
                ui::BadgeVariant::Info
            } else {
                ui::BadgeVariant::Neutral
            }),
        );
    }
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
                    view()
                        .flex_1()
                        .min_w_0()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_x_2()
                        .child(text(name.clone()).font_weight(FontWeight::SEMIBOLD))
                        .child(
                            text(if name == key {
                                String::new()
                            } else {
                                key.clone()
                            })
                            .text_size(px(ui::scale::text::CAPTION))
                            .text_color(move || palette(model).muted),
                        ),
                )
                .child(
                    ui::button(model, "")
                        .label_fn(move || model.get().word(Msg::Details).into())
                        .spoken(move || spoken.clone().into())
                        .size(ui::ButtonSize::Small)
                        .variant(ui::ButtonVariant::Ghost)
                        .on_press(move || model.update(|m| m.select_rule(index))),
                ),
        )
        .child(text(sanitize(&preview, 170)))
        .child(meta)
        .into_element()
}

/// The repository's committed rules: what outranks preferences of their key.
/// Read-only; uncommitted and invalid files are named, not shown.
fn contracts(model: Signal<Model>, current: &Model) -> impl IntoElement {
    let context = current.project().map(|p| p.context.clone());
    let rules = context
        .as_ref()
        .map(|c| c.repository_rules.clone())
        .unwrap_or_default();
    let note = |s: String| {
        text(s)
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted)
            .into_element()
    };
    let mut section = view()
        .gap_2()
        .child(text(current.word(Msg::ContractsHeading)).font_weight(FontWeight::SEMIBOLD));
    for (i, r) in rules.iter().take(50).enumerate() {
        section = section.child(rule_row(model, r, i));
    }
    if rules.is_empty() {
        section = section.child(note(current.word(Msg::ContractsNone).into()));
    }
    if let Some(c) = &context {
        let pending = c.pending_rules.len();
        if pending > 0 {
            section = section.child(note(
                current
                    .word(Msg::RulesPending)
                    .replace("{count}", &pending.to_string()),
            ));
        }
        match c
            .contract_resolution
            .get("status")
            .and_then(serde_json::Value::as_str)
        {
            Some("invalid_source") => {
                let files: Vec<String> = c
                    .contract_resolution
                    .get("invalid_files")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(serde_json::Value::as_str)
                    .map(|f| sanitize(f, 120))
                    .collect();
                section = section.child(
                    ui::badge(
                        model,
                        current
                            .word(Msg::RulesInvalid)
                            .replace("{files}", &files.join(", ")),
                    )
                    .variant(ui::BadgeVariant::Danger),
                );
            }
            Some("source_unavailable") => {
                section = section.child(note(current.word(Msg::RulesUnreadable).into()));
            }
            _ => {}
        }
    }
    section.child(note(current.word(Msg::ContractsSource).into()))
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
    // Managing or adding records navigates cleanly to the reviewed Rules page.
    if current.live {
        body = body.child(
            view().items_start().child(
                ui::button(model, "")
                    .label_fn(move || model.get().word(Msg::OpenPreferencesManager).into())
                    .size(ui::ButtonSize::Small)
                    .disabled(move || model.get().selected_project.is_none())
                    .on_press(move || {
                        shell::go_review(model, precept_ui_core::operator::ReviewMode::Project)
                    }),
            ),
        );
    }
    body = body.child(features::rule_lint::panel(model));
    body = body.child(contracts(model, &current));
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
