use super::access::entry_shared;
use super::state::{apply_hook, apply_pointer, caption, current_snippet, set_gate, word};
use super::*;

pub(super) fn configure(model: Signal<Model>) -> impl IntoElement {
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
                                drop(set_clipboard(text));
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
pub(super) fn hook_detail(m: &Model, vendor: &str, event: &str) -> String {
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

pub(super) fn hook_tile(
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
pub(super) fn gate_row(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    let Some(entry) = active_entry(&m) else {
        return view().into_element();
    };
    let on = entry.gate;
    let name = entry.name.clone();
    let spoken = entry.label.clone().unwrap_or_else(|| entry.name.clone());
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

pub(super) fn pointer_tile(model: Signal<Model>) -> impl IntoElement {
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
pub(super) fn group(
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
pub(super) fn levels(model: Signal<Model>) -> impl IntoElement {
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
