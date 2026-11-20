//! Agents (MCP): per-project external agent sharing, the exact configuration
//! to paste into Claude Code, Codex or any MCP client, and recent agent
//! activity for the currently selected workspace.
//!
//! `precept-mcp` is a stdio program the client starts itself (no port, no
//! listener). The daemon supplies the launch line (`project.mcp`); the page only
//! formats it. Sharing is switched per project by the daemon (`project.agents`),
//! which withdraws or republishes that project's agent credential.
use crate::*;
use precept_ui_core::{
    ProjectEntry,
    connection::Target,
    i18n::Msg,
    mcp::{Client, Host, Launch, Pointer, hook_snippet, server_name, snippet},
};
use tokens::palette;
use ui::{BadgeVariant, ButtonSize, ButtonVariant, badge, button, scale, surface, tabs};

fn word(model: Signal<Model>, key: Msg) -> impl Fn() -> SharedString + 'static {
    move || model.get().word(key).into()
}
fn caption(model: Signal<Model>, f: impl Fn(&Model) -> Option<Msg> + 'static) -> impl IntoElement {
    text(move || {
        let m = model.get();
        f(&m).map(|k| m.word(k)).unwrap_or_default()
    })
    .text_size(px(scale::text::CAPTION))
    .text_color(move || palette(model).muted)
}

fn directory(m: &Model) -> Vec<ProjectEntry> {
    m.data
        .as_ref()
        .map(|d| d.directory.clone())
        .unwrap_or_default()
}

fn active_entry(m: &Model) -> Option<ProjectEntry> {
    let entries = directory(m);
    let wanted = m
        .selected_project
        .as_deref()
        .or(m.agents.project.as_deref());
    wanted
        .and_then(|name| entries.iter().find(|e| e.name == name).cloned())
        .or_else(|| entries.into_iter().next())
}

/// Where the MCP client runs: a Windows client reaches a WSL daemon through
/// `wsl.exe`, unless the operator says the client runs inside WSL.
fn host(m: &Model) -> Host {
    if m.connection.target == Target::Wsl && !m.agents.inside_wsl {
        Host::WindowsToWsl {
            distribution: m.connection.distribution.clone(),
            user: m.connection.user.clone(),
        }
    } else {
        Host::Same
    }
}

/// The text to copy for the current project, client and host.
fn current_snippet(m: &Model) -> Option<String> {
    let launch = m.agents.launch.as_ref()?;
    (Some(&launch.name) == m.agents.project.as_ref())
        .then(|| snippet(m.agents.client, launch, &host(m)))
        .flatten()
}

fn current_hook(m: &Model) -> Option<String> {
    let launch = m.agents.launch.as_ref()?;
    (Some(&launch.name) == m.agents.project.as_ref())
        .then(|| hook_snippet(m.agents.client, launch, &host(m)))
        .flatten()
}

/// Reads the project's AGENTS.md pointer status.
fn load_pointer(model: Signal<Model>, name: String) {
    model.update(|m| {
        m.agents.pointer_busy = true;
        m.agents.pointer_error = None;
    });
    super::profile::call(
        model,
        "integration.pointer",
        serde_json::json!({ "project": name }),
        move |_, m, r| {
            if m.agents.project.as_deref() != Some(name.as_str()) {
                return;
            }
            m.agents.pointer_busy = false;
            match r.and_then(|v| serde_json::from_value::<Pointer>(v).map_err(|_| "Unavailable")) {
                Ok(p) => m.agents.pointer = Some(p),
                Err(code) => {
                    m.agents.pointer = None;
                    m.agents.pointer_error = Some(code.into());
                }
            }
        },
    );
}

