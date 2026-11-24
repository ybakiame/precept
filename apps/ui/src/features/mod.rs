//! Feature screens. Each composes the `ui` layer; none styles controls itself.
pub(crate) mod agents;
pub mod brand_preview;
pub(crate) mod connection;
pub(crate) mod doctor;
pub(crate) mod handoff;
pub(crate) mod memory;
pub(crate) mod overview;
pub(crate) mod preferences;
pub(crate) mod profile;
pub(crate) mod projects;
pub(crate) mod review;
pub(crate) mod rule_lint;
pub(crate) mod sessions;
pub(crate) mod settings;
pub(crate) mod setup;
pub(crate) mod tags;

use crate::*;
use handoff::handoff;
use overview::{daemon, dashboard, onboarding};
use precept_ui_core::Section;
use precept_ui_core::i18n::Msg;
use preferences::preferences;
use shell::*;
use tokens::compact;

/// The current section's page (Dashboard, Preferences, Handoff, Diagnostics).
pub(crate) fn page(model: Signal<Model>) -> impl IntoElement {
    let current = model.get();
    let body = view()
        .flex_shrink_0()
        .gap_3()
        .style_when(compact, |s| s.gap_2())
        .on_layout(move |_: &Bounds<Points>| {
            if model.get().phase == Phase::Ready && std::env::var_os("PRECEPT_UI_PROBE").is_some() {
                eprintln!("PRECEPT_NSH_LAYOUT_READY elapsed={:.6}", probe_elapsed());
            }
        });
    if current.phase != Phase::Ready {
        if current.section == Section::Dashboard
            && matches!(current.phase, Phase::Empty | Phase::Unavailable)
        {
            return body.child(onboarding(model)).child(daemon(model));
        }
        return body.child(daemon(model)).child(card(
            model,
            current.read_label(),
            vec![current.status(), current.word(Msg::RetryHelp).into()],
        ));
    }
    if current.section == Section::Dashboard {
        return body.child(dashboard(model));
    }
    if current.section == Section::Rules {
        return body.child(preferences(model));
    }
    if current.section == Section::Handoff {
        return body.child(handoff(model));
    }
    let mut lines = current.visible_lines();
    if current.section == Section::Diagnostics {
        lines.extend(super::runtime_lines(current.locale));
        return body.child(
            view()
                .flex_row()
                .items_start()
                .gap_3()
                .style_when(compact, |s| s.flex_col().gap_2())
                .child(view().flex_1().min_w_0().child(daemon(model)))
                .child(view().flex_1().min_w_0().child(card(
                    model,
                    current.word(Msg::Diagnostics),
                    lines,
                ))),
        );
    }
    let title = match current.section {
        Section::Dashboard => "Selected project / repository snapshot",
        Section::Handoff => current.word(Msg::ResumeSummary),
        _ => current.word(Msg::Diagnostics),
    };
    body.child(card(model, title, lines))
}
