//! Shared responsive shell and small, scoped, read-only page components.
use super::*;
use acp_ui_spike_model::i18n::Msg;
use acp_ui_spike_model::{PAGE_SIZE, Section, presentation::sanitize};
use nsh_core::element::input;
use tokens::{compact, palette};

pub(super) fn action(
    model: Signal<Model>,
    label: impl Into<String>,
    enabled: impl Fn() -> bool + 'static,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    let label = label.into();
    button()
        .accessibility_label(label.clone())
        .px_2()
        .py_1()
        .rounded(px(6.0))
        .border_2()
        .bg(move || palette(model).inset)
        .text_color(move || palette(model).text)
        .border_color(move || palette(model).border)
        .hover(|s| s.border_color(palette(model).border))
        .focus(|s| s.border_color(palette(model).primary))
        .disabled(move || !enabled())
        .disabled_style(|s| s.opacity(0.45))
        .on_press(callback)
        .child(text(label))
}
pub(super) fn word_action(
    model: Signal<Model>,
    key: Msg,
    enabled: impl Fn() -> bool + 'static,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    button()
        .accessibility_label(move || Some(model.get().word(key).into()))
        .px_2()
        .py_1()
        .rounded(px(6.0))
        .border_2()
        .bg(move || palette(model).inset)
        .text_color(move || palette(model).text)
        .border_color(move || palette(model).border)
        .hover(|s| s.border_color(palette(model).primary))
        .focus(|s| s.border_color(palette(model).text))
        .disabled(move || !enabled())
        .disabled_style(|s| s.opacity(0.45))
        .on_press(move || {
            #[cfg(not(target_arch = "wasm32"))]
            if std::env::var_os("ACP_UI_PROBE").is_some() {
                eprintln!("ACP_UI_ACTION key={key:?}");
            }
            callback();
        })
        .child(text(move || model.get().word(key)))
}
fn control(model: Signal<Model>, key: Msg, callback: impl Fn() + 'static) -> impl IntoElement {
    word_action(model, key, || true, callback)
}
fn section_key(section: Section) -> Msg {
    match section {
        Section::Dashboard => Msg::Dashboard,
        Section::Projects => Msg::Projects,
        Section::Preferences => Msg::Preferences,
        Section::Handoff => Msg::Handoff,
        Section::Diagnostics => Msg::Diagnostics,
    }
}
pub(super) fn card(
    model: Signal<Model>,
    title: &'static str,
    lines: Vec<String>,
) -> impl IntoElement {
    view()
        .flex_shrink_0()
        .p_3()
        .gap_2()
        .rounded(px(8.0))
        .border_1()
        .border_color(move || palette(model).border)
        .bg(move || palette(model).panel)
        .child(
            text(title)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(14.0)),
        )
        .children(lines.into_iter().map(|line| text(sanitize(&line, 512))))
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
            })
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
                    input()
                        .element_ref(&task_ref)
                        .label(move || model.get().word(Msg::TaskId))
                        .h(px(30.0))
                        .flex_1()
                        .border_2()
                        .border_color(move || palette(model).border)
                        .bg(move || palette(model).panel)
                        .text_color(move || palette(model).text)
                        .focus(|s| s.border_color(palette(model).primary))
                        .value(move || model.get().task_id)
                        .placeholder("Task ID")
                        .on_change(move |v: &str| {
                            model.update(|m| m.task_id = v.chars().take(121).collect())
                        })
                        .on_submit(move |_: &str| reconnect(model)),
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
            input()
                .element_ref(&filter_ref)
                .label(move || model.get().word(Msg::Filter))
                .h(px(30.0))
                .border_2()
                .border_color(move || palette(model).border)
                .bg(move || palette(model).panel)
                .text_color(move || palette(model).text)
                .focus(|s| s.border_color(palette(model).primary))
                .value(move || model.get().filter)
                .on_change(move |v: &str| model.update(|m| m.set_filter(v))),
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
pub(super) fn shell(
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
                                                if m.operator.open {
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
                                .child(
                                    scroll().flex_1().child(
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
                                                    .child(settings::connection(model)),
                                            )
                                            .child(
                                                view()
                                                    .style_when(
                                                        move || !model.get().operator.open,
                                                        |s| s.hidden(),
                                                    )
                                                    .child(operator::panel(model)),
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
fn preferences(model: Signal<Model>) -> impl IntoElement {
    let current = model.get();
    let rows = current.rows();
    let page = current.page;
    let mut body = view().gap_2().child(
        text(move || model.get().word(Msg::PreferencesHeading)).font_weight(FontWeight::SEMIBOLD),
    );
    if rows.is_empty() {
        return body.child(card(
            model,
            current.word(Msg::NoMatching),
            vec![
                current.word(Msg::EmptyPreferences).into(),
                current.word(Msg::FilterRecovery).into(),
            ],
        ));
    }
    if let Some(row) = current.preference_detail() {
        body = body
            .child(card(
                model,
                current.word(Msg::SelectedPreference),
                vec![
                    sanitize(&row.key, 120),
                    current.detail_chunk(),
                    format!(
                        "{} {}/{} · {} / {}",
                        current.word(Msg::ValueDetail),
                        current.detail_page + 1,
                        current.detail_pages(),
                        sanitize(&row.scope, 64),
                        sanitize(&row.source, 80)
                    ),
                ],
            ))
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_1()
                    .child(word_action(
                        model,
                        Msg::EarlierValue,
                        move || model.get().detail_page > 0,
                        move || model.update(|m| m.detail_by(-1)),
                    ))
                    .child(word_action(
                        model,
                        Msg::LaterValue,
                        move || model.get().detail_page + 1 < model.get().detail_pages(),
                        move || model.update(|m| m.detail_by(1)),
                    ))
                    .child(control(model, Msg::CopyDetail, move || {
                        set_clipboard(model.get().detail_chunk());
                        model.update(|m| m.copied = true);
                    }))
                    .child(control(model, Msg::CloseDetail, move || {
                        model.update(|m| m.selected_preference = None)
                    })),
            );
    }
    for (offset, row) in rows
        .iter()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .enumerate()
    {
        let index = page * PAGE_SIZE + offset;
        body = body.child(
            view()
                .p_2()
                .gap_1()
                .rounded(px(6.0))
                .bg(move || palette(model).panel)
                .border_1()
                .border_color(move || palette(model).border)
                .child(
                    view()
                        .flex_row()
                        .gap_2()
                        .child(
                            text(sanitize(&row.key, 64))
                                .font_weight(FontWeight::SEMIBOLD)
                                .flex_1(),
                        )
                        .child(action(
                            model,
                            format!("{} {}", current.word(Msg::Details), offset + 1),
                            || true,
                            move || model.update(|m| m.select_preference(index)),
                        )),
                )
                .child(text(sanitize(&row.value, 160)))
                .child(
                    text(format!(
                        "{} / {}",
                        sanitize(&row.scope, 64),
                        sanitize(&row.source, 80)
                    ))
                    .text_size(px(12.0))
                    .text_color(move || palette(model).muted),
                ),
        );
    }
    body
}
fn page(model: Signal<Model>) -> impl IntoElement {
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
    if current.section == Section::Projects {
        body = body.child(
            view().flex_row().flex_wrap().gap_2().children(
                current
                    .project_names
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(move |(index, name)| {
                        action(
                            model,
                            sanitize(&name, 120),
                            || true,
                            move || switch_project(model, index),
                        )
                    }),
            ),
        );
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

fn daemon(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    let target = match m.connection.target {
        acp_ui_spike_model::connection::Target::Synthetic => Msg::SyntheticTarget,
        acp_ui_spike_model::connection::Target::Local => Msg::LocalTarget,
        acp_ui_spike_model::connection::Target::Wsl => Msg::WslTarget,
    };
    let transport = match m.connection.target {
        acp_ui_spike_model::connection::Target::Synthetic => "—",
        acp_ui_spike_model::connection::Target::Wsl => "wsl.exe --exec → Unix IPC",
        _ => {
            if cfg!(windows) {
                "Windows named pipe"
            } else {
                "Unix IPC"
            }
        }
    };
    let mut lines = vec![
        format!("{} · {}", m.word(target), m.read_label()),
        format!("{}: {}", m.word(Msg::TransportLabel), transport),
        m.read_time(),
    ];
    if let Some(d) = &m.data {
        lines.push(format!(
            "{}: {} · {}: {}",
            m.word(Msg::Version),
            d.health.api_version,
            m.word(Msg::Schema),
            sanitize(&d.health.schema, 64)
        ));
    }
    if let Some(duration) = m.last_duration_ms {
        lines.push(format!("{}: {} ms", m.word(Msg::Duration), duration));
    }
    lines.push(
        m.word(if m.live {
            Msg::HealthHelp
        } else {
            Msg::SnapshotHealth
        })
        .into(),
    );
    card(model, m.word(Msg::Daemon), lines)
}
fn dashboard(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    let p = m.project().expect("ready project");
    let mut context = vec![
        format!("{}: {}", m.word(Msg::Project), sanitize(&p.name, 120)),
        format!(
            "{}: {} · HEAD: {}",
            m.word(Msg::Branch),
            sanitize(
                p.context
                    .repository
                    .branch
                    .as_deref()
                    .unwrap_or(m.word(Msg::Unknown)),
                80
            ),
            sanitize(
                p.context
                    .repository
                    .head
                    .as_deref()
                    .unwrap_or(m.word(Msg::Unknown)),
                80
            )
        ),
        format!(
            "{}: {}",
            m.word(Msg::Worktree),
            m.word(match p.context.repository.dirty {
                Some(true) => Msg::HasChanges,
                Some(false) => Msg::NoChanges,
                None => Msg::Unknown,
            })
        ),
        format!(
            "{}: {} · {}: {}",
            m.word(Msg::ConfirmedPreferences),
            p.context.preferences.len(),
            m.word(Msg::ContractReferences),
            p.context.documents.len()
        ),
        m.word(Msg::ContextHelp).into(),
    ];
    context.extend(p.context.warnings.iter().take(8).cloned());
    let mut resume = vec![m.word(Msg::EarlierNotCurrent).into()];
    if let Some(h) = p.handoff.as_ref().filter(|h| h.task_id == m.queried_task) {
        resume.push(format!("{} {}", m.word(Msg::Handoff), h.task_id));
        resume.push(format!("{}: {}", m.word(Msg::Objective), h.objective));
        resume.push(format!("{}: {}", m.word(Msg::NextAction), h.next_action));
        resume.extend(h.resume_warnings.iter().take(8).cloned());
    } else if m.queried_task.is_empty() {
        resume.push(m.word(Msg::NoHandoff).into());
    } else {
        resume.push(format!(
            "{} {} {}",
            m.word(Msg::HandoffMissing),
            m.queried_task,
            m.word(Msg::InProject)
        ));
    }
    let agent = card(
        model,
        m.word(Msg::ClientAccess),
        vec![
            m.word(Msg::ClientPresence).into(),
            m.word(Msg::Tools).into(),
            m.word(Msg::ClientHelp).into(),
        ],
    );
    let cards = if compact() {
        view()
            .gap_2()
            .flex_shrink_0()
            .child(daemon(model))
            .child(agent)
    } else {
        view()
            .flex_row()
            .gap_2()
            .flex_shrink_0()
            .child(view().flex_1().child(daemon(model)))
            .child(view().flex_1().child(agent))
    };
    view()
        .flex_shrink_0()
        .gap_3()
        .child(cards)
        .child(card(model, m.word(Msg::EffectiveContext), context))
        .child(card(model, m.word(Msg::ResumeSummary), resume))
        .child(card(
            model,
            m.word(Msg::ReviewSuggestions),
            vec![
                m.word(Msg::ReviewDeferred).into(),
                m.word(Msg::ReviewHelp).into(),
            ],
        ))
}

fn handoff(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    let p = m.project().expect("ready project");
    let mut body = view().flex_shrink_0().gap_3();
    let Some(h) = p.handoff.as_ref().filter(|h| h.task_id == m.queried_task) else {
        return body.child(card(model, m.word(Msg::ResumeSummary), m.visible_lines()));
    };
    body = body
        .child(card(
            model,
            m.word(Msg::Objective),
            vec![
                format!("{} {} · {}", m.word(Msg::Handoff), h.task_id, p.name),
                h.objective.clone(),
            ],
        ))
        .child(card(
            model,
            m.word(Msg::NextAction),
            vec![h.next_action.clone()],
        ));
    let mut uncertainty = vec![
        m.word(Msg::EarlierNotCurrent).into(),
        format!(
            "{}: {}",
            m.word(Msg::Fingerprint),
            h.current_repository
                .get("worktree_fingerprint")
                .and_then(|v| v.as_str())
                .unwrap_or(m.word(Msg::FingerprintUnavailable))
        ),
    ];
    uncertainty.extend(h.resume_warnings.iter().take(8).cloned());
    body = body.child(
        view()
            .border_2()
            .rounded(px(8.0))
            .border_color(move || palette(model).warning)
            .child(card(model, m.word(Msg::CurrentUncertainty), uncertainty)),
    );
    let mut evidence = vec![
        m.word(Msg::CurrentValidation).into(),
        m.word(Msg::EvidenceHelp).into(),
    ];
    if m.expanded {
        evidence.extend(
            h.validation
                .iter()
                .skip(m.evidence_page * 6)
                .take(6)
                .map(|v| v.to_string()),
        );
    } else {
        evidence.push(format!(
            "{} {}",
            h.validation.len(),
            m.word(Msg::HistoricalEntries)
        ));
    }
    body.child(card(model, m.word(Msg::HistoricalEvidence), evidence))
}
