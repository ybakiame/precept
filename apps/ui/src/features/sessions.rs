//! Native-host session controls; never a Windows->WSL execution bridge.
use crate::*;
use acp_ui_core::{connection::Target, i18n::Msg};
use nsh_core::element::input;
use serde_json::{Value, json};
use shell::{card, word_action};
use tokens::palette;
pub(crate) fn open(model: Signal<Model>) {
    model.update(|m| {
        m.operator.open = false;
        m.operator.review = None;
        m.sessions = true;
        m.settings = false;
        m.connection_editor = false;
        m.profile_page = false;
        m.agents_page = false;
        m.session.confirm_start = false;
    });
    request(model, "session.refresh", json!({}));
}
fn enabled(model: Signal<Model>) -> bool {
    let m = model.get();
    // Local and WSL targets share the daemon-owned session policy; agents run
    // wherever that daemon runs (inside WSL for the WSL target).
    m.phase == Phase::Ready
        && matches!(m.connection.target, Target::Local | Target::Wsl)
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
                acp_ui_client::native::Inputs::from_connection(&c)?.session_call(&p, method, fields)
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
pub(crate) fn poll(model: Signal<Model>) {
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
        // Every review starts with context sharing off; it is a separate opt-in.
        model.update(|m| {
            m.session.confirm_start = true;
            m.session.share_context = false;
        });
    }
}
/// Keyboard choice of the configured program (Alt E): next in the daemon's list.
/// Only without a live agent; it abandons any pending Start review and opt-in.
fn cycle_executable(model: Signal<Model>) {
    let m = model.get();
    if !m.sessions || !enabled(model) || m.session.view.as_ref().is_some_and(|v| v.process_alive) {
        return;
    }
    let Some(list) = m
        .session
        .capabilities
        .as_ref()
        .map(|c| c.executables.clone())
    else {
        return;
    };
    if list.is_empty() {
        return;
    }
    let next = list
        .iter()
        .position(|e| *e == m.session.executable)
        .map_or(0, |i| (i + 1) % list.len());
    model.update(|m| {
        m.session.executable = list[next].clone();
        m.session.confirm_start = false;
        m.session.share_context = false;
    });
}
/// Explicit, separately confirmed per-start opt-in; only inside a Start review.
fn toggle_share(model: Signal<Model>) {
    if model.get().sessions && enabled(model) && model.get().session.confirm_start {
        model.update(|m| m.session.share_context = !m.session.share_context);
    }
}
/// The one Start request: `share_context` is sent only when opted in, and the
/// opt-in is consumed by this request.
fn start_request(model: Signal<Model>) {
    let m = model.get();
    let mut fields = json!({"executable":m.session.executable,"confirmed":true});
    if m.session.share_context {
        fields["share_context"] = true.into();
    }
    request(model, "session.start", fields);
    model.update(|m| m.session.share_context = false);
}
fn review_auth_retry(model: Signal<Model>) {
    if model.get().sessions && enabled(model) {
        model.update(|m| {
            m.session.review_auth_retry();
        });
    }
}
fn confirm_start(model: Signal<Model>) {
    if enabled(model) && model.get().session.confirm_start {
        start_request(model);
    }
}
fn command(model: Signal<Model>, method: &'static str, mut fields: Value) {
    let Some(v) = model.get().session.view else {
        return;
    };
    fields["session_id"] = v.session_id.into();
    request(model, method, fields);
}
fn review_permission(model: Signal<Model>) {
    if !model.get().sessions || !enabled(model) {
        return;
    }
    model.update(|m| {
        if let Some(v) = &m.session.view
            && !v.permissions.is_empty()
        {
            let next = m
                .session
                .permission_review
                .as_ref()
                .and_then(|id| v.permissions.iter().position(|p| &p.request_id == id))
                .map(|n| (n + 1) % v.permissions.len())
                .unwrap_or(0);
            m.session.permission_review = Some(v.permissions[next].request_id.clone());
        }
    });
}
fn decide_permission(model: Signal<Model>, allow: bool) {
    if !model.get().sessions || !enabled(model) {
        return;
    }
    let m = model.get();
    let Some(id) = &m.session.permission_review else {
        return;
    };
    let Some(scope) = m.session_scope() else {
        return;
    };
    apply_permission(model, &scope, id, allow);
}
fn apply_permission(
    model: Signal<Model>,
    scope: &acp_ui_core::session::SessionScope,
    id: &str,
    allow: bool,
) {
    if !model.get().sessions || !enabled(model) {
        return;
    }
    if let Some(fields) = model.get().permission_decision(scope, id, allow) {
        request(model, "session.permission", fields);
    }
}

