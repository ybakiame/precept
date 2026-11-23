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
    mcp::{Client, Hooks, Host, Launch, McpEntries, Pointer, SERVER, snippet},
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

/// Reads the project's session-start hook states.
/// (A refusal shown by `apply_hook` stays until the next switch.)
fn load_hooks(model: Signal<Model>, name: String) {
    super::profile::call(
        model,
        "integration.hooks",
        serde_json::json!({ "project": name }),
        move |_, m, r| {
            if m.agents.project.as_deref() != Some(name.as_str()) {
                return;
            }
            match r.and_then(|v| serde_json::from_value::<Hooks>(v).map_err(|_| "Unavailable")) {
                Ok(h) => m.agents.hooks = Some(h),
                Err(code) => {
                    m.agents.hooks = None;
                    m.agents.hooks_error = Some(code.into());
                }
            }
        },
    );
}

/// Switches one harness's hook for `event` (`session_start` or `pre_tool`),
/// guarded by the digest last read.
fn apply_hook(model: Signal<Model>, vendor: &'static str, event: &'static str, enabled: bool) {
    let m = model.get();
    let Some(name) = m.agents.project.clone() else {
        return;
    };
    let expected = m
        .agents
        .hooks
        .as_ref()
        .and_then(|h| h.of(vendor, event))
        .and_then(|h| h.sha256.clone());
    model.update(|m| {
        m.agents.hook_busy = Some(vendor.into());
        m.agents.hooks_error = None;
    });
    super::profile::call(
        model,
        "integration.hooks.apply",
        serde_json::json!({ "project": name, "vendor": vendor, "event": event, "enabled": enabled, "expected_sha256": expected }),
        move |model, m, r| {
            m.agents.hook_busy = None;
            if let Err(code) = r {
                m.agents.hooks_error = Some(if code == "Conflict" {
                    m.word(Msg::HookChanged).into()
                } else {
                    code.into()
                });
            }
            let name = name.clone();
            spawn(async move { load_hooks(model, name) });
        },
    );
}

/// Reads the project's MCP entry in each client's user configuration.
fn load_mcp(model: Signal<Model>, name: String) {
    super::profile::call(
        model,
        "integration.mcp",
        serde_json::json!({}),
        move |_, m, r| {
            if m.agents.project.as_deref() != Some(name.as_str()) {
                return;
            }
            match r.and_then(|v| serde_json::from_value::<McpEntries>(v).map_err(|_| "Unavailable"))
            {
                Ok(e) => {
                    m.agents.mcp = Some(e);
                    m.agents.mcp_error = None;
                }
                // Shown on the tiles, in words: never a bare code.
                Err(code) => {
                    m.agents.mcp = None;
                    m.agents.mcp_error = Some(code.into());
                }
            }
        },
    );
}

/// Switches one client's MCP entry, guarded by the digest last read.
fn apply_mcp(model: Signal<Model>, client: &'static str, enabled: bool) {
    let m = model.get();
    let Some(name) = m.agents.project.clone() else {
        return;
    };
    let expected = m
        .agents
        .mcp
        .as_ref()
        .and_then(|e| e.get(client))
        .and_then(|e| e.sha256.clone());
    model.update(|m| {
        m.agents.mcp_busy = Some(client.into());
        m.agents.hooks_error = None;
    });
    super::profile::call(
        model,
        "integration.mcp.apply",
        serde_json::json!({ "client": client, "enabled": enabled, "expected_sha256": expected }),
        move |model, m, r| {
            m.agents.mcp_busy = None;
            if let Err(code) = r {
                m.agents.hooks_error = Some(if code == "Conflict" {
                    m.word(Msg::HookChanged).into()
                } else {
                    code.into()
                });
            }
            let name = name.clone();
            spawn(async move { load_mcp(model, name) });
        },
    );
}

fn mcp_detail(m: &Model, client: &str) -> String {
    let Some(entries) = m.agents.mcp.as_ref() else {
        return m
            .word(match m.agents.mcp_error.as_deref() {
                Some("Unsupported") => Msg::McpNeedsDaemon,
                Some(_) => Msg::McpReadFailed,
                None => Msg::TileOff,
            })
            .into();
    };
    if !entries.available {
        return m.word(Msg::McpUnavailable).into();
    }
    match entries.get(client) {
        Some(e) if e.malformed => m.word(Msg::HookMalformed).replace("{file}", &e.file),
        Some(e) if e.installed && !e.current => m.word(Msg::HookOutdated).into(),
        Some(e) if e.installed => m.word(Msg::McpOn).replace("{file}", &e.file),
        _ => m.word(Msg::TileOff).into(),
    }
}

