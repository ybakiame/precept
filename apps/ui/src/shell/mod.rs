//! The window shell: a sidebar of destinations beside one page with its header
//! and the only scroll region (see DESIGN.md, "Window shell"). Also the shared
//! action helpers the feature pages use.
use crate::*;
pub(crate) mod chrome;
use acp_operator_core::i18n::Msg;
use acp_operator_core::{PAGE_SIZE, Section};
use features::{agents, connection, page, profile, projects, review, sessions, settings};
use tokens::{compact, palette};

pub(crate) fn action(
    model: Signal<Model>,
    label: impl Into<String>,
    enabled: impl Fn() -> bool + 'static,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    ui::button(model, label.into())
        .disabled(move || !enabled())
        .on_press(callback)
}
/// A labelled action whose focus stop reports a closed catalog key (e.g. an
/// index), never its label or other user/daemon content.
pub(crate) fn keyed_action(
    model: Signal<Model>,
    label: impl Into<String>,
    key: &'static str,
    index: usize,
    enabled: impl Fn() -> bool + 'static,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    ui::button(model, label.into())
        .probe_key(move || format!("{key}{index}"))
        .disabled(move || !enabled())
        .on_press(callback)
}
pub(crate) fn word_action(
    model: Signal<Model>,
    key: Msg,
    enabled: impl Fn() -> bool + 'static,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    ui::button(model, "")
        .label_fn(move || model.get().word(key).into())
        .probe_key(move || format!("{key:?}"))
        .disabled(move || !enabled())
        .on_press(move || {
            #[cfg(not(target_arch = "wasm32"))]
            if std::env::var_os("ACP_UI_PROBE").is_some() {
                eprintln!("ACP_UI_ACTION key={key:?}");
            }
            callback();
        })
}
pub(crate) fn control(
    model: Signal<Model>,
    key: Msg,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    word_action(model, key, || true, callback)
}
pub(crate) fn section_key(section: Section) -> Msg {
    match section {
        Section::Dashboard => Msg::Dashboard,
        Section::Projects => Msg::Projects,
        Section::Preferences => Msg::Preferences,
        Section::Handoff => Msg::Handoff,
        Section::Diagnostics => Msg::Diagnostics,
    }
}
pub(crate) fn card(
    model: Signal<Model>,
    title: &'static str,
    lines: Vec<String>,
) -> impl IntoElement {
    ui::card(model, title, lines)
}
/// Where the window is: one of the sections, or a workspace destination.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Destination {
    Section(Section),
    Review,
    Sessions,
    Connection,
    Profile,
    Agents,
    Settings,
}
pub(crate) fn current(m: &Model) -> Destination {
    if m.operator.open {
        Destination::Review
    } else if m.sessions {
        Destination::Sessions
    } else if m.settings {
        Destination::Settings
    } else if m.connection_editor {
        Destination::Connection
    } else if m.profile_page {
        Destination::Profile
    } else if m.agents_page {
        Destination::Agents
    } else {
        Destination::Section(m.section)
    }
}
fn leave_workspaces(m: &mut Model) {
    m.operator.open = false;
    m.operator.review = None;
    m.sessions = false;
    m.session.confirm_start = false;
    m.settings = false;
    m.connection_editor = false;
    m.profile_page = false;
    m.agents_page = false;
}
/// Goes to `destination` (each opener keeps its own reads and guards).
pub(crate) fn go(model: Signal<Model>, destination: Destination) {
    match destination {
        Destination::Review => review::open(model),
        Destination::Sessions => sessions::open(model),
        Destination::Connection => {
            model.update(|m| {
                leave_workspaces(m);
                m.connection_editor = true;
            });
            connection::opened(model);
        }
        Destination::Settings => model.update(|m| {
            leave_workspaces(m);
            m.settings = true;
        }),
        Destination::Profile => model.update(|m| {
            leave_workspaces(m);
            m.profile_page = true;
        }),
        Destination::Agents => model.update(|m| {
            leave_workspaces(m);
            m.agents_page = true;
        }),
        Destination::Section(section) => {
            model.update(|m| {
                leave_workspaces(m);
                m.set_section(section)
            });
        }
    }
}
fn title_key(destination: Destination) -> Msg {
    match destination {
        Destination::Section(section) => section_key(section),
        Destination::Review => Msg::ProfileReview,
        Destination::Sessions => Msg::Sessions,
        Destination::Connection => Msg::Connection,
        Destination::Profile => Msg::ProfileSection,
        Destination::Agents => Msg::AgentsDestination,
        Destination::Settings => Msg::Settings,
    }
}
/// Whether a set-up step is complete, from the last daemon read.
fn step_done(m: &Model, destination: Destination) -> Option<bool> {
    let data = m.data.as_ref();
    Some(match destination {
        Destination::Connection => m.live && data.is_some(),
        Destination::Profile => data.is_some_and(|d| d.health.schema != "Unselected"),
        Destination::Section(acp_operator_core::Section::Projects) => {
            data.is_some_and(|d| !d.projects.is_empty())
        }
        Destination::Agents => {
            data.is_some_and(|d| d.directory.iter().any(|p| p.agents && p.state == "ready"))
        }
        _ => return None,
    })
}
/// A set-up step: its number, its name and, once complete, a check mark.
fn step(
    model: Signal<Model>,
    number: u8,
    item: Destination,
    path: &'static str,
) -> impl IntoElement {
    ui::step_item(
        model,
        path,
        move || {
            let m = model.get();
            let done = step_done(&m, item) == Some(true);
            format!(
                "{number}. {}{}",
                m.word(title_key(item)),
                if done { "  ✓" } else { "" }
            )
            .into()
        },
        move || {
            let m = model.get();
            format!(
                "{} {number}: {}",
                m.word(Msg::StepWord),
                m.word(title_key(item))
            )
            .into()
        },
        move || current(&model.get()) == item,
        move || go(model, item),
    )
}
fn group_label(model: Signal<Model>, key: Msg) -> impl IntoElement {
    text(move || model.get().word(key))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(ui::scale::text::CAPTION))
        .text_color(move || palette(model).muted)
        .px_2()
        .style_when(compact, |s| s.hidden())
}
fn destination(model: Signal<Model>, item: Destination, path: &'static str) -> impl IntoElement {
    ui::nav_item(
        model,
        path,
        move || model.get().word(title_key(item)).into(),
        move || current(&model.get()) == item,
        move || go(model, item),
    )
}
/// The project menu: the daemon's ready projects, the current one checked.
fn projects(model: Signal<Model>) -> impl IntoElement {
    let names = computed(move || model.get().project_names.clone());
    dynamic(move || {
        let names = names.get();
        if names.is_empty() {
            return text(move || model.get().word(Msg::ChooseProject))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted)
                .into_element();
        }
        names
            .into_iter()
            .enumerate()
            .fold(
                ui::menu(model, move || current_project(model).into()),
                |menu, (index, name)| {
                    menu.item(
                        name,
                        move || model.get().selected == index,
                        move || switch_project(model, index),
                    )
                },
            )
            .into_element()
    })
}
fn current_project(model: Signal<Model>) -> String {
    let m = model.get();
    m.selected_project
        .clone()
        .or_else(|| m.project().map(|p| p.name.clone()))
        .unwrap_or_else(|| m.word(Msg::ChooseProject).into())
}
/// Where the data comes from, in words: the target and, for WSL, the distribution.
pub(crate) fn target_words(m: &Model) -> String {
    match m.connection.target {
        acp_operator_core::connection::Target::Wsl if !m.connection.distribution.is_empty() => {
            format!("{} · {}", m.target_label(), m.connection.distribution)
        }
        _ => m.target_label().into(),
    }
}
fn sidebar(model: Signal<Model>) -> impl IntoElement {
    use Destination::*;
    use acp_operator_core::Section as S;
    // Set up: the guided path, in order. Project: the current project's work.
    let setup = [
        (Connection, icons::CONNECTION),
        (Profile, icons::PROFILE),
        (Section(S::Projects), icons::section_path(S::Projects)),
        (Agents, icons::AGENTS),
    ];
    let work = [
        (Section(S::Dashboard), icons::section_path(S::Dashboard)),
        (Section(S::Preferences), icons::section_path(S::Preferences)),
        (Review, icons::REVIEW),
        (Section(S::Handoff), icons::section_path(S::Handoff)),
        (Sessions, icons::SESSIONS),
    ];
    let foot = [
        (Section(S::Diagnostics), icons::section_path(S::Diagnostics)),
        (Settings, icons::SETTINGS),
    ];
    let row = || {
        view()
            .gap_0p5()
            .style_when(compact, |s| s.flex_row().flex_wrap().gap_1())
    };
    let setup_group = row().children(
        setup
            .into_iter()
            .zip(1u8..)
            .map(move |((d, path), n)| step(model, n, d, path)),
    );
    let work_group = row().children(
        work.into_iter()
            .map(move |(d, path)| destination(model, d, path)),
    );
    let foot_group = row().children(
        foot.into_iter()
            .map(move |(d, path)| destination(model, d, path)),
    );
    view()
        .w(move || {
            px(if compact() {
                (window().width.0 - 16.0).max(1.0)
            } else {
                ui::scale::layout::SIDEBAR
            })
        })
        .flex_shrink_0()
        .gap_4()
        .p_3()
        // Full height so the foot group sits at the bottom; a wrapped row when compact.
        .style_when(move || !compact(), |s| s.h_full())
        .style_when(compact, |s| s.p_2().gap_2())
        .child(
            text("Agent Control Plane")
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(ui::scale::text::SUBTITLE))
                .style_when(compact, |s| s.hidden()),
        )
        .child(
            view()
                .gap_1()
                .child(group_label(model, Msg::SetupGroup))
                .child(setup_group),
        )
        .child(
            view()
                .gap_1()
                .child(group_label(model, Msg::ProjectGroup))
                .child(view().px_2().child(projects(model)))
                .child(work_group),
        )
        .child(view().flex_1().style_when(compact, |s| s.hidden()))
        .child(foot_group)
}
/// The page header: the destination's title, one human status line and the
/// page-wide actions.
fn header(model: Signal<Model>) -> impl IntoElement {
    view()
        .flex_row()
        .items_start()
        .gap_3()
        .child(
            view()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    text(move || model.get().word(title_key(current(&model.get()))))
                        .role(AccessibilityRole::Header)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(ui::scale::text::TITLE)),
                )
                .child(
                    view()
                        .accessible(true)
                        .label("Daemon connection state")
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            text(move || {
                                let m = model.get();
                                if let Some(reason) = m.wsl_reason() {
                                    format!("{} · {}", target_words(&m), m.word(reason))
                                } else if m.operator.open {
                                    format!(
                                        "{} · {}",
                                        target_words(&m),
                                        m.word(if m.operator.busy {
                                            Msg::OperatorBusy
                                        } else if m.operator.error.is_some() {
                                            Msg::OperatorFailed
                                        } else if m.operator.info.is_some() {
                                            Msg::OperatorObserved
                                        } else {
                                            Msg::OperatorUnobserved
                                        })
                                    )
                                } else {
                                    format!("{} · {}", target_words(&m), m.read_label())
                                }
                            })
                            .text_size(px(ui::scale::text::CAPTION))
                            .text_color(move || {
                                match model.get().phase {
                                    _ if model.get().wsl_reason().is_some() => palette(model).error,
                                    Phase::Error | Phase::Unavailable => palette(model).error,
                                    Phase::Loading => palette(model).warning,
                                    _ => palette(model).muted,
                                }
                            }),
                        )
                        .child(
                            text(move || {
                                let m = model.get();
                                if m.copied {
                                    m.word(Msg::Copied).into()
                                } else {
                                    m.read_time()
                                }
                            })
                            .style_when(move || model.get().operator.open, |s| s.hidden())
                            .text_size(px(ui::scale::text::CAPTION))
                            .text_color(move || palette(model).muted),
                        ),
                ),
        )
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(
                    ui::button(model, "")
                        .label_fn(move || model.get().word(Msg::Reconnect).into())
                        .icon(icons::REFRESH)
                        .size(ui::ButtonSize::Small)
                        .on_press(move || reconnect(model)),
                )
                .child(
                    ui::button(model, "")
                        .label_fn(move || model.get().word(Msg::CopyPage).into())
                        .size(ui::ButtonSize::Small)
                        .disabled(move || {
                            let m = model.get();
                            m.phase != Phase::Ready
                                || !matches!(current(&m), Destination::Section(_))
                        })
                        .on_press(move || copy_page(model)),
                ),
        )
}
fn task_input(model: Signal<Model>, task_ref: ElementRef) -> impl IntoElement {
    view()
        .gap_1()
        .style_when(
            move || {
                model.get().settings
                    || model.get().sessions
                    || model.get().operator.open
                    || model.get().connection_editor
                    || model.get().section != Section::Handoff
            },
            |s| s.hidden(),
        )
        .child(text(move || model.get().word(Msg::TaskHint)).text_size(px(12.0)))
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
fn preference_input(model: Signal<Model>, filter_ref: ElementRef) -> impl IntoElement {
    view()
        .gap_1()
        .style_when(
            move || {
                model.get().settings
                    || model.get().sessions
                    || model.get().operator.open
                    || model.get().connection_editor
                    || model.get().section != Section::Preferences
            },
            |s| s.hidden(),
        )
        .child(text(move || model.get().word(Msg::FilterHelp)).text_size(px(12.0)))
        .child(
            ui::input(model, "")
                .label_fn(move || model.get().word(Msg::Filter).into())
                .element_ref(&filter_ref)
                .value(move || model.get().filter)
                .on_change(move |v| model.update(|m| m.set_filter(v))),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_1()
                .child(word_action(
                    model,
                    Msg::PreviousPage,
                    move || model.get().page > 0,
                    move || model.update(|m| m.page_by(-1)),
                ))
                .child(word_action(
                    model,
                    Msg::NextPage,
                    move || (model.get().page + 1) * PAGE_SIZE < model.get().rows().len(),
                    move || model.update(|m| m.page_by(1)),
                ))
                .child(text(move || model.get().page_label())),
        )
}
pub(crate) fn shell(
    model: Signal<Model>,
    task_ref: ElementRef,
    filter_ref: ElementRef,
) -> impl IntoElement {
    profile::watch(model);
    agents::watch(model);
    let on = move |d: fn(&Model) -> bool| move || !d(&model.get());
    view()
        .flex_1()
        .bg(move || palette(model).background)
        .child(
            view()
                .style_when(
                    || TERMINAL_SURFACE.load(std::sync::atomic::Ordering::Relaxed),
                    |s| s.hidden(),
                )
                .child(chrome::titlebar(model)),
        )
        .child(
            view()
                .flex_1()
                .flex_row()
                .style_when(compact, |s| s.flex_col())
                .font_family("sans-serif")
                .bg(move || palette(model).background)
                .text_color(move || palette(model).text)
                .text_size(px(ui::scale::text::BODY))
                .child(
                    view()
                        .bg(move || palette(model).panel)
                        .border_color(move || palette(model).border)
                        .style_when(move || !compact(), |s| s.border_r_1())
                        .style_when(compact, |s| s.border_b_1())
                        .child(sidebar(model)),
                )
                .child(
                    view()
                        .flex_1()
                        .min_w_0()
                        .px_6()
                        .pt_4()
                        .gap_4()
                        .style_when(compact, |s| s.px_2().pt_2().gap_2())
                        .child(header(model))
                        .child(task_input(model, task_ref))
                        .child(preference_input(model, filter_ref))
                        .child(review::panel(model))
                        .child(projects::remove_dialog(model))
                        .child(
                            scroll()
                                .flex_1()
                                .style_when(move || model.get().operator.open, |s| s.hidden())
                                .child(
                                    view()
                                        .gap_4()
                                        .pb_6()
                                        .flex_shrink_0()
                                        .child(
                                            view()
                                                .style_when(on(|m| m.settings), |s| s.hidden())
                                                .child(settings::appearance(model)),
                                        )
                                        .child(
                                            view()
                                                .style_when(on(|m| m.connection_editor), |s| {
                                                    s.hidden()
                                                })
                                                .child(connection::connection(model)),
                                        )
                                        .child(
                                            view()
                                                .gap_4()
                                                .max_w(px(ui::scale::layout::FORM))
                                                .style_when(on(|m| m.profile_page), |s| s.hidden())
                                                .child(
                                                    text(move || {
                                                        model.get().word(Msg::ProfileIntro)
                                                    })
                                                    .text_size(px(ui::scale::text::BODY)),
                                                )
                                                .child(profile::section(model)),
                                        )
                                        .child(
                                            view()
                                                .style_when(on(|m| m.agents_page), |s| s.hidden())
                                                .child(agents::page(model)),
                                        )
                                        .child(
                                            view()
                                                .style_when(on(|m| m.sessions), |s| s.hidden())
                                                .child(sessions::panel(model)),
                                        )
                                        .child(
                                            // First-run Profile setup where it is needed.
                                            view()
                                                .style_when(
                                                    move || {
                                                        !matches!(
                                                            current(&model.get()),
                                                            Destination::Section(
                                                                Section::Dashboard
                                                                    | Section::Projects
                                                            )
                                                        )
                                                    },
                                                    |s| s.hidden(),
                                                )
                                                .child(profile::setup_when_needed(model)),
                                        )
                                        .child(
                                            view()
                                                .style_when(
                                                    move || {
                                                        current(&model.get())
                                                            != Destination::Section(
                                                                Section::Projects,
                                                            )
                                                    },
                                                    |s| s.hidden(),
                                                )
                                                .child(projects::page(model)),
                                        )
                                        .child(
                                            view()
                                                .style_when(
                                                    move || {
                                                        !matches!(
                                                            current(&model.get()),
                                                            Destination::Section(_)
                                                        )
                                                    },
                                                    |s| s.hidden(),
                                                )
                                                .child(dynamic(move || page(model))),
                                        ),
                                ),
                        ),
                ),
        )
        .child(
            view()
                .style_when(
                    || TERMINAL_SURFACE.load(std::sync::atomic::Ordering::Relaxed),
                    |s| s.hidden(),
                )
                .child(chrome::grip(model)),
        )
}