fn auth_notice(model: Signal<Model>) -> impl IntoElement {
    let notice = computed(move || {
        let m = model.get();
        (m.locale, m.session.view.map(|v| v.authentication.status))
    });
    dynamic(move || {
        use acp_ui_core::session::AuthStatus;
        let (locale, status) = notice.get();
        let _ = locale;
        let mut body = view().gap_2().flex_shrink_0();
        if status == Some(AuthStatus::Required) {
            let m = model.get_untracked();
            body = body
                .child(card(
                    model,
                    m.word(Msg::SessionAuthRequired),
                    vec![m.word(Msg::AgentAuthHelp).to_owned()],
                ))
                .child(word_action(
                    model,
                    Msg::AuthRetry,
                    move || {
                        enabled(model) && model.get().session.view.is_some_and(|v| !v.process_alive)
                    },
                    move || review_auth_retry(model),
                ));
        }
        body
    })
}
fn permissions(model: Signal<Model>) -> impl IntoElement {
    let pending = computed(move || {
        let m = model.get();
        (
            m.locale,
            m.session.view.as_ref().map(|v| v.permissions.clone()),
            m.session.permission_review.clone(),
            m.session_scope(),
        )
    });
    dynamic(move || {
        let _ = pending.get();
        let m = model.get_untracked();
        let mut body = view().gap_2().flex_shrink_0();
        if let (Some(v), Some(scope)) = (&m.session.view, m.session_scope()) {
            for p in &v.permissions {
                let deny_id = p.request_id.clone();
                let allow_id = p.request_id.clone();
                let available = p.allow_once.is_some();
                let deny_scope = scope.clone();
                let allow_scope = scope.clone();
                let mut lines = vec![
                    m.word(Msg::PermissionHelp).to_owned(),
                    format!(
                        "{} {}",
                        if m.session.permission_review.as_ref() == Some(&p.request_id) {
                            m.word(Msg::Selected)
                        } else {
                            m.word(Msg::PermissionTitle)
                        },
                        p.request_id
                    ),
                ];
                lines.extend(
                    p.operation
                        .chars()
                        .collect::<Vec<_>>()
                        .chunks(256)
                        .map(|s| s.iter().collect::<String>()),
                );
                body = body
                    .child(card(model, m.word(Msg::PermissionTitle), lines))
                    .child(word_action(
                        model,
                        Msg::DenyRequest,
                        move || enabled(model),
                        move || apply_permission(model, &deny_scope, &deny_id, false),
                    ))
                    .child(word_action(
                        model,
                        Msg::AllowOnce,
                        move || enabled(model) && available,
                        move || apply_permission(model, &allow_scope, &allow_id, true),
                    ));
            }
        }
        body
    })
}
fn state_word(state: &acp_ui_core::session::State) -> Msg {
    match state {
        acp_ui_core::session::State::AuthRequired => Msg::SessionAuthRequired,
        acp_ui_core::session::State::Idle => Msg::SessionIdle,
        acp_ui_core::session::State::Starting => Msg::SessionStarting,
        acp_ui_core::session::State::Ready => Msg::SessionReady,
        acp_ui_core::session::State::Running => Msg::SessionRunning,
        acp_ui_core::session::State::WaitingPermission => Msg::SessionWaiting,
        acp_ui_core::session::State::Completed => Msg::SessionCompleted,
        acp_ui_core::session::State::Failed => Msg::SessionFailed,
        acp_ui_core::session::State::Stopping => Msg::SessionStopping,
        acp_ui_core::session::State::Stopped => Msg::SessionStopped,
    }
}
pub(crate) fn panel(model: Signal<Model>) -> impl IntoElement {
    let prompt_ref = element_ref();
    let global = KeyOptions {
        when: nsh_core::keymap::When::Always,
        ..Default::default()
    };
    let bind = |id: &'static str, handler: Box<dyn Fn()>| {
        crate::shortcuts::bind(model, id, global, handler)
    };
    bind(
        "sessions.prompt",
        Box::new(move || {
            open(model);
            prompt_ref.focus();
        }),
    );
    bind(
        "sessions.send",
        Box::new(move || {
            if model.get().sessions {
                send_prompt(model);
            }
        }),
    );
    bind(
        "sessions.start",
        Box::new(move || {
            if model.get().sessions {
                start_review(model);
            }
        }),
    );
    bind(
        "sessions.confirm",
        Box::new(move || {
            if model.get().sessions {
                confirm_start(model);
            }
        }),
    );
    bind("sessions.auth", Box::new(move || review_auth_retry(model)));
    bind("sessions.share", Box::new(move || toggle_share(model)));
    bind(
        "sessions.executable",
        Box::new(move || cycle_executable(model)),
    );
    bind(
        "sessions.cancel",
        Box::new(move || {
            if model.get().sessions && enabled(model) {
                command(model, "session.cancel", json!({}));
            }
        }),
    );
    bind(
        "sessions.stop",
        Box::new(move || {
            if model.get().sessions && enabled(model) {
                command(model, "session.stop", json!({}));
            }
        }),
    );
    bind(
        "sessions.permission",
        Box::new(move || review_permission(model)),
    );
    bind(
        "sessions.deny",
        Box::new(move || decide_permission(model, false)),
    );
    bind(
        "sessions.allow",
        Box::new(move || decide_permission(model, true)),
    );
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
        .child(word_action(
            model,
            Msg::SessionRefresh,
            move || enabled(model),
            move || request(model, "session.refresh", json!({})),
        ))
        .child(auth_notice(model))
        .child(permissions(model))
        .child(dynamic(move || {
            let _ = summary.get();
            let m = model.get_untracked();
            let mut body = view().gap_2().flex_shrink_0();
            if !matches!(m.connection.target, Target::Local | Target::Wsl)
                || m.phase != Phase::Ready
            {
                return body.child(text(m.word(Msg::SessionDisabled)));
            }
            if let Some(c) = &m.session.capabilities {
                body = body
                    .child(text(format!(
                        "ACP v{} · session API v{} · resume: {}",
                        c.protocol_version, c.session_api_version, "unsupported"
                    )))
                    .child(view().flex_row().flex_wrap().gap_1().children(
                        c.executables.iter().enumerate().map(move |(index, label)| {
                            let selected = label.clone();
                            let applied = label.clone();
                            shell::keyed_action(
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
                                "SessionExecutable",
                                index,
                                move || enabled(model),
                                move || {
                                    model.update(|m| {
                                        m.session.executable = applied.clone();
                                        m.session.confirm_start = false;
                                        m.session.share_context = false;
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
                        move || start_request(model),
                    ))
                    .child(text(m.word(if m.session.share_context {
                        Msg::ContextShareOn
                    } else {
                        Msg::ContextShareOff
                    })))
                    .child(word_action(
                        model,
                        Msg::ToggleContextShare,
                        move || enabled(model),
                        move || toggle_share(model),
                    ))
                    .child(word_action(
                        model,
                        Msg::AbortStart,
                        || true,
                        move || {
                            model.update(|m| {
                                m.session.confirm_start = false;
                                m.session.share_context = false;
                            })
                        },
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
                        match m.session.context {
                            Some(c) if c.enabled => format!(
                                "{}: {} · {}: {} / {}",
                                m.word(Msg::ContextLeaseLabel),
                                m.word(if c.active {
                                    Msg::LeaseActive
                                } else {
                                    Msg::LeaseRevoked
                                }),
                                m.word(Msg::ContextCallCounts),
                                c.context_calls,
                                c.handoff_calls
                            ),
                            _ => format!(
                                "{}: {}",
                                m.word(Msg::ContextLeaseLabel),
                                m.word(Msg::LeaseNotShared)
                            ),
                        },
                        format!("{}: {} · ACP v1", m.word(Msg::AgentVersion), v.agent),
                        format!(
                            "{}: {}",
                            m.word(Msg::AuthState),
                            m.word(match v.authentication.status {
                                acp_ui_core::session::AuthStatus::Unobserved => Msg::AuthUnobserved,
                                acp_ui_core::session::AuthStatus::Required =>
                                    Msg::SessionAuthRequired,
                                acp_ui_core::session::AuthStatus::SessionAccepted =>
                                    Msg::AuthSessionAccepted,
                            })
                        ),
                        format!(
                            "{}: {} / {} / {}",
                            m.word(Msg::AuthMethods),
                            v.authentication
                                .methods
                                .iter()
                                .filter(
                                    |k| **k == acp_ui_core::session::AuthMethodKind::Agent
                                )
                                .count(),
                            v.authentication
                                .methods
                                .iter()
                                .filter(|k| **k
                                    == acp_ui_core::session::AuthMethodKind::Terminal)
                                .count(),
                            v.authentication
                                .methods
                                .iter()
                                .filter(|k| **k
                                    == acp_ui_core::session::AuthMethodKind::Unsupported)
                                .count()
                        ),
                        format!(
                            "{}: {} / {} / {} / {} / {} / {}",
                            m.word(Msg::AuthCapabilities),
                            v.agent_features.image,
                            v.agent_features.audio,
                            v.agent_features.embedded_context,
                            v.agent_features.mcp_http,
                            v.agent_features.mcp_sse,
                            v.agent_features.logout
                        ),
                        m.word(Msg::AuthCapabilitiesHelp).to_owned(),
                        format!(
                            "{}: {} / {} / {}",
                            m.word(Msg::SessionActivity),
                            v.activity.response_chunks,
                            v.activity.response_characters,
                            v.activity.tool_events
                        ),
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
                body = body.child(text(m.word(Msg::SessionEmpty))).child(
                    view()
                        .gap_2()
                        .items_start()
                        .child(
                            text(m.word(Msg::SessionsMcpNote))
                                .text_size(px(ui::scale::text::CAPTION))
                                .text_color(move || tokens::palette(model).muted),
                        )
                        .child(
                            ui::button(model, "")
                                .label_fn(move || model.get().word(Msg::OpenAgentActivity).into())
                                .size(ui::ButtonSize::Small)
                                .on_press(move || shell::go(model, shell::Destination::Agents)),
                        ),
                );
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
                .h(px(ui::scale::control::MD))
                .px_3()
                .rounded(px(ui::scale::radius::CONTROL))
                .border_1()
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
                                    acp_ui_core::session::State::Running
                                        | acp_ui_core::session::State::WaitingPermission
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
