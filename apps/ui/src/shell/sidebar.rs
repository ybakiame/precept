use super::navigation::{Destination, current, go, go_setup, title_key};
use crate::*;
use features::setup;
use precept_ui_core::{Section, SetupTab, i18n::Msg};
use tokens::{compact, palette};

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
                "{} {} · {} · {} · {}",
                m.word(Msg::StatusOpensConnection),
                m.word(status_of(&m).1),
                target_words(&m),
                m.read_label(),
                m.read_time()
            )
            .into()
        })
        .variant(ui::ButtonVariant::Default)
        .size(ui::ButtonSize::Small)
        .full_width()
        .leading(ui::status_dot(model, move || state.get()))
        .tooltip(ui::tooltip(model, status_details(model)))
        .selected(move || {
            let m = model.get();
            current(&m) == Destination::RuntimeChecks && m.setup_tab == SetupTab::Connection
        })
        .on_press(move || go_setup(model, SetupTab::Connection))
}

fn status_details(model: Signal<Model>) -> impl IntoElement {
    view()
        .gap_1()
        .child(
            text(move || {
                let m = model.get();
                format!("{} · {}", target_words(&m), m.read_label())
            })
            .font_weight(FontWeight::MEDIUM)
            .text_size(px(ui::scale::text::BODY)),
        )
        .child(
            text(move || model.get().read_time())
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
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
            if n == 2 {
                let pop = ui::motion::pop();
                return view()
                    .text_color(move || palette(model).success)
                    .bind_motion(&pop)
                    .child(ui::icon::glyph(icons::CHECK, ui::scale::icon::SM))
                    .into_element();
            }
            text(format!("{n}/2"))
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

pub(super) fn sidebar(model: Signal<Model>) -> impl IntoElement {
    use Destination::*;
    // Workspace: every page scoped to the active project chosen above.
    let work = [
        (Dashboard, icons::section_path(Section::Dashboard)),
        (Rules, icons::section_path(Section::Rules)),
        (Agents, icons::AGENTS),
        (Sessions, icons::LIST),
        (Memory, icons::MEMORY),
        (Handoff, icons::section_path(Section::Handoff)),
    ];
    // Context: pending review separately from the reviewed rule records.
    let review_items = [
        (Review, icons::REVIEW),
        (ReviewedRules, icons::section_path(Section::Rules)),
    ];
    // Set up: two runtime checks, then independent workspace management.
    let setup_items = [
        (RuntimeChecks, icons::CONNECTION),
        (Workspaces, icons::section_path(Section::Projects)),
    ];
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
        // The column's rest under the brand bar, so the foot group sits at
        // the bottom; a wrapped row when compact.
        .style_when(move || !compact(), |s| s.flex_1())
        .style_when(compact, |s| s.p_2().gap_2())
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

/// The brand at the top of the sidebar, level with the window's buttons;
/// dragging it moves the window.
pub(super) fn brand_bar(model: Signal<Model>) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .h(px(ui::scale::layout::TITLE_BAR))
        .px_4()
        .flex_shrink_0()
        .window_drag(true)
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
        )
}
