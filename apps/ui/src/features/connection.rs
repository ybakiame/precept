//! Connection: pick where the daemon runs, check it, connect.
//!
//! WSL (Windows): choose a detected distribution; a read-only check runs inside
//! it (`acp-wsl-bridge --discover`) and reports systemd, the packaged user unit,
//! the daemon and the Profile. "Start" is the one explicit write it offers
//! (`systemctl --user start acp-daemon.service`). This computer (Linux): the
//! same check in-process. The manual fields stay available under Advanced for a
//! non-standard install. The applied choice is saved privately for the next
//! launch. No shell, TCP or credential crosses any boundary.
use crate::*;
use acp_ui_core::{
    Probe,
    connection::{Connection, DaemonStatus, Discovery, Target, discovery_status, wsl_status},
    i18n::Msg,
};
use tokens::palette;
use ui::{
    BadgeVariant, ButtonSize, ButtonVariant, badge, button, field, input, radio_group, scale, tabs,
};

/// Applies the draft (if valid), reads through it and saves it for the next launch.
pub(crate) fn apply(model: Signal<Model>) {
    let mut apply = false;
    model.update(|m| apply = m.apply_connection());
    if !apply {
        return;
    }
    #[cfg(feature = "ipc")]
    {
        let connection = model.get().connection.clone();
        spawn(async move {
            // Leaving WSL closes its bridge (never the WSL daemon); off the UI thread.
            if connection.target != Target::Wsl {
                let _ = blocking(acp_ui_client::bridge::close_active).await;
            }
            // Unit tests never write the developer's settings.
            #[cfg(not(test))]
            {
                let saved = blocking(move || acp_ui_client::settings::save(&connection))
                    .await
                    .unwrap_or(Err(acp_ui_client::settings::UNAVAILABLE));
                model.update(|m| m.save_failure = saved.err());
            }
        });
    }
    load(model);
}

/// Lists WSL distributions (read-only `wsl.exe --list --quiet`).
fn list_distributions(model: Signal<Model>) {
    #[cfg(feature = "ipc")]
    spawn(async move {
        let listed = blocking(acp_ui_client::discover::wsl_distributions)
            .await
            .unwrap_or(Err(wsl_status::HOST_UNAVAILABLE));
        model.update(|m| m.distributions = Some(listed));
    });
    #[cfg(not(feature = "ipc"))]
    model.update(|m| m.distributions = Some(Err(wsl_status::HOST_UNAVAILABLE)));
}

/// Runs the check for the draft's target, or the explicit daemon start.
fn check(model: Signal<Model>, start: bool) {
    let mut epoch = 0;
    model.update(|m| epoch = m.begin_probe(start));
    let draft = model.get().connection_draft.clone();
    #[cfg(feature = "ipc")]
    spawn(async move {
        let result = blocking(move || probe(&draft, start))
            .await
            .unwrap_or(Err(wsl_status::LAUNCH_FAILED));
        model.update(|m| m.finish_probe(epoch, result));
    });
    #[cfg(not(feature = "ipc"))]
    {
        let _ = (draft, start);
        model.update(|m| m.finish_probe(epoch, Err(wsl_status::HOST_UNAVAILABLE)));
    }
}

#[cfg(feature = "ipc")]
fn probe(draft: &Connection, start: bool) -> Result<Discovery, &'static str> {
    use acp_ui_client::discover;
    let helper = if draft.helper.is_empty() {
        discover::DEFAULT_HELPER
    } else {
        draft.helper.as_str()
    };
    match draft.target {
        Target::Wsl if start => {
            discover::wsl_start_daemon(&draft.distribution, &draft.user, helper)
        }
        Target::Wsl => discover::wsl_discover(&draft.distribution, helper),
        #[cfg(unix)]
        Target::Local if start => discover::start_local(),
        #[cfg(unix)]
        Target::Local => Ok(discover::local()),
        _ => Err(wsl_status::HOST_UNAVAILABLE),
    }
}

/// Whether this platform can check its target automatically.
fn discoverable(target: Target) -> bool {
    match target {
        Target::Wsl => cfg!(target_os = "windows"),
        Target::Local => cfg!(all(unix, feature = "ipc")),
        Target::Synthetic => false,
    }
}

