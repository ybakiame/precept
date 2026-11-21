//! The window shell: a task-oriented sidebar with one persistent workspace
//! switcher beside one page with its header and primary scroll region (see
//! docs/product-ui.md). Also the shared action helpers the feature pages use.
use crate::*;
pub(crate) mod chrome;
use features::{
    agents, connection, doctor, page, profile, projects, review, sessions, settings, setup,
};
use precept_ui_core::{Section, SetupTab, i18n::Msg, operator::ReviewMode};
use tokens::{compact, palette};

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
            if std::env::var_os("PRECEPT_UI_PROBE").is_some() {
                eprintln!("PRECEPT_UI_ACTION key={key:?}");
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
pub(crate) fn card(
    model: Signal<Model>,
    title: &'static str,
    lines: Vec<String>,
) -> impl IntoElement {
    ui::card(model, title, lines)
}

/// Every top-level sidebar destination: flat, unambiguous, mutually exclusive.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Destination {
    Dashboard,
    /// Preferences & contracts.
    Rules,
    Agents,
    /// Agents' work sessions.
    Sessions,
    Handoff,
    Review,
    RuntimeChecks,
    Diagnostics,
    Settings,
}

pub(crate) fn current(m: &Model) -> Destination {
    if m.operator.open {
        Destination::Review
    } else if m.setup_open {
        Destination::RuntimeChecks
    } else if m.agents_page {
        Destination::Agents
    } else if m.sessions_page {
        Destination::Sessions
    } else if m.settings {
        Destination::Settings
    } else {
        match m.section {
            Section::Dashboard => Destination::Dashboard,
            Section::Projects => Destination::RuntimeChecks,
            Section::Rules => Destination::Rules,
            Section::Handoff => Destination::Handoff,
            Section::Diagnostics => Destination::Diagnostics,
        }
    }
}

fn leave_workspaces(m: &mut Model) {
    m.operator.open = false;
    m.operator.review = None;
    m.settings = false;
    m.setup_open = false;
    m.agents_page = false;
    m.sessions_page = false;
}

/// Opens Runtime checks directly on `tab` (Connection, Profile, or Workspaces).
pub(crate) fn go_setup(model: Signal<Model>, tab: SetupTab) {
    model.update(|m| {
        leave_workspaces(m);
        m.setup_open = true;
        m.setup_tab = tab;
    });
    if tab == SetupTab::Connection {
        connection::opened(model);
    }
}

/// Opens the unified Review & preferences workspace directly on `mode`.
pub(crate) fn go_review(model: Signal<Model>, mode: ReviewMode) {
    review::open_mode(model, mode);
}

/// Goes to `destination` (each opener keeps its own reads and guards).
pub(crate) fn go(model: Signal<Model>, destination: Destination) {
    match destination {
        Destination::Review => review::open(model),
        Destination::RuntimeChecks => {
            let tab = model.get().setup_tab;
            go_setup(model, tab);
        }
        Destination::Settings => model.update(|m| {
            leave_workspaces(m);
            m.settings = true;
        }),
        Destination::Agents => model.update(|m| {
            leave_workspaces(m);
            m.agents_page = true;
        }),
        Destination::Sessions => model.update(|m| {
            leave_workspaces(m);
            m.sessions_page = true;
        }),
        Destination::Dashboard => model.update(|m| {
            leave_workspaces(m);
            m.set_section(Section::Dashboard);
        }),
        Destination::Rules => model.update(|m| {
            leave_workspaces(m);
            m.set_section(Section::Rules);
        }),
        Destination::Handoff => model.update(|m| {
            leave_workspaces(m);
            m.set_section(Section::Handoff);
        }),
        Destination::Diagnostics => model.update(|m| {
            leave_workspaces(m);
            m.set_section(Section::Diagnostics);
        }),
    }
}

