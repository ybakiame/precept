use super::*;

pub(super) fn word(model: Signal<Model>, key: Msg) -> impl Fn() -> SharedString + 'static {
    move || model.get().word(key).into()
}
pub(super) fn caption(
    model: Signal<Model>,
    f: impl Fn(&Model) -> Option<Msg> + 'static,
) -> impl IntoElement {
    text(move || {
        let m = model.get();
        f(&m).map(|k| m.word(k)).unwrap_or_default()
    })
    .text_size(px(scale::text::CAPTION))
    .text_color(move || palette(model).muted)
}

pub(super) fn directory(m: &Model) -> Vec<ProjectEntry> {
    m.data
        .as_ref()
        .map(|d| d.directory.clone())
        .unwrap_or_default()
}

pub(crate) fn active_entry(m: &Model) -> Option<ProjectEntry> {
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
pub(super) fn host(m: &Model) -> Host {
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
pub(super) fn current_snippet(m: &Model) -> Option<String> {
    let launch = m.agents.launch.as_ref()?;
    (Some(&launch.name) == m.agents.project.as_ref())
        .then(|| snippet(m.agents.client, launch, &host(m)))
        .flatten()
}

/// Reads the project's session-start hook states.
/// (A refusal shown by `apply_hook` stays until the next switch.)
pub(super) fn load_hooks(model: Signal<Model>, name: String) {
    crate::features::profile::call(
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
pub(super) fn apply_hook(
    model: Signal<Model>,
    vendor: &'static str,
    event: &'static str,
    enabled: bool,
) {
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
    crate::features::profile::call(
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
pub(super) fn load_mcp(model: Signal<Model>, name: String) {
    crate::features::profile::call(
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
pub(super) fn apply_mcp(model: Signal<Model>, client: &'static str, enabled: bool) {
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
    crate::features::profile::call(
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

pub(super) fn mcp_detail(m: &Model, client: &str) -> String {
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

pub(super) fn mcp_tile(model: Signal<Model>, client: &'static str, title: Msg) -> impl IntoElement {
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
pub(super) fn load_pointer(model: Signal<Model>, name: String) {
    model.update(|m| {
        m.agents.pointer_busy = true;
        m.agents.pointer_error = None;
    });
    crate::features::profile::call(
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
pub(super) fn apply_pointer(model: Signal<Model>, add: bool) {
    let m = model.get();
    let (Some(name), Some(p)) = (m.agents.project.clone(), m.agents.pointer.clone()) else {
        return;
    };
    model.update(|m| {
        m.agents.pointer_busy = true;
        m.agents.pointer_error = None;
    });
    crate::features::profile::call(
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
    crate::features::profile::call(
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
    crate::features::profile::call(
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

pub(super) fn set_gate(model: Signal<Model>, name: String, enabled: bool) {
    model.update(|m| m.agents.busy = true);
    crate::features::profile::call(
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

pub(super) fn set_propose(model: Signal<Model>, name: String, enabled: bool) {
    model.update(|m| m.agents.busy = true);
    crate::features::profile::call(
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

pub(super) fn set_shared(model: Signal<Model>, name: String, enabled: bool) {
    model.update(|m| m.agents.busy = true);
    crate::features::profile::call(
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
        ) || (m.onboarding_open
            && m.onboarding_step == precept_ui_core::OnboardingStep::Agents);
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