fn select(model: Signal<Model>, target: Target, distribution: Option<&str>) {
    model.update(|m| m.select_target(target, distribution));
    let m = model.get();
    let ready = match target {
        Target::Wsl => !m.connection_draft.distribution.is_empty(),
        Target::Local => true,
        Target::Synthetic => false,
    };
    if discoverable(target) && ready {
        check(model, false);
    }
    if target == Target::Wsl && m.distributions.is_none() && cfg!(target_os = "windows") {
        list_distributions(model);
    }
}

/// Opens the page: lists distributions and re-checks a remembered selection.
pub(crate) fn opened(model: Signal<Model>) {
    let m = model.get();
    if m.connection_draft.target == Target::Wsl && m.distributions.is_none() {
        list_distributions(model);
    }
    if m.probe == Probe::Idle
        && discoverable(m.connection_draft.target)
        && (m.connection_draft.target != Target::Wsl || !m.connection_draft.distribution.is_empty())
    {
        check(model, false);
    }
}

fn word(model: Signal<Model>, key: Msg) -> impl Fn() -> SharedString + 'static {
    move || model.get().word(key).into()
}

fn caption(
    model: Signal<Model>,
    key: impl Fn(&Model) -> Option<Msg> + 'static,
) -> impl IntoElement {
    text(move || {
        let m = model.get();
        key(&m).map(|k| m.word(k)).unwrap_or_default()
    })
    .text_size(px(scale::text::CAPTION))
    .text_color(move || palette(model).muted)
}

fn heading(model: Signal<Model>, key: Msg) -> impl IntoElement {
    text(move || model.get().word(key))
        .role(AccessibilityRole::Header)
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(scale::text::BODY))
        .text_color(move || palette(model).text)
}

/// The page.
pub(crate) fn connection(model: Signal<Model>) -> impl IntoElement {
    let mut targets = tabs(model);
    if cfg!(target_os = "windows") {
        targets = targets.add_fn(
            word(model, Msg::WslTarget),
            move || model.get().connection_draft.target == Target::Wsl,
            move || select(model, Target::Wsl, None),
        );
    }
    targets = targets
        .add_fn(
            word(model, Msg::LocalTarget),
            move || model.get().connection_draft.target == Target::Local,
            move || select(model, Target::Local, None),
        )
        .add_fn(
            word(model, Msg::SyntheticTarget),
            move || model.get().connection_draft.target == Target::Synthetic,
            move || select(model, Target::Synthetic, None),
        );
    view()
        .gap_6()
        .max_w(px(scale::layout::FORM))
        .child(targets)
        .child({
            // Rebuild the body only when the target changes, never per keystroke.
            let target = computed(move || model.get().connection_draft.target);
            dynamic(move || body(model, target.get()))
        })
        .child(
            view()
                .gap_1()
                .child(caption(model, |m| {
                    m.connection_invalid.then_some(Msg::ConnectionInvalid)
                }))
                .child(caption(model, |m| m.save_failure.map(|_| Msg::SaveFailed)))
                .child(caption(model, |_| Some(Msg::ConnectionHelp))),
        )
}

fn body(model: Signal<Model>, target: Target) -> AnyElement {
    match target {
        Target::Synthetic => view()
            .gap_4()
            .items_start()
            .child(caption(model, |_| Some(Msg::DemoHelp)))
            .child(
                button(model, "")
                    .label_fn(word(model, Msg::UseDemo))
                    .variant(ButtonVariant::Primary)
                    .on_press(move || apply(model)),
            )
            .into_element(),
        _ => {
            let mut page = view().gap_6();
            if target == Target::Wsl {
                page = page.child(distributions(model));
            }
            if discoverable(target) {
                page = page.child(checks(model));
            } else {
                page = page.child(caption(model, |_| Some(Msg::LocalManual)));
            }
            page.child(actions(model, target))
                .child(advanced(model, target, !discoverable(target)))
                .into_element()
        }
    }
}