fn title_key(destination: Destination) -> Msg {
    match destination {
        Destination::Dashboard => Msg::Dashboard,
        Destination::Rules => Msg::RulesAndContracts,
        Destination::Agents => Msg::AgentsDestination,
        Destination::Sessions => Msg::SessionsDestination,
        Destination::Handoff => Msg::Handoff,
        Destination::Review => Msg::ReviewHub,
        Destination::RuntimeChecks => Msg::RuntimeChecks,
        Destination::Diagnostics => Msg::Diagnostics,
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
        Destination::RuntimeChecks => match m.setup_tab {
            SetupTab::Connection => words(&[Msg::RuntimeChecksIntro, Msg::ConnectionHelp]),
            SetupTab::Profile => words(&[Msg::ProfileIntro]),
            SetupTab::Workspaces => words(&[Msg::WorkspacesIntro]),
        },
        Destination::Agents => words(&[Msg::AgentsIntro]),
        Destination::Sessions => words(&[Msg::SessionsIntro]),
        Destination::Review => words(&[
            Msg::OperatorHelp,
            Msg::ReviewProvenanceNote,
            Msg::OperatorCliGate,
        ]),
        Destination::Rules => words(&[Msg::PreferencesSources]),
        _ => return None,
    })
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
/// opens Runtime checks on the Connection tab.
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
        .selected(move || {
            let m = model.get();
            current(&m) == Destination::RuntimeChecks && m.setup_tab == SetupTab::Connection
        })
        .on_press(move || go_setup(model, SetupTab::Connection))
}

