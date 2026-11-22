//! Handoff: the explicitly selected task's saved handoff.
use crate::*;
use precept_ui_core::i18n::Msg;
use precept_ui_core::presentation::sanitize;
use shell::*;
use tokens::palette;

pub(crate) fn handoff(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    let p = m.project().expect("ready project");
    let mut body = view().flex_shrink_0().gap_3();
    let Some(h) = p.handoff.as_ref().filter(|h| h.task_id == m.queried_task) else {
        return body.child(card(model, m.word(Msg::ResumeSummary), m.visible_lines()));
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
    body.child(checks)
}