fn distributions(model: Signal<Model>) -> impl IntoElement {
    let listed = computed(move || model.get().distributions.clone());
    let list = dynamic(move || match &listed.get() {
        None => caption(model, |_| Some(Msg::Checking)).into_element(),
        Some(Err(reason)) => {
            let reason = *reason;
            caption(model, move |_| wsl_status::message(reason)).into_element()
        }
        Some(Ok(names)) if names.is_empty() => {
            caption(model, |_| Some(Msg::NoDistributions)).into_element()
        }
        Some(Ok(names)) => {
            let mut group = radio_group(model, word(model, Msg::DistributionList));
            for name in names.clone() {
                let chosen = name.clone();
                group = group.option(
                    name.clone(),
                    move || model.get().connection_draft.distribution == name,
                    move || select(model, Target::Wsl, Some(&chosen)),
                );
            }
            group.into_element()
        }
    });
    view()
        .gap_2()
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .child(heading(model, Msg::DistributionList))
                .child(
                    button(model, "")
                        .label_fn(word(model, Msg::Rescan))
                        .size(ButtonSize::Small)
                        .on_press(move || {
                            model.update(|m| m.distributions = None);
                            list_distributions(model);
                        }),
                ),
        )
        .child(list)
}

/// One check: name, state badge, optional hint and optional action, on fixed lanes.
fn row(
    model: Signal<Model>,
    name: Msg,
    state: Msg,
    variant: BadgeVariant,
    hint: Option<Msg>,
    action: Option<AnyElement>,
) -> AnyElement {
    let mut line = view()
        .flex_row()
        .items_center()
        .gap_3()
        .min_h(px(scale::control::MD))
        .child(
            text(move || model.get().word(name))
                .flex_1()
                .text_size(px(scale::text::BODY))
                .text_color(move || palette(model).text),
        )
        .child(badge(model, model.get_untracked().word(state)).variant(variant));
    if let Some(action) = action {
        line = line.child(action);
    }
    let mut cell = view().gap_0p5().child(line);
    if let Some(hint) = hint {
        cell = cell.child(caption(model, move |_| Some(hint)));
    }
    cell.into_element()
}

fn checks(model: Signal<Model>) -> impl IntoElement {
    let state = computed(move || {
        let m = model.get();
        let unchosen =
            m.connection_draft.target == Target::Wsl && m.connection_draft.distribution.is_empty();
        (m.probe.clone(), unchosen)
    });
    let rows = dynamic(move || {
        let (probe, unchosen) = state.get();
        match &probe {
            Probe::Idle if unchosen => {
                caption(model, |_| Some(Msg::ChooseDistribution)).into_element()
            }
            Probe::Idle | Probe::Checking => caption(model, |_| Some(Msg::Checking)).into_element(),
            Probe::Starting => caption(model, |_| Some(Msg::StartingDaemon)).into_element(),
            Probe::Failed(reason) => {
                let reason = *reason;
                caption(model, move |_| {
                    discovery_status::message(reason).or_else(|| wsl_status::message(reason))
                })
                .into_element()
            }
            Probe::Found(d) => found(model, d),
        }
    });
    view().gap_2().child(heading(model, Msg::Checks)).child(
        view()
            .gap_3()
            .p_4()
            .rounded(px(scale::radius::CONTAINER))
            .border_1()
            .border_color(move || palette(model).hairline)
            .bg(move || palette(model).panel)
            .child(rows),
    )
}

