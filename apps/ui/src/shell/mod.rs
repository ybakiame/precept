//! The window shell: a sidebar of destinations beside one page with its header
//! and the only scroll region (see docs/product-ui.md). Also the shared
//! action helpers the feature pages use.
use crate::*;
pub(crate) mod chrome;
use acp_ui_core::Section;
use acp_ui_core::i18n::Msg;
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
    PersonalPreferences,
    Settings,
}
pub(crate) fn current(m: &Model) -> Destination {
    if m.operator.open {
        // The workspace's entry decides where the window is.
        match m.operator.mode {
            acp_ui_core::operator::ReviewMode::Inbox => Destination::Review,
            acp_ui_core::operator::ReviewMode::Personal => Destination::PersonalPreferences,
            acp_ui_core::operator::ReviewMode::Project => {
                Destination::Section(acp_ui_core::Section::Preferences)
            }
        }
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
        Destination::PersonalPreferences => {
            review::open_mode(model, acp_ui_core::operator::ReviewMode::Personal)
        }
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
        Destination::PersonalPreferences => Msg::PersonalPreferences,
        Destination::Settings => Msg::Settings,
    }
}
/// The page's longer explanation, shown behind the header's info button.
fn page_help(m: &Model) -> Option<String> {
    let words = |keys: &[Msg]| {
        keys.iter()
            .map(|k| m.word(*k).to_owned())
            .collect::<Vec<_>>()
            .join(" ")
    };
    Some(match current(m) {
        Destination::Profile => words(&[Msg::ProfileIntro]),
        Destination::Agents => words(&[Msg::AgentsIntro]),
        Destination::Review | Destination::PersonalPreferences => words(&[
            Msg::OperatorHelp,
            Msg::ReviewProvenanceNote,
            Msg::OperatorCliGate,
        ]),
        Destination::Section(acp_ui_core::Section::Preferences) if m.operator.open => words(&[
            Msg::OperatorHelp,
            Msg::ReviewProvenanceNote,
            Msg::OperatorCliGate,
        ]),
        Destination::Section(acp_ui_core::Section::Preferences) => {
            words(&[Msg::PreferencesSources])
        }
        _ => return None,
    })
}
/// Whether a set-up step is complete, from the last daemon read.
fn step_done(m: &Model, destination: Destination) -> Option<bool> {
    let data = m.data.as_ref();
    Some(match destination {
        Destination::Connection => m.live && data.is_some(),
        Destination::Profile => data.is_some_and(|d| d.health.schema != "Unselected"),
        Destination::Section(acp_ui_core::Section::Projects) => {
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
        move || model.get().word(title_key(item)).into(),
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
        move || step_done(&model.get(), item) == Some(true),
        move || go(model, item),
    )
}
/// The daemon's state in one short word, and the dot that reinforces it.
fn status_of(m: &Model) -> (ui::Status, Msg) {
    use ui::Status;
    if m.wsl_reason().is_some() {
        return (Status::Down, Msg::StatusUnreachable);
    }
    match m.phase {
        Phase::Loading => (Status::Busy, Msg::StatusReading),
        Phase::Ready | Phase::Empty if m.live => (Status::Live, Msg::StatusConnected),
        Phase::Ready | Phase::Empty => (Status::Idle, Msg::StatusPreview),
        Phase::Error => (Status::Attention, Msg::StatusAttention),
        Phase::Unavailable if m.permission_denied => (Status::Down, Msg::StatusRefused),
        Phase::Unavailable => (Status::Down, Msg::StatusUnreachable),
    }
}
/// The sidebar's foot: the daemon's state and target, always visible; it
/// opens Connection.
fn status_row(model: Signal<Model>) -> impl IntoElement {
    let state = computed(move || status_of(&model.get()).0);
    ui::button(model, "")
        .label_fn(move || {
            let m = model.get();
            format!("{} · {}", m.word(status_of(&m).1), target_words(&m)).into()
        })
        .spoken(move || {
            let m = model.get();
            format!(
                "{} {} · {}",
                m.word(Msg::StatusOpensConnection),
                m.word(status_of(&m).1),
                target_words(&m)
            )
            .into()
        })
        .variant(ui::ButtonVariant::Default)
        .size(ui::ButtonSize::Small)
        .full_width()
        .leading(ui::status_dot(model, move || state.get()))
        .selected(move || current(&model.get()) == Destination::Connection)
        .on_press(move || go(model, Destination::Connection))
}
/// Whether a destination shows one project's data (its header carries the
/// project switcher).
pub(crate) fn project_scoped(d: Destination) -> bool {
    matches!(
        d,
        Destination::Section(Section::Dashboard | Section::Preferences | Section::Handoff)
            | Destination::Sessions
            | Destination::Agents
    )
}
/// The sidebar's project group heading: which project its pages show. The
/// switcher itself is in those pages' headers, beside what it changes.
fn project_label(model: Signal<Model>) -> impl IntoElement {
    text(move || {
        let m = model.get();
        match m.selected_project.as_deref() {
            Some(name) => format!(
                "{} · {}",
                m.word(Msg::ProjectGroup),
                acp_ui_core::presentation::sanitize(name, 64)
            ),
            None => m.word(Msg::ProjectGroup).into(),
        }
    })
    .font_weight(FontWeight::SEMIBOLD)
    .text_size(px(ui::scale::text::CAPTION))
    .text_color(move || palette(model).muted)
    .px_2()
    .style_when(compact, |s| s.hidden())
}
/// The header's project switcher on project pages: a caption and the menu.
fn header_project(model: Signal<Model>) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .ml_3()
        .style_when(
            move || {
                let m = model.get();
                !project_scoped(current(&m)) || m.project_names.is_empty()
            },
            |s| s.hidden(),
        )
        .child(
            text(move || model.get().word(Msg::ProjectGroup))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
        .child(view().w(px(220.0)).child(projects(model)))
}
fn group_label(model: Signal<Model>, key: Msg) -> impl IntoElement {
    text(move || model.get().word(key))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(ui::scale::text::CAPTION))
        .text_color(move || palette(model).muted)
        .px_2()
        .style_when(compact, |s| s.hidden())
}
fn destination(model: Signal<Model>, item: Destination, path: &'static str) -> AnyElement {
    if item == Destination::Review {
        // Work waiting: an exact count from the daemon, or nothing.
        let count = computed(move || model.get().review_count.filter(|n| *n > 0));
        return ui::nav_item_counted(
            model,
            path,
            move || model.get().word(title_key(item)).into(),
            move || count.get(),
            move || {
                let m = model.get();
                match count.get() {
                    Some(n) => format!(
                        "{}, {}",
                        m.word(title_key(item)),
                        m.word(Msg::ReviewWaiting).replace("{n}", &n.to_string())
                    )
                    .into(),
                    None => m.word(title_key(item)).into(),
                }
            },
            move || current(&model.get()) == item,
            move || go(model, item),
        )
        .into_element();
    }
    ui::nav_item(
        model,
        path,
        move || model.get().word(title_key(item)).into(),
        move || current(&model.get()) == item,
        move || go(model, item),
    )
    .into_element()
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
        acp_ui_core::connection::Target::Wsl if !m.connection.distribution.is_empty() => {
            format!("{} · {}", m.target_label(), m.connection.distribution)
        }
        _ => m.target_label().into(),
    }
}
fn sidebar(model: Signal<Model>) -> impl IntoElement {
    use Destination::*;
    use acp_ui_core::Section as S;
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
        (Section(S::Handoff), icons::section_path(S::Handoff)),
        (Sessions, icons::SESSIONS),
    ];
    // Personal: yours across projects — global preferences and the decisions
    // agents' suggestions wait for.
    let personal = [
        (PersonalPreferences, icons::PERSON),
        (Review, icons::REVIEW),
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
    // One selection slides between every destination row, across groups.
    let slide = ui::Slide::<Destination>::default();
    let rows = &slide;
    let setup_group = row().children(
        setup
            .into_iter()
            .zip(1u8..)
            .map(move |((d, path), n)| rows.row(d, step(model, n, d, path))),
    );
    let work_group = row().children(
        work.into_iter()
            .map(move |(d, path)| rows.row(d, destination(model, d, path))),
    );
    let personal_group = row().children(
        personal
            .into_iter()
            .map(move |(d, path)| rows.row(d, destination(model, d, path))),
    );
    let foot_group = row().children(
        foot.into_iter()
            .map(move |(d, path)| rows.row(d, destination(model, d, path))),
    );
    let container = element_ref();
    let indicator = slide.indicator(model, container, move || current(&model.get()));
    view()
        .element_ref(&container)
        .child(indicator)
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
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .px_2()
                .pb_1()
                .style_when(compact, |s| s.hidden())
                .child(
                    view()
                        .text_color(move || palette(model).primary)
                        .child(ui::icon::glyph(icons::BRAND, ui::scale::icon::LG)),
                )
                .child(
                    text("Agent Control Plane")
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(ui::scale::text::BODY)),
                ),
        )
        .child(
            slide.group(
                view()
                    .gap_1()
                    .child(group_label(model, Msg::SetupGroup))
                    .child(setup_group),
            ),
        )
        .child(slide.group(view().gap_1().child(project_label(model)).child(work_group)))
        .child(
            slide.group(
                view()
                    .gap_1()
                    .child(group_label(model, Msg::PersonalGroup))
                    .child(personal_group),
            ),
        )
        .child(view().flex_1().style_when(compact, |s| s.hidden()))
        .child(slide.group(foot_group))
        // The foot: the daemon's state, and the keyboard shortcuts.
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_1()
                .child(view().flex_1().min_w_0().child(status_row(model)))
                .child(
                    ui::icon_button(model, icons::KEYBOARD, move || {
                        model.get().word(Msg::KeysTitle).into()
                    })
                    .variant(ui::ButtonVariant::Ghost)
                    .size(ui::ButtonSize::Small)
                    .selected(move || model.get().shortcuts_open)
                    .on_press(move || crate::shortcuts::open(model)),
                ),
        )
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
                    view()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .child({
                            // The title arrives with each destination (a short
                            // fade and rise), so a page change reads as one.
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
                        // A page's longer explanation, behind an info button.
                        .child(
                            view()
                                .style_when(
                                    move || page_help(&model.get()).is_none(),
                                    |s| s.hidden(),
                                )
                                .child(ui::info(
                                    model,
                                    move || model.get().word(Msg::AboutThisPage).into(),
                                    move || page_help(&model.get()).unwrap_or_default().into(),
                                )),
                        )
                        .child(header_project(model)),
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
                        .variant(ui::ButtonVariant::Ghost)
                        .size(ui::ButtonSize::Small)
                        .on_press(move || reconnect(model)),
                )
                .child(
                    ui::copy_button(
                        model,
                        move || model.get().word(Msg::CopyPage).into(),
                        move || model.get().word(Msg::CopiedShort).into(),
                        move || copy_page(model),
                    )
                    .variant(ui::ButtonVariant::Ghost)
                    .size(ui::ButtonSize::Small)
                    .disabled(move || {
                        let m = model.get();
                        m.phase != Phase::Ready || !matches!(current(&m), Destination::Section(_))
                    }),
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
pub(crate) fn shell(
    model: Signal<Model>,
    task_ref: ElementRef,
    filter_ref: ElementRef,
) -> impl IntoElement {
    profile::watch(model);
    profile::watch_count(model);
    agents::watch(model);
    let on = move |d: fn(&Model) -> bool| move || !d(&model.get());
    // Sibling pages cross-fade: the page's content fades in (`fast`) each
    // time the destination changes; nothing remounts, so inputs keep state.
    let page_fade = animated_value(1.0);
    let destination = computed(move || current(&model.get()));
    effect(move || {
        let _ = destination.get();
        if !reduce_motion() {
            page_fade.set(0.0);
            page_fade.timing(1.0, ui::motion::FAST, Easing::EaseOut);
        }
    });
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
                        .bg(move || palette(model).sidebar)
                        .border_color(move || palette(model).hairline)
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
                                .bind_opacity(&page_fade)
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
        // The resize grip sits in the window's corner over the page, so the
        // sidebar runs to the bottom edge.
        .child(
            view()
                .absolute()
                .bottom_0()
                .right_0()
                .style_when(
                    || TERMINAL_SURFACE.load(std::sync::atomic::Ordering::Relaxed),
                    |s| s.hidden(),
                )
                .child(chrome::grip(model)),
        )
        .child(crate::shortcuts::panel(model))
        .child(crate::features::tags::panel(model))
        // Last, so toasts float above the page; they never take its space.
        .child(ui::toaster(model))
}
