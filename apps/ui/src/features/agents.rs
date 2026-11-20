//! Set-up step 4, Agents: which projects external agents may read, and the
//! exact configuration to paste into Claude Code, Codex or any MCP client.
//!
//! `acp-mcp` is a stdio program the client starts itself (no port, no
//! listener). The daemon supplies the launch line (`project.mcp`); the page only
//! formats it. Sharing is switched per project by the daemon (`project.agents`),
//! which withdraws or republishes that project's agent credential.
use crate::*;
use acp_ui_core::{
    ProjectEntry,
    connection::Target,
    i18n::Msg,
    mcp::{Client, Host, Launch, server_name, snippet},
};
use tokens::palette;
use ui::{BadgeVariant, ButtonSize, ButtonVariant, badge, button, scale, surface};

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

fn fetch(model: Signal<Model>, name: String) {
    model.update(|m| {
        m.agents.project = Some(name.clone());
        m.agents.busy = true;
        m.agents.error = None;
        m.agents.copied = false;
    });
    load_activity(model, name.clone());
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
            match r.and_then(acp_ui_core::activity::Page::parse) {
                Ok(page) => a.page = Some(page),
                Err(code) => a.error = Some(code.into()),
            }
        },
    );
}

/// A row chosen on this page: a served project becomes the app's current
/// project (the read that follows refreshes this page); one the daemon does
/// not serve yet is shown here only.
fn choose(model: Signal<Model>, name: String) {
    let index = model.get().project_names.iter().position(|n| *n == name);
    match index {
        Some(index) => crate::switch_project(model, index),
        None => fetch(model, name),
    }
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

/// Picks a project and reads its launch line when the page becomes visible
/// with daemon data (tracked: visibility and the read epoch only). The app's
/// current project comes first, so this page and the header switcher agree.
pub(crate) fn watch(model: Signal<Model>) {
    let ready = computed(move || {
        let m = model.get();
        (m.agents_page && m.live && m.data.is_some(), m.epoch)
    });
    effect(move || {
        if !ready.get().0 {
            return;
        }
        untracked(|| {
            let m = model.get();
            let entries = directory(&m);
            let listed = |p: &String| entries.iter().any(|e| &e.name == p);
            let current = m.selected_project.clone().filter(listed);
            let keep = m.agents.project.clone().filter(listed);
            if let Some(name) = current
                .or(keep)
                .or_else(|| entries.first().map(|e| e.name.clone()))
            {
                fetch(model, name);
            }
        });
    });
}

fn rows(model: Signal<Model>) -> impl IntoElement {
    let state = computed(move || {
        let m = model.get();
        (directory(&m), m.agents.project.clone(), m.locale)
    });
    dynamic(move || {
        let (entries, chosen, _) = state.get();
        if entries.is_empty() {
            return view()
                .gap_2()
                .items_start()
                .child(caption(model, |_| Some(Msg::AgentsNeedsProject)))
                .child(
                    button(model, "")
                        .label_fn(word(model, Msg::Projects))
                        .on_press(move || {
                            shell::go(
                                model,
                                shell::Destination::Section(acp_ui_core::Section::Projects),
                            )
                        }),
                )
                .into_element();
        }
        let m = model.get_untracked();
        view()
            .gap_1()
            .children(entries.into_iter().map(move |entry| {
                let name = entry.name.clone();
                let toggle_name = entry.name.clone();
                let shared = entry.agents;
                view()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .min_h(px(scale::control::LG))
                    .child(
                        view().flex_1().min_w_0().child(
                            button(model, acp_ui_core::presentation::sanitize(&entry.name, 64))
                                .variant(ButtonVariant::Ghost)
                                .selected({
                                    let chosen = chosen.clone();
                                    let name = entry.name.clone();
                                    move || chosen.as_deref() == Some(name.as_str())
                                })
                                .on_press(move || choose(model, name.clone())),
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
                            .spoken({
                                let name = entry.name.clone();
                                move || {
                                    let m = model.get();
                                    format!(
                                        "{} {name}",
                                        m.word(if shared {
                                            Msg::StopSharing
                                        } else {
                                            Msg::ShareProject
                                        })
                                    )
                                    .into()
                                }
                            })
                            .size(ButtonSize::Small)
                            .disabled(move || model.get().agents.busy)
                            .on_press(move || set_shared(model, toggle_name.clone(), !shared)),
                    )
            }))
            .into_element()
    })
}

fn choice(
    model: Signal<Model>,
    key: Msg,
    selected: impl Fn(&Model) -> bool + 'static,
    pick: impl Fn(&mut Model) + 'static,
) -> impl IntoElement {
    button(model, "")
        .label_fn(word(model, key))
        .size(ButtonSize::Small)
        .selected(move || selected(&model.get()))
        .on_press(move || {
            model.update(|m| {
                pick(m);
                m.agents.copied = false;
            })
        })
}

fn configure(model: Signal<Model>) -> impl IntoElement {
    let clients = Client::ALL.map(|client| {
        let key = match client {
            Client::ClaudeCode => Msg::ClientClaude,
            Client::Codex => Msg::ClientCodex,
            Client::Json => Msg::ClientJson,
        };
        choice(
            model,
            key,
            move |m| m.agents.client == client,
            move |m| m.agents.client = client,
        )
    });
    surface(model)
        .title_fn(move || {
            let m = model.get();
            let name = m.agents.project.clone().unwrap_or_default();
            format!("{} · {}", m.word(Msg::ConfigureFor), server_name(&name)).into()
        })
        .child(
            view()
                .gap_3()
                .child(view().flex_row().flex_wrap().gap_2().children(clients))
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .style_when(
                            move || model.get().connection.target != Target::Wsl,
                            |s| s.hidden(),
                        )
                        .child(choice(
                            model,
                            Msg::ClientOnWindows,
                            |m| !m.agents.inside_wsl,
                            |m| m.agents.inside_wsl = false,
                        ))
                        .child(choice(
                            model,
                            Msg::ClientInWsl,
                            |m| m.agents.inside_wsl,
                            |m| m.agents.inside_wsl = true,
                        )),
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
                    text(move || model.get().agents.error.clone().unwrap_or_default())
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).error),
                ),
        )
}

/// One event: time, what happened, who asked and the outcome.
fn activity_row(model: Signal<Model>, m: &Model, e: &acp_ui_core::activity::Event) -> AnyElement {
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
            text(acp_ui_core::activity::clock(e.at_ms))
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
                            .map(|p| acp_ui_core::activity::date_time(p.daemon_started_ms))
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
                    .replace("{time}", &acp_ui_core::activity::clock(e.at_ms))
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
    view()
        .gap_6()
        .max_w(px(scale::layout::FORM))
        .child(rows(model))
        .child(
            view()
                .style_when(
                    move || {
                        let m = model.get();
                        m.agents.project.is_none() || directory(&m).is_empty()
                    },
                    |s| s.hidden(),
                )
                .child(configure(model))
                .child(view().h(px(24.0)))
                .child(activity(model)),
        )
}
