//! Minimal native operator panel; all business transitions live in shared::Model.
//! The Queue / Detail / History review workspace, the destructive delete tier and
//! their motion are rendered here for Desktop and TUI from the one shared Model.
use super::*;
use acp_operator_model::{
    connection::Target,
    i18n::{Locale, Msg, tr},
    operator::{
        Channel, Command, DecisionAction, DeleteArm, Origin, Pane, Review, ReviewTab, Scope,
    },
    presentation::sanitize,
};
use components::{action, card, word_action};
use nsh_core::element::input;
use tokens::{mix, palette};

pub(super) fn open(model: Signal<Model>) {
    model.update(|m| {
        m.operator.open = true;
        m.sessions = false;
        m.settings = false;
        m.connection_editor = false;
        m.operator.review = None;
        m.operator.delete_arm = None;
    });
    refresh(model, "profile.info");
}
fn available(model: Signal<Model>) -> bool {
    let m = model.get();
    m.operator.open
        && matches!(m.connection.target, Target::Local | Target::Wsl)
        && m.connection.validate()
        && !m.operator.busy
        && !m.operator.outcome_unknown
}
fn write_grant(model: Signal<Model>) -> bool {
    model
        .get()
        .operator
        .info
        .as_ref()
        .is_some_and(|i| i.write_enabled)
}
fn status_is(model: Signal<Model>, status: &str) -> bool {
    model
        .get()
        .operator
        .record()
        .is_some_and(|r| r.status == status)
}
fn refresh(model: Signal<Model>, method: &str) {
    let mut request = None;
    model.update(|m| request = m.operator_read(method));
    if let Some(request) = request {
        send(model, request);
    }
}
fn send(model: Signal<Model>, request: Review) {
    let connection = model.get().connection;
    // Anchor the in-flight request to a detached owner: a read launched from a
    // button inside a re-rendering region must still settle when that region
    // rebuilds. The shared model's guard fences any stale completion.
    Owner::detached().run(|| {
        spawn(async move {
            #[cfg(feature = "ipc")]
            let result = {
                let request = request.clone();
                blocking(move || {
                    acp_operator_model::native::Inputs::from_connection(&connection)?
                        .operator_call(&request)
                })
                .await
                .unwrap_or(Err("Unavailable"))
            };
            #[cfg(not(feature = "ipc"))]
            let result = {
                let _ = connection;
                Err("Unauthorized")
            };
            model.update(|m| {
                m.complete_operator(&request, result);
            });
            // Explicit Refresh updates effective context after inspection. Do not
            // destroy retained uncertain request identity through an automatic read.
        });
    });
}
/// Inspect is read-only; a missing exact snapshot never prepares an action.
/// The operator must invoke the decision again after seeing its disclosures.
fn ensure_inspected(model: Signal<Model>) -> bool {
    let exact = model.with(Model::operator_inspected);
    if !exact {
        model.update(|m| m.operator.review_tab = ReviewTab::Detail);
        refresh(model, "profile.review.inspect");
    }
    exact
}
fn review(model: Signal<Model>, action: &str) {
    if !available(model) {
        return;
    }
    if matches!(action, "confirm" | "reject" | "deprecate" | "delete") && !ensure_inspected(model) {
        return;
    }
    #[cfg(feature = "ipc")]
    let id = acp_operator_model::operator::operation_id();
    #[cfg(not(feature = "ipc"))]
    let id: Result<String, &str> = Err("Unauthorized");
    if let Ok(id) = id {
        model.update(|m| {
            // Preparing any non-destructive decision disarms a held delete tier.
            if action != "delete" {
                m.operator.delete_arm = None;
            }
            m.operator_review(action, &id);
        });
    }
}
fn submit(model: Signal<Model>) {
    let mut request = None;
    model.update(|m| request = m.operator_confirm());
    if let Some(request) = request {
        send(model, request);
    }
}
/// Cycle the review tabs (Queue / Detail / History) for the alt+left/right chords.
pub(super) fn tab_by(model: Signal<Model>, delta: i32) {
    model.update(|m| {
        let tabs = [ReviewTab::Queue, ReviewTab::Detail, ReviewTab::History];
        let index = tabs
            .iter()
            .position(|t| *t == m.operator.review_tab)
            .unwrap_or(0) as i32;
        m.operator.review_tab = tabs[(index + delta).rem_euclid(3) as usize];
    });
}
fn set_tab(model: Signal<Model>, tab: ReviewTab) {
    model.update(|m| m.operator.review_tab = tab);
}
/// The delete tier is armed only against the exact reviewed id+revision+digest
/// with the id retyped verbatim; mirrors the shared model's submit-time guard.
fn delete_armed(model: Signal<Model>) -> bool {
    let m = model.get();
    m.operator
        .delete_arm
        .as_ref()
        .zip(m.operator.record())
        .is_some_and(|(a, r)| {
            a.retype == r.id.to_string()
                && a.id == r.id
                && a.revision == r.revision
                && a.proposal_digest == r.proposal_digest
        })
}
fn retype_mismatch(model: Signal<Model>) -> bool {
    let m = model.get();
    m.operator
        .delete_arm
        .as_ref()
        .zip(m.operator.record())
        .is_some_and(|(a, r)| !a.retype.is_empty() && a.retype != r.id.to_string())
}
/// Open the destructive tier for the exact selected record (never auto-armed).
fn open_delete(model: Signal<Model>, delete_open: Signal<bool>) {
    if !available(model) || !write_grant(model) {
        return;
    }
    if !ensure_inspected(model) {
        return;
    }
    let m = model.get();
    let Some(r) = m.operator.record() else {
        return;
    };
    let arm = DeleteArm {
        retype: String::new(),
        id: r.id,
        revision: r.revision,
        proposal_digest: r.proposal_digest.clone(),
    };
    model.update(|m| {
        m.operator.delete_arm = Some(arm);
        m.operator.review = None;
    });
    delete_open.set(true);
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var_os("ACP_UI_PROBE").is_some() {
        eprintln!(
            "ACP_UI_DIALOG requested_open={} armed={}",
            delete_open.get(),
            model.with(|m| m.operator.delete_arm.is_some()),
        );
    }
}
fn disarm(model: Signal<Model>, delete_open: Signal<bool>) {
    delete_open.set(false);
    model.update(|m| m.operator.delete_arm = None);
}
/// Prepare the destructive delete only when exactly armed; the shared model
/// rechecks the arm again at submit. A blind activation prepares nothing.
fn commit_delete(model: Signal<Model>, delete_open: Signal<bool>) {
    if delete_open.get() && delete_armed(model) && available(model) && write_grant(model) {
        review(model, "delete");
        delete_open.set(false);
    }
}
fn error_msg(code: &str, wsl: bool) -> Msg {
    match code {
        "Unavailable" if wsl => Msg::ReviewErrorUnavailableWsl,
        "ReviewMismatch" => Msg::ReviewMismatch,
        "Conflict" => Msg::ReviewErrorConflict,
        "InvalidTransition" => Msg::ReviewErrorInvalidTransition,
        "Unauthorized" => Msg::ReviewErrorUnauthorized,
        "Busy" => Msg::ReviewErrorBusy,
        "Capacity" => Msg::ReviewErrorCapacity,
        "Unavailable" => Msg::ReviewErrorUnavailable,
        "OutcomeUnknown" => Msg::OperatorUnknown,
        _ => Msg::ReviewErrorGeneric,
    }
}
fn scope_label(locale: Locale, scope: &Scope) -> String {
    match scope {
        Scope::Global => tr(locale, Msg::ReviewScopeGlobal).into(),
        Scope::Project { name } => tr(locale, Msg::ReviewScopeProject).replace("{name}", name),
    }
}
fn project_name(scope: &Scope) -> String {
    match scope {
        Scope::Global => "—".into(),
        Scope::Project { name } => sanitize(name, 120),
    }
}
fn origin_label(origin: Origin) -> String {
    serde_json::to_value(origin)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}
