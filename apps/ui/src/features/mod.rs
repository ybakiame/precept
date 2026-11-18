//! Feature screens. Each composes the `ui` layer; none styles controls itself.
pub mod brand_preview;
pub(crate) mod connection;
pub(crate) mod handoff;
pub(crate) mod overview;
pub(crate) mod preferences;
pub(crate) mod projects;
pub(crate) mod review;
pub(crate) mod sessions;
pub(crate) mod settings;

use crate::*;
use acp_operator_core::Section;
use acp_operator_core::i18n::Msg;
use handoff::handoff;
use overview::{daemon, dashboard};
use preferences::preferences;
use shell::*;
use tokens::compact;

/// The current section's page.
pub(crate) fn page(model: Signal<Model>) -> impl IntoElement {
    let current = model.get();
    let mut body = view()
        .flex_shrink_0()
        .gap_3()
        .style_when(compact, |s| s.gap_2())
        .on_layout(move |_: &Bounds<Points>| {
            #[cfg(not(target_arch = "wasm32"))]
            if model.get().phase == Phase::Ready && std::env::var_os("ACP_UI_PROBE").is_some() {
                eprintln!("ACP_NSH_LAYOUT_READY elapsed={:.6}", probe_elapsed());
            }
        });
    if current.phase != Phase::Ready {
        return body.child(daemon(model)).child(card(
            model,
            current.read_label(),
            vec![current.status(), current.word(Msg::RetryHelp).into()],
        ));
    }
    if current.section == Section::Dashboard {
        return body.child(dashboard(model));
    }
    if current.section == Section::Preferences {
        return body.child(preferences(model));
    }
    if matches!(current.section, Section::Handoff | Section::Diagnostics) {
        body = body.child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_1()
                .child(control(model, Msg::ExpandCollapse, move || {
                    model.update(|m| m.expanded = !m.expanded)
                }))
                .child(word_action(
                    model,
                    Msg::EarlierEvidence,
                    move || model.get().evidence_page > 0,
                    move || model.update(|m| m.evidence_by(-1)),
                ))
                .child(word_action(
                    model,
                    Msg::LaterEvidence,
                    move || model.get().evidence_page + 1 < model.get().evidence_pages(),
                    move || model.update(|m| m.evidence_by(1)),
                ))
                .child(text(format!(
                    "{} {}/{}",
                    current.word(Msg::EvidencePage),
                    current.evidence_page + 1,
                    current.evidence_pages()
                ))),
        );
    }
    if current.section == Section::Handoff {
        return body.child(handoff(model));
    }
    let mut lines = current.visible_lines();
    if current.section == Section::Diagnostics {
        body = body.child(daemon(model));
        lines.extend(super::runtime_lines(current.locale));
    }
    let title = match current.section {
        Section::Dashboard => "Selected project / repository snapshot",
        Section::Projects => current.word(Msg::Projects),
        Section::Handoff => current.word(Msg::ResumeSummary),
        _ => current.word(Msg::Diagnostics),
    };
    body.child(card(model, title, lines))
}
