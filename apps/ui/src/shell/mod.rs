//! The window shell: title bar, navigation, toolbar and shared action helpers.
//! Shared responsive shell and small, scoped, read-only page components.
use crate::*;
pub(crate) mod chrome;
use acp_operator_core::i18n::Msg;
use acp_operator_core::{PAGE_SIZE, Section};
use features::{connection, page, review, sessions, settings};
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
fn navigation(model: Signal<Model>) -> impl IntoElement {
    view()
        .gap_1()
        .style_when(compact, |s| s.flex_row().flex_wrap())
        .children(
            Section::ALL
                .into_iter()
                .enumerate()
                .map(move |(index, section)| {
                    button()
                        .accessibility_label(move || {
                            Some(model.get().word(section_key(section)).into())
                        })
                        .px_2()
                        .py_1()
                        .border_2()
                        .rounded(px(6.0))
                        .selected(move || model.get().section == section)
                        .bg(move || {
                            if model.get().section == section {
                                palette(model).primary
                            } else {
                                palette(model).panel
                            }
                        })
                        .text_color(move || {
                            if model.get().section == section {
                                palette(model).on_primary
                            } else {
                                palette(model).text
                            }
                        })
                        .border_color(move || {
                            if model.get().section == section {
                                palette(model).primary
                            } else {
                                palette(model).panel
                            }
                        })
                        .hover(|s| s.border_color(palette(model).border))
                        .focus(|s| s.border_color(palette(model).text))
                        .on_press(move || {
                            model.update(|m| {
                                m.operator.open = false;
                                m.operator.review = None;
                                m.settings = false;
                                m.sessions = false;
                                m.session.confirm_start = false;
                                m.connection_editor = false;
                                m.set_section(section)
                            })
                        })
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(icons::section(section))
                        .child(text(move || {
                            format!("{}  {}", index + 1, model.get().word(section_key(section)))
                        }))
                }),
        )
        .child(control(model, Msg::Sessions, move || sessions::open(model)))
}
fn toolbar(model: Signal<Model>) -> impl IntoElement {
    view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(
            text(move || {
                let m = model.get();
                m.word(if m.operator.open {
                    Msg::ProfileOperator
                } else if m.sessions {
                    Msg::Sessions
                } else if m.settings {
                    Msg::Settings
                } else if m.connection_editor {
                    Msg::Connection
                } else {
                    section_key(m.section)
                })
            })
            .font_weight(FontWeight::SEMIBOLD)
            .text_size(px(22.0)),
        )
        .child(control(model, Msg::Reconnect, move || reconnect(model)))
        .child(control(model, Msg::Settings, move || {
            model.update(|m| {
                m.operator.open = false;
                m.operator.review = None;
                m.sessions = false;
                m.session.confirm_start = false;
                m.settings = !m.settings;
                m.connection_editor = false;
            })
        }))
        .child(control(model, Msg::Connection, move || {
            model.update(|m| {
                m.operator.open = false;
                m.operator.review = None;
                m.sessions = false;
                m.session.confirm_start = false;
                m.connection_editor = !m.connection_editor;
                m.settings = false;
            });
            if model.get().connection_editor {
                connection::opened(model);
            }
        }))
        .child(word_action(
            model,
            Msg::CopyPage,
            move || {
                model.get().phase == Phase::Ready
                    && !model.get().settings
                    && !model.get().connection_editor
                    && !model.get().sessions
                    && !model.get().operator.open
            },
            move || copy_page(model),
        ))
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
                .p_4()
                .gap_3()
                .style_when(compact, |s| s.p_2().gap_1())
                .font_family("sans-serif")
                .bg(move || palette(model).background)
                .text_color(move || palette(model).text)
                .text_size(px(14.0))
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            text("Agent Control Plane")
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(20.0)),
                        )
                        .child(
                            text(move || {
                                format!(
                                    " / {}",
                                    model
                                        .get()
                                        .selected_project
                                        .or_else(|| model.get().project().map(|p| p.name.clone()))
                                        .unwrap_or_else(|| model
                                            .get()
                                            .word(Msg::ChooseProject)
                                            .into())
                                )
                            })
                            .text_color(move || palette(model).muted),
                        )
                        .child(control(model, Msg::NextProject, move || {
                            next_project(model)
                        })),
                )
                .child(
                    view()
                        .flex_row()
                        .gap_3()
                        .flex_1()
                        .style_when(compact, |s| s.flex_col().gap_1())
                        .child(
                            view()
                                .w(move || {
                                    px(if compact() {
                                        (window().width.0 - 16.0).max(1.0)
                                    } else {
                                        176.0
                                    })
                                })
                                .gap_3()
                                .child(navigation(model))
                                .child(
                                    view()
                                        .gap_2()
                                        .style_when(compact, |s| s.hidden())
                                        .child(
                                            text(move || model.get().word(Msg::OperatorConsole))
                                                .text_size(px(12.0))
                                                .text_color(move || palette(model).muted),
                                        )
                                        .child(
                                            text(move || {
                                                model.get().word(if model.get().operator.open {
                                                    Msg::OperatorHelp
                                                } else if model.get().sessions {
                                                    Msg::SessionAuthority
                                                } else {
                                                    Msg::ReadOnlyHelp
                                                })
                                            })
                                            .text_color(move || palette(model).muted),
                                        )
                                        .child(
                                            text(move || model.get().word(Msg::ConsoleHelp))
                                                .text_size(px(12.0)),
                                        ),
                                ),
                        )
                        .child(
                            view()
                                .flex_1()
                                .gap_2()
                                .child(toolbar(model))
                                .child(
                                    view()
                                        .accessible(true)
                                        .label("Daemon connection state")
                                        .gap_1()
                                        .child(
                                            text(move || {
                                                let m = model.get();
                                                // A WSL link/launch problem names its exact cause on
                                                // every page, including the open Operator panel.
                                                if let Some(reason) = m.wsl_reason() {
                                                    format!(
                                                        "{} · {}",
                                                        m.target_label(),
                                                        m.word(reason)
                                                    )
                                                } else if m.operator.open {
                                                    format!(
                                                        "{} · {}",
                                                        m.target_label(),
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
                                                    format!(
                                                        "{} · {} · {}",
                                                        m.source_label(),
                                                        m.target_label(),
                                                        m.read_label()
                                                    )
                                                }
                                            })
                                            .text_color(move || match model.get().phase {
                                                _ if model.get().wsl_reason().is_some() => {
                                                    palette(model).error
                                                }
                                                Phase::Error | Phase::Unavailable => {
                                                    palette(model).error
                                                }
                                                Phase::Loading => palette(model).warning,
                                                _ => palette(model).muted,
                                            }),
                                        )
                                        .child(
                                            text(move || model.get().read_time())
                                                .style_when(
                                                    move || model.get().operator.open,
                                                    |s| s.hidden(),
                                                )
                                                .text_size(px(12.0))
                                                .text_color(move || palette(model).muted),
                                        ),
                                )
                                .child(task_input(model, task_ref))
                                .child(preference_input(model, filter_ref))
                                .child(review::panel(model))
                                .child(
                                    scroll()
                                        .flex_1()
                                        .style_when(
                                            move || model.get().operator.open,
                                            |s| s.hidden(),
                                        )
                                        .child(
                                            view()
                                                .gap_3()
                                                .flex_shrink_0()
                                                .child(
                                                    view()
                                                        .style_when(
                                                            move || !model.get().settings,
                                                            |s| s.hidden(),
                                                        )
                                                        .child(settings::appearance(model)),
                                                )
                                                .child(
                                                    view()
                                                        .style_when(
                                                            move || !model.get().connection_editor,
                                                            |s| s.hidden(),
                                                        )
                                                        .child(connection::connection(model)),
                                                )
                                                .child(
                                                    view()
                                                        .style_when(
                                                            move || !model.get().sessions,
                                                            |s| s.hidden(),
                                                        )
                                                        .child(sessions::panel(model)),
                                                )
                                                .child(
                                                    view()
                                                        .style_when(
                                                            move || {
                                                                model.get().settings
                                                                    || model.get().sessions
                                                                    || model.get().operator.open
                                                                    || model.get().connection_editor
                                                            },
                                                            |s| s.hidden(),
                                                        )
                                                        .child(dynamic(move || page(model))),
                                                ),
                                        ),
                                )
                                .child(
                                    text(move || {
                                        model.get().word(if model.get().copied {
                                            Msg::Copied
                                        } else {
                                            Msg::ConsoleHelp
                                        })
                                    })
                                    .text_size(px(12.0))
                                    .text_color(move || palette(model).muted),
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