/// Writes (`add`) or removes the pointer, guarded by the digest shown.
fn apply_pointer(model: Signal<Model>, add: bool) {
    let m = model.get();
    let (Some(name), Some(p)) = (m.agents.project.clone(), m.agents.pointer.clone()) else {
        return;
    };
    model.update(|m| {
        m.agents.pointer_busy = true;
        m.agents.pointer_error = None;
    });
    super::profile::call(
        model,
        "integration.pointer.apply",
        serde_json::json!({ "project": name, "enabled": add, "expected_sha256": p.sha256 }),
        move |model, m, r| {
            m.agents.pointer_busy = false;
            m.agents.pointer_dialog = None;
            match r.and_then(|v| serde_json::from_value::<Pointer>(v).map_err(|_| "Unavailable")) {
                Ok(p) => {
                    ui::notify(
                        ui::Tone::Success,
                        m.word(if add {
                            Msg::ToastPointerAdded
                        } else {
                            Msg::ToastPointerRemoved
                        }),
                    );
                    m.agents.pointer = Some(p);
                }
                Err(code) => {
                    m.agents.pointer_error = Some(if code == "Conflict" {
                        m.word(Msg::PointerChanged).into()
                    } else {
                        code.into()
                    });
                    let name = name.clone();
                    spawn(async move { load_pointer(model, name) });
                }
            }
        },
    );
}

fn fetch(model: Signal<Model>, name: String) {
    model.update(|m| {
        m.agents.project = Some(name.clone());
        m.agents.busy = true;
        m.agents.error = None;
        m.agents.copied = false;
    });
    load_activity(model, name.clone());
    load_pointer(model, name.clone());
    super::profile::call(
        model,
        "project.mcp",
        serde_json::json!({ "name": name }),
        |_, m, r| {
            m.agents.busy = false;
            match r.and_then(|v| serde_json::from_value::<Launch>(v).map_err(|_| "Unavailable")) {
                Ok(launch) => m.agents.launch = Some(launch),
                Err(code) => {
                    m.agents.launch = None;
                    m.agents.error = Some(code.into());
                }
            }
        },
    );
}

/// Reads the recent agent activity of `name` (the daemon's memory since it
/// started; counts and outcomes only).
pub(crate) fn load_activity(model: Signal<Model>, name: String) {
    model.update(|m| {
        let a = &mut m.agents.activity;
        if a.project.as_deref() != Some(name.as_str()) {
            a.page = None;
        }
        a.project = Some(name.clone());
        a.busy = true;
        a.error = None;
    });
    super::profile::call(
        model,
        "activity.list",
        serde_json::json!({ "project": name }),
        move |_, m, r| {
            let a = &mut m.agents.activity;
            if a.project.as_deref() != Some(name.as_str()) {
                return;
            }
            a.busy = false;
            match r.and_then(precept_ui_core::activity::Page::parse) {
                Ok(page) => a.page = Some(page),
                Err(code) => a.error = Some(code.into()),
            }
        },
    );
}

fn set_shared(model: Signal<Model>, name: String, enabled: bool) {
    model.update(|m| m.agents.busy = true);
    super::profile::call(
        model,
        "project.agents",
        serde_json::json!({ "name": name, "enabled": enabled }),
        move |model, m, r| {
            m.agents.busy = false;
            match r {
                Ok(_) => ui::notify(
                    ui::Tone::Success,
                    m.word(if enabled {
                        Msg::ToastShared
                    } else {
                        Msg::ToastUnshared
                    })
                    .replace("{name}", &name),
                ),
                Err(code) => m.agents.error = Some(code.into()),
            }
            spawn(async move {
                load(model);
                fetch(model, name);
            });
        },
    );
}

/// Keeps this page aligned with the one current project chosen in the sidebar.
pub(crate) fn watch(model: Signal<Model>) {
    let ready = computed(move || {
        let m = model.get();
        (
            m.agents_page && m.live && m.data.is_some(),
            m.epoch,
            m.selected_project.clone(),
        )
    });
    effect(move || {
        if !ready.get().0 {
            return;
        }
        untracked(|| {
            let m = model.get();
            if let Some(entry) = active_entry(&m) {
                fetch(model, entry.name);
            }
        });
    });
}

