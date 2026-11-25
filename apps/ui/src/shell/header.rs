use super::{
    chrome, control,
    navigation::{Destination, copyable_destination, current, title_key},
    workspace_scoped,
};
use crate::*;
use precept_ui_core::{SetupTab, i18n::Msg};
use tokens::palette;

/// The page's longer explanation, shown behind the header's info button.
fn page_help(m: &Model) -> Option<String> {
    let words = |keys: &[Msg]| {
        keys.iter()
            .map(|k| m.word(*k).to_owned())
            .collect::<Vec<_>>()
            .join(" ")
    };
    Some(match current(m) {
        Destination::RuntimeChecks => match m.setup_tab {
            SetupTab::Connection => words(&[Msg::RuntimeChecksIntro, Msg::ConnectionHelp]),
            SetupTab::Profile => words(&[Msg::ProfileIntro]),
            SetupTab::Workspaces => words(&[Msg::RuntimeChecksIntro]),
        },
        Destination::Workspaces => words(&[Msg::WorkspacesIntro]),
        Destination::Agents => words(&[Msg::AgentsIntro]),
        Destination::Sessions => words(&[Msg::SessionsIntro]),
        Destination::Memory => words(&[Msg::MemoryIntro]),
        Destination::Review | Destination::ReviewedRules => words(&[
            Msg::OperatorHelp,
            Msg::ReviewProvenanceNote,
            Msg::OperatorCliGate,
        ]),
        Destination::Rules => words(&[Msg::PreferencesSources]),
        _ => return None,
    })
}

/// Calm scope badge shown beside the page title on workspace-scoped pages.
fn header_scope_badge(model: Signal<Model>) -> impl IntoElement {
    let state = computed(move || {
        let m = model.get();
        if workspace_scoped(current(&m)) {
            m.selected_project
                .clone()
                .or_else(|| m.project().map(|p| p.name.clone()))
        } else {
            None
        }
    });
    dynamic(move || {
        let Some(name) = state.get() else {
            return view().into_element();
        };
        ui::badge(model, precept_ui_core::presentation::sanitize(&name, 48))
            .variant(ui::BadgeVariant::Neutral)
            .into_element()
    })
}

/// The destination's title, the workspace it is scoped to, and its help.
fn title_cluster(model: Signal<Model>) -> impl IntoElement {
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child({
            // The title arrives with each destination (a short
            // rise), so a page change reads as one.
            let destination = computed(move || current(&model.get()));
            dynamic(move || {
                let _ = destination.get();
                let arrive = ui::motion::page();
                text(move || model.get().word(title_key(current(&model.get()))))
                    .role(AccessibilityRole::Header)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(ui::scale::text::TITLE))
                    .bind_motion(&arrive)
                    .into_element()
            })
        })
        .child(header_scope_badge(model))
        // A page's longer explanation, behind an info button.
        .child(
            view()
                .style_when(move || page_help(&model.get()).is_none(), |s| s.hidden())
                .child(ui::info(
                    model,
                    move || model.get().word(Msg::AboutThisPage).into(),
                    move || page_help(&model.get()).unwrap_or_default().into(),
                )),
        )
}

/// The top bar of the page column: show or hide the sidebar, the page's
/// title, the version, then the window's buttons. Its empty space moves the
/// window. With the sidebar hidden, the brand mark leads it.
pub(super) fn top_bar(model: Signal<Model>) -> impl IntoElement {
    let collapsed = move || model.get().sidebar_collapsed;
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .h(px(ui::scale::layout::TITLE_BAR))
        .pl_3()
        .flex_shrink_0()
        .child(
            view()
                .style_when(move || !collapsed(), |s| s.hidden())
                .text_color(move || palette(model).primary)
                .child(ui::icon::glyph(icons::BRAND, ui::scale::icon::MD)),
        )
        .child(
            ui::icon_button(model, icons::SIDEBAR, move || {
                let m = model.get();
                m.word(if m.sidebar_collapsed {
                    Msg::SidebarShow
                } else {
                    Msg::SidebarHide
                })
                .into()
            })
            .test_id("sidebar.toggle")
            .variant(ui::ButtonVariant::Ghost)
            .size(ui::ButtonSize::Small)
            .on_press(move || model.update(|m| m.sidebar_collapsed = !m.sidebar_collapsed)),
        )
        .child(
            view()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex_row()
                .items_center()
                .window_drag(true)
                .child(title_cluster(model)),
        )
        .child(version_badge(model))
        .child(chrome::window_buttons(model))
}