fn mcp_tile(model: Signal<Model>, client: &'static str, title: Msg) -> impl IntoElement {
    let installed = move || {
        model
            .get()
            .agents
            .mcp
            .as_ref()
            .and_then(|e| e.get(client))
            .is_some_and(|e| e.installed)
    };
    ui::quick_tile(model, icons::AGENTS, installed, move |on| {
        apply_mcp(model, client, on)
    })
    .title_fn(word(model, title))
    .spoken(move || {
        let m = model.get();
        m.word(Msg::McpSpoken)
            .replace("{name}", m.word(title))
            .into()
    })
    .detail(move || mcp_detail(&model.get(), client).into())
    .disabled(move || {
        let m = model.get();
        m.agents.mcp_busy.is_some()
            || m.agents
                .mcp
                .as_ref()
                .is_none_or(|e| !e.available || e.get(client).is_none_or(|s| s.malformed))
    })
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

pub(crate) fn fetch(model: Signal<Model>, name: String) {
    model.update(|m| {
        m.agents.project = Some(name.clone());
        m.agents.busy = true;
        m.agents.error = None;
        m.agents.copied = false;
    });
    load_activity(model, name.clone());
    load_pointer(model, name.clone());
    model.update(|m| m.agents.hooks_error = None);
    load_hooks(model, name.clone());
    load_mcp(model, name.clone());
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

fn set_gate(model: Signal<Model>, name: String, enabled: bool) {
    model.update(|m| m.agents.busy = true);
    super::profile::call(
        model,
        "project.gate",
        serde_json::json!({ "name": name, "enabled": enabled }),
        move |model, m, r| {
            m.agents.busy = false;
            match r {
                Ok(_) => ui::notify(
                    ui::Tone::Success,
                    m.word(if enabled {
                        Msg::ToastGateOn
                    } else {
                        Msg::ToastGateOff
                    })
                    .replace("{name}", &name),
                ),
                Err(code) => m.agents.error = Some(code.into()),
            }
            spawn(async move { load(model) });
        },
    );
}

fn set_propose(model: Signal<Model>, name: String, enabled: bool) {
    model.update(|m| m.agents.busy = true);
    super::profile::call(
        model,
        "project.propose",
        serde_json::json!({ "name": name, "enabled": enabled }),
        move |model, m, r| {
            m.agents.busy = false;
            match r {
                Ok(_) => ui::notify(
                    ui::Tone::Success,
                    m.word(if enabled {
                        Msg::ToastSuggestOn
                    } else {
                        Msg::ToastSuggestOff
                    })
                    .replace("{name}", &name),
                ),
                Err(code) => m.agents.error = Some(code.into()),
            }
            spawn(async move { load(model) });
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

/// Keeps this page (and Doctor, which reads the same integrations and
/// activity) aligned with the one current project chosen in the sidebar.
pub(crate) fn watch(model: Signal<Model>) {
    let ready = computed(move || {
        let m = model.get();
        let shown = matches!(
            shell::current(&m),
            shell::Destination::Agents
                | shell::Destination::Diagnostics
                | shell::Destination::Dashboard
        );
        (
            shown && m.live && m.data.is_some(),
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
                        text(m.word(if shared {
                            Msg::AgentsShared
                        } else {
                            Msg::AgentsNotShared
                        }))
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                    )
                    .child(ui::switch(
                        model,
                        move || {
                            model
                                .get()
                                .word(Msg::ShareWithAgents)
                                .replace("{name}", &spoken_name)
                                .into()
                        },
                        move || shared,
                        move || model.get().agents.busy,
                        move |on| set_shared(model, toggle_name.clone(), on),
                    )),
            )
            .child(suggest_row(model, &entry))
            .into_element()
    })
}

/// Whether the project's agents may suggest rules (`suggest_rule`).
fn suggest_row(model: Signal<Model>, entry: &ProjectEntry) -> impl IntoElement {
    let m = model.get_untracked();
    let on = entry.propose;
    let name = entry.name.clone();
    let spoken = entry.name.clone();
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_3()
        .pt_3()
        .border_t_1()
        .border_color(move || palette(model).hairline)
        .child(
            view()
                .flex_1()
                .min_w(px(160.0))
                .gap_0p5()
                .child(
                    text(m.word(if on { Msg::SuggestOn } else { Msg::SuggestOff }))
                        .font_weight(FontWeight::MEDIUM),
                )
                .child(
                    text(m.word(Msg::SuggestHelp))
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                ),
        )
        .child(ui::switch(
            model,
            move || {
                model
                    .get()
                    .word(Msg::SuggestSwitch)
                    .replace("{name}", &spoken)
                    .into()
            },
            move || on,
            move || model.get().agents.busy || !entry_shared(&model.get()),
            move |enabled| set_propose(model, name.clone(), enabled),
        ))
}
fn entry_shared(m: &Model) -> bool {
    active_entry(m).is_some_and(|e| e.agents)
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
            format!("{} · {}", m.word(Msg::ConfigureFor), SERVER).into()
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
                    text(move || model.get().agents.error.clone().unwrap_or_default())
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).error),
                ),
        )
}