/// Whether a destination displays one workspace's data.
pub(crate) fn workspace_scoped(d: Destination) -> bool {
    matches!(
        d,
        Destination::Dashboard
            | Destination::Rules
            | Destination::Agents
            | Destination::Sessions
            | Destination::Handoff
    )
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
    if item == Destination::RuntimeChecks {
        let done = computed(move || setup::completed_count(&model.get()));
        let trailing = dynamic(move || {
            let n = done.get();
            if n == 3 {
                if ui::terminal() {
                    return text("✓")
                        .text_color(move || palette(model).success)
                        .into_element();
                }
                let pop = ui::motion::pop();
                return view()
                    .text_color(move || palette(model).success)
                    .bind_motion(&pop)
                    .child(ui::icon::glyph(icons::CHECK, ui::scale::icon::SM))
                    .into_element();
            }
            text(format!("{n}/3"))
                .tabular_nums()
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted)
                .into_element()
        });
        return ui::button(model, "")
            .label_fn(move || model.get().word(title_key(item)).into())
            .variant(ui::ButtonVariant::Ghost)
            .icon(path)
            .full_width()
            .selected(move || current(&model.get()) == item)
            .quiet_selection(move || !compact())
            .trailing(trailing)
            .on_press(move || go(model, item))
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

/// The ONE workspace switcher in the app: lists the daemon's ready workspaces
/// with the active one checked, or guides the user straight to Runtime checks
/// → Workspaces when none exist yet.
fn workspace_switcher(model: Signal<Model>) -> impl IntoElement {
    let names = computed(move || model.get().project_names.clone());
    dynamic(move || {
        let names = names.get();
        if names.is_empty() {
            return ui::button(model, "")
                .label_fn(move || model.get().word(Msg::AddWorkspaceCta).into())
                .icon(icons::PLUS)
                .variant(ui::ButtonVariant::Default)
                .size(ui::ButtonSize::Small)
                .full_width()
                .on_press(move || go_setup(model, SetupTab::Workspaces))
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
        precept_ui_core::connection::Target::Wsl if !m.connection.distribution.is_empty() => {
            format!("{} · {}", m.target_label(), m.connection.distribution)
        }
        _ => m.target_label().into(),
    }
}

fn sidebar(model: Signal<Model>) -> impl IntoElement {
    use Destination::*;
    // Workspace: every page scoped to the active project chosen above.
    let work = [
        (Dashboard, icons::section_path(Section::Dashboard)),
        (Rules, icons::section_path(Section::Rules)),
        (Agents, icons::AGENTS),
        (Sessions, icons::LIST),
        (Handoff, icons::section_path(Section::Handoff)),
    ];
    // Review: merged pending queue + personal preferences + project preferences.
    let review_items = [(Review, icons::REVIEW)];
    // Set up: one guided Runtime checks surface (Connection, Profile, Workspaces).
    let setup_items = [(RuntimeChecks, icons::CONNECTION)];
    let foot = [
        (Diagnostics, icons::section_path(Section::Diagnostics)),
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
    let work_group = row().children(
        work.into_iter()
            .map(move |(d, path)| rows.row(d, destination(model, d, path))),
    );
    let review_group = row().children(
        review_items
            .into_iter()
            .map(move |(d, path)| rows.row(d, destination(model, d, path))),
    );
    let setup_group = row().children(
        setup_items
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
                    text("Precept")
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(ui::scale::text::BODY)),
                ),
        )
        .child(
            slide.group(
                view()
                    .gap_1p5()
                    .child(group_label(model, Msg::WorkspaceGroup))
                    .child(
                        view()
                            .style_when(compact, |s| s.w(px(180.0)))
                            .child(workspace_switcher(model)),
                    )
                    .child(work_group),
            ),
        )
        .child(
            slide.group(
                view()
                    .gap_1()
                    .child(group_label(model, Msg::ReviewGroup))
                    .child(review_group),
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

/// Whether the current destination has a copyable read-only snapshot page.
fn copyable_destination(d: Destination) -> bool {
    matches!(
        d,
        Destination::Dashboard
            | Destination::Rules
            | Destination::Handoff
            | Destination::Diagnostics
    )
}

/// The page header: the destination's title, calm workspace scope indicator,
/// one human status line and the page-wide actions.
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
                        .flex_wrap()
                        .items_center()
                        .gap_2()
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
                        .child(header_scope_badge(model))
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
                        ),
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

fn task_input(model: Signal<Model>, task_ref: ElementRef) -> impl IntoElement {
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

fn preference_input(model: Signal<Model>, filter_ref: ElementRef) -> impl IntoElement {
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

pub(crate) fn shell(
    model: Signal<Model>,
    task_ref: ElementRef,
    filter_ref: ElementRef,
) -> impl IntoElement {
    profile::watch(model);
    profile::watch_count(model);
    agents::watch(model);
    sessions::watch(model);
    let on = move |d: Destination| move || current(&model.get()) != d;
    // A new destination starts at its top and its content settles in with a
    // short rise (`fast`); nothing remounts, so inputs keep state. Movement
    // only: an opacity cross-fade left content that changed mid-fade drawn
    // dimmed on Windows until the next fade.
    let page_rise = animated_value(0.0);
    let page_scroll = element_ref();
    let destination = computed(move || current(&model.get()));
    effect(move || {
        let _ = destination.get();
        page_scroll.scroll_to_offset(Points(0.0), false);
        if !reduce_motion() {
            page_rise.set(ui::motion::RISE);
            page_rise.timing(0.0, ui::motion::FAST, Easing::EaseOut);
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
                        .child(features::preferences::detail_dialog(model))
                        .child(features::review::export_dialog(model))
                        .child(agents::pointer_dialog(model))
                        .child(
                            scroll()
                                .flex_1()
                                .element_ref(&page_scroll)
                                .style_when(move || model.get().operator.open, |s| s.hidden())
                                .child(
                                    view()
                                        .bind_translate_y(&page_rise)
                                        .gap_4()
                                        .pb_6()
                                        .flex_shrink_0()
                                        .child(
                                            view()
                                                .style_when(on(Destination::Settings), |s| {
                                                    s.hidden()
                                                })
                                                .child(settings::appearance(model)),
                                        )
                                        .child(
                                            view()
                                                .style_when(on(Destination::RuntimeChecks), |s| {
                                                    s.hidden()
                                                })
                                                .child(setup::page(model)),
                                        )
                                        .child(
                                            view()
                                                .style_when(on(Destination::Agents), |s| s.hidden())
                                                .child(agents::page(model)),
                                        )
                                        .child(
                                            view()
                                                .style_when(on(Destination::Sessions), |s| {
                                                    s.hidden()
                                                })
                                                .child(sessions::page(model)),
                                        )
                                        .child(
                                            view()
                                                .style_when(on(Destination::Diagnostics), |s| {
                                                    s.hidden()
                                                })
                                                .child(doctor::panel(model)),
                                        )
                                        .child(
                                            view()
                                                .style_when(
                                                    move || {
                                                        !copyable_destination(current(&model.get()))
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