/// The app's version, quiet, beside the window's buttons.
fn version_badge(model: Signal<Model>) -> impl IntoElement {
    let version = env!("CARGO_PKG_VERSION");
    view()
        .accessible(true)
        .label(move || {
            model
                .get()
                .word(Msg::VersionLabel)
                .replace("{version}", version)
        })
        .px_2()
        .py_0p5()
        .mr_2()
        .rounded(px(ui::scale::radius::PILL))
        .bg(move || palette(model).inset)
        .child(
            text(format!("v{version}"))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
}

/// The page-wide actions. Target and read details stay in the sidebar status
/// tooltip rather than a second status line in the content area.
pub(super) fn header(model: Signal<Model>) -> impl IntoElement {
    view().flex_row().items_start().justify_end().gap_3().child(
        view()
            .flex_row()
            .gap_2()
            .child(
                ui::button(model, "")
                    .test_id("page.reconnect")
                    .label_fn(move || model.get().word(Msg::Reconnect).into())
                    .icon(icons::REFRESH)
                    .variant(ui::ButtonVariant::Ghost)
                    .size(ui::ButtonSize::Small)
                    .on_press(move || reconnect(model)),
            )
            .child(
                view()
                    .style_when(
                        move || !copyable_destination(current(&model.get())),
                        |s| s.hidden(),
                    )
                    .child(
                        ui::copy_button(
                            model,
                            move || model.get().word(Msg::CopyPage).into(),
                            move || model.get().word(Msg::CopiedShort).into(),
                            move || copy_page(model),
                        )
                        .test_id("page.copy")
                        .variant(ui::ButtonVariant::Ghost)
                        .size(ui::ButtonSize::Small)
                        .disabled(move || {
                            let m = model.get();
                            m.phase != Phase::Ready || !copyable_destination(current(&m))
                        }),
                    ),
            ),
    )
}

pub(super) fn task_input(model: Signal<Model>, task_ref: ElementRef) -> impl IntoElement {
    view()
        .gap_1()
        .style_when(
            move || current(&model.get()) != Destination::Handoff,
            |s| s.hidden(),
        )
        .child(
            text(move || model.get().word(Msg::TaskHint))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(
                    view().flex_1().child(
                        ui::input(model, "")
                            .label_fn(move || model.get().word(Msg::TaskId).into())
                            .element_ref(&task_ref)
                            .value(move || model.get().task_id)
                            .placeholder("Task ID")
                            .on_change(move |v| {
                                model.update(|m| m.task_id = v.chars().take(121).collect())
                            })
                            .on_submit(move |_| reconnect(model)),
                    ),
                )
                .child(control(model, Msg::Lookup, move || reconnect(model))),
        )
}

pub(super) fn preference_input(model: Signal<Model>, filter_ref: ElementRef) -> impl IntoElement {
    view()
        .gap_1()
        .style_when(
            move || current(&model.get()) != Destination::Rules,
            |s| s.hidden(),
        )
        .gap_2()
        .child(
            text(move || model.get().word(Msg::FilterHelp))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
        .child(
            ui::input(model, "")
                .label_fn(move || model.get().word(Msg::Filter).into())
                .element_ref(&filter_ref)
                .value(move || model.get().filter)
                .on_change(move |v| model.update(|m| m.set_filter(v))),
        )
}