/// Access toggle for the currently selected workspace (no second project list).
fn project_access(model: Signal<Model>) -> impl IntoElement {
    let state = computed(move || {
        let m = model.get();
        (active_entry(&m), m.locale)
    });
    dynamic(move || {
        let (Some(entry), _) = state.get() else {
            return view().into_element();
        };
        let m = model.get_untracked();
        let name = entry.name.clone();
        let shared = entry.agents;
        let toggle_name = name.clone();
        let spoken_name = name.clone();
        surface(model)
            .title_fn(word(model, Msg::AgentsProjectStatus))
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .child(
                        view()
                            .flex_1()
                            .min_w(px(160.0))
                            .gap_0p5()
                            .child(
                                text(precept_ui_core::presentation::sanitize(&name, 64))
                                    .font_weight(FontWeight::SEMIBOLD),
                            )
                            .child(
                                text(precept_ui_core::presentation::sanitize(&entry.root, 512))
                                    .font_family("monospace")
                                    .text_size(px(scale::text::CAPTION))
                                    .text_color(move || palette(model).muted),
                            ),
                    )
                    .child(
                        badge(
                            model,
                            m.word(if shared {
                                Msg::AgentsShared
                            } else {
                                Msg::AgentsNotShared
                            }),
                        )
                        .variant(if shared {
                            BadgeVariant::Success
                        } else {
                            BadgeVariant::Neutral
                        }),
                    )
                    .child(
                        button(model, "")
                            .label_fn(word(
                                model,
                                if shared {
                                    Msg::StopSharing
                                } else {
                                    Msg::ShareProject
                                },
                            ))
                            .spoken(move || {
                                let m = model.get();
                                format!(
                                    "{} {spoken_name}",
                                    m.word(if shared {
                                        Msg::StopSharing
                                    } else {
                                        Msg::ShareProject
                                    })
                                )
                                .into()
                            })
                            .size(ButtonSize::Small)
                            .disabled(move || model.get().agents.busy)
                            .on_press(move || set_shared(model, toggle_name.clone(), !shared)),
                    ),
            )
            .into_element()
    })
}

