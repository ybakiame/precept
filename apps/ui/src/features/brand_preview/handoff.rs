use super::chrome::compact;
use super::controls::{Ui, action, color, muted, panel};
use nsh_core::prelude::*;
use precept_ui_core::{Model, i18n::Msg, presentation::sanitize};

pub(super) fn handoff_page(ui: Signal<Ui>, model: Signal<Model>) -> impl IntoElement {
    let s = ui.get();
    let m = model.get();
    let p = m.project().expect("built-in preview snapshot");
    let Some(h) = p.handoff.as_ref().filter(|h| h.task_id == m.queried_task) else {
        return view().child(panel(ui, Msg::Handoff, vec![s.word(Msg::NoHandoff).into()]));
    };
    let mut body = view()
        .gap_2()
        .child(muted(
            ui,
            format!(
                "{} / {} / {}",
                s.word(Msg::ExactTask),
                sanitize(&p.name, 120),
                sanitize(&h.task_id, 120)
            ),
        ))
        .child(panel(ui, Msg::Objective, vec![h.objective.clone()]))
        .child(panel(ui, Msg::NextAction, vec![h.next_action.clone()]))
        .child(
            view()
                .p_3()
                .gap_1()
                .style_when(compact, |s| s.p_2())
                .rounded(px(8.0))
                .border_2()
                .border_color(color(s.colors().warning))
                .child(
                    text(s.word(Msg::CurrentUncertainty))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(color(s.colors().warning)),
                )
                .child(text(format!(
                    "HEAD / {}: {}",
                    s.word(Msg::Fingerprint),
                    s.word(Msg::Unknown)
                )))
                .child(muted(ui, s.word(Msg::EarlierNotCurrent))),
        )
        .child(panel(
            ui,
            Msg::RecordedEvidence,
            vec![
                format!("{}: {}", s.word(Msg::RecordedEvidence), h.validation.len()),
                s.word(Msg::CurrentValidation).into(),
                s.word(Msg::EvidenceHelp).into(),
            ],
        ))
        .child(action(
            ui,
            if s.expanded {
                Msg::CollapseEvidence
            } else {
                Msg::ExpandEvidence
            },
            || false,
            move || ui.update(|s| s.expanded = !s.expanded),
        ));
    if s.expanded {
        body = body.child(
            view().p_2().gap_1().bg(color(s.colors().inset)).children(
                h.validation
                    .iter()
                    .take(6)
                    .map(|entry| text(sanitize(&entry.to_string(), 512))),
            ),
        );
    }
    body.child(muted(ui, s.word(Msg::PlainData)))
}
