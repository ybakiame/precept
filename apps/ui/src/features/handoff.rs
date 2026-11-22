//! Handoff: the explicitly selected task's saved handoff.
use crate::*;
use precept_ui_core::i18n::Msg;
use precept_ui_core::presentation::sanitize;

use tokens::palette;

/// Reads the selected project's saved handoffs while the Handoff page shows.
pub(crate) fn watch(model: Signal<Model>) {
    let ready = computed(move || {
        let m = model.get();
        (
            shell::current(&m) == shell::Destination::Handoff && m.live && m.data.is_some(),
            m.epoch,
            m.selected_project.clone(),
        )
    });
    effect(move || {
        let (shown, _, project) = ready.get();
        if let (true, Some(project)) = (shown, project) {
            untracked(|| load(model, project));
        }
    });
}
fn load(model: Signal<Model>, project: String) {
    super::profile::call(
        model,
        "profile.handoff.list",
        serde_json::json!({ "project": project }),
        move |_, m, r| {
            if m.selected_project.as_deref() != Some(project.as_str()) {
                return;
            }
            if let Ok(list) = r.and_then(|v| {
                serde_json::from_value::<precept_ui_core::operator::HandoffList>(v)
                    .map_err(|_| "Unavailable")
            }) {
                m.handoff_list = Some((project.clone(), list.handoffs));
            }
        },
    );
}

/// The project's saved handoffs: pressing one opens it.
pub(crate) fn saved(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    let list = m
        .handoff_list
        .as_ref()
        .filter(|(p, _)| m.selected_project.as_deref() == Some(p.as_str()))
        .map(|(_, l)| l.clone());
    let mut card = ui::surface(model).title(m.word(Msg::SavedHandoffs));
    let Some(list) = list else {
        return card.into_element();
    };
    if list.is_empty() {
        return card
            .child(
                text(m.word(Msg::SavedHandoffsNone))
                    .text_size(px(ui::scale::text::CAPTION))
                    .text_color(move || palette(model).muted),
            )
            .into_element();
    }
    for h in list {
        let task = h.task_id.clone();
        let spoken = m
            .word(Msg::OpenHandoffNamed)
            .replace("{task}", &sanitize(&h.task_id, 120));
        let current = m.queried_task == h.task_id;
        card = card.child(
            view()
                .role(AccessibilityRole::Button)
                .label(spoken)
                .on_press(move || {
                    let task = task.clone();
                    model.update(|m| m.task_id = task);
                    crate::reconnect(model);
                })
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_x_3()
                .gap_y_0p5()
                .px_2()
                .py_1p5()
                .rounded(px(ui::scale::radius::CONTROL))
                .bg(move || {
                    if current {
                        ui::tokens::mix(palette(model).panel, palette(model).primary, 0.10)
                    } else {
                        palette(model).panel
                    }
                })
                .hover(move |s| {
                    s.bg(ui::tokens::mix(
                        palette(model).panel,
                        palette(model).text,
                        0.06,
                    ))
                })
                .child(text(sanitize(&h.task_id, 120)).font_weight(FontWeight::SEMIBOLD))
                .child(text(sanitize(&h.objective, 200)).flex_1().min_w(px(160.0)))
                .child(
                    text(format!(
                        "{} · {}",
                        sanitize(&h.branch, 60),
                        precept_ui_core::presentation::utc_minute(&h.updated_at)
                    ))
                    .text_size(px(ui::scale::text::CAPTION))
                    .text_color(move || palette(model).muted),
                ),
        );
    }
    card.into_element()
}

pub(crate) fn handoff(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    let p = m.project().expect("ready project");
    let mut body = view().flex_shrink_0().gap_3();
    let Some(h) = p.handoff.as_ref().filter(|h| h.task_id == m.queried_task) else {
        return body
            .child(shell::card(
                model,
                m.word(Msg::ResumeSummary),
                m.visible_lines(),
            ))
            .child(saved(model));
    };
    let caption = |s: String| {
        text(s)
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted)
    };
    // The task: what it is for and what comes next.
    body = body.child(
        ui::surface(model)
            .title(
                m.word(Msg::HandoffTask)
                    .replace("{task}", &sanitize(&h.task_id, 120))
                    .replace("{project}", &sanitize(&p.name, 64)),
            )
            .child(
                ui::properties(model)
                    .row(m.word(Msg::Objective), sanitize(&h.objective, 1200))
                    .row(m.word(Msg::NextAction), sanitize(&h.next_action, 1200)),
            ),
    );
    // What may have changed since: the daemon's warnings in the operator's
    // language, and a short fingerprint.
    let mut before = ui::surface(model).title(m.word(Msg::HandoffBefore));
    for w in h.resume_warnings.iter().take(8) {
        before = before.child(
            view()
                .flex_row()
                .items_start()
                .gap_2()
                .child(
                    view()
                        .pt_0p5()
                        .text_color(move || palette(model).warning)
                        .child(ui::icon::glyph(ui::icons::ALERT, ui::scale::icon::SM)),
                )
                .child(
                    text(precept_ui_core::presentation::warning(m.locale, w))
                        .flex_1()
                        .min_w_0(),
                ),
        );
    }
    let fingerprint = h
        .current_repository
        .get("worktree_fingerprint")
        .and_then(|v| v.as_str())
        .map(|f| format!("{}…", f.chars().take(12).collect::<String>()))
        .unwrap_or_else(|| m.word(Msg::FingerprintUnavailable).to_owned());
    before = before.child(caption(format!(
        "{}: {}",
        m.word(Msg::Fingerprint),
        fingerprint
    )));
    body = body.child(before);
    // The checks recorded with the handoff: name and result, as history.
    let mut checks = ui::surface(model)
        .title(m.word(Msg::HandoffChecks))
        .child(caption(m.word(Msg::HandoffChecksHelp).into()));
    if h.validation.is_empty() {
        checks = checks.child(caption(m.word(Msg::HandoffChecksNone).into()));
    }
    for v in h.validation.iter().take(20) {
        let name = v
            .get("check")
            .and_then(|c| c.as_str())
            .map(|c| sanitize(c, 200))
            .unwrap_or_else(|| "—".to_owned());
        let status = v
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("?")
            .to_owned();
        let variant = match status.as_str() {
            "PASS" => ui::BadgeVariant::Success,
            "FAIL" => ui::BadgeVariant::Danger,
            _ => ui::BadgeVariant::Neutral,
        };
        checks = checks.child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(ui::badge(model, sanitize(&status, 16)).variant(variant))
                .child(text(name).flex_1().min_w_0()),
        );
    }
    body.child(checks).child(saved(model))
}