/// The detail line of a hook tile.
fn hook_detail(m: &Model, vendor: &str, event: &str) -> String {
    let Some(hooks) = m.agents.hooks.as_ref() else {
        return m.word(Msg::TileOff).into();
    };
    if !hooks.available {
        return m.word(Msg::HookUnavailable).into();
    }
    let Some(h) = hooks.of(vendor, event) else {
        return m.word(Msg::TileOff).into();
    };
    if h.malformed {
        return m.word(Msg::HookMalformed).replace("{file}", &h.file);
    }
    if !h.installed {
        return m.word(Msg::TileOff).into();
    }
    if !h.current {
        return m.word(Msg::HookOutdated).into();
    }
    if event == "pre_tool" && !active_entry(m).is_some_and(|e| e.gate) {
        return m.word(Msg::GateHookNeedsGate).into();
    }
    m.word(if vendor == "codex" {
        Msg::HookOnCodex
    } else if event == "post_read" {
        Msg::ReadHookOn
    } else if event == "pre_compact" {
        Msg::CompactHookOn
    } else if event == "pre_tool" {
        Msg::GateHookOn
    } else {
        Msg::HookOn
    })
    .replace("{file}", &h.file)
}

fn hook_tile(
    model: Signal<Model>,
    vendor: &'static str,
    event: &'static str,
    title: Msg,
) -> impl IntoElement {
    let installed = move || {
        model
            .get()
            .agents
            .hooks
            .as_ref()
            .and_then(|h| h.of(vendor, event))
            .is_some_and(|h| h.installed)
    };
    let spoken = match event {
        "pre_tool" => Msg::GateHookSpoken,
        "post_read" => Msg::ReadHookSpoken,
        "pre_compact" => Msg::CompactHookSpoken,
        _ => Msg::HookSpoken,
    };
    ui::quick_tile(model, icons::AGENT, installed, move |on| {
        apply_hook(model, vendor, event, on)
    })
    .title_fn(word(model, title))
    .spoken(move || {
        let m = model.get();
        m.word(spoken).replace("{name}", m.word(title)).into()
    })
    .detail(move || hook_detail(&model.get(), vendor, event).into())
    .disabled(move || {
        let m = model.get();
        m.agents.hook_busy.is_some()
            || m.agents
                .hooks
                .as_ref()
                .is_none_or(|h| !h.available || h.of(vendor, event).is_none_or(|s| s.malformed))
    })
}

/// Whether the pre-tool gate answers for the project (`project.gate`).
fn gate_row(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    let Some(entry) = active_entry(&m) else {
        return view().into_element();
    };
    let on = entry.gate;
    let name = entry.name.clone();
    let spoken = entry.name.clone();
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(
            text(m.word(if on { Msg::GateOn } else { Msg::GateOff }))
                .font_weight(FontWeight::MEDIUM)
                .flex_1()
                .min_w(px(160.0)),
        )
        .child(ui::switch(
            model,
            move || {
                model
                    .get()
                    .word(Msg::GateSwitch)
                    .replace("{name}", &spoken)
                    .into()
            },
            move || on,
            move || model.get().agents.busy || !entry_shared(&model.get()),
            move |enabled| set_gate(model, name.clone(), enabled),
        ))
        .into_element()
}

