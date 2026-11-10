//! Native-host session controls; never a Windows->WSL execution bridge.
use super::*;
use acp_ui_spike_model::{connection::Target, i18n::Msg};
use components::{card, word_action};
use nsh_core::element::input;
use serde_json::{Value, json};
use tokens::palette;
pub(super) fn open(model: Signal<Model>) {
    model.update(|m| {
        m.sessions = true;
        m.settings = false;
        m.connection_editor = false;
        m.session.confirm_start = false;
    });
    request(model, "session.refresh", json!({}));
}
fn enabled(model: Signal<Model>) -> bool {
    let m = model.get();
    m.phase == Phase::Ready
        && m.connection.target == Target::Local
        && m.selected_project.is_some()
        && !m.session.busy
}
fn request(model: Signal<Model>, method: &'static str, fields: Value) {
    if !enabled(model) {
        return;
    }
    let m = model.get();
    let epoch = m.epoch;
    let project = m.selected_project.clone().unwrap();
    let connection = m.connection.clone();
    let refresh = method == "session.refresh";
    if refresh && !m.session.polling.acquire() {
        return;
    }
    let mut serial = m.session.command_epoch;
    model.update(|m| {
        if !refresh {
            m.session.command_epoch += 1;
            serial = m.session.command_epoch;
            m.session.busy = true;
            m.session.confirm_start = false;
        }
    });
    spawn(async move {
        #[cfg(feature = "ipc")]
        let result = {
            let c = connection.clone();
            let p = project.clone();
            blocking(move || {
                acp_ui_spike_model::native::Inputs::from_connection(&c)?
                    .session_call(&p, method, fields)
            })
            .await
            .unwrap_or(Err("Session unavailable"))
        };
        #[cfg(not(feature = "ipc"))]
        let result = {
            let _ = (method, fields);
            Err("Session unavailable")
        };
        model.update(|m| {
            m.complete_session(epoch, &project, &connection, serial, refresh, result);
        });
    });
}
pub(super) fn poll(model: Signal<Model>) {
    spawn(async move {
        loop {
            timer(std::time::Duration::from_millis(750)).await;
            if model.get().sessions {
                request(model, "session.refresh", json!({}));
            }
        }
    });
}
fn send_prompt(model: Signal<Model>) {
    if enabled(model) && model.get().session.can_prompt() {
        let prompt = model.get().session.prompt;
        command(model, "session.prompt", json!({"prompt":prompt}));
        model.update(|m| m.session.prompt.clear());
    }
}
fn start_review(model: Signal<Model>) {
    if enabled(model)
        && !model.get().session.executable.is_empty()
        && model
            .get()
            .session
            .view
            .as_ref()
            .is_none_or(|v| !v.process_alive)
    {
        model.update(|m| m.session.confirm_start = true);
    }
}
fn confirm_start(model: Signal<Model>) {
    if enabled(model) && model.get().session.confirm_start {
        let exe = model.get().session.executable;
        request(
            model,
            "session.start",
            json!({"executable":exe,"confirmed":true}),
        );
    }
}
fn command(model: Signal<Model>, method: &'static str, mut fields: Value) {
    let Some(v) = model.get().session.view else {
        return;
    };
    fields["session_id"] = v.session_id.into();
    request(model, method, fields);
}
fn state_word(state: &acp_ui_spike_model::session::State) -> Msg {
    match state {
        acp_ui_spike_model::session::State::Idle => Msg::SessionIdle,
        acp_ui_spike_model::session::State::Starting => Msg::SessionStarting,
        acp_ui_spike_model::session::State::Ready => Msg::SessionReady,
        acp_ui_spike_model::session::State::Running => Msg::SessionRunning,
        acp_ui_spike_model::session::State::WaitingPermission => Msg::SessionWaiting,
        acp_ui_spike_model::session::State::Completed => Msg::SessionCompleted,
        acp_ui_spike_model::session::State::Failed => Msg::SessionFailed,
        acp_ui_spike_model::session::State::Stopping => Msg::SessionStopping,
        acp_ui_spike_model::session::State::Stopped => Msg::SessionStopped,
    }
}
pub(super) fn panel(model: Signal<Model>) -> impl IntoElement {
    let prompt_ref = element_ref();
    let global = KeyOptions {
        when: nsh_core::keymap::When::Always,
        ..Default::default()
    };
    on_key_with("alt+j", global, move |_| {
        open(model);
        prompt_ref.focus();
    });
    on_key_with("mod+Enter", global, move |_| {
        if model.get().sessions {
            send_prompt(model);
        }
    });
    on_key_with("mod+shift+s", global, move |_| {
        if model.get().sessions {
            start_review(model);
        }
    });
    on_key_with("mod+shift+Enter", global, move |_| {
        if model.get().sessions {
            confirm_start(model);
        }
    });

    on_key_with("alt+7", global, move |_| {
        if model.get().sessions {
            start_review(model);
        }
    });
    on_key_with("alt+8", global, move |_| {
        if model.get().sessions {
            confirm_start(model);
        }
    });
    on_key_with("alt+9", global, move |_| {
        if model.get().sessions {
            send_prompt(model);
        }
    });
    on_key_with("alt+q", global, move |_| {
        if model.get().sessions && enabled(model) {
            command(model, "session.cancel", json!({}));
        }
    });
    on_key_with("alt+x", global, move |_| {
        if model.get().sessions && enabled(model) {
            command(model, "session.stop", json!({}));
        }
    });
    effect(move || {
        if !model.get().sessions || model.get().settings || model.get().connection_editor {
            prompt_ref.blur();
        }
    });
    let summary = computed(move || {
        let m = model.get();
        (
            m.phase,
            m.connection.target,
            m.locale,
            m.session.capabilities,
            m.session.view,
            m.session.executable,
            m.session.confirm_start,
            m.session.error,
        )
    });
    let evidence = computed(move || {
        let m = model.get();
        (m.phase, m.locale, m.session.view, m.session.event_page)
    });
    view()
        .on_layout(move |_: &Bounds<Points>| {
            #[cfg(not(target_arch = "wasm32"))]
            if model.get().sessions
                && model.get().phase == Phase::Ready
                && std::env::var_os("ACP_UI_PROBE").is_some()
            {
                eprintln!("ACP_NSH_LAYOUT_READY elapsed={:.6}", probe_elapsed());
            }
        })
        .gap_2()
        .flex_shrink_0()
        .child(text(move || model.get().word(Msg::SessionHelp)))
        .child(text(move || model.get().word(Msg::SessionKeys)))
        .child(word_action(
            model,
            Msg::SessionRefresh,
            move || enabled(model),
            move || request(model, "session.refresh", json!({})),
        ))
        .child(dynamic(move || {
            let _ = summary.get();
            let m = model.get_untracked();
            let mut body = view().gap_2().flex_shrink_0();
            if m.connection.target != Target::Local || m.phase != Phase::Ready {
                return body.child(text(m.word(Msg::SessionDisabled)));
            }
            if let Some(c) = &m.session.capabilities {
                body = body
                    .child(text(format!(
                        "ACP v{} · session API v{} · resume: {}",
                        c.protocol_version, c.session_api_version, "unsupported"
                    )))
                    .child(view().flex_row().flex_wrap().gap_1().children(
                        c.executables.iter().map(move |label| {
                            let selected = label.clone();
                            let applied = label.clone();
                            components::action(
                                model,
                                format!(
                                    "{}{}",
                                    if model.get().session.executable == selected {
                                        "> "
                                    } else {
                                        ""
                                    },
                                    label
                                ),
                                move || enabled(model),
                                move || {
                                    model.update(|m| {
                                        m.session.executable = applied.clone();
                                        m.session.confirm_start = false;
                                    })
                                },
                            )
                        }),
                    ));
            }
            body = body.child(text(format!(
                "{}: {}",
                m.word(Msg::Executable),
                m.session.executable
            )));
            if m.session.confirm_start {
                let git = m
                    .project()
                    .map(|p| {
                        format!(
                            "project={} · branch={} · HEAD={} · dirty={}",
                            p.name,
                            p.context.repository.branch.as_deref().unwrap_or("Unknown"),
                            p.context.repository.head.as_deref().unwrap_or("Unknown"),
                            p.context
                                .repository
                                .dirty
                                .map(|v| v.to_string())
                                .unwrap_or("Unknown".into())
                        )
                    })
                    .unwrap_or_default();
                body = body
                    .child(card(
                        model,
                        m.word(Msg::StartSession),
                        vec![m.word(Msg::StartWarning).into(), git],
                    ))
                    .child(word_action(
                        model,
                        Msg::ConfirmStart,
                        move || enabled(model),
                        move || {
                            let exe = model.get().session.executable;
                            request(
                                model,
                                "session.start",
                                json!({"executable":exe,"confirmed":true}),
                            );
                        },
                    ))
                    .child(word_action(
                        model,
                        Msg::AbortStart,
                        || true,
                        move || model.update(|m| m.session.confirm_start = false),
                    ));
            } else {
                body = body.child(word_action(
                    model,
                    Msg::StartSession,
                    move || {
                        enabled(model)
                            && !model.get().session.executable.is_empty()
                            && model
                                .get()
                                .session
                                .view
                                .as_ref()
                                .is_none_or(|v| !v.process_alive)
                    },
                    move || start_review(model),
                ));
            }
            if let Some(v) = &m.session.view {
                body = body.child(card(
                    model,
                    m.word(Msg::SessionState),
                    vec![
                        format!("{} · {}", m.word(state_word(&v.state)), v.session_id),
                        format!("{}: {}", m.word(Msg::ActiveExecutable), v.executable),
                        format!("{}: {}", m.word(Msg::ProcessAlive), v.process_alive),
                        format!("{}: {} · ACP v1", m.word(Msg::AgentVersion), v.agent),
                        format!(
                            "{}: {} UTC epoch ms",
                            m.word(Msg::LastSessionEvent),
                            v.last_event_ms
                        ),
                        format!("stop: {}", v.stop_reason.as_deref().unwrap_or("—")),
                        format!(
                            "{}: {}",
                            m.word(Msg::AgentCapabilities),
                            v.load_session_advertised
                        ),
                    ],
                ));
                if v.failure.is_some() {
                    body = body.child(text(m.word(
                        if v.failure.as_deref() == Some("Authentication required") {
                            Msg::AgentAuthHelp
                        } else {
                            Msg::SessionError
                        },
                    )));
                }
            } else {
                body = body.child(text(m.word(Msg::SessionEmpty)));
            }
            if m.session.error.is_some() {
                body = body.child(text(m.word(Msg::SessionError)));
            }
            body
        }))
        .child(text(move || model.get().word(Msg::SessionPrompt)))
        .child(text(move || model.get().word(Msg::PromptLimit)))
        .child(
            input()
                .element_ref(&prompt_ref)
                .label(move || model.get().word(Msg::SessionPrompt))
                .h(px(32.0))
                .border_2()
                .border_color(move || palette(model).border)
                .bg(move || palette(model).panel)
                .text_color(move || palette(model).text)
                .focus(|s| s.border_color(palette(model).primary))
                .value(move || model.get().session.prompt.clone())
                .on_change(move |s: &str| {
                    model.update(|m| m.session.prompt = s.chars().take(4096).collect())
                }),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(word_action(
                    model,
                    Msg::SendPrompt,
                    move || enabled(model) && model.get().session.can_prompt(),
                    move || {
                        let prompt = model.get().session.prompt;
                        command(model, "session.prompt", json!({"prompt":prompt}));
                        model.update(|m| m.session.prompt.clear());
                    },
                ))
                .child(word_action(
                    model,
                    Msg::CancelTurn,
                    move || {
                        enabled(model)
                            && model.get().session.view.as_ref().is_some_and(|v| {
                                matches!(
                                    v.state,
                                    acp_ui_spike_model::session::State::Running
                                        | acp_ui_spike_model::session::State::WaitingPermission
                                )
                            })
                    },
                    move || command(model, "session.cancel", json!({})),
                ))
                .child(word_action(
                    model,
                    Msg::StopSession,
                    move || {
                        enabled(model)
                            && model
                                .get()
                                .session
                                .view
                                .as_ref()
                                .is_some_and(|v| v.process_alive)
                    },
                    move || command(model, "session.stop", json!({})),
                )),
        )
        .child(dynamic(move || {
            let _ = evidence.get();
            let m = model.get_untracked();
            let mut body = view().gap_2().flex_shrink_0();
            if let Some(v) = &m.session.view {
                for p in &v.permissions {
                    let deny_id = p.request_id.clone();
                    let allow_id = p.request_id.clone();
                    let option = p.allow_once.clone();
                    // Complete bounded JSON, never an abbreviated operation next to approval.
                    let lines = std::iter::once(m.word(Msg::PermissionHelp).to_owned())
                        .chain(
                            p.operation
                                .chars()
                                .collect::<Vec<_>>()
                                .chunks(256)
                                .map(|s| s.iter().collect::<String>())
                                .collect::<Vec<_>>(),
                        )
                        .collect();
                    body = body
                        .child(card(model, m.word(Msg::PermissionTitle), lines))
                        .child(word_action(
                            model,
                            Msg::DenyRequest,
                            move || enabled(model),
                            move || {
                                command(
                                    model,
                                    "session.permission",
                                    json!({"request_id":deny_id,"option_id":null}),
                                )
                            },
                        ))
                        .child(word_action(
                            model,
                            Msg::AllowOnce,
                            {
                                let available = option.is_some();
                                move || enabled(model) && available
                            },
                            move || {
                                command(
                                    model,
                                    "session.permission",
                                    json!({"request_id":allow_id,"option_id":option}),
                                )
                            },
                        ));
                }
                let start = m.session.event_page * 6;
                let lines = v.events.iter().skip(start).take(6).cloned().collect();
                body = body
                    .child(card(model, m.word(Msg::SessionEvents), lines))
                    .child(word_action(
                        model,
                        Msg::EventNext,
                        move || {
                            model
                                .get()
                                .session
                                .view
                                .as_ref()
                                .is_some_and(|v| v.events.len() > 6)
                        },
                        move || {
                            model.update(|m| {
                                let pages = m
                                    .session
                                    .view
                                    .as_ref()
                                    .map(|v| v.events.len().div_ceil(6))
                                    .unwrap_or(1)
                                    .max(1);
                                m.session.event_page = (m.session.event_page + 1) % pages;
                            })
                        },
                    ));
            }
            body
        }))
}