fn found(model: Signal<Model>, d: &Discovery) -> AnyElement {
    use BadgeVariant::{Danger, Neutral, Success, Warning};
    let systemd = if d.systemd {
        row(
            model,
            Msg::CheckSystemd,
            Msg::StateEnabled,
            Success,
            None,
            None,
        )
    } else {
        row(
            model,
            Msg::CheckSystemd,
            Msg::StateDisabled,
            Danger,
            Some(Msg::HintSystemd),
            None,
        )
    };
    let service = if d.unit_installed {
        row(
            model,
            Msg::CheckService,
            Msg::StateInstalled,
            Success,
            None,
            None,
        )
    } else {
        row(
            model,
            Msg::CheckService,
            Msg::StateMissing,
            Warning,
            Some(Msg::HintService),
            None,
        )
    };
    let startable = d.systemd && d.unit_installed;
    let daemon = match d.daemon {
        DaemonStatus::Ready { .. } => row(
            model,
            Msg::CheckDaemon,
            Msg::StateRunning,
            Success,
            None,
            None,
        ),
        DaemonStatus::Refused => row(
            model,
            Msg::CheckDaemon,
            Msg::StateRefused,
            Danger,
            Some(Msg::HintRefused),
            None,
        ),
        DaemonStatus::NotRunning => row(
            model,
            Msg::CheckDaemon,
            Msg::StateStopped,
            Warning,
            startable.then_some(Msg::HintStopped),
            startable.then(|| {
                button(model, "")
                    .label_fn(word(model, Msg::StartDaemon))
                    .size(ButtonSize::Small)
                    .on_press(move || check(model, true))
                    .into_element()
            }),
        ),
    };
    let profile = match d.daemon {
        DaemonStatus::Ready {
            profile_selected: true,
        } => row(
            model,
            Msg::CheckProfile,
            Msg::StateSelected,
            Success,
            None,
            None,
        ),
        DaemonStatus::Ready {
            profile_selected: false,
        } => row(
            model,
            Msg::CheckProfile,
            Msg::StateUnselected,
            Neutral,
            Some(Msg::HintProfile),
            None,
        ),
        // Without a daemon answer the Profile is not observed, not "none".
        _ => row(
            model,
            Msg::CheckProfile,
            Msg::StateUnknown,
            Neutral,
            None,
            None,
        ),
    };
    view()
        .gap_3()
        .child(systemd)
        .child(service)
        .child(daemon)
        .child(profile)
        .into_element()
}

fn actions(model: Signal<Model>, target: Target) -> impl IntoElement {
    let target_ready = move || {
        let m = model.get();
        // A checked target, or coordinates entered by hand (still validated).
        !discoverable(m.connection_draft.target)
            || m.connection_advanced
            || matches!(m.probe, Probe::Found(_))
    };
    let busy = move || matches!(model.get().probe, Probe::Checking | Probe::Starting);
    let mut row = view().flex_row().flex_wrap().items_center().gap_2().child(
        button(model, "")
            .label_fn(word(model, Msg::Connect))
            .variant(ButtonVariant::Primary)
            .disabled(move || !target_ready() || busy())
            .on_press(move || apply(model)),
    );
    if discoverable(target) {
        row = row.child(
            button(model, "")
                .label_fn(word(model, Msg::CheckAgain))
                .loading(busy)
                .disabled(move || {
                    let m = model.get();
                    m.connection_draft.target == Target::Wsl
                        && m.connection_draft.distribution.is_empty()
                })
                .on_press(move || check(model, false)),
        );
    }
    row.child(
        button(model, "")
            .label_fn(word(model, Msg::AdvancedFields))
            .on_press(move || model.update(|m| m.connection_advanced = !m.connection_advanced)),
    )
}

/// The manual coordinates. Shown when the target cannot be checked here, or on
/// request; edits only change the draft until Connect.
fn advanced(model: Signal<Model>, target: Target, always: bool) -> impl IntoElement {
    let edit = |get: fn(&Connection) -> &String, set: fn(&mut Connection, String), key: Msg| {
        field(
            model,
            word(model, key),
            input(model, "")
                .label_fn(word(model, key))
                .value(move || get(&model.get().connection_draft).clone())
                .invalid(move || model.get().connection_invalid)
                .on_submit(move |_: &str| apply(model))
                .on_change(move |v: &str| {
                    let value: String = v.chars().take(1024).collect();
                    model.update(|m| m.edit_draft(|draft| set(draft, value)))
                }),
        )
    };
    let fields = view().gap_4();
    let fields = match target {
        Target::Wsl => fields
            .child(edit(|c| &c.user, |c, v| c.user = v, Msg::Owner))
            .child(edit(|c| &c.helper, |c, v| c.helper = v, Msg::Helper))
            .child(edit(
                |c| &c.wsl_runtime,
                |c, v| c.wsl_runtime = v,
                Msg::WslRuntime,
            ))
            .child(edit(
                |c| &c.wsl_endpoint,
                |c, v| c.wsl_endpoint = v,
                Msg::WslEndpoint,
            )),
        _ => fields
            .child(edit(|c| &c.runtime, |c, v| c.runtime = v, Msg::Runtime))
            .child(edit(|c| &c.endpoint, |c, v| c.endpoint = v, Msg::Endpoint)),
    };
    fields.style_when(
        move || !always && !model.get().connection_advanced,
        |s| s.hidden(),
    )
}