fn configure(model: Signal<Model>) -> impl IntoElement {
    let mut client_tabs = tabs(model);
    for client in Client::ALL {
        let key = match client {
            Client::ClaudeCode => Msg::ClientClaude,
            Client::Codex => Msg::ClientCodex,
            Client::Json => Msg::ClientJson,
        };
        client_tabs = client_tabs.add_fn(
            word(model, key),
            move || model.get().agents.client == client,
            move || {
                model.update(|m| {
                    m.agents.client = client;
                    m.agents.copied = false;
                })
            },
        );
    }
    let host_tabs = tabs(model)
        .add_fn(
            word(model, Msg::ClientOnWindows),
            move || !model.get().agents.inside_wsl,
            move || {
                model.update(|m| {
                    m.agents.inside_wsl = false;
                    m.agents.copied = false;
                })
            },
        )
        .add_fn(
            word(model, Msg::ClientInWsl),
            move || model.get().agents.inside_wsl,
            move || {
                model.update(|m| {
                    m.agents.inside_wsl = true;
                    m.agents.copied = false;
                })
            },
        );
    surface(model)
        .title_fn(move || {
            let m = model.get();
            let name = m.agents.project.clone().unwrap_or_default();
            format!("{} · {}", m.word(Msg::ConfigureFor), server_name(&name)).into()
        })
        .child(
            view()
                .gap_3()
                .child(client_tabs)
                .child(
                    view()
                        .style_when(
                            move || model.get().connection.target != Target::Wsl,
                            |s| s.hidden(),
                        )
                        .child(host_tabs),
                )
                .child(caption(model, |m| {
                    Some(match m.agents.client {
                        Client::ClaudeCode => Msg::HowClaude,
                        Client::Codex => Msg::HowCodex,
                        Client::Json => Msg::HowJson,
                    })
                }))
                .child(
                    view()
                        .p_3()
                        .rounded(px(scale::radius::CONTROL))
                        .border_1()
                        .border_color(move || palette(model).hairline)
                        .bg(move || palette(model).background)
                        .child(
                            text(move || current_snippet(&model.get()).unwrap_or_default())
                                .font_family("monospace")
                                .text_size(px(scale::text::CAPTION)),
                        ),
                )
                .child(
                    view().flex_row().items_center().gap_2().child(
                        ui::copy_button(
                            model,
                            move || model.get().word(Msg::CopyConfig).into(),
                            move || model.get().word(Msg::CopiedConfig).into(),
                            move || {
                                let Some(text) = current_snippet(&model.get()) else {
                                    return false;
                                };
                                set_clipboard(text);
                                model.update(|m| m.agents.copied = true);
                                true
                            },
                        )
                        .variant(ButtonVariant::Primary)
                        .disabled(move || current_snippet(&model.get()).is_none())
                        .loading(move || model.get().agents.busy),
                    ),
                )
                .child(caption(model, |m| {
                    let launch = m.agents.launch.as_ref()?;
                    if launch.executable.is_none() {
                        Some(Msg::McpNoExecutable)
                    } else if !launch.shared {
                        Some(Msg::McpNotShared)
                    } else {
                        None
                    }
                }))
                .child(caption(model, |_| Some(Msg::McpRestartNote)))
                .child(
                    view()
                        .gap_2()
                        .pt_2()
                        .style_when(move || current_hook(&model.get()).is_none(), |s| s.hidden())
                        .child(text(word(model, Msg::HookTitle)).font_weight(FontWeight::MEDIUM))
                        .child(caption(model, |m| {
                            Some(match m.agents.client {
                                Client::Codex => Msg::HowHookCodex,
                                _ => Msg::HowHookClaude,
                            })
                        }))
                        .child(
                            view()
                                .p_3()
                                .rounded(px(scale::radius::CONTROL))
                                .border_1()
                                .border_color(move || palette(model).hairline)
                                .bg(move || palette(model).background)
                                .child(
                                    text(move || current_hook(&model.get()).unwrap_or_default())
                                        .font_family("monospace")
                                        .text_size(px(scale::text::CAPTION)),
                                ),
                        )
                        .child(view().flex_row().child(ui::copy_button(
                            model,
                            move || model.get().word(Msg::CopyHook).into(),
                            move || model.get().word(Msg::CopiedConfig).into(),
                            move || {
                                let Some(text) = current_hook(&model.get()) else {
                                    return false;
                                };
                                set_clipboard(text);
                                true
                            },
                        ))),
                )
                .child(
                    text(move || model.get().agents.error.clone().unwrap_or_default())
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).error),
                ),
        )
}

/// The AGENTS.md pointer: its state and the explicit add, update or remove.
fn pointer_section(model: Signal<Model>) -> impl IntoElement {
    surface(model)
        .title_fn(word(model, Msg::PointerTitle))
        .child(caption(model, |_| Some(Msg::PointerHelp)))
        .child(dynamic(move || {
            let m = model.get();
            let Some(p) = m.agents.pointer.clone() else {
                return text(m.agents.pointer_error.clone().unwrap_or_default())
                    .text_size(px(scale::text::CAPTION))
                    .text_color(move || palette(model).error)
                    .into_element();
            };
            if p.malformed {
                return text(m.word(Msg::PointerMalformed))
                    .text_color(move || palette(model).warning)
                    .into_element();
            }
            let state = p.state.clone().unwrap_or_default();
            let (key, variant) = match state.as_str() {
                "current" => (Msg::PointerCurrent, BadgeVariant::Success),
                "outdated" => (Msg::PointerOutdated, BadgeVariant::Warning),
                _ => (Msg::PointerAbsent, BadgeVariant::Neutral),
            };
            let mut row = view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(text(p.path.clone()).font_family("monospace"))
                .child(badge(model, m.word(key)).variant(variant))
                .child(view().flex_1());
            if state != "current" {
                row = row.child(
                    button(model, "")
                        .label_fn(word(
                            model,
                            if state == "outdated" {
                                Msg::PointerUpdate
                            } else {
                                Msg::PointerAdd
                            },
                        ))
                        .variant(ButtonVariant::Primary)
                        .size(ButtonSize::Small)
                        .loading(move || model.get().agents.pointer_busy)
                        .on_press(move || model.update(|m| m.agents.pointer_dialog = Some(true))),
                );
            }
            if state != "absent" {
                row = row.child(
                    button(model, "")
                        .label_fn(word(model, Msg::PointerRemove))
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .disabled(move || model.get().agents.pointer_busy)
                        .on_press(move || model.update(|m| m.agents.pointer_dialog = Some(false))),
                );
            }
            view()
                .gap_2()
                .child(row)
                .child(
                    text(m.agents.pointer_error.clone().unwrap_or_default())
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).error),
                )
                .into_element()
        }))
}

