//! Runtime checks: one guided setup surface combining Connection, Profile and
//! Workspaces as three tabs with live completion state.
use crate::*;
use precept_ui_core::{SetupTab, i18n::Msg};
use tokens::palette;
use ui::scale;

/// Completion status of each runtime-setup step, from fresh daemon reads.
pub(crate) fn step_done(m: &Model, tab: SetupTab) -> bool {
    let data = m.data.as_ref();
    match tab {
        SetupTab::Connection => m.live && data.is_some(),
        SetupTab::Profile => {
            m.profile_info.as_ref().is_some_and(|i| i.selected)
                || data.is_some_and(|d| d.health.schema != "Unselected")
        }
        SetupTab::Workspaces => data.is_some_and(|d| !d.projects.is_empty()),
    }
}

/// How many of the three runtime-setup steps are currently complete (0..=3).
pub(crate) fn completed_count(m: &Model) -> u8 {
    [
        SetupTab::Connection,
        SetupTab::Profile,
        SetupTab::Workspaces,
    ]
    .into_iter()
    .filter(|tab| step_done(m, *tab))
    .count() as u8
}

fn tab_label(tab: SetupTab) -> Msg {
    match tab {
        SetupTab::Connection => Msg::Connection,
        SetupTab::Profile => Msg::ProfileSection,
        SetupTab::Workspaces => Msg::Projects,
    }
}

fn tab_icon(tab: SetupTab) -> &'static str {
    match tab {
        SetupTab::Connection => icons::CONNECTION,
        SetupTab::Profile => icons::PROFILE,
        SetupTab::Workspaces => icons::section_path(precept_ui_core::Section::Projects),
    }
}

fn step_tab(model: Signal<Model>, tab: SetupTab) -> impl IntoElement {
    ui::step_item(
        model,
        tab_icon(tab),
        move || model.get().word(tab_label(tab)).into(),
        move || model.get().word(tab_label(tab)).into(),
        move || model.get().setup_tab == tab,
        move || step_done(&model.get(), tab),
        move || shell::go_setup(model, tab),
    )
}

/// The Runtime checks page (Connection, Profile, Workspaces).
pub(crate) fn page(model: Signal<Model>) -> impl IntoElement {
    let bar = view()
        .flex_row()
        .flex_wrap()
        .gap_1()
        .p_1()
        .max_w(px(scale::layout::FORM))
        .rounded(px(scale::radius::CONTAINER))
        .border_1()
        .border_color(move || palette(model).hairline)
        .bg(move || palette(model).panel)
        .child(
            view()
                .flex_1()
                .min_w(px(140.0))
                .child(step_tab(model, SetupTab::Connection)),
        )
        .child(
            view()
                .flex_1()
                .min_w(px(140.0))
                .child(step_tab(model, SetupTab::Profile)),
        )
        .child(
            view()
                .flex_1()
                .min_w(px(140.0))
                .child(step_tab(model, SetupTab::Workspaces)),
        );

    view()
        .gap_6()
        .child(bar)
        .child(
            view()
                .style_when(
                    move || model.get().setup_tab != SetupTab::Connection,
                    |s| s.hidden().invisible(),
                )
                .child(super::connection::connection(model)),
        )
        .child(
            view()
                .style_when(
                    move || model.get().setup_tab != SetupTab::Profile,
                    |s| s.hidden().invisible(),
                )
                .child(super::profile::section(model)),
        )
        .child(
            view()
                .style_when(
                    move || model.get().setup_tab != SetupTab::Workspaces,
                    |s| s.hidden().invisible(),
                )
                .child(super::projects::page(model)),
        )
}
