use super::chrome::{compact, stack_cards};
use super::controls::{Ui, action, color, muted, panel};
use nsh_base::button;
use nsh_core::prelude::*;
use precept_ui_core::{Model, i18n::Msg, presentation::sanitize};

fn workspace_cards(ui: Signal<Ui>, model: Signal<Model>) -> impl IntoElement {
    let s = ui.get();
    let m = model.get();
    view()
        .flex_row()
        .gap_2()
        .style_when(stack_cards, |s| s.flex_col())
        .child(
            view()
                .flex_1()
                .p_3()
                .gap_2()
                .style_when(super::chrome::compact, |s| s.p_2().gap_1())
                .rounded(px(8.0))
                .border_1()
                .border_color(color(s.colors().border))
                .bg(color(s.colors().panel))
                .child(text(s.word(Msg::Projects)).font_weight(FontWeight::SEMIBOLD))
                .child(
                    view().flex_row().flex_wrap().gap_2().children(
                        m.project_names
                            .iter()
                            .cloned()
                            .enumerate()
                            .map(move |(index, name)| {
                                button()
                                    .px_2()
                                    .py_1()
                                    .rounded(px(6.0))
                                    .border_2()
                                    .accessibility_label(format!(
                                        "{} {name}",
                                        s.word(Msg::CurrentWorkspace)
                                    ))
                                    .selected(move || model.get().selected == index)
                                    .bg(move || {
                                        let p = ui.get().colors();
                                        color(if model.get().selected == index {
                                            p.accent
                                        } else {
                                            p.inset
                                        })
                                    })
                                    .text_color(move || {
                                        let p = ui.get().colors();
                                        color(if model.get().selected == index {
                                            p.on_accent
                                        } else {
                                            p.text
                                        })
                                    })
                                    .border_color(move || color(ui.get().colors().border))
                                    .focus(|s| s.border_color(color(ui.get().colors().text)))
                                    .on_press(move || {
                                        model.update(|m| {
                                            m.select(index);
                                            m.task_id.clear();
                                            m.queried_task.clear();
                                        })
                                    })
                                    .child(text(name))
                            }),
                    ),
                )
                .child(muted(ui, s.word(Msg::WorkspaceHelp)).style_when(compact, |s| s.hidden())),
        )
        .child(view().flex_1().child(panel(
            ui,
            Msg::ClientAccess,
            vec![
                s.word(Msg::ClientPresence).into(),
                s.word(Msg::Tools).into(),
                s.word(Msg::Transport).into(),
            ],
        )))
}
pub(super) fn dashboard(ui: Signal<Ui>, model: Signal<Model>) -> impl IntoElement {
    let s = ui.get();
    let m = model.get();
    let p = m.project().expect("built-in preview snapshot");
    let r = &p.context.repository;
    let h = p.handoff.as_ref().filter(|h| h.task_id == m.queried_task);
    view()
        .gap_2()
        .child(workspace_cards(ui, model))
        .child(panel(
            ui,
            Msg::EffectiveContext,
            vec![
                format!(
                    "{} · {}",
                    s.word(Msg::CurrentWorkspace),
                    sanitize(&p.name, 120)
                ),
                format!(
                    "{}: {} · HEAD: {}",
                    s.word(Msg::Branch),
                    r.branch.as_deref().unwrap_or(s.word(Msg::Unknown)),
                    r.head.as_deref().unwrap_or(s.word(Msg::Unknown))
                ),
                format!(
                    "{}: {}",
                    s.word(Msg::Worktree),
                    s.word(match r.dirty {
                        Some(true) => Msg::HasChanges,
                        Some(false) => Msg::NoChanges,
                        None => Msg::Unknown,
                    })
                ),
                format!(
                    "{}: {} · {}",
                    s.word(Msg::ContractReferences),
                    p.context.documents.len(),
                    s.word(Msg::MetadataOnly)
                ),
                format!(
                    "{}: {} · {}",
                    s.word(Msg::ConfirmedPreferences),
                    p.context.preferences.len(),
                    s.word(Msg::ConfirmedOnly)
                ),
            ],
        ))
        .child(panel(
            ui,
            Msg::ResumeSummary,
            vec![
                h.map_or_else(
                    || s.word(Msg::NoHandoff).into(),
                    |h| {
                        format!(
                            "{}: {} · {}",
                            s.word(Msg::ExactTask),
                            sanitize(&h.task_id, 120),
                            sanitize(&h.objective, 160)
                        )
                    },
                ),
                s.word(Msg::EarlierNotCurrent).into(),
            ],
        ))
        .child(action(
            ui,
            Msg::OpenHandoff,
            || false,
            move || ui.update(|s| s.config.page = super::config::Page::Handoff),
        ))
        .child(panel(
            ui,
            Msg::ReviewSuggestions,
            vec![
                s.word(Msg::ReviewDeferred).into(),
                s.word(Msg::ReviewHelp).into(),
            ],
        ))
        .child(muted(ui, s.word(Msg::ClientHelp)))
}