/// The pointer confirmation, previewing the exact block; a sibling of the
/// page's scroll region.
pub(crate) fn pointer_dialog(model: Signal<Model>) -> impl IntoElement {
    ui::dialog(model, "")
        .title_fn(move || {
            let m = model.get();
            m.word(if m.agents.pointer_dialog == Some(false) {
                Msg::PointerRemoveTitle
            } else {
                Msg::PointerAddTitle
            })
            .into()
        })
        .description_fn(move || {
            let m = model.get();
            m.word(if m.agents.pointer_dialog == Some(false) {
                Msg::PointerRemoveBody
            } else {
                Msg::PointerAddBody
            })
            .into()
        })
        .content_fn(move || {
            view()
                .style_when(
                    move || model.get().agents.pointer_dialog != Some(true),
                    |s| s.hidden(),
                )
                .p_3()
                .rounded(px(scale::radius::CONTROL))
                .border_1()
                .border_color(move || palette(model).hairline)
                .bg(move || palette(model).background)
                .child(
                    text(move || {
                        model
                            .get()
                            .agents
                            .pointer
                            .as_ref()
                            .map(|p| p.block.clone())
                            .unwrap_or_default()
                    })
                    .font_family("monospace")
                    .text_size(px(scale::text::CAPTION)),
                )
                .into_element()
        })
        .cancel_fn(word(model, Msg::Cancel))
        .confirm_fn(move || {
            let m = model.get();
            m.word(if m.agents.pointer_dialog == Some(false) {
                Msg::RemoveConfirm
            } else {
                Msg::PointerApply
            })
            .into()
        })
        .confirm_disabled(move || model.get().agents.pointer_busy)
        .open(move || model.get().agents.pointer_dialog.is_some())
        .on_cancel(move || model.update(|m| m.agents.pointer_dialog = None))
        .on_confirm(move || {
            let add = model.get().agents.pointer_dialog == Some(true);
            apply_pointer(model, add)
        })
}

/// One event: time, what happened, who asked and the outcome.
fn activity_row(
    model: Signal<Model>,
    m: &Model,
    e: &precept_ui_core::activity::Event,
) -> AnyElement {
    let ok = e.outcome == "ok";
    let refused = e.kind == "refused";
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x_3()
        .gap_y_1()
        .py_1p5()
        .border_t_1()
        .border_color(move || palette(model).hairline)
        .child(
            text(precept_ui_core::activity::clock(e.at_ms))
                .font_family("monospace")
                .text_size(px(scale::text::CAPTION))
                .text_color(move || palette(model).muted)
                .flex_shrink_0(),
        )
        .child(
            view()
                .flex_1()
                .min_w(px(200.0))
                .gap_0p5()
                .child(text(m.activity_summary(e)))
                .child(
                    text(m.activity_who(e))
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                ),
        )
        .child(
            badge(
                model,
                if ok {
                    "OK".to_owned()
                } else {
                    e.outcome.clone()
                },
            )
            .variant(if ok {
                BadgeVariant::Success
            } else if refused || e.outcome == "Unauthorized" {
                BadgeVariant::Danger
            } else {
                BadgeVariant::Warning
            }),
        )
        .into_element()
}

/// The most events listed (the daemon keeps more; Refresh reads the newest).
const SHOWN: usize = 50;