fn pointer_tile(model: Signal<Model>) -> impl IntoElement {
    let state = move || {
        model
            .get()
            .agents
            .pointer
            .as_ref()
            .and_then(|p| p.state.clone())
            .unwrap_or_default()
    };
    ui::quick_tile(
        model,
        icons::FILE,
        move || state() != "absent" && !state().is_empty(),
        move |on| {
            if on {
                model.update(|m| m.agents.pointer_dialog = Some(true));
            } else {
                apply_pointer(model, false);
            }
        },
    )
    .title_fn(word(model, Msg::TileAgentsMd))
    .spoken(word(model, Msg::PointerSpoken))
    .detail(move || {
        let m = model.get();
        let key = match m.agents.pointer.as_ref() {
            Some(p) if p.malformed => Msg::PointerMalformed,
            Some(p) if p.state.as_deref() == Some("current") => Msg::PointerTileOn,
            Some(p) if p.state.as_deref() == Some("outdated") => Msg::PointerTileOutdated,
            _ => Msg::TileOff,
        };
        m.word(key).into()
    })
    .disabled(move || {
        let m = model.get();
        m.agents.pointer_busy || m.agents.pointer.as_ref().is_none_or(|p| p.malformed)
    })
}

/// One group of integration switches: what the mechanism is, what it can do
/// and where it reaches, then its tiles.
fn group(
    model: Signal<Model>,
    title: Msg,
    summary: Msg,
    scope: Msg,
    tiles: impl IntoElement,
) -> impl IntoElement {
    view()
        .gap_2()
        .pt_2()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_1()
                .child(text(word(model, title)).font_weight(FontWeight::SEMIBOLD))
                // The full scope and limits, behind the info button.
                .child(ui::info(
                    model,
                    move || {
                        let m = model.get();
                        m.word(Msg::AboutGroup)
                            .replace("{name}", m.word(title))
                            .into()
                    },
                    move || model.get().word(scope).into(),
                )),
        )
        .child(caption(model, move |_| Some(summary)))
        .child(tiles)
}

/// Each agent's connection level (hooks, MCP, both) with what it means;
/// the recommended setup and the add-ons in one line under it.
fn levels(model: Signal<Model>) -> impl IntoElement {
    level_rows(model, true).child(caption(model, |_| Some(Msg::LevelHelp)))
}
/// The heading and one row per agent with its level badge (also the
/// Dashboard's agent card).
pub(crate) fn level_rows(model: Signal<Model>, heading: bool) -> View {
    use precept_ui_core::mcp::Level;
    let row = move |agent: &'static str, name: Msg| {
        view()
            .flex_row()
            .items_center()
            .gap_3()
            .child(
                text(word(model, name))
                    .font_weight(FontWeight::MEDIUM)
                    .w(px(120.0)),
            )
            .child(dynamic(move || {
                let m = model.get();
                let (msg, variant) = match m.agents.level(agent) {
                    Some(Level::Full) => (Msg::LevelFull, BadgeVariant::Success),
                    Some(Level::Hooks) => (Msg::LevelHooks, BadgeVariant::Info),
                    Some(Level::Mcp) => (Msg::LevelMcp, BadgeVariant::Info),
                    Some(Level::None) => (Msg::LevelNone, BadgeVariant::Neutral),
                    None => (Msg::LevelUnknown, BadgeVariant::Neutral),
                };
                badge(model, m.word(msg)).variant(variant).into_element()
            }))
    };
    view()
        .gap_2()
        .pt_2()
        .child(
            text(word(model, Msg::LevelHeading))
                .font_weight(FontWeight::SEMIBOLD)
                .style_when(move || !heading, |s| s.hidden()),
        )
        .child(row("claude", Msg::TileClaude))
        .child(row("codex", Msg::TileCodex))
}

/// The integrations, by mechanism: MCP tools, the session-start briefing and
/// the AGENTS.md pointer. One switch per agent each.
fn integrations(model: Signal<Model>) -> impl IntoElement {
    let row = || view().flex_row().flex_wrap().gap_2();
    surface(model)
        .title_fn(word(model, Msg::IntegrationsTitle))
        .child(caption(model, |_| Some(Msg::IntegrationsHelp)))
        .child(caption(model, |_| Some(Msg::OtherAgents)))
        .child(levels(model))
        .child(group(
            model,
            Msg::McpGroup,
            Msg::McpSummary,
            Msg::McpScope,
            row()
                .child(mcp_tile(model, "claude", Msg::TileClaude))
                .child(mcp_tile(model, "codex", Msg::TileCodex)),
        ))
        .child(group(
            model,
            Msg::HookGroup,
            Msg::HookSummary,
            Msg::HookScope,
            row()
                .child(hook_tile(model, "claude", "session_start", Msg::TileClaude))
                .child(hook_tile(model, "codex", "session_start", Msg::TileCodex)),
        ))
        .child(group(
            model,
            Msg::GateGroup,
            Msg::GateSummary,
            Msg::GateScope,
            view()
                .gap_2()
                .child(dynamic(move || gate_row(model)))
                .child(
                    row()
                        .child(hook_tile(model, "claude", "pre_tool", Msg::TileClaude))
                        .child(hook_tile(model, "codex", "pre_tool", Msg::TileCodex)),
                ),
        ))
        .child(group(
            model,
            Msg::ReadGroup,
            Msg::ReadSummary,
            Msg::ReadScope,
            row().child(hook_tile(model, "claude", "post_read", Msg::TileClaude)),
        ))
        .child(group(
            model,
            Msg::CompactGroup,
            Msg::CompactSummary,
            Msg::CompactScope,
            row().child(hook_tile(model, "claude", "pre_compact", Msg::TileClaude)),
        ))
        .child(group(
            model,
            Msg::PointerGroup,
            Msg::PointerSummary,
            Msg::PointerScope,
            // One tile wide, like the tiles of the groups above.
            row()
                .child(pointer_tile(model))
                .child(ui::tile_spacer())
                .child(ui::tile_spacer()),
        ))
        .child(
            text(move || {
                let m = model.get();
                m.agents
                    .hooks_error
                    .clone()
                    .or_else(|| m.agents.pointer_error.clone())
                    .unwrap_or_default()
            })
            .text_size(px(scale::text::CAPTION))
            .text_color(move || palette(model).error),
        )
}

