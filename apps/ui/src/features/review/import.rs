use super::{available, review, write_grant};
use crate::*;
use precept_ui_core::{i18n::Msg, operator::Scope, presentation::sanitize};
use tokens::palette;

/// Reads the selected project's known rule files for import (a read; the
/// dialog opens at once and fills in).
pub(super) fn open_import(model: Signal<Model>) {
    let Some(project) = model.get().selected_project.clone() else {
        return;
    };
    model.update(|m| {
        m.operator.import = Some(precept_ui_core::operator::ImportScan {
            project: String::new(),
            candidates: Vec::new(),
            skipped: Vec::new(),
        });
        m.operator.import_picked.clear();
        m.operator.error = None;
    });
    super::super::profile::call(
        model,
        "import.scan",
        serde_json::json!({ "project": project }),
        move |_, m, r| {
            if m.operator.import.is_none() {
                return; // Closed meanwhile.
            }
            match r
                .ok()
                .and_then(|v| precept_ui_core::operator::ImportScan::parse(&project, v))
            {
                Some((scan, picked)) => {
                    m.operator.import = Some(scan);
                    m.operator.import_picked = picked;
                }
                None => {
                    m.operator.import = None;
                    m.operator.error = Some("Unavailable".into());
                }
            }
        },
    );
}
fn import_reason(m: &Model, reason: &str) -> String {
    m.word(match reason {
        "too_long" => Msg::ImportReasonTooLong,
        "empty" => Msg::ImportReasonEmpty,
        "malformed_frontmatter" => Msg::ImportReasonMalformed,
        "unsupported_metadata" => Msg::ImportReasonUnsupportedMetadata,
        "unsupported_applicability" => Msg::ImportReasonUnsupportedApplicability,
        "duplicate" => Msg::ImportReasonDuplicate,
        "budget_exhausted" => Msg::ImportReasonBudget,
        _ => Msg::ImportReasonUnreadable,
    })
    .into()
}
/// One found rule: its checkbox, where it came from, its key and scope, and
/// the first line of its text.
fn import_row(
    model: Signal<Model>,
    index: usize,
    c: &precept_ui_core::operator::ImportCandidate,
) -> AnyElement {
    let m = model.get_untracked();
    let key = c.key.clone();
    let exists = c.exists;
    let first: String = c
        .description
        .as_deref()
        .or_else(|| c.value.lines().find(|l| !l.trim().is_empty()))
        .unwrap_or_default()
        .chars()
        .take(100)
        .collect();
    let scope = m.word(if matches!(c.scope, Scope::Global) {
        Msg::ImportPersonal
    } else {
        Msg::ImportProject
    });
    view()
        .flex_row()
        .items_start()
        .gap_2()
        .py_1p5()
        .style_when(move || index > 0, |s| s.border_t_1())
        .border_color(move || palette(model).hairline)
        .child(ui::checkbox(
            model,
            move || {
                model
                    .get()
                    .word(Msg::ImportPick)
                    .replace("{key}", &key)
                    .into()
            },
            move || model.get().operator.import_picked.contains(&index),
            move || exists || model.get().operator.busy,
            move |on| {
                model.update(|m| {
                    let picked = &mut m.operator.import_picked;
                    if on
                        && !picked.contains(&index)
                        && picked.len() < precept_ui_core::operator::MAX_BATCH_UI
                    {
                        picked.push(index);
                        picked.sort_unstable();
                    } else if !on {
                        picked.retain(|i| *i != index);
                    }
                    m.operator.review = None;
                })
            },
        ))
        .child(
            view()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(text(sanitize(&c.key, 120)).font_weight(FontWeight::MEDIUM))
                        .child(ui::badge(model, c.kind.label()).variant(ui::BadgeVariant::Neutral))
                        .child(ui::badge(model, scope).variant(ui::BadgeVariant::Neutral))
                        .child(if exists {
                            ui::badge(model, m.word(Msg::ImportExists))
                                .variant(ui::BadgeVariant::Success)
                                .into_element()
                        } else {
                            view().into_element()
                        }),
                )
                .child(
                    text(format!(
                        "{} · {}",
                        sanitize(&c.source, 120),
                        sanitize(&c.title, 120)
                    ))
                    .text_size(px(ui::scale::text::CAPTION))
                    .text_color(move || palette(model).muted),
                )
                .child(
                    text(sanitize(&first, 100))
                        .text_size(px(ui::scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                ),
        )
        .into_element()
}
/// What the project's files hold, to pick from; saving prepares one write
/// that files the picks as suggestions.
pub(super) fn import_dialog(model: Signal<Model>) -> impl IntoElement {
    let count = move || model.get().operator.import_picked.len();
    ui::dialog(model, "")
        .size(ui::DialogSize::Large)
        .icon(icons::FILE, ui::DialogTone::Accent)
        .title_fn(move || {
            let m = model.get();
            m.word(Msg::ImportTitle)
                .replace(
                    "{name}",
                    &sanitize(m.selected_project.as_deref().unwrap_or(""), 64),
                )
                .into()
        })
        .description_fn(move || model.get().word(Msg::ImportHelp).into())
        .content_fn(move || {
            view()
                .w_full()
                .gap_2()
                .child(dynamic(move || {
                    let m = model.get();
                    let Some(scan) = &m.operator.import else {
                        return view().into_element();
                    };
                    if scan.project.is_empty() {
                        return text(m.word(Msg::ImportScanning))
                            .text_color(move || palette(model).muted)
                            .into_element();
                    }
                    let mut list = view();
                    if scan.candidates.is_empty() {
                        list = list.child(ui::empty(model, m.word(Msg::ImportNone)));
                    }
                    for (i, c) in scan.candidates.iter().enumerate() {
                        list = list.child(import_row(model, i, c));
                    }
                    for s in scan.skipped.iter().take(20) {
                        let reason = import_reason(&m, &s.reason);
                        let line = match &s.detail {
                            Some(detail) => m
                                .word(Msg::ImportSkippedDetail)
                                .replace("{source}", &sanitize(&s.source, 120))
                                .replace("{title}", &sanitize(&s.title, 120))
                                .replace("{reason}", &reason)
                                .replace("{detail}", &sanitize(detail, 256)),
                            None => m
                                .word(Msg::ImportSkippedLine)
                                .replace("{source}", &sanitize(&s.source, 120))
                                .replace("{title}", &sanitize(&s.title, 120))
                                .replace("{reason}", &reason),
                        };
                        list = list.child(
                            text(line)
                                .text_size(px(ui::scale::text::CAPTION))
                                .text_color(move || palette(model).warning),
                        );
                    }
                    scroll().max_h(px(420.0)).child(list).into_element()
                }))
                .into_element()
        })
        .cancel_fn(move || model.get().word(Msg::Cancel).into())
        .confirm_fn(move || {
            model
                .get()
                .word(Msg::ImportSubmit)
                .replace("{n}", &count().to_string())
                .into()
        })
        .confirm_disabled(move || count() == 0 || !available(model) || !write_grant(model))
        .open(move || {
            let m = model.get();
            m.operator.open && m.operator.import.is_some() && m.operator.review.is_none()
        })
        .on_cancel(move || {
            model.update(|m| {
                m.operator.import = None;
                m.operator.import_picked.clear();
            })
        })
        .on_confirm(move || review(model, "import"))
}