fn activity(model: Signal<Model>) -> impl IntoElement {
    let state = computed(move || {
        let m = model.get();
        (m.agents.activity.clone(), m.locale)
    });
    surface(model)
        .title_fn(word(model, Msg::ActivityTitle))
        .child(
            view()
                .flex_row()
                .items_start()
                .gap_3()
                .child(
                    text(move || {
                        let m = model.get();
                        let started = m
                            .agents
                            .activity
                            .page
                            .as_ref()
                            .map(|p| precept_ui_core::activity::date_time(p.daemon_started_ms))
                            .unwrap_or_else(|| "…".into());
                        m.word(Msg::ActivityHelp).replace("{time}", &started)
                    })
                    .flex_1()
                    .text_size(px(scale::text::CAPTION))
                    .text_color(move || palette(model).muted),
                )
                .child(
                    button(model, "")
                        .label_fn(word(model, Msg::RefreshList))
                        .spoken(word(model, Msg::ActivityRefresh))
                        .size(ButtonSize::Small)
                        .variant(ButtonVariant::Ghost)
                        .icon(ui::icons::RESET)
                        .loading(move || model.get().agents.activity.busy)
                        .on_press(move || {
                            if let Some(name) = model.get().agents.project.clone() {
                                load_activity(model, name);
                            }
                        }),
                ),
        )
        .child(dynamic(move || {
            let (a, _) = state.get();
            let m = model.get_untracked();
            if let Some(code) = a.error {
                return text(code)
                    .text_size(px(scale::text::CAPTION))
                    .text_color(move || palette(model).error)
                    .into_element();
            }
            let Some(page) = a.page else {
                return view().into_element();
            };
            if page.events.is_empty() {
                return ui::empty(model, m.word(Msg::ActivityEmpty))
                    .icon(ui::icons::AGENTS)
                    .into_element();
            }
            let headline = match page.last_read() {
                Some(e) => m
                    .word(Msg::ActivityLastRead)
                    .replace("{time}", &precept_ui_core::activity::clock(e.at_ms))
                    .replace("{client}", &m.activity_who(e)),
                None => m.word(Msg::ActivityNeverRead).to_owned(),
            };
            let mut list = view().child(text(headline).font_weight(FontWeight::MEDIUM).pb_2());
            for e in page.events.iter().take(SHOWN) {
                list = list.child(activity_row(model, &m, e));
            }
            list.into_element()
        }))
}

/// The Agents page (mounted by the shell outside the per-change page rebuild).
pub(crate) fn page(model: Signal<Model>) -> impl IntoElement {
    let empty_state = dynamic(move || {
        let m = model.get();
        if !m.live {
            return surface(model)
                .child(
                    ui::empty(model, m.word(Msg::AgentsOfflineTitle))
                        .icon(icons::AGENTS)
                        .description(m.word(Msg::AgentsOfflineBody))
                        .action(
                            button(model, "")
                                .label_fn(word(model, Msg::GoToRuntimeChecks))
                                .variant(ButtonVariant::Primary)
                                .on_press(move || {
                                    shell::go_setup(model, precept_ui_core::SetupTab::Connection)
                                }),
                        ),
                )
                .into_element();
        }
        if directory(&m).is_empty() {
            return surface(model)
                .child(
                    ui::empty(model, m.word(Msg::AgentsNeedsProject))
                        .icon(icons::AGENTS)
                        .action(
                            button(model, "")
                                .label_fn(word(model, Msg::GoToWorkspaces))
                                .variant(ButtonVariant::Primary)
                                .on_press(move || {
                                    shell::go_setup(model, precept_ui_core::SetupTab::Workspaces)
                                }),
                        ),
                )
                .into_element();
        }
        view().into_element()
    });
    view()
        .gap_6()
        .max_w(px(scale::layout::FORM))
        .child(empty_state)
        .child(
            view()
                .gap_6()
                .style_when(
                    move || {
                        let m = model.get();
                        !m.live || active_entry(&m).is_none()
                    },
                    |s| s.hidden(),
                )
                .child(project_access(model))
                .child(configure(model))
                .child(pointer_section(model))
                .child(activity(model)),
        )
}