fn decision_word(locale: Locale, action: DecisionAction) -> &'static str {
    match action {
        DecisionAction::Confirm => tr(locale, Msg::DecisionConfirm),
        DecisionAction::Reject => tr(locale, Msg::DecisionReject),
        DecisionAction::Deprecate => tr(locale, Msg::DecisionDeprecate),
        DecisionAction::Demote => tr(locale, Msg::DecisionDemote),
        DecisionAction::Delete => tr(locale, Msg::DecisionDelete),
    }
}
fn channel_word(locale: Locale, channel: Channel) -> &'static str {
    match channel {
        Channel::Cli => tr(locale, Msg::ChannelCli),
        Channel::Desktop => tr(locale, Msg::ChannelDesktop),
        Channel::Tui => tr(locale, Msg::ChannelTui),
    }
}
fn field(
    model: Signal<Model>,
    key: Msg,
    get: fn(&acp_operator_model::operator::Operator) -> &str,
    set: fn(&mut acp_operator_model::operator::Operator, String),
    max: usize,
) -> impl IntoElement {
    let reference = element_ref();
    let shortcut = match key {
        Msg::ProfilePath => "alt+p",
        Msg::OperatorKey => "alt+k",
        Msg::OperatorValue => "alt+w",
        Msg::OperatorSource => "alt+f",
        _ => "alt+t",
    };
    let pane = match key {
        Msg::ProfilePath => Pane::Profile,
        Msg::OperatorTask => Pane::Handoff,
        _ => Pane::Preferences,
    };
    on_key_with(
        shortcut,
        KeyOptions {
            when: nsh_core::keymap::When::Always,
            ..Default::default()
        },
        move |_| {
            if model.get().operator.open {
                model.update(|m| {
                    m.operator.pane = pane;
                    m.operator.review = None;
                    m.operator.delete_arm = None;
                });
                // Native Windows cannot focus a field before its pane is shown.
                nsh_core::element::request_animation_frame(move |_| {
                    if model.with(|m| {
                        m.operator.open
                            && m.operator.pane == pane
                            && !m.operator.busy
                            && !m.operator.outcome_unknown
                    }) {
                        reference.focus();
                    }
                });
            }
        },
    );
    effect(move || {
        if !model.get().operator.open {
            reference.blur();
        }
    });
    view()
        .gap_1()
        .child(text(move || model.get().word(key)))
        .child(
            input()
                .element_ref(&reference)
                .label(move || model.get().word(key))
                .value(move || get(&model.get().operator).to_owned())
                .on_focus(move || {
                    #[cfg(not(target_arch = "wasm32"))]
                    if std::env::var_os("ACP_UI_PROBE").is_some() {
                        eprintln!("ACP_UI_FOCUS shortcut={shortcut}");
                    }
                })
                .border_2()
                .border_color(move || palette(model).border)
                .bg(move || palette(model).panel)
                .text_color(move || palette(model).text)
                .focus(|s| s.border_color(palette(model).primary))
                .disabled(move || model.get().operator.busy || model.get().operator.outcome_unknown)
                .on_change(move |v: &str| {
                    model.update(|m| {
                        m.operator.review = None;
                        m.operator.delete_arm = None;
                        m.operator.serial += 1;
                        set(&mut m.operator, v.chars().take(max + 1).collect());
                    })
                }),
        )
}
/// The displayed preview never reveals the full digest without deliberate
/// expansion. The immutable mutation/request bytes remain untouched.
fn review_literal(review: &Review, expanded: bool, value: &str) -> Vec<String> {
    if !expanded && let Some(record) = &review.record {
        let prefix = record.proposal_digest.chars().take(16).collect::<String>();
        return literal(&value.replace(&record.proposal_digest, &format!("{prefix}…")));
    }
    literal(value)
}
/// Preserve all record characters for review; make control/bidi content visible.
fn literal(s: &str) -> Vec<String> {
    let escaped: String = s
        .chars()
        .flat_map(|c| {
            if c.is_control() || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}') {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect();
    escaped
        .chars()
        .collect::<Vec<_>>()
        .chunks(120)
        .map(|c| c.iter().collect())
        .collect()
}
/// The Queue tab: suggested-only rows, ascending id, bounded paging and an
/// explicit watermark delta. An empty queue is a valid state, never an error.
fn queue_tab(model: Signal<Model>, delete_open: Signal<bool>) -> AnyElement {
    let m = model.get();
    let mut body = view().gap_2().flex_shrink_0();
    body = body.child(
        view()
            .flex_row()
            .flex_wrap()
            .gap_2()
            .child(word_action(
                model,
                Msg::ReviewQueueLoad,
                move || {
                    available(model)
                        && model
                            .get()
                            .operator
                            .info
                            .as_ref()
                            .is_some_and(|i| i.selected)
                },
                move || refresh(model, "profile.review.queue"),
            ))
            .child(word_action(
                model,
                Msg::ReviewQueueFirst,
                move || available(model) && model.get().operator.queue_after_id > 0,
                move || {
                    disarm(model, delete_open);
                    model.update(|m| {
                        m.operator.queue_after_id = 0;
                        m.operator.review = None;
                    });
                    refresh(model, "profile.review.queue");
                },
            ))
            .child(word_action(
                model,
                Msg::ReviewQueueNext,
                move || {
                    available(model)
                        && model
                            .get()
                            .operator
                            .queue
                            .as_ref()
                            .is_some_and(|q| q.next_after_id.is_some())
                },
                move || {
                    disarm(model, delete_open);
                    model.update(|m| {
                        if let Some(id) = m.operator.queue.as_ref().and_then(|q| q.next_after_id) {
                            m.operator.queue_after_id = id;
                        }
                        m.operator.review = None;
                    });
                    refresh(model, "profile.review.queue");
                },
            )),
    );
    let Some(queue) = &m.operator.queue else {
        return body
            .child(ui::empty(model, m.word(Msg::ReviewQueueUnobserved)))
            .into_element();
    };
    body = body.child(text(
        tr(m.locale, Msg::ReviewQueueWatermark)
            .replace("{sequence}", &queue.commit_sequence.to_string()),
    ));
    body = body.child(
        text(m.word(Msg::ReviewQueuePaging))
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted),
    );
    // Even an empty later page can disclose drift; paging is not a snapshot.
    if queue.commit_sequence > m.operator.queue_watermark {
        let delta = queue.commit_sequence - m.operator.queue_watermark;
        body = body.child(
            // Keep the drift disclosure readable as a native text row.
            text(format!("{} · +{delta}", m.word(Msg::ReviewWatermark)))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).warning),
        );
    }
    if queue.records.is_empty() {
        return body
            .child(
                ui::empty(model, m.word(Msg::ReviewQueueEmpty))
                    .description(m.word(Msg::ReviewQueueEmptyHelp)),
            )
            .into_element();
    }
    for (index, r) in queue.records.iter().enumerate() {
        let id = r.id;
        let selected = m.operator.selected_id == Some(id);
        let label = format!("#{} · {} · r{}", id, sanitize(&r.key, 120), r.revision);
        let status = r.status.clone();
        // The page arrives as a wave: a capped index stagger, instant when reduced.
        let arrive = ui::motion::arrive((index.min(11) as f32) * 50.0);
        let row = view()
            .flex_row()
            .items_center()
            .gap_2()
            .p_2()
            .rounded(px(ui::scale::radius::CONTROL))
            .transition_colors()
            .duration(ui::motion::ms(ui::motion::FAST))
            .ease_out()
            .bg(move || {
                if selected {
                    mix(palette(model).inset, palette(model).primary, 0.10)
                } else {
                    palette(model).panel
                }
            })
            .child(view().flex_1().child(action(
                model,
                label,
                move || available(model),
                move || {
                    disarm(model, delete_open);
                    model.update(|m| {
                        m.operator.selected_id = Some(id);
                        m.operator.inspect = None;
                        m.operator.inspect_revision = 0;
                        m.operator.digest_expanded = false;
                        m.operator.review_tab = ReviewTab::Detail;
                        m.operator.review = None;
                        m.operator.serial += 1;
                    });
                    refresh(model, "profile.review.inspect");
                },
            )))
            .child(ui::badge(model, status).variant(ui::BadgeVariant::Warning))
            .child(if selected {
                ui::badge(model, m.word(Msg::Selected))
                    .variant(ui::BadgeVariant::Info)
                    .into_element()
            } else {
                view().into_element()
            });
        body = body.child(row.bind_motion(&arrive));
    }
    body.into_element()
}
/// The Detail tab: the exact inspected record, digest prefix with explicit
/// expand, peer-demotion disclosure and the decision / destructive actions.
fn detail_tab(model: Signal<Model>, delete_open: Signal<bool>) -> AnyElement {
    let m = model.get();
    let mut body = view().gap_2().flex_shrink_0();
    let Some(record) = m.operator.record() else {
        return body
            .child(
                ui::empty(model, m.word(Msg::ReviewNoSelection))
                    .description(m.word(Msg::ReviewQueueEmptyHelp)),
            )
            .into_element();
    };
    let Some(inspect) = m
        .operator
        .inspect
        .as_ref()
        .filter(|i| i.record.id == record.id)
    else {
        return body
            .child(word_action(
                model,
                Msg::ReviewInspect,
                move || available(model) && model.get().operator.record().is_some(),
                move || refresh(model, "profile.review.inspect"),
            ))
            .into_element();
    };
    let r = &inspect.record;
    let digest = if m.operator.digest_expanded {
        r.proposal_digest.clone()
    } else {
        format!(
            "{}…",
            r.proposal_digest.chars().take(16).collect::<String>()
        )
    };
    let mut lines = vec![
        format!("ID: {}", r.id),
        format!(
            "{}: {}",
            m.word(Msg::Scope),
            scope_label(m.locale, &r.scope)
        ),
        format!("{}: {}", m.word(Msg::Project), project_name(&r.scope)),
        format!("{}: {}", m.word(Msg::ReviewStatus), r.status),
        format!("{}:", m.word(Msg::OperatorKey)),
    ];
    lines.extend(literal(&r.key));
    lines.push(format!("{}:", m.word(Msg::OperatorValue)));
    lines.extend(literal(&r.value));
    lines.push(format!("{}:", m.word(Msg::OperatorSource)));
    lines.extend(literal(&r.source));
    lines.push(format!(
        "{}: {}",
        m.word(Msg::ReviewOrigin),
        origin_label(r.origin)
    ));
    lines.push(format!("{}: {}", m.word(Msg::ReviewRevision), r.revision));
    lines.push(format!("{}: {}", m.word(Msg::ReviewDigest), digest));
    body = body.child(card(model, m.word(Msg::ReviewDetailTab), lines));
    body = body.child(
        text(m.word(Msg::ReviewProvenanceNote))
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted),
    );
    body = body.child(
        text(m.word(Msg::ReviewContractNote))
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted),
    );
    body = body.child(word_action(
        model,
        if m.operator.digest_expanded {
            Msg::ReviewDigestCollapse
        } else {
            Msg::ReviewDigestExpand
        },
        || true,
        move || model.update(|m| m.operator.digest_expanded = !m.operator.digest_expanded),
    ));
    // Peer-demotion disclosure read from inspect; the authoritative set is
    // computed at commit. A truncated preview still names the full total.
    if inspect.peer_demotion_total > 0 {
        let mut ids = inspect
            .peer_demotions
            .iter()
            .map(|p| p.id.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        if (inspect.peer_demotion_total as usize) > inspect.peer_demotions.len() {
            ids.push_str(", …");
        }
        let disclosure = tr(m.locale, Msg::ReviewPeerDemotion)
            .replace("{count}", &inspect.peer_demotion_total.to_string())
            .replace("{ids}", &ids);
        body = body.child(
            text(disclosure)
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).warning),
        );
    }
    if r.status == "confirmed" {
        body = body.child(
            text(m.word(Msg::ReviewDeprecateImpact))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        );
    }
    body = body.child(
        view()
            .flex_row()
            .flex_wrap()
            .gap_2()
            .child(word_action(
                model,
                Msg::OperatorConfirm,
                move || available(model) && write_grant(model) && status_is(model, "suggested"),
                move || review(model, "confirm"),
            ))
            .child(word_action(
                model,
                Msg::OperatorReject,
                move || available(model) && write_grant(model) && status_is(model, "suggested"),
                move || review(model, "reject"),
            ))
            .child(word_action(
                model,
                Msg::ReviewDeprecate,
                move || available(model) && write_grant(model) && status_is(model, "confirmed"),
                move || review(model, "deprecate"),
            ))
            .child(
                ui::button(model, "")
                    .label_fn(move || model.get().word(Msg::ReviewDelete).into())
                    .variant(ui::ButtonVariant::Danger)
                    .disabled(move || {
                        !(available(model)
                            && write_grant(model)
                            && model.get().operator.record().is_some())
                    })
                    .on_press(move || open_delete(model, delete_open)),
            ),
    );
    body.into_element()
}
/// The History tab: bounded decision rows for the inspected id. A deleted record
/// keeps no content or history snapshot, so this falls back to the empty state.
fn history_tab(model: Signal<Model>) -> AnyElement {
    let m = model.get();
    let mut body = view().gap_2().flex_shrink_0();
    let Some(inspect) = m
        .operator
        .inspect
        .as_ref()
        .filter(|i| m.operator.selected_id == Some(i.record.id))
    else {
        return body
            .child(
                ui::empty(model, m.word(Msg::ReviewNoSelection))
                    .description(m.word(Msg::ReviewQueueEmptyHelp)),
            )
            .into_element();
    };
    if inspect.decisions.is_empty() {
        return body
            .child(ui::empty(model, m.word(Msg::ReviewHistoryEmpty)))
            .into_element();
    }
    body = body.child(
        text(m.word(Msg::ReviewChannelNote))
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted),
    );
    let lines = inspect
        .decisions
        .iter()
        .take(8)
        .map(|d| {
            format!(
                "{} · r{}→r{} · {} · {} · {} {}",
                decision_word(m.locale, d.action),
                d.from_revision,
                d.to_revision,
                channel_word(m.locale, d.channel),
                sanitize(&d.decided_at, 40),
                m.word(Msg::ReviewCommit),
                d.receipt_commit_sequence
            )
        })
        .collect();
    body = body.child(card(model, m.word(Msg::ReviewHistoryTab), lines));
    body.into_element()
}
/// The review workspace: one sliding tab selection over Queue / Detail / History.
fn review_workspace(model: Signal<Model>, delete_open: Signal<bool>) -> impl IntoElement {
    let tabs = ui::tabs(model)
        .add_fn(
            move || model.get().word(Msg::ReviewQueueTab).into(),
            move || model.get().operator.review_tab == ReviewTab::Queue,
            move || set_tab(model, ReviewTab::Queue),
        )
        .add_fn(
            move || model.get().word(Msg::ReviewDetailTab).into(),
            move || model.get().operator.review_tab == ReviewTab::Detail,
            move || set_tab(model, ReviewTab::Detail),
        )
        .add_fn(
            move || model.get().word(Msg::ReviewHistoryTab).into(),
            move || model.get().operator.review_tab == ReviewTab::History,
            move || set_tab(model, ReviewTab::History),
        );
    view()
        .gap_2()
        .flex_shrink_0()
        .child(tabs)
        .child(dynamic(move || {
            let tab = model.get().operator.review_tab;
            match tab {
                ReviewTab::Queue => queue_tab(model, delete_open),
                ReviewTab::Detail => detail_tab(model, delete_open),
                ReviewTab::History => history_tab(model),
            }
        }))
}
/// The destructive delete tier: names the exact scope+id, states the irreversible
/// removal of content and decision history (no forensic-erasure claim) and the
/// continuing identity-only receipt, and requires the exact id retyped into a
/// focused field before the commitment can fire.
fn delete_dialog(
    model: Signal<Model>,
    delete_open: Signal<bool>,
    retype_ref: ElementRef,
) -> impl IntoElement {
    ui::alert_dialog(model, "")
        .title_fn(move || {
            let m = model.get();
            match m.operator.delete_arm.as_ref().zip(m.operator.record()) {
                Some((a, r)) => tr(m.locale, Msg::ReviewDeleteTitle)
                    .replace("{scope}", &scope_label(m.locale, &r.scope))
                    .replace("{id}", &a.id.to_string())
                    .into(),
                None => String::new().into(),
            }
        })
        .description_fn(move || model.get().word(Msg::ReviewDeleteBody).into())
        .cancel_fn(move || model.get().word(Msg::Cancel).into())
        .confirm_fn(move || model.get().word(Msg::ReviewDeleteConfirm).into())
        .confirm_disabled(move || !delete_armed(model))
        .open(move || {
            delete_open.get() && model.with(|m| m.operator.open && m.operator.delete_arm.is_some())
        })
        .initial_focus(&retype_ref)
        .on_cancel(move || disarm(model, delete_open))
        .on_confirm(move || commit_delete(model, delete_open))
        .content_fn(move || {
            view()
                .gap_1()
                .child(
                    ui::input(model, "")
                        .label_fn(move || model.get().word(Msg::ReviewDeleteRetype).into())
                        .element_ref(&retype_ref)
                        .value(move || {
                            model
                                .get()
                                .operator
                                .delete_arm
                                .as_ref()
                                .map(|a| a.retype.clone())
                                .unwrap_or_default()
                        })
                        .placeholder(model.get().word(Msg::ReviewDeleteRetypeHint))
                        .max_length(20)
                        .invalid(move || retype_mismatch(model))
                        .on_change(move |v| {
                            model.update(|m| {
                                if let Some(a) = &mut m.operator.delete_arm {
                                    a.retype = v.chars().take(20).collect();
                                }
                            })
                        })
                        .on_submit(move |_| commit_delete(model, delete_open)),
                )
                .child(
                    text(move || {
                        let m = model.get();
                        if delete_armed(model) {
                            m.word(Msg::ReviewDeleteArmed)
                        } else if retype_mismatch(model) {
                            m.word(Msg::ReviewDeleteMismatch)
                        } else {
                            m.word(Msg::ReviewDeleteRetypeHint)
                        }
                    })
                    .text_size(px(ui::scale::text::CAPTION))
                    .text_color(move || {
                        if delete_armed(model) {
                            palette(model).success
                        } else if retype_mismatch(model) {
                            palette(model).danger
                        } else {
                            palette(model).muted
                        }
                    }),
                )
                .into_element()
        })
}
pub(super) fn panel(model: Signal<Model>) -> impl IntoElement {
    let global = KeyOptions {
        when: nsh_core::keymap::When::Always,
        ..Default::default()
    };
    // The destructive tier's presentation state; the armed identity itself lives
    // in the shared model so Desktop and TUI enforce one guard.
    let delete_open = signal(false);
    let retype_ref = element_ref();
    let scroll_ref = element_ref();
    let review_origin = signal(px(0.0));
    let preview_origin = signal(px(0.0));
    let navigation = computed(move || {
        model.with(|m| {
            let o = &m.operator;
            (
                o.open,
                o.review_tab,
                o.inspect
                    .as_ref()
                    .map(|i| (i.record.id, i.record.revision, i.commit_sequence)),
                o.queue
                    .as_ref()
                    .map(|q| (q.commit_sequence, q.next_after_id, q.records.len())),
                o.review.is_some(),
                o.outcome_unknown,
            )
        })
    });
    let previous_navigation = signal(None);
    effect(move || {
        let next = navigation.get();
        let previous = untracked(|| previous_navigation.get());
        previous_navigation.set(Some(next));
        let (open, tab, inspect, queue, prepared, unknown) = next;
        let changed = previous.is_none_or(
            |(was_open, old_tab, old_inspect, old_queue, was_prepared, was_unknown)| {
                !was_open
                    || tab != old_tab
                    || (inspect.is_some() && inspect != old_inspect)
                    || (queue.is_some() && queue != old_queue)
                    || (prepared && !was_prepared)
                    || (unknown && !was_unknown)
            },
        );
        if open
            && changed
            && (prepared
                || unknown
                || (tab == ReviewTab::Queue && queue.is_some())
                || (tab != ReviewTab::Queue && inspect.is_some()))
        {
            // Reveal the tab header or preview after layout, not the document's
            // end: long Detail content must not bury its own navigation.
            nsh_core::element::request_animation_frame(move |_| {
                if navigation.get() == next {
                    let y = if prepared || unknown {
                        preview_origin.get()
                    } else {
                        review_origin.get()
                    };
                    scroll_ref.scroll_to(nsh_core::geometry::Point::new(px(0.0), y), false);
                }
            });
        }
    });
    for (key, action) in [
        ("alt+shift+u", "create"),
        ("alt+shift+m", "select"),
        ("alt+shift+e", "enable"),
        ("alt+shift+x", "disable"),
        ("alt+shift+s", "suggest"),
        ("alt+shift+y", "confirm"),
        ("alt+shift+n", "reject"),
        ("alt+shift+w", "deprecate"),
    ] {
        on_key_with(key, global, move |_| {
            if model.get().operator.open {
                review(model, action);
            }
        });
    }
    on_key_with("alt+shift+q", global, move |_| {
        if model.get().operator.open {
            open_delete(model, delete_open);
        }
    });
    on_key_with("alt+shift+v", global, move |_| {
        if !model.get().operator.open {
            return;
        }
        // An open, exactly armed delete tier prepares then applies in one chord;
        // a blind or stale retype prepares nothing and applies nothing.
        if delete_open.get() && model.get().operator.delete_arm.is_some() {
            commit_delete(model, delete_open);
            if model.get().operator.review.is_some() {
                submit(model);
            }
            return;
        }
        submit(model);
    });
    on_key_with("alt+shift+r", global, move |_| {
        if model.get().operator.open {
            refresh(model, "profile.info");
        }
    });
    on_key_with("alt+shift+l", global, move |_| {
        if model.get().operator.open {
            model.update(|m| m.operator.pane = Pane::Preferences);
            refresh(model, "profile.preferences.list");
        }
    });
    // Windows' native Tab chain visits text fields; expose the existing exact
    // saved-Handoff read without depending on a button's platform traversal.
    on_key_with("alt+shift+h", global, move |_| {
        let m = model.get();
        if m.operator.pane == Pane::Handoff && m.selected_project.is_some() && available(model) {
            refresh(model, "profile.handoff.get");
        }
    });
    on_key_with("alt+shift+z", global, move |_| {
        if model.get().operator.open {
            let mut request = None;
            model.update(|m| request = m.operator_retry());
            if let Some(r) = request {
                send(model, r);
            }
        }
    });
    on_key_with("alt+shift+i", global, move |_| {
        if model.get().operator.open && available(model) {
            model.update(|m| {
                if let Some(page) = &m.operator.page {
                    let next = m
                        .operator
                        .selected_id
                        .and_then(|id| page.records.iter().position(|r| r.id == id))
                        .map(|n| (n + 1) % page.records.len().max(1))
                        .unwrap_or(0);
                    m.operator.selected_id = page.records.get(next).map(|r| r.id);
                    m.operator.inspect = None;
                    m.operator.inspect_revision = 0;
                    m.operator.digest_expanded = false;
                    m.operator.serial += 1;
                    m.operator.review = None;
                    // A selection change never carries an armed delete forward.
                    m.operator.delete_arm = None;
                }
            });
        }
    });
    effect(move || {
        if model.get().operator.delete_arm.is_none() {
            delete_open.set(false);
        }
    });
    let summary = computed(move || {
        let m = model.get();
        (m.locale, m.operator)
    });
    let rows = computed(move || {
        let m = model.get();
        (m.locale, m.operator.page)
    });
    effect(move || {
        #[cfg(not(target_arch = "wasm32"))]
        if std::env::var_os("ACP_UI_PROBE").is_some() {
            let m = model.get();
            let o = &m.operator;
            if o.open {
                eprintln!(
                    "ACP_NSH_OPERATOR selected={} grant={} busy={} review={} unknown={} rows={} record_revision={} receipt={} error={} handoff_revision={} review_tab={:?} queue={} inspect={} delete_armed={} digest_expanded={} inspection_current={} inspected_id={} inspected_revision={} peer_total={} delete_retype_chars={} delete_ready={}",
                    o.info.as_ref().is_some_and(|i| i.selected),
                    o.info.as_ref().is_some_and(|i| i.write_enabled),
                    o.busy,
                    o.review.is_some(),
                    o.outcome_unknown,
                    o.page.as_ref().map_or(0, |p| p.records.len()),
                    o.record().map_or(0, |r| r.revision),
                    o.receipt.as_ref().map_or(0, |r| r.commit_sequence),
                    o.error.as_deref().unwrap_or("none"),
                    o.handoff
                        .as_ref()
                        .and_then(|h| h.record.as_ref())
                        .map_or(0, |r| r.revision),
                    o.review_tab,
                    o.queue.as_ref().map_or(0, |q| q.records.len()),
                    o.inspect.as_ref().map_or(0, |i| i.decisions.len()),
                    o.delete_arm.is_some(),
                    o.digest_expanded,
                    m.operator_inspected(),
                    o.inspect.as_ref().map_or(0, |i| i.record.id),
                    o.inspect.as_ref().map_or(0, |i| i.record.revision),
                    o.inspect.as_ref().map_or(0, |i| i.peer_demotion_total),
                    o.delete_arm
                        .as_ref()
                        .map_or(0, |a| a.retype.chars().count()),
                    delete_armed(model),
                );
                eprintln!(
                    "ACP_NSH_REVIEW global={} selected_id={} queue_after={} queue_first={} queue_last={} queue_sequence={} queue_baseline={} queue_stale={} inspected_status={} inspected_origin={:?} history={}",
                    o.global,
                    o.selected_id.unwrap_or(0),
                    o.queue_after_id,
                    o.queue
                        .as_ref()
                        .and_then(|q| q.records.first())
                        .map_or(0, |r| r.id),
                    o.queue
                        .as_ref()
                        .and_then(|q| q.records.last())
                        .map_or(0, |r| r.id),
                    o.queue.as_ref().map_or(0, |q| q.commit_sequence),
                    o.queue_watermark,
                    o.queue
                        .as_ref()
                        .is_some_and(|q| q.commit_sequence > o.queue_watermark),
                    o.inspect
                        .as_ref()
                        .map_or("none", |i| i.record.status.as_str()),
                    o.inspect.as_ref().map(|i| i.record.origin),
                    o.inspect.as_ref().map_or(0, |i| i.decisions.len()),
                );
            }
        }
    });
    // Display-none removes layout; visibility also removes native Win32 focus
    // stops. A concealed pane must not leave its retained fields/actions tabbable.
    let content = view()
        .on_layout(move |_: &Bounds<Points>| {
            #[cfg(not(target_arch = "wasm32"))]
            if model.get().operator.open && std::env::var_os("ACP_UI_PROBE").is_some() {
                eprintln!("ACP_NSH_LAYOUT_READY elapsed={:.6}", probe_elapsed());
            }
        })
        .gap_3()
        .flex_shrink_0()
        .child(text(move || model.get().word(Msg::OperatorHelp)))
        .child(text(move || model.get().word(Msg::OperatorKeys)))
        .child(
            view().flex_row().flex_wrap().gap_2().children(
                [
                    (Pane::Profile, Msg::ProfileOperator),
                    (Pane::Preferences, Msg::OperatorPreferences),
                    (Pane::Handoff, Msg::OperatorSavedHandoff),
                ]
                .into_iter()
                .map(move |(pane, key)| {
                    word_action(
                        model,
                        key,
                        move || !model.get().operator.busy,
                        move || {
                            model.update(|m| {
                                m.operator.pane = pane;
                                m.operator.review = None;
                                m.operator.delete_arm = None;
                                m.operator.serial += 1;
                            })
                        },
                    )
                }),
            ),
        )
        .child(word_action(
            model,
            Msg::OperatorRefresh,
            move || available(model),
            move || refresh(model, "profile.info"),
        ))
        .child(dynamic(move || {
            let (_, o) = summary.get();
            let m = model.get_untracked();
            let mut lines = Vec::new();
            if let Some(info) = &o.info {
                lines.push(
                    m.word(if info.selected {
                        Msg::OperatorSelected
                    } else {
                        Msg::OperatorUnselected
                    })
                    .into(),
                );
                lines.push(
                    m.word(if info.write_enabled {
                        Msg::OperatorGrantOn
                    } else {
                        Msg::OperatorGrantOff
                    })
                    .into(),
                );
                if let Some(b) = &info.binding {
                    lines.push(format!(
                        "Profile {} · generation {} · epoch {}",
                        b.profile_id, b.generation, b.owner_epoch
                    ));
                }
            }
            if o.busy {
                lines.push(m.word(Msg::OperatorBusy).into());
            }
            if let Some(r) = &o.receipt {
                // A deleted record leaves identity-only proof, never content.
                if r.affected.iter().any(|a| a.deleted) {
                    lines.push(m.word(Msg::ReviewDeleted).into());
                }
                lines.push(m.word(Msg::ReviewReceiptNote).into());
            }
            if let Some(p) = o.pending.and_then(|r| r.mutation) {
                lines.push(format!(
                    "Operation {} · epoch {}",
                    p.operation_id, p.binding.owner_epoch
                ));
            }
            let mut body = view()
                .gap_2()
                .child(card(model, m.word(Msg::ProfileOperator), lines));
            // Apply feedback: the receipt label swaps into place and the success
            // badge fades in beside the summary.
            if let Some(r) = &o.receipt {
                let swap = ui::motion::label();
                let fade = ui::motion::fade();
                let receipt_label =
                    format!("{} {}", m.word(Msg::OperatorLastReceipt), r.operation_id);
                let commit_label = format!("commit {}", r.commit_sequence);
                body = body.child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            view().bind_motion(&swap).child(
                                text(receipt_label)
                                    .text_size(px(ui::scale::text::CAPTION))
                                    .text_color(move || palette(model).muted),
                            ),
                        )
                        .child(view().bind_motion(&fade).child(
                            ui::badge(model, commit_label).variant(ui::BadgeVariant::Success),
                        )),
                );
            }
            // Error toast: a precise, sanitized refusal slides in from its edge.
            if let Some(e) = &o.error
                && !o.outcome_unknown
            {
                let toast = ui::motion::toast();
                let wsl = m.connection.target == acp_operator_model::connection::Target::Wsl;
                let message = format!("{e}: {}", tr(m.locale, error_msg(e, wsl)));
                body = body.child(
                    view()
                        .bind_motion(&toast)
                        .child(text(message).text_color(move || palette(model).error)),
                );
            }
            body
        }))
        .child(
            view()
                .gap_2()
                .style_when(
                    move || {
                        model.get().operator.pane != Pane::Profile
                            || model.get().operator.review.is_some()
                            || model.get().operator.outcome_unknown
                    },
                    |s| s.hidden().invisible(),
                )
                .child(field(
                    model,
                    Msg::ProfilePath,
                    |o| &o.path,
                    |o, v| o.path = v,
                    32768,
                ))
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(word_action(
                            model,
                            Msg::ProfileCreate,
                            move || {
                                available(model)
                                    && model.get().operator.info.is_some_and(|i| !i.selected)
                            },
                            move || review(model, "create"),
                        ))
                        .child(word_action(
                            model,
                            Msg::ProfileSelect,
                            move || {
                                available(model)
                                    && model.get().operator.info.is_some_and(|i| !i.selected)
                            },
                            move || review(model, "select"),
                        ))
                        .child(word_action(
                            model,
                            Msg::WriteEnable,
                            move || {
                                available(model)
                                    && model
                                        .get()
                                        .operator
                                        .info
                                        .is_some_and(|i| i.selected && !i.write_enabled)
                            },
                            move || review(model, "enable"),
                        ))
                        .child(word_action(
                            model,
                            Msg::WriteDisable,
                            move || {
                                available(model)
                                    && model.get().operator.info.is_some_and(|i| i.write_enabled)
                            },
                            move || review(model, "disable"),
                        )),
                ),
        )
        .child(
            view()
                .gap_2()
                .style_when(
                    move || {
                        model.get().operator.pane != Pane::Preferences
                            || model.get().operator.review.is_some()
                            || model.get().operator.outcome_unknown
                    },
                    |s| s.hidden().invisible(),
                )
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(word_action(
                            model,
                            Msg::OperatorGlobal,
                            move || available(model),
                            move || {
                                model.update(|m| {
                                    m.operator.invalidate();
                                    m.operator.global = true;
                                    m.operator.after_id = 0;
                                })
                            },
                        ))
                        .child(word_action(
                            model,
                            Msg::OperatorProject,
                            move || available(model) && model.get().selected_project.is_some(),
                            move || {
                                model.update(|m| {
                                    m.operator.invalidate();
                                    m.operator.global = false;
                                    m.operator.after_id = 0;
                                })
                            },
                        ))
                        .child(text(move || {
                            let m = model.get();
                            if m.operator.global {
                                m.word(Msg::OperatorGlobal).into()
                            } else {
                                format!(
                                    "{}: {}",
                                    m.word(Msg::OperatorProject),
                                    m.selected_project.as_deref().unwrap_or("—")
                                )
                            }
                        })),
                )
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(word_action(
                            model,
                            Msg::OperatorList,
                            move || {
                                available(model)
                                    && model
                                        .get()
                                        .operator
                                        .info
                                        .as_ref()
                                        .is_some_and(|i| i.selected)
                            },
                            move || refresh(model, "profile.preferences.list"),
                        ))
                        .child(word_action(
                            model,
                            Msg::OperatorFirst,
                            move || available(model) && model.get().operator.after_id > 0,
                            move || {
                                model.update(|m| {
                                    m.operator.after_id = 0;
                                    m.operator.review = None;
                                    m.operator.serial += 1;
                                });
                                refresh(model, "profile.preferences.list");
                            },
                        ))
                        .child(word_action(
                            model,
                            Msg::OperatorNext,
                            move || {
                                available(model)
                                    && model
                                        .get()
                                        .operator
                                        .page
                                        .as_ref()
                                        .is_some_and(|p| p.next_after_id.is_some())
                            },
                            move || {
                                model.update(|m| {
                                    if let Some(id) =
                                        m.operator.page.as_ref().and_then(|p| p.next_after_id)
                                    {
                                        m.operator.after_id = id;
                                    }
                                    m.operator.review = None;
                                });
                                refresh(model, "profile.preferences.list");
                            },
                        )),
                )
                .child(dynamic(move || {
                    let (locale, page) = rows.get();
                    let mut body = view().gap_2().flex_shrink_0();
                    let Some(page) = page else {
                        return body;
                    };
                    if page.records.is_empty() {
                        body = body.child(text(acp_operator_model::i18n::tr(
                            locale,
                            Msg::OperatorEmpty,
                        )));
                    }
                    for r in page.records {
                        let id = r.id;
                        body = body.child(action(
                            model,
                            format!("#{} · {} · {} · r{}", id, r.key, r.status, r.revision),
                            move || available(model),
                            move || {
                                model.update(|m| {
                                    m.operator.selected_id = Some(id);
                                    m.operator.review = None;
                                    m.operator.delete_arm = None;
                                    m.operator.serial += 1;
                                    m.operator.inspect = None;
                                    m.operator.inspect_revision = 0;
                                    m.operator.digest_expanded = false;
                                })
                            },
                        ));
                    }
                    body
                }))
                .child(field(
                    model,
                    Msg::OperatorKey,
                    |o| &o.key,
                    |o, v| o.key = v,
                    120,
                ))
                .child(field(
                    model,
                    Msg::OperatorValue,
                    |o| &o.value,
                    |o, v| o.value = v,
                    4096,
                ))
                .child(field(
                    model,
                    Msg::OperatorSource,
                    |o| &o.source,
                    |o, v| o.source = v,
                    512,
                ))
                .child(word_action(
                    model,
                    Msg::OperatorSuggest,
                    move || {
                        available(model)
                            && model.get().operator.info.is_some_and(|i| i.write_enabled)
                    },
                    move || review(model, "suggest"),
                )),
        )
        .child(
            view()
                .gap_2()
                .style_when(
                    move || {
                        model.get().operator.pane != Pane::Handoff
                            || model.get().operator.review.is_some()
                            || model.get().operator.outcome_unknown
                    },
                    |s| s.hidden().invisible(),
                )
                .child(field(
                    model,
                    Msg::OperatorTask,
                    |o| &o.task,
                    |o, v| {
                        o.task = v;
                        o.handoff = None;
                    },
                    120,
                ))
                .child(word_action(
                    model,
                    Msg::OperatorHandoff,
                    move || available(model) && model.get().selected_project.is_some(),
                    move || refresh(model, "profile.handoff.get"),
                ))
                .child(dynamic(move || {
                    let m = model.get();
                    let mut lines = vec![m.word(Msg::OperatorHistorical).into()];
                    let mut total = 0;
                    if let Some(h) = &m.operator.handoff {
                        let all = literal(&serde_json::to_string(&h.record).unwrap_or_default());
                        total = all.len().div_ceil(24);
                        lines.push(format!("{} / {}", m.operator.handoff_page + 1, total));
                        lines.extend(all.into_iter().skip(m.operator.handoff_page * 24).take(24));
                    }
                    view()
                        .gap_2()
                        .child(card(model, m.word(Msg::Handoff), lines))
                        .child(word_action(
                            model,
                            Msg::PreviousPage,
                            move || model.get().operator.handoff_page > 0,
                            move || {
                                model.update(|m| {
                                    m.operator.handoff_page =
                                        m.operator.handoff_page.saturating_sub(1)
                                })
                            },
                        ))
                        .child(word_action(
                            model,
                            Msg::NextPage,
                            move || model.get().operator.handoff_page + 1 < total,
                            move || model.update(|m| m.operator.handoff_page += 1),
                        ))
                })),
        )
        .child(
            view()
                .on_layout(move |bounds: &Bounds<Points>| review_origin.set(bounds.origin.y))
                .gap_2()
                .style_when(
                    move || {
                        model.get().operator.review.is_some()
                            || model.get().operator.outcome_unknown
                    },
                    |s| s.hidden().invisible(),
                )
                .child(review_workspace(model, delete_open)),
        )
        .child(dynamic(move || {
            let m = model.get();
            let mut body = view()
                .on_layout(move |bounds: &Bounds<Points>| preview_origin.set(bounds.origin.y))
                .gap_2()
                .flex_shrink_0();
            if let Some(review) = &m.operator.review {
                let mut lines = vec![m.word(Msg::OperatorPending).into(), review.method.clone()];
                if let Some(mutation) = &review.mutation {
                    match &mutation.command {
                        Command::ConfirmPreference { .. } => {
                            lines.push(m.word(Msg::OperatorPeerWarning).into());
                        }
                        Command::DeprecatePreference { .. } => {
                            lines.push(m.word(Msg::ReviewDeprecateImpact).into());
                        }
                        Command::DeletePreference { .. } => {
                            lines.push(m.word(Msg::ReviewDeleteBody).into());
                        }
                        _ => {}
                    }
                }
                if let Some(record) = &review.record {
                    lines.extend(review_literal(
                        review,
                        m.operator.digest_expanded,
                        &serde_json::to_string(record).unwrap_or_default(),
                    ));
                }
                if let Some(mutation) = &review.mutation {
                    lines.extend(review_literal(
                        review,
                        m.operator.digest_expanded,
                        &serde_json::to_string(mutation).unwrap_or_default(),
                    ));
                } else {
                    lines.extend(literal(&review.params.to_string()));
                }
                // Prepare feedback: the reviewed operation grows into place.
                let grow = ui::motion::grow();
                body = body
                    .child(view().bind_motion(&grow).child(card(
                        model,
                        m.word(Msg::OperatorReview),
                        lines,
                    )))
                    .child(
                        ui::button(model, "")
                            .label_fn(move || model.get().word(Msg::OperatorCommit).into())
                            .variant(
                                if matches!(
                                    review.mutation.as_ref().map(|m| &m.command),
                                    Some(Command::DeletePreference { .. })
                                ) {
                                    ui::ButtonVariant::Danger
                                } else {
                                    ui::ButtonVariant::Primary
                                },
                            )
                            .loading(move || model.get().operator.busy)
                            .disabled(move || !available(model))
                            .on_press(move || submit(model)),
                    )
                    .child(word_action(
                        model,
                        Msg::OperatorCancel,
                        || true,
                        move || {
                            model.update(|m| {
                                m.operator.review = None;
                                m.operator.delete_arm = None;
                            })
                        },
                    ));
            }
            if m.operator.outcome_unknown {
                body = body
                    .child(text(m.word(Msg::OperatorUnknown)))
                    .child(word_action(
                        model,
                        Msg::OperatorRetry,
                        move || {
                            let m = model.get();
                            !m.operator.busy
                                && m.operator
                                    .pending
                                    .as_ref()
                                    .is_some_and(|p| m.operator_current(p))
                        },
                        move || {
                            let mut request = None;
                            model.update(|m| request = m.operator_retry());
                            if let Some(r) = request {
                                send(model, r);
                            }
                        },
                    ))
                    .child(word_action(
                        model,
                        Msg::OperatorAbandon,
                        move || !model.get().operator.busy,
                        move || {
                            model.update(|m| {
                                m.operator.outcome_unknown = false;
                                m.operator.review = None;
                                m.operator.invalidate();
                                m.operator.error = Some("OutcomeUnknown".into());
                            })
                        },
                    ));
            }
            body.child(text(m.word(Msg::OperatorCliGate)))
        }));
    // The native modal belongs to the visible workspace root, never to the
    // clipped/scrolled document that may place its insertion point off-screen.
    view()
        .flex_1()
        .style_when(
            move || !model.get().operator.open,
            |s| s.hidden().invisible(),
        )
        .child(scroll().element_ref(&scroll_ref).flex_1().child(content))
        .child(delete_dialog(model, delete_open, retype_ref))
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp_operator_model::operator::{
        Binding, PeerDemotion, PreferencePage, PreferenceRecord, ProfileInfo, ReviewInspect,
        ReviewQueue, digest_hex, proposal_digest,
    };
    use nsh_engine::{EngineConfig, Root};
    use std::time::Duration;

    fn binding() -> Binding {
        Binding {
            profile_id: "a".repeat(32),
            generation: "b".repeat(32),
            owner_epoch: 1,
        }
    }
    fn record(id: i64, status: &str, revision: i64) -> PreferenceRecord {
        let key = format!("synthetic-key-{id}");
        PreferenceRecord {
            id,
            scope: Scope::Global,
            key: key.clone(),
            value: "synthetic-value".into(),
            source: "synthetic-source".into(),
            status: status.into(),
            revision,
            origin: Origin::AgentReported,
            proposal_digest: digest_hex(&proposal_digest(
                &Scope::Global,
                None,
                &key,
                "synthetic-value",
                Origin::AgentReported,
                "synthetic-source",
            )),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }
    fn observe_inspect(m: &mut Model) {
        let inspect = m.operator.inspect.clone().unwrap();
        let request = m.operator_read("profile.review.inspect").unwrap();
        assert!(m.complete_operator(&request, Ok(serde_json::to_value(inspect).unwrap()),));
    }
    /// A local, write-granted operator with one suggested queue row and its exact
    /// inspect snapshot. No daemon: these tests exercise rendering and the
    /// prepare/refuse guards, never a real submit.
    fn queue_model() -> Model {
        let mut m = Model::loading();
        m.connection.target = Target::Local;
        m.connection.runtime = std::env::temp_dir().to_str().unwrap().into();
        m.connection.endpoint = "synthetic".into();
        m.operator.open = true;
        m.operator.global = true;
        m.operator.info = Some(ProfileInfo {
            selected: true,
            binding: Some(binding()),
            write_enabled: true,
        });
        m.operator.queue = Some(ReviewQueue {
            binding: binding(),
            commit_sequence: 5,
            records: vec![record(7, "suggested", 1)],
            next_after_id: None,
        });
        m.operator.queue_watermark = 5;
        m.operator.selected_id = Some(7);
        m.operator.inspect = Some(ReviewInspect {
            binding: binding(),
            commit_sequence: 6,
            record: record(7, "suggested", 1),
            decisions: vec![],
            peer_demotions: vec![],
            peer_demotion_total: 0,
        });
        m.operator.inspect_revision = 1;
        observe_inspect(&mut m);
        m
    }
    fn peer_model() -> Model {
        let mut m = queue_model();
        m.operator.review_tab = ReviewTab::Detail;
        let inspect = m.operator.inspect.as_mut().unwrap();
        inspect.peer_demotions = vec![
            PeerDemotion {
                id: 100,
                revision: 1,
            },
            PeerDemotion {
                id: 101,
                revision: 1,
            },
        ];
        inspect.peer_demotion_total = 20;
        m
    }
    fn confirmed_model() -> Model {
        let mut m = queue_model();
        m.operator.review_tab = ReviewTab::Detail;
        m.operator.queue = None;
        m.operator.inspect = Some(ReviewInspect {
            binding: binding(),
            commit_sequence: 6,
            record: record(7, "confirmed", 2),
            decisions: vec![],
            peer_demotions: vec![],
            peer_demotion_total: 0,
        });
        m.operator.inspect_revision = 2;
        observe_inspect(&mut m);
        m
    }
    fn empty_queue_model() -> Model {
        let mut m = queue_model();
        m.operator.queue = Some(ReviewQueue {
            binding: binding(),
            commit_sequence: 5,
            records: vec![],
            next_after_id: None,
        });
        m.operator.inspect = None;
        m.operator.selected_id = None;
        m
    }
    fn stale_arm_model() -> Model {
        let mut m = queue_model();
        m.operator.delete_arm = Some(DeleteArm {
            retype: "7".into(),
            id: 7,
            revision: 2,
            proposal_digest: record(7, "suggested", 1).proposal_digest,
        });
        m
    }

    #[component]
    fn QueueConsole() -> impl IntoElement {
        crate::console(signal(queue_model()))
    }
    #[component]
    fn SelectedListConsole() -> impl IntoElement {
        let mut m = queue_model();
        m.operator.pane = Pane::Preferences;
        m.operator.review_tab = ReviewTab::Detail;
        m.operator.page = Some(PreferencePage {
            binding: binding(),
            records: vec![record(7, "suggested", 1)],
            next_after_id: None,
        });
        crate::console(signal(m))
    }
    #[component]
    fn PeerConsole() -> impl IntoElement {
        crate::console(signal(peer_model()))
    }
    #[component]
    fn ConfirmedConsole() -> impl IntoElement {
        crate::console(signal(confirmed_model()))
    }
    #[component]
    fn EmptyQueueConsole() -> impl IntoElement {
        crate::console(signal(empty_queue_model()))
    }
    #[component]
    fn UnobservedQueueConsole() -> impl IntoElement {
        let mut m = empty_queue_model();
        m.operator.queue = None;
        crate::console(signal(m))
    }
    #[component]
    fn DriftedEmptyQueueConsole() -> impl IntoElement {
        let mut m = empty_queue_model();
        m.operator.queue.as_mut().unwrap().commit_sequence = 8;
        crate::console(signal(m))
    }
    #[component]
    fn SwitchedHistoryConsole() -> impl IntoElement {
        let mut m = queue_model();
        m.operator.selected_id = Some(8);
        m.operator.review_tab = ReviewTab::History;
        crate::console(signal(m))
    }
    #[component]
    fn LiteralSourceConsole() -> impl IntoElement {
        let mut m = peer_model();
        m.operator.inspect.as_mut().unwrap().record.source =
            format!("{}\n\u{202e}source-tail", "s".repeat(490));
        crate::console(signal(m))
    }
    #[component]
    fn StaleDeleteDialog() -> impl IntoElement {
        let model = signal(stale_arm_model());
        let delete_open = signal(true);
        view()
            .child(text("delete-tier-fixture"))
            .child(delete_dialog(model, delete_open, element_ref()))
    }
    #[component]
    fn HiddenDeleteDialog() -> impl IntoElement {
        let model = signal(stale_arm_model());
        let delete_open = signal(false);
        view()
            .child(text("delete-tier-fixture"))
            .child(delete_dialog(model, delete_open, element_ref()))
    }
    fn start<C: nsh_core::element::Component>() -> nsh_testing::Headless
    where
        C::Props: Default,
    {
        let mut cfg = EngineConfig::new(Root::of::<C>());
        cfg.viewport = Size::new(px(1080.0), px(760.0));
        nsh_testing::Headless::start(cfg).unwrap()
    }

    #[test]
    fn review_tabs_navigate_by_chord_and_click() {
        let mut app = start::<QueueConsole>();
        app.wait_for("#7 · synthetic-key-7 · r1").unwrap();
        // Alt+right walks Queue -> Detail -> History while the operator is open.
        app.press_key("alt+ArrowRight").unwrap();
        app.wait_for("Show full digest").unwrap();
        app.press_key("alt+ArrowRight").unwrap();
        app.wait_for("No decisions recorded for this record")
            .unwrap();
        app.press_key("alt+ArrowLeft").unwrap();
        app.wait_for("Show full digest").unwrap();
        // The tab control itself moves the selection by click.
        app.press("History").unwrap();
        app.wait_for("No decisions recorded for this record")
            .unwrap();
        app.press("Detail").unwrap();
        app.wait_for("Show full digest").unwrap();
    }

    #[test]
    fn detail_digest_prefix_expands_and_peer_disclosure_names_total() {
        let mut app = start::<PeerConsole>();
        app.wait_for("Show full digest").unwrap();
        let full = record(7, "suggested", 1).proposal_digest;
        // Only the 16-hex prefix is shown until an explicit expand.
        assert!(!app.screen().texts().iter().any(|t| t.contains(&full)));
        assert!(app.screen().texts().iter().any(|t| t.contains(&full[..16])));
        app.press("Show full digest").unwrap();
        app.wait_until("full digest", Duration::from_secs(5), |s| {
            s.texts().iter().any(|t| t.contains(&full))
        })
        .unwrap();
        // A truncated peer preview still discloses the authoritative total.
        app.wait_for("Deprecates 20 same-key confirmed records (ids 100, 101, …)")
            .unwrap();
    }

    #[test]
    fn deprecate_confirmed_discloses_impact_and_prepares() {
        let mut app = start::<ConfirmedConsole>();
        app.wait_for("Deprecation makes the record inactive in agent effective context.")
            .unwrap();
        app.press("Deprecate preference…").unwrap();
        app.wait_for("Prepared operation; no write sent yet")
            .unwrap();
        // The prepared destructive-tier card repeats the impact disclosure.
        assert!(
            app.screen()
                .shows("Deprecation makes the record inactive in agent effective context.")
        );
    }

    #[test]
    fn delete_tier_refuses_blind_enter_and_wrong_retype_then_arms_exact() {
        let mut app = start::<QueueConsole>();
        app.wait_for("#7 · synthetic-key-7 · r1").unwrap();
        app.press_key("alt+shift+q").unwrap();
        app.wait_for("Delete global preference #7?").unwrap();
        app.wait_for("Type the ID exactly to arm deletion").unwrap();
        // A blind Enter on the focused, empty retype field prepares nothing.
        app.type_text("Retype the exact ID", "").unwrap();
        app.press_key("Enter").unwrap();
        assert!(!app.screen().shows("Prepared operation; no write sent yet"));
        // The exact id arms the tier; Enter prepares the delete (no submit here).
        app.type_text("Retype the exact ID", "7").unwrap();
        app.wait_for("Deletion armed; apply to commit").unwrap();
        app.press_key("Enter").unwrap();
        app.wait_for("Prepared operation; no write sent yet")
            .unwrap();
        assert!(app.screen().shows(
            "This permanently removes the record's content and its decision history. Receipts remain as identity-only proof; the database file is not securely erased."
        ));
        // Cancelling the prepared delete disarms; a fresh tier refuses a wrong id.
        app.press("Cancel review").unwrap();
        app.wait_for("#7 · synthetic-key-7 · r1").unwrap();
        app.press_key("alt+shift+q").unwrap();
        app.wait_for("Delete global preference #7?").unwrap();
        app.type_text("Retype the exact ID", "8").unwrap();
        app.wait_for("The typed ID does not match").unwrap();
        app.press_key("Enter").unwrap();
        assert!(!app.screen().shows("Prepared operation; no write sent yet"));
    }

    #[test]
    fn delete_dialog_stale_arm_never_arms_even_with_exact_text() {
        let mut app = start::<StaleDeleteDialog>();
        app.wait_for("Delete global preference #7?").unwrap();
        // The armed revision no longer matches the reviewed record: the exact
        // retype text still cannot arm the commitment.
        app.type_text("Retype the exact ID", "").unwrap();
        app.press_key("Enter").unwrap();
        assert!(!app.screen().shows("Deletion armed; apply to commit"));
        assert!(!app.screen().shows("Prepared operation; no write sent yet"));
    }

    #[test]
    fn delete_dialog_hidden_when_not_open() {
        let mut app = start::<HiddenDeleteDialog>();
        app.wait_for("delete-tier-fixture").unwrap();
        assert!(!app.screen().shows("Delete global preference #7?"));
        assert!(!app.screen().shows("Type the ID exactly to arm deletion"));
    }

    #[test]
    fn empty_queue_is_a_valid_state_never_unavailable() {
        let mut app = start::<EmptyQueueConsole>();
        app.wait_for("No pending suggestions").unwrap();
        assert!(!app.screen().shows("Unavailable"));
        app.press_key("mod+l").unwrap();
        app.wait_for("没有待审建议").unwrap();
    }

    #[test]
    fn review_copy_ships_in_english_and_chinese() {
        let mut app = start::<QueueConsole>();
        app.wait_for("Queue").unwrap();
        app.press_key("mod+l").unwrap();
        app.wait_for("待审队列").unwrap();
        app.press_key("alt+ArrowRight").unwrap();
        app.wait_for("显示完整摘要").unwrap();
        app.press_key("alt+ArrowRight").unwrap();
        app.wait_for("该记录尚无决定历史").unwrap();
    }

    #[test]
    fn compact_and_large_viewports_render_the_review_workspace() {
        let mut app = start::<QueueConsole>();
        app.wait_for("#7 · synthetic-key-7 · r1").unwrap();
        app.set_viewport(Size::new(px(640.0), px(480.0))).unwrap();
        app.wait_for("#7 · synthetic-key-7 · r1").unwrap();
        app.press_key("alt+ArrowRight").unwrap();
        app.wait_for("Show full digest").unwrap();
        app.set_viewport(Size::new(px(1080.0), px(760.0))).unwrap();
        app.wait_for("Show full digest").unwrap();
    }

    #[test]
    fn delete_dialog_escape_dismisses_and_disarms() {
        let mut app = start::<QueueConsole>();
        app.wait_for("#7 · synthetic-key-7 · r1").unwrap();
        app.press_key("alt+shift+q").unwrap();
        app.wait_for("Delete global preference #7?").unwrap();
        app.type_text("Retype the exact ID", "7").unwrap();
        app.press_key("Escape").unwrap();
        app.wait_for_gone("Delete global preference #7?", Duration::from_secs(5))
            .unwrap();
        assert!(!app.screen().shows("Prepared operation; no write sent yet"));
        app.press_key("alt+shift+q").unwrap();
        app.wait_for("Delete global preference #7?").unwrap();
        assert_eq!(
            app.screen().input_text("Retype the exact ID").unwrap(),
            Some("")
        );
        app.press_key("Enter").unwrap();
        assert!(!app.screen().shows("Prepared operation; no write sent yet"));
    }

    #[test]
    fn unloaded_queue_does_not_claim_no_pending_suggestions() {
        let mut app = start::<UnobservedQueueConsole>();
        app.wait_for("Inspect the queue to load pending suggestions")
            .unwrap();
        assert!(!app.screen().shows("No pending suggestions"));
    }

    #[test]
    fn empty_later_queue_page_still_discloses_watermark_and_drift() {
        let mut app = start::<DriftedEmptyQueueConsole>();
        app.wait_for("No pending suggestions").unwrap();
        app.wait_for("Queue watermark: commit 8").unwrap();
        app.wait_for("Queue changed since page loaded · +3")
            .unwrap();
        app.wait_for("Pages are separate reads, not a shared snapshot.")
            .unwrap();
        app.press_key("mod+l").unwrap();
        app.wait_for("队列水位：提交 8").unwrap();
        app.wait_for("页面加载后队列已变化 · +3").unwrap();
    }

    #[test]
    fn history_never_renders_the_previous_selection() {
        let mut app = start::<SwitchedHistoryConsole>();
        app.wait_for("Select a queue row to inspect").unwrap();
        assert!(!app.screen().shows("No decisions recorded for this record"));
    }

    #[test]
    fn exact_detail_preserves_source_characters_and_explains_authority() {
        let mut app = start::<LiteralSourceConsole>();
        app.wait_for("Show full digest").unwrap();
        app.wait_until("escaped source tail", Duration::from_secs(5), |s| {
            s.texts()
                .iter()
                .any(|t| t.contains("\\n\\u{202e}source-tail"))
        })
        .unwrap();
        app.wait_for("Source and origin are unverified claims; the digest proves byte equality, not authorship.").unwrap();
        app.wait_for("Repository ProjectContracts outrank personal preferences. Confirming a preference does not approve a ProjectContract.").unwrap();
        app.press_key("mod+l").unwrap();
        app.wait_for("来源类别: agent_reported").unwrap();
        app.wait_for("修订: 1").unwrap();
    }

    #[test]
    fn focus_shortcut_disarms_delete_before_apply() {
        let mut app = start::<QueueConsole>();
        app.wait_for("#7 · synthetic-key-7 · r1").unwrap();
        app.press_key("alt+shift+q").unwrap();
        app.wait_for("Delete global preference #7?").unwrap();
        app.type_text("Retype the exact ID", "7").unwrap();
        app.wait_for("Deletion armed; apply to commit").unwrap();
        app.press_key("alt+k").unwrap();
        app.wait_for_gone("Delete global preference #7?", Duration::from_secs(5))
            .unwrap();
        app.press_key("alt+shift+v").unwrap();
        assert!(!app.screen().shows("Prepared operation; no write sent yet"));
        app.press_key("alt+shift+q").unwrap();
        assert!(!app.screen().shows("Prepared operation; no write sent yet"));
        assert!(!app.screen().shows("Deletion armed; apply to commit"));
    }

    #[test]
    fn precise_review_errors_map_without_raw_diagnostics() {
        for (code, expected) in [
            ("ReviewMismatch", Msg::ReviewMismatch),
            ("Conflict", Msg::ReviewErrorConflict),
            ("InvalidTransition", Msg::ReviewErrorInvalidTransition),
            ("Unauthorized", Msg::ReviewErrorUnauthorized),
            ("Busy", Msg::ReviewErrorBusy),
            ("Capacity", Msg::ReviewErrorCapacity),
            ("Unavailable", Msg::ReviewErrorUnavailable),
            ("OutcomeUnknown", Msg::OperatorUnknown),
            ("unknown", Msg::ReviewErrorGeneric),
        ] {
            assert_eq!(error_msg(code, false), expected);
        }
    }

    #[test]
    fn preparing_a_decision_does_not_implicitly_expand_its_digest() {
        let mut app = start::<QueueConsole>();
        app.wait_for("#7 · synthetic-key-7 · r1").unwrap();
        let full = record(7, "suggested", 1).proposal_digest;
        app.press_key("alt+shift+y").unwrap();
        app.wait_for("Prepared operation; no write sent yet")
            .unwrap();
        assert!(!app.screen().texts().join("").contains(&full));
        app.press("Cancel review").unwrap();
        app.press("Detail").unwrap();
        app.press("Show full digest").unwrap();
        app.press_key("alt+shift+y").unwrap();
        app.wait_for("Prepared operation; no write sent yet")
            .unwrap();
        assert!(app.screen().texts().join("").contains(&full));
    }

    #[test]
    fn selected_list_uses_one_exact_inspector_and_one_set_of_decisions() {
        let mut app = start::<SelectedListConsole>();
        app.set_viewport(Size::new(px(640.0), px(480.0))).unwrap();
        app.wait_for("#7 · synthetic-key-7 · suggested · r1")
            .unwrap();
        app.wait_for("Show full digest").unwrap();
        assert!(!app.screen().shows("Review exact operation"));
        assert_eq!(
            app.screen()
                .count("role:button[Confirm preference…]")
                .unwrap(),
            1
        );
        assert_eq!(
            app.screen()
                .count("role:button[Reject preference…]")
                .unwrap(),
            1
        );
        app.press("Confirm preference…").unwrap();
        app.wait_for("Prepared operation; no write sent yet")
            .unwrap();
    }

    #[test]
    fn compact_delete_tier_keeps_retype_validation_and_explicit_apply() {
        let mut app = start::<SelectedListConsole>();
        app.set_viewport(Size::new(px(640.0), px(480.0))).unwrap();
        app.press_key("alt+shift+q").unwrap();
        app.wait_for("Delete global preference #7?").unwrap();
        app.type_text("Retype the exact ID", "8").unwrap();
        app.wait_for("The typed ID does not match").unwrap();
        app.press_key("Enter").unwrap();
        assert!(!app.screen().shows("Prepared operation; no write sent yet"));
        app.press_key("alt+shift+v").unwrap();
        assert!(!app.screen().shows("Prepared operation; no write sent yet"));
        app.press_key("Escape").unwrap();
        app.wait_for_gone("Delete global preference #7?", Duration::from_secs(5))
            .unwrap();
    }
}