/// The exact hook command, for setting a hook up by hand.
fn hook_command(model: Signal<Model>) -> impl IntoElement {
    view()
        .gap_1()
        .style_when(
            move || {
                model
                    .get()
                    .agents
                    .hooks
                    .as_ref()
                    .and_then(|h| h.hooks.first())
                    .is_none()
            },
            |s| s.hidden(),
        )
        .child(caption(model, |_| Some(Msg::IntegrationsCommand)))
        .child(
            text(move || {
                model
                    .get()
                    .agents
                    .hooks
                    .as_ref()
                    .and_then(|h| h.get("claude"))
                    .map(|h| {
                        h.command
                            .replace("--vendor claude", "--vendor <claude|codex>")
                    })
                    .unwrap_or_default()
            })
            .font_family("monospace")
            .text_size(px(scale::text::CAPTION))
            .text_color(move || palette(model).muted),
        )
}

/// Manual setup, folded away: the MCP configuration to paste (for agents on
/// Windows or other clients) and the hook command.
fn advanced(model: Signal<Model>) -> impl IntoElement {
    ui::disclosure(
        model,
        word(model, Msg::AdvancedSetup),
        word(model, Msg::AdvancedHint),
        view()
            .gap_6()
            .child(configure(model))
            .child(surface(model).child(hook_command(model))),
    )
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
pub(crate) fn activity_row(
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
                match (ok, &e.rule) {
                    (true, _) => "OK".to_owned(),
                    (false, Some(rule)) => format!(
                        "{} · {}",
                        e.outcome,
                        precept_ui_core::presentation::sanitize(rule, 64)
                    ),
                    (false, None) => e.outcome.clone(),
                },
            )
            // A gate's answer is a decision, not a failure: a denial is the
            // one worth a look; allow and off are routine.
            .variant(match (e.kind.as_str(), e.outcome.as_str()) {
                (_, "ok") | ("gate", "allow") => BadgeVariant::Success,
                ("gate", "off") => BadgeVariant::Neutral,
                ("gate", "deny") => BadgeVariant::Warning,
                _ if refused || e.outcome == "Unauthorized" => BadgeVariant::Danger,
                _ => BadgeVariant::Warning,
            }),
        )
        .into_element()
}

/// The most events listed (the daemon keeps more; Refresh reads the newest).
const SHOWN: usize = 50;

/// Recent agent activity of the selected project, with its chain status
/// (shown on the Sessions page).
pub(crate) fn activity(model: Signal<Model>) -> impl IntoElement {
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
                        let chain = m
                            .agents
                            .activity
                            .page
                            .as_ref()
                            .and_then(|p| p.chain.clone());
                        let status = match chain {
                            Some(c) if !c.ok => m
                                .word(Msg::ChainBroken)
                                .replace("{n}", &c.broken_at.unwrap_or(0).to_string()),
                            Some(c) => m
                                .word(if c.dropped > 0 {
                                    Msg::ChainOkDropped
                                } else {
                                    Msg::ChainOk
                                })
                                .replace("{n}", &c.entries.to_string()),
                            None => String::new(),
                        };
                        format!("{} {status}", m.word(Msg::ActivityHelp))
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
                            if let Some(name) = model.get().selected_project.clone() {
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
        .max_w(px(scale::layout::CONTENT))
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
                .child(integrations(model))
                .child(advanced(model)),
        )
}
