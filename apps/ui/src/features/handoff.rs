//! Handoff: the explicitly selected task's saved handoff.
use crate::*;
use acp_ui_core::i18n::Msg;
use shell::*;
use tokens::palette;

pub(crate) fn handoff(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    let p = m.project().expect("ready project");
    let mut body = view().flex_shrink_0().gap_3();
    let Some(h) = p.handoff.as_ref().filter(|h| h.task_id == m.queried_task) else {
        return body.child(card(model, m.word(Msg::ResumeSummary), m.visible_lines()));
    };
    body = body
        .child(card(
            model,
            m.word(Msg::Objective),
            vec![
                format!("{} {} · {}", m.word(Msg::Handoff), h.task_id, p.name),
                h.objective.clone(),
            ],
        ))
        .child(card(
            model,
            m.word(Msg::NextAction),
            vec![h.next_action.clone()],
        ));
    let mut uncertainty = vec![
        m.word(Msg::EarlierNotCurrent).into(),
        format!(
            "{}: {}",
            m.word(Msg::Fingerprint),
            h.current_repository
                .get("worktree_fingerprint")
                .and_then(|v| v.as_str())
                .unwrap_or(m.word(Msg::FingerprintUnavailable))
        ),
    ];
    uncertainty.extend(h.resume_warnings.iter().take(8).cloned());
    body = body.child(
        view()
            .border_2()
            .rounded(px(8.0))
            .border_color(move || palette(model).warning)
            .child(card(model, m.word(Msg::CurrentUncertainty), uncertainty)),
    );
    let mut evidence = vec![
        m.word(Msg::CurrentValidation).into(),
        m.word(Msg::EvidenceHelp).into(),
    ];
    if m.expanded {
        evidence.extend(
            h.validation
                .iter()
                .skip(m.evidence_page * 6)
                .take(6)
                .map(|v| v.to_string()),
        );
    } else {
        evidence.push(format!(
            "{} {}",
            h.validation.len(),
            m.word(Msg::HistoricalEntries)
        ));
    }
    body.child(card(model, m.word(Msg::HistoricalEvidence), evidence))
}
