//! Minimal native operator panel; all business transitions live in shared::Model.
//! The Queue / Detail / History review workspace, the destructive delete tier and
//! their motion are rendered here from the one shared Model.
use crate::*;
use precept_ui_core::{
    connection::Target,
    i18n::{Locale, Msg, tr},
    operator::{
        Channel, Command, DecisionAction, DeleteArm, Origin, Pane, PreferenceRecord, Review,
        ReviewMode, ReviewTab, Scope,
    },
    presentation::sanitize,
};
use shell::{card, word_action};
use tokens::{mix, palette};

pub(crate) fn open(model: Signal<Model>) {
    open_mode(model, ReviewMode::Inbox);
}
/// Opens the unified Review & rules workspace on one of its three tabs:
/// To review (pending suggestions), Personal (global records) or Project
/// (records for the currently selected workspace).
pub(crate) fn open_mode(model: Signal<Model>, mode: ReviewMode) {
    model.update(|m| {
        let global = match mode {
            // The inbox keeps its scope, but a project scope needs a project.
            ReviewMode::Inbox => m.operator.global || m.selected_project.is_none(),
            ReviewMode::Personal => true,
            ReviewMode::Project => false,
        };
        if global != m.operator.global
            || mode != m.operator.mode
            || (mode != ReviewMode::Inbox && m.operator.pane != Pane::Preferences)
        {
            m.operator.invalidate();
            m.operator.global = global;
            m.operator.after_id = 0;
        }
        if mode != ReviewMode::Inbox {
            m.operator.pane = Pane::Preferences;
        }
        // Entering the workspace re-reads the Profile (writes may have changed
        // in Runtime checks while it was closed). Cleared here, before that read
        // is built: clearing it later would make the read's reply stale.
        if !m.operator.open {
            m.operator.info = None;
        }
        if m.operator.mode != mode {
            m.operator.batch.clear();
        }
        m.operator.mode = mode;
        m.operator.open = true;
        m.settings = false;
        m.setup_open = false;
        m.agents_page = false;
        m.sessions_page = false;
        m.memory_page = false;
        m.operator.review = None;
        m.operator.delete_arm = None;
        // The pending queue loads with the page, as the record lists do; the
        // page's Refresh reads it again.
        if mode == ReviewMode::Inbox && m.operator.queue.is_none() {
            m.operator.follow_up = Some("profile.review.queue");
        }
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
                    precept_ui_client::native::Inputs::from_connection(&connection)?
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
            let mut next = None;
            let mut update_of = None;
            model.update(|m| {
                m.complete_operator(&request, result);
                next = m.operator.follow_up.take();
                // An inspected suggestion that would replace a confirmed
                // rule: read that rule to show the change.
                if request.method == "profile.review.inspect"
                    && let Some(i) = &m.operator.inspect
                    && i.record.status == "suggested"
                    && let Some(peer) = i.peer_demotions.first()
                    && m.operator.update_of.as_ref().map(|o| o.id) != Some(peer.id)
                {
                    update_of = Some((i.record.scope.clone(), peer.id));
                }
            });
            if let Some((scope, id)) = update_of {
                fetch_update_of(model, scope, id);
            }
            // Save, then confirm: continue with the read the model asked for.
            // A read already in flight defers it to that read's completion.
            if let Some(method) = next {
                let mut request = None;
                model.update(|m| {
                    request = m.operator_read(method);
                    if request.is_none() {
                        m.operator.follow_up = Some(method);
                    }
                });
                if let Some(request) = request {
                    send(model, request);
                }
            }
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
    if matches!(
        action,
        "confirm" | "reject" | "deprecate" | "delete" | "switch_on" | "switch_off" | "revise"
    ) && !ensure_inspected(model)
    {
        return;
    }
    #[cfg(feature = "ipc")]
    let id = precept_ui_client::operator::operation_id();
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
    if std::env::var_os("PRECEPT_UI_PROBE").is_some() {
        eprintln!(
            "PRECEPT_UI_DIALOG requested_open={} armed={}",
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
        "InvalidRule" => Msg::RuleInvalid,
        "InvalidNote" => Msg::NoteInvalid,
        _ => Msg::ReviewErrorGeneric,
    }
}
fn scope_label(locale: Locale, scope: &Scope) -> String {
    match scope {
        Scope::Global => tr(locale, Msg::ReviewScopeGlobal).into(),
        Scope::Project { name } => tr(locale, Msg::ReviewScopeProject).replace("{name}", name),
    }
}
/// Who a record comes from, in the operator's words.
fn origin_label(locale: Locale, origin: Origin) -> &'static str {
    tr(
        locale,
        match origin {
            Origin::Operator => Msg::OriginOperatorShort,
            Origin::Agent => Msg::OriginAgentShort,
        },
    )
}
/// A record's origin as a small badge: who, with its glyph.
fn decision_word(locale: Locale, action: DecisionAction) -> &'static str {
    match action {
        DecisionAction::Confirm => tr(locale, Msg::DecisionConfirm),
        DecisionAction::Reject => tr(locale, Msg::DecisionReject),
        DecisionAction::Deprecate => tr(locale, Msg::DecisionDeprecate),
        DecisionAction::Demote => tr(locale, Msg::DecisionDemote),
        DecisionAction::Delete => tr(locale, Msg::DecisionDelete),
        DecisionAction::Enable => tr(locale, Msg::DecisionEnable),
        DecisionAction::Disable => tr(locale, Msg::DecisionDisable),
        DecisionAction::Replace => tr(locale, Msg::DecisionReplace),
    }
}
fn channel_word(locale: Locale, channel: Channel) -> &'static str {
    match channel {
        Channel::Cli => tr(locale, Msg::ChannelCli),
        Channel::Desktop => tr(locale, Msg::ChannelDesktop),
    }
}
/// Binds `shortcut` to focus `reference` in `pane` of the open workspace, and
/// lets the field go when the workspace closes.
fn focus_shortcut(model: Signal<Model>, shortcut: &'static str, pane: Pane, reference: ElementRef) {
    let target = reference;
    crate::shortcuts::bind(
        model,
        shortcut,
        KeyOptions {
            when: nsh_core::keymap::When::Always,
            ..Default::default()
        },
        move || {
            if model.get().operator.open {
                model.update(|m| {
                    m.operator.pane = pane;
                    m.operator.review = None;
                    m.operator.delete_arm = None;
                });
                // Native Windows cannot focus a field before its pane is shown.
                let target = target;
                nsh_core::element::request_animation_frame(move |_| {
                    if model.with(|m| {
                        m.operator.open
                            && m.operator.pane == pane
                            && !m.operator.busy
                            && !m.operator.outcome_unknown
                    }) {
                        target.focus();
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
}
fn field(
    model: Signal<Model>,
    key: Msg,
    get: fn(&precept_ui_core::operator::Operator) -> &str,
    set: fn(&mut precept_ui_core::operator::Operator, String),
    max: usize,
) -> impl IntoElement {
    let reference = element_ref();
    let shortcut = match key {
        Msg::OperatorKey => "review.field.key",
        Msg::OperatorValue => "review.field.value",
        _ => "review.field.task",
    };
    let pane = match key {
        Msg::OperatorTask => Pane::Handoff,
        _ => Pane::Preferences,
    };
    focus_shortcut(model, shortcut, pane, reference);
    ui::field(
        model,
        move || model.get().word(key).into(),
        ui::input(model, "")
            .label_fn(move || model.get().word(key).into())
            .element_ref(&reference)
            .value(move || get(&model.get().operator).to_owned())
            .on_focus(move || {
                if std::env::var_os("PRECEPT_UI_PROBE").is_some() {
                    eprintln!("PRECEPT_UI_FOCUS shortcut={shortcut}");
                }
            })
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
/// Whether `s` holds characters that could disguise what it says (control
/// characters other than line breaks and tabs, bidirectional overrides): it
/// is then shown escaped, never rendered.
fn disguising(s: &str) -> bool {
    s.chars().any(|c| {
        (c.is_control() && c != '\n' && c != '\t')
            || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
    })
}
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
    let Some(queue) = &m.operator.queue else {
        return body
            .child(ui::empty(model, m.word(Msg::ReviewQueueUnobserved)))
            .into_element();
    };
    // Even an empty later page can disclose drift; paging is not a snapshot.
    if queue.commit_sequence > m.operator.queue_watermark {
        let delta = queue.commit_sequence - m.operator.queue_watermark;
        body = body.child(
            text(
                m.word(Msg::ReviewQueueChanged)
                    .replace("{n}", &delta.to_string()),
            )
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
    let records = std::rc::Rc::new(queue.records.clone());
    let selected_id = m.operator.selected_id;
    let more = queue.next_after_id.is_some();
    let list = loaded_list(
        model,
        records.iter().map(|r| r.id).collect(),
        {
            let records = records.clone();
            move |index| queue_row(model, delete_open, &records[index], index, selected_id)
        },
        more,
        move || {
            disarm(model, delete_open);
            let mut go = false;
            model.update(|m| {
                if let Some(id) = m.operator.queue.as_ref().and_then(|q| q.next_after_id) {
                    m.operator.queue_after_id = id;
                    go = true;
                }
            });
            if go {
                refresh(model, "profile.review.queue");
            }
        },
    );
    body.child(list).into_element()
}
/// One pending suggestion: pressing its title inspects the exact record.
fn queue_row(
    model: Signal<Model>,
    delete_open: Signal<bool>,
    r: &PreferenceRecord,
    index: usize,
    selected_id: Option<i64>,
) -> AnyElement {
    let m = model.get();
    let id = r.id;
    let selected = selected_id == Some(id);
    let label = format!("#{} · {} · r{}", id, sanitize(&r.key, 120), r.revision);
    let mut trailing = view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(ui::badge(model, status_word(&m, &r.status)).variant(ui::BadgeVariant::Warning));
    if selected {
        trailing =
            trailing.child(ui::badge(model, m.word(Msg::Selected)).variant(ui::BadgeVariant::Info));
    }
    // A batch arrives as a wave: a capped index stagger, instant when reduced.
    let arrive = ui::motion::arrive(((index % 12) as f32) * 50.0);
    view()
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
        .child(record_line(
            model,
            r,
            label,
            &format!("#{id} · r{}", r.revision),
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
            trailing.into_element(),
        ))
        .bind_motion(&arrive)
        .into_element()
}
/// A record's row: its checkbox, its origin as an icon in a column of its
/// own, then the title (the key, then number and revision; it wraps, and its
/// accessible name is `label`), the value's first line and the tags beneath,
/// with `trailing` (status) at the right.
fn record_line(
    model: Signal<Model>,
    r: &PreferenceRecord,
    label: String,
    detail: &str,
    on_open: impl Fn() + 'static,
    trailing: AnyElement,
) -> AnyElement {
    let m = model.get_untracked();
    let origin = origin_label(m.locale, r.origin);
    let glyph = match r.origin {
        Origin::Operator => icons::PERSON,
        Origin::Agent => icons::AGENT,
    };
    let preview = precept_ui_core::presentation::preview_line(&r.value, 140);
    let title = view()
        .role(AccessibilityRole::Button)
        .label(label)
        .on_press(move || {
            if available(model) {
                on_open()
            }
        })
        .px_1()
        .rounded(px(ui::scale::radius::SMALL))
        .transition_colors()
        .duration(ui::motion::ms(ui::motion::FAST))
        .hover(move |s| s.bg(mix(palette(model).panel, palette(model).text, 0.06)))
        .focus(move |s| {
            s.ring(px(2.0))
                .ring_color(palette(model).primary.opacity(0.55))
        })
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_x_2()
                .child(
                    text(sanitize(&r.key, 120))
                        .font_weight(FontWeight::SEMIBOLD)
                        .min_w_0(),
                )
                .child(
                    text(detail.to_owned())
                        .text_size(px(ui::scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                ),
        );
    view()
        .flex_row()
        .items_start()
        .gap_3()
        .child(view().pt_0p5().child(pick(model, r)))
        .child(
            view()
                .w(px(ui::scale::icon::SM))
                .pt_1()
                .flex_shrink_0()
                .label(origin)
                .text_color(move || palette(model).muted)
                .child(ui::icon::glyph(glyph, ui::scale::icon::SM)),
        )
        .child(
            view()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(title)
                .child(
                    text(sanitize(&preview, 140))
                        .px_1()
                        .text_size(px(ui::scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                )
                .child(view().px_1().child(record_meta(model, r))),
        )
        .child(view().pt_0p5().flex_shrink_0().child(trailing))
        .into_element()
}
/// The batch checkbox of a suggested row (nothing for other statuses).
fn pick(model: Signal<Model>, r: &PreferenceRecord) -> AnyElement {
    if r.status != "suggested" {
        // A checkbox's room, so every row's label starts at one edge.
        return view()
            .w(px(ui::CHECKBOX_SIZE))
            .flex_shrink_0()
            .into_element();
    }
    let id = r.id;
    ui::checkbox(
        model,
        move || {
            model
                .get()
                .word(Msg::BatchPick)
                .replace("{id}", &id.to_string())
                .into()
        },
        move || model.get().operator.batch.binary_search(&id).is_ok(),
        move || !write_grant(model) || model.get().operator.busy,
        move |_| model.update(|m| m.operator.toggle_batch(id)),
    )
    .into_element()
}
/// What to do with the picked suggestions: a note, then confirm or reject
/// them all in one write. Shown only while something is picked.
fn batch_bar(model: Signal<Model>) -> impl IntoElement {
    let count = move || model.get().operator.batch.len();
    let words = move |key: Msg| {
        move || -> SharedString {
            model
                .get()
                .word(key)
                .replace("{n}", &count().to_string())
                .into()
        }
    };
    let ready = move || available(model) && write_grant(model) && !model.get().operator.busy;
    view()
        .style_when(move || count() == 0, |s| s.hidden())
        .gap_2()
        .p_3()
        .rounded(px(ui::scale::radius::CONTAINER))
        .border_1()
        .border_color(move || palette(model).primary)
        .bg(move || mix(palette(model).panel, palette(model).primary, 0.06))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    text(words(Msg::BatchPicked))
                        .font_weight(FontWeight::MEDIUM)
                        .flex_1(),
                )
                .child(
                    ui::button(model, "")
                        .label_fn(words(Msg::BatchConfirm))
                        .size(ui::ButtonSize::Small)
                        .variant(ui::ButtonVariant::Primary)
                        .disabled(move || !ready())
                        .on_press(move || review(model, "batch_confirm")),
                )
                .child(
                    ui::button(model, "")
                        .label_fn(words(Msg::BatchReject))
                        .size(ui::ButtonSize::Small)
                        .disabled(move || !ready())
                        .on_press(move || review(model, "batch_reject")),
                )
                .child(
                    ui::button(model, "")
                        .label_fn(move || model.get().word(Msg::BatchClear).into())
                        .size(ui::ButtonSize::Small)
                        .variant(ui::ButtonVariant::Ghost)
                        .on_press(move || model.update(|m| m.operator.batch.clear())),
                ),
        )
        .child(
            ui::input(model, "")
                .label_fn(move || model.get().word(Msg::DecisionNote).into())
                .value(move || model.get().operator.note.clone())
                .disabled(move || model.get().operator.busy || model.get().operator.outcome_unknown)
                .on_change(move |v: &str| {
                    model.update(|m| {
                        m.operator.review = None;
                        m.operator.note = v.chars().take(600).collect();
                    })
                }),
        )
        .child(
            text(move || model.get().word(Msg::BatchHelp))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
}
/// A record's tags and origin on one wrapping line.
fn record_meta(model: Signal<Model>, r: &PreferenceRecord) -> AnyElement {
    let m = model.get();
    let mut row = view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(ui::tag_list(model, &r.tags));
    if r.status == "confirmed" && !r.enabled {
        row = row.child(ui::badge(model, m.word(Msg::RuleOff)).variant(ui::BadgeVariant::Warning));
    }
    if r.rule.enforcement != precept_ui_core::operator::Enforcement::Advisory {
        row = row.child(
            ui::badge(model, enforcement_word(&m, r.rule.enforcement))
                .variant(ui::BadgeVariant::Info),
        );
    }
    if r.rule.priority != 0 {
        row = row.child(
            text(
                m.word(Msg::PriorityShort)
                    .replace("{n}", &r.rule.priority.to_string()),
            )
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted),
        );
    }
    if let Some(what) = precept_ui_core::presentation::applies_words(&m, &r.rule.applies_to) {
        row = row.child(
            text(what)
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        );
    }
    row.into_element()
}
fn enforcement_word(m: &Model, e: precept_ui_core::operator::Enforcement) -> String {
    m.word(match e {
        precept_ui_core::operator::Enforcement::Advisory => Msg::EnforcementAdvisory,
        precept_ui_core::operator::Enforcement::Verify => Msg::EnforcementVerify,
        precept_ui_core::operator::Enforcement::Hard => Msg::EnforcementHard,
    })
    .into()
}
/// Rows loaded in batches: a virtualized list in the page's scroll that asks
/// for the next batch as it nears the end, and a footer that says whether more
/// exist (with the same load as a button, for keyboard and terminal).
fn loaded_list(
    model: Signal<Model>,
    ids: Vec<i64>,
    row: impl Fn(usize) -> AnyElement + 'static,
    more: bool,
    load: impl Fn() + 'static,
) -> AnyElement {
    let load = std::rc::Rc::new(load);
    let count = ids.len();
    let footer = if more {
        let load = load.clone();
        ui::button(model, "")
            .label_fn(move || model.get().word(Msg::LoadMore).into())
            .size(ui::ButtonSize::Small)
            .variant(ui::ButtonVariant::Ghost)
            .icon(icons::REFRESH)
            .disabled(move || !available(model) || model.get().operator.busy)
            .loading(move || model.get().operator.busy)
            .on_press(move || load())
            .into_element()
    } else {
        text(model.get().word(Msg::AllLoaded))
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted)
            .into_element()
    };
    let body = view().gap_2().flex_shrink_0();
    let row = std::rc::Rc::new(row);
    let mut list = virtual_list(count, move |at: Position| {
        let row = row.clone();
        dynamic(move || {
            let index = at.index();
            if index < count {
                row(index)
            } else {
                view().into_element()
            }
        })
    })
    .keys(move || ids.clone())
    .viewport(ListViewport::Parent)
    .estimated_item_size(px(64.0))
    .separator(|| view().h(px(4.0)));
    if more {
        list = list.on_load_more(move || load());
    }
    body.child(list)
        .child(view().items_center().child(footer))
        .into_element()
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
    // A suggestion that would replace a confirmed rule of its key: the
    // change, against that rule's text.
    // (Read when the inspection arrives; see `send`.)
    if r.status == "suggested"
        && let Some(peer) = inspect.peer_demotions.first()
        && let Some(old) = m.operator.update_of.as_ref().filter(|o| o.id == peer.id)
    {
        body = body.child(update_section(model, &m, old, r));
    }
    let digest = if m.operator.digest_expanded {
        r.proposal_digest.clone()
    } else {
        format!(
            "{}…",
            r.proposal_digest.chars().take(16).collect::<String>()
        )
    };
    let variant = match r.status.as_str() {
        "confirmed" => ui::BadgeVariant::Success,
        "suggested" => ui::BadgeVariant::Warning,
        _ => ui::BadgeVariant::Neutral,
    };
    let caption = |s: String| {
        text(s)
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted)
    };
    // The record: its key and state, its text as written, where it applies.
    body = body.child(
        view()
            .gap_2()
            .p_3()
            .rounded(px(ui::scale::radius::CONTAINER))
            .bg(move || palette(model).panel)
            .border_1()
            .border_color(move || palette(model).hairline)
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        text(sanitize(&r.key, 120))
                            .font_weight(FontWeight::SEMIBOLD)
                            .min_w_0(),
                    )
                    .child(ui::badge(model, status_word(&m, &r.status)).variant(variant))
                    .child(caption(format!(
                        "{} · {}",
                        scope_label(m.locale, &r.scope),
                        origin_label(m.locale, r.origin)
                    ))),
            )
            .child(if disguising(&r.value) || disguising(&r.key) {
                // Escaped, exactly as stored: nothing is rendered or hidden.
                let mut lines = view().gap_0p5();
                for line in literal(&r.key).into_iter().chain(literal(&r.value)) {
                    lines = lines.child(text(line).font_family("monospace"));
                }
                lines.into_element()
            } else {
                scroll()
                    .max_h(px(320.0))
                    .w_full()
                    .child(ui::markdown_view(model, r.value.clone()))
                    .into_element()
            })
            .child(record_meta(model, r)),
    );
    // What only matters when checking exactly what is decided: one quiet
    // line, the notes behind its info button.
    body = body.child(
        view()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(caption(format!(
                "#{} · {} {} · {} {}",
                r.id,
                m.word(Msg::ReviewRevision),
                r.revision,
                m.word(Msg::ReviewDigest),
                digest
            )))
            .child(
                ui::button(model, "")
                    .label_fn(move || {
                        let m = model.get();
                        m.word(if m.operator.digest_expanded {
                            Msg::ReviewDigestCollapse
                        } else {
                            Msg::ReviewDigestExpand
                        })
                        .into()
                    })
                    .size(ui::ButtonSize::Small)
                    .variant(ui::ButtonVariant::Ghost)
                    .on_press(move || {
                        model.update(|m| m.operator.digest_expanded = !m.operator.digest_expanded)
                    }),
            )
            .child(ui::info(
                model,
                move || model.get().word(Msg::ReviewNotesTitle).into(),
                move || {
                    let m = model.get();
                    format!(
                        "{}\n\n{}",
                        m.word(Msg::ReviewProvenanceNote),
                        m.word(Msg::ReviewContractNote)
                    )
                    .into()
                },
            )),
    );
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
                Msg::EditSuggestion,
                move || {
                    available(model)
                        && write_grant(model)
                        && model
                            .get()
                            .operator
                            .record()
                            .is_some_and(precept_ui_core::operator::editable)
                },
                move || {
                    model.update(|m| {
                        m.operator.start_revision();
                    })
                },
            ))
            .child(word_action(
                model,
                Msg::ReviewDeprecate,
                move || available(model) && write_grant(model) && status_is(model, "confirmed"),
                move || review(model, "deprecate"),
            ))
            .child(if m.operator.mode == ReviewMode::Project {
                word_action(
                    model,
                    Msg::ExportRule,
                    move || {
                        available(model)
                            && status_is(model, "confirmed")
                            && !model.get().operator.export_busy
                    },
                    move || open_export(model),
                )
                .into_element()
            } else {
                view().into_element()
            })
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
/// Reads the confirmed rule `id` an inspected suggestion would replace.
fn fetch_update_of(model: Signal<Model>, scope: precept_ui_core::operator::Scope, id: i64) {
    super::profile::call(
        model,
        "profile.preferences.get",
        serde_json::json!({"scope": scope, "id": id}),
        move |_, m, r| {
            if let Ok(v) = r
                && let Ok(record) = serde_json::from_value::<
                    precept_ui_core::operator::PreferenceRecord,
                >(v["record"].clone())
                && record.id == id
            {
                m.operator.update_of = Some(record);
            }
        },
    );
}

/// "Updates #N": the confirmed rule's text against the suggestion's, and
/// what else changes.
fn update_section(
    model: Signal<Model>,
    m: &Model,
    old: &precept_ui_core::operator::PreferenceRecord,
    new: &precept_ui_core::operator::PreferenceRecord,
) -> AnyElement {
    let caption = |s: String| {
        text(s)
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted)
    };
    let mut section = view()
        .gap_2()
        .p_3()
        .rounded(px(ui::scale::radius::CONTAINER))
        .bg(move || palette(model).panel)
        .border_1()
        .border_color(move || palette(model).hairline)
        .child(
            text(
                m.word(Msg::UpdatesRule)
                    .replace("{id}", &old.id.to_string())
                    .replace("{key}", &sanitize(&old.key, 120)),
            )
            .font_weight(FontWeight::SEMIBOLD),
        )
        .child(caption(m.word(Msg::UpdatesRuleHelp).into()));
    section = section.child(ui::text_diff(model, old.value.clone(), new.value.clone()));

    if old.tags != new.tags {
        section = section.child(caption(
            m.word(Msg::UpdatesTags)
                .replace("{old}", &precept_ui_core::presentation::hashtags(&old.tags))
                .replace("{new}", &precept_ui_core::presentation::hashtags(&new.tags)),
        ));
    }
    if old.rule != new.rule {
        section = section.child(caption(m.word(Msg::UpdatesOptions).into()));
    }
    section.into_element()
}

/// Reads what writing the inspected confirmed convention as a rule file
/// would do, and opens the preview.
fn open_export(model: Signal<Model>) {
    let m = model.get();
    let (Some(project), Some(id)) = (m.project().map(|p| p.name.clone()), m.operator.selected_id)
    else {
        return;
    };
    model.update(|m| m.operator.export_busy = true);
    super::profile::call(
        model,
        "rules.export",
        serde_json::json!({"project": project, "id": id}),
        move |_, m, r| {
            m.operator.export_busy = false;
            match r.and_then(|v| {
                serde_json::from_value::<precept_ui_core::operator::RuleExport>(v)
                    .map_err(|_| "Unavailable")
            }) {
                Ok(preview) => m.operator.export = Some((id, preview)),
                Err(code) => ui::notify(ui::Tone::Error, code),
            }
        },
    );
}

/// Writes the previewed rule file, guarded by the digest the preview saw.
fn write_export(model: Signal<Model>) {
    let m = model.get();
    let (Some(project), Some((id, preview))) = (
        m.project().map(|p| p.name.clone()),
        m.operator.export.clone(),
    ) else {
        return;
    };
    model.update(|m| m.operator.export_busy = true);
    super::profile::call(
        model,
        "rules.export.apply",
        serde_json::json!({"project": project, "id": id, "enabled": true,
            "expected_sha256": preview.sha256}),
        move |model, m, r| {
            m.operator.export_busy = false;
            match r {
                Ok(v) => {
                    m.operator.export = None;
                    ui::notify(
                        ui::Tone::Success,
                        m.word(if v["action"] == "updated" {
                            Msg::ToastExportUpdated
                        } else {
                            Msg::ToastExported
                        }),
                    );
                    // The new file shows as pending on Preferences & contracts.
                    // Not inside this update: the loader updates the model itself.
                    spawn(async move { crate::load(model) });
                }
                Err("Conflict") => ui::notify(ui::Tone::Error, m.word(Msg::ExportChanged)),
                Err(code) => ui::notify(ui::Tone::Error, code),
            }
        },
    );
}

/// The rule file preview: path, what writing does, and the exact text.
/// Writing is offered only for an absent path and a valid file.
pub(crate) fn export_dialog(model: Signal<Model>) -> impl IntoElement {
    let preview = move || model.get().operator.export.clone().map(|(_, p)| p);
    ui::dialog(model, "")
        .size(ui::DialogSize::Large)
        .title_fn(move || model.get().word(Msg::ExportTitle).into())
        .description_fn(move || {
            preview()
                .map(|p| sanitize(&p.path, 160))
                .unwrap_or_default()
                .into()
        })
        .content_fn(move || {
            let m = model.get();
            let Some(p) = preview() else {
                return view().into_element();
            };
            let state = match (p.valid, p.state.as_str()) {
                (false, _) => Some((Msg::ExportInvalid, palette(model).danger)),
                (_, "same") => Some((Msg::ExportSame, palette(model).muted)),
                (_, "different") => Some((Msg::ExportDifferent, palette(model).muted)),
                (_, "invalid") => Some((Msg::ExportInvalidFile, palette(model).danger)),
                _ => None,
            };
            let mut body = view().gap_3().w_full().child(
                text(m.word(Msg::ExportHelp))
                    .text_size(px(ui::scale::text::CAPTION))
                    .text_color(move || palette(model).muted),
            );
            if let Some((msg, color)) = state {
                body = body.child(
                    text(m.word(msg))
                        .text_size(px(ui::scale::text::CAPTION))
                        .text_color(move || color),
                );
            }
            // An update shows the change; a new file, its text.
            if p.state == "different"
                && let Some(current) = p.current.clone()
            {
                return body
                    .child(ui::text_diff(model, current, p.text))
                    .into_element();
            }
            body.child(
                scroll()
                    .max_h(px(360.0))
                    .w_full()
                    .rounded(px(ui::scale::radius::CONTROL))
                    .bg(move || palette(model).inset)
                    .child(
                        text(p.text)
                            .p_3()
                            .selectable(true)
                            .font_family("monospace")
                            .text_size(px(ui::scale::text::CAPTION)),
                    ),
            )
            .into_element()
        })
        .cancel_fn(move || model.get().word(Msg::CloseDialog).into())
        .confirm_fn(move || {
            let m = model.get();
            m.word(if preview().is_some_and(|p| p.state == "different") {
                Msg::ExportUpdate
            } else {
                Msg::ExportWrite
            })
            .into()
        })
        .confirm_disabled(move || {
            model.get().operator.export_busy
                || preview()
                    .is_none_or(|p| !p.valid || !matches!(p.state.as_str(), "absent" | "different"))
        })
        .open(move || preview().is_some())
        .on_cancel(move || model.update(|m| m.operator.export = None))
        .on_confirm(move || write_export(model))
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
            let mut line = format!(
                "{} · r{}→r{} · {} · {} · {} {}",
                decision_word(m.locale, d.action),
                d.from_revision,
                d.to_revision,
                channel_word(m.locale, d.channel),
                precept_ui_core::presentation::utc_minute(&d.decided_at),
                m.word(Msg::ReviewCommit),
                d.receipt_commit_sequence
            );
            if let Some(by) = d.replaced_by {
                line.push_str(&format!(
                    " · {}",
                    m.word(Msg::ReplacedBy).replace("{id}", &by.to_string())
                ));
            }
            if let Some(note) = &d.note {
                line.push_str(&format!(" — {}", sanitize(note, 500)));
            }
            line
        })
        .collect();
    body = body.child(card(model, m.word(Msg::ReviewHistoryTab), lines));
    body.into_element()
}
fn delete_dialog(
    model: Signal<Model>,
    delete_open: Signal<bool>,
    retype_ref: ElementRef,
) -> impl IntoElement {
    ui::alert_dialog(model, "")
        .icon(icons::TRASH, ui::DialogTone::Danger)
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
                .gap_3()
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            text(move || {
                                let m = model.get();
                                let id = m.operator.delete_arm.as_ref().map_or(0, |a| a.id);
                                m.word(Msg::ReviewDeleteId).replace("{id}", &id.to_string())
                            })
                            .flex_1()
                            .font_family("monospace")
                            .text_size(px(ui::scale::text::BODY)),
                        )
                        .child(
                            ui::copy_button(
                                model,
                                move || model.get().word(Msg::ReviewCopyId).into(),
                                move || model.get().word(Msg::CopiedShort).into(),
                                move || {
                                    let id = model
                                        .get()
                                        .operator
                                        .delete_arm
                                        .as_ref()
                                        .map_or(0, |a| a.id);
                                    if id <= 0 {
                                        return false;
                                    }
                                    set_clipboard(id.to_string());
                                    true
                                },
                            )
                            .size(ui::ButtonSize::Small),
                        ),
                )
                .child(ui::field(
                    model,
                    move || model.get().word(Msg::ReviewDeleteRetype).into(),
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
                ))
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
pub(crate) fn panel(model: Signal<Model>) -> impl IntoElement {
    let global = KeyOptions {
        when: nsh_core::keymap::When::Always,
        ..Default::default()
    };
    // The destructive tier's presentation state; the armed identity itself lives
    // in the shared model so every view enforces one guard.
    let delete_open = signal(false);
    let retype_ref = element_ref();
    let bind = |id: &'static str, handler: Box<dyn Fn()>| {
        crate::shortcuts::bind(model, id, global, handler)
    };
    for (key, action) in [
        ("review.suggest", "suggest"),
        ("review.confirm", "confirm"),
        ("review.reject", "reject"),
        ("review.deprecate", "deprecate"),
    ] {
        bind(
            key,
            Box::new(move || {
                if model.get().operator.open {
                    review(model, action);
                }
            }),
        );
    }
    bind(
        "review.delete",
        Box::new(move || {
            if model.get().operator.open {
                open_delete(model, delete_open);
            }
        }),
    );
    bind(
        "review.apply",
        Box::new(move || {
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
        }),
    );
    bind(
        "review.profile",
        Box::new(move || {
            if model.get().operator.open {
                refresh(model, "profile.info");
            }
        }),
    );
    bind(
        "review.records",
        Box::new(move || {
            if model.get().operator.open {
                model.update(|m| m.operator.pane = Pane::Preferences);
                refresh(model, "profile.preferences.list");
            }
        }),
    );
    // Windows' native Tab chain visits text fields; expose the existing exact
    // saved-Handoff read without depending on a button's platform traversal.
    bind(
        "review.handoff",
        Box::new(move || {
            let m = model.get();
            if m.operator.pane == Pane::Handoff && m.selected_project.is_some() && available(model)
            {
                refresh(model, "profile.handoff.get");
            }
        }),
    );
    bind(
        "review.retry",
        Box::new(move || {
            if model.get().operator.open {
                let mut request = None;
                model.update(|m| request = m.operator_retry());
                if let Some(r) = request {
                    send(model, r);
                }
            }
        }),
    );
    bind(
        "review.next",
        Box::new(move || {
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
        }),
    );
    effect(move || {
        if model.get().operator.delete_arm.is_none() {
            delete_open.set(false);
        }
    });
    effect(move || {
        if std::env::var_os("PRECEPT_UI_PROBE").is_some() {
            let m = model.get();
            let o = &m.operator;
            if o.open {
                eprintln!(
                    "PRECEPT_NSH_OPERATOR selected={} grant={} busy={} review={} unknown={} rows={} record_revision={} receipt={} error={} handoff_revision={} review_tab={:?} queue={} inspect={} delete_armed={} digest_expanded={} inspection_current={} inspected_id={} inspected_revision={} peer_total={} delete_retype_chars={} delete_ready={}",
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
                    "PRECEPT_NSH_REVIEW global={} selected_id={} queue_after={} queue_first={} queue_last={} queue_sequence={} queue_baseline={} queue_stale={} inspected_status={} inspected_origin={:?} history={}",
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
    let offline_view = dynamic(move || {
        let m = model.get();
        if matches!(m.connection.target, Target::Local | Target::Wsl) {
            return view().into_element();
        }
        ui::surface(model)
            .child(
                ui::empty(model, m.word(Msg::ReviewOfflineTitle))
                    .icon(icons::REVIEW)
                    .description(m.word(Msg::ReviewOfflineBody))
                    .action(
                        ui::button(model, "")
                            .label_fn(move || model.get().word(Msg::GoToRuntimeChecks).into())
                            .variant(ui::ButtonVariant::Primary)
                            .on_press(move || {
                                shell::go_setup(model, precept_ui_core::SetupTab::Connection)
                            }),
                    ),
            )
            .into_element()
    });
    let live_body = view()
        .gap_4()
        .style_when(
            move || !matches!(model.get().connection.target, Target::Local | Target::Wsl),
            |s| s.hidden().invisible(),
        )
        .child(mode_tabs(model))
        .child(writes_notice(model))
        .child(page_toolbar(model))
        .child(batch_bar(model))
        .child(
            view()
                .style_when(
                    move || model.get().operator.pane != Pane::Preferences,
                    |s| s.hidden().invisible(),
                )
                .child(dynamic(move || {
                    let m = model.get();
                    let mode = m.operator.mode;
                    if mode == ReviewMode::Project && m.selected_project.is_none() {
                        return ui::surface(model)
                            .child(
                                ui::empty(model, m.word(Msg::ReviewNeedsProjectTitle))
                                    .icon(icons::section_path(precept_ui_core::Section::Projects))
                                    .description(m.word(Msg::ReviewNeedsProjectBody))
                                    .action(
                                        ui::button(model, "")
                                            .label_fn(move || {
                                                model.get().word(Msg::GoToWorkspaces).into()
                                            })
                                            .variant(ui::ButtonVariant::Primary)
                                            .on_press(move || {
                                                shell::go_setup(
                                                    model,
                                                    precept_ui_core::SetupTab::Workspaces,
                                                )
                                            }),
                                    ),
                            )
                            .into_element();
                    }
                    if mode == ReviewMode::Inbox {
                        queue_tab(model, delete_open)
                    } else {
                        records_list(model, delete_open)
                    }
                })),
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
        .child(unknown_banner(model));
    let content = view()
        .on_layout(move |_: &Bounds<Points>| {
            if model.get().operator.open && std::env::var_os("PRECEPT_UI_PROBE").is_some() {
                eprintln!("PRECEPT_NSH_LAYOUT_READY elapsed={:.6}", probe_elapsed());
            }
        })
        .gap_4()
        .flex_shrink_0()
        .max_w(px(ui::scale::layout::CONTENT))
        .child(offline_view)
        .child(live_body);
    // Toasts report what happened (applied receipts, refusals); the page shows
    // the resulting state itself.
    let feedback = computed(move || {
        let m = model.get();
        (
            m.operator.receipt.as_ref().map(|r| r.commit_sequence),
            m.operator.error.clone(),
            m.operator.outcome_unknown,
            // A deleted record leaves identity-only proof, never content.
            m.operator
                .receipt
                .as_ref()
                .is_some_and(|r| r.affected.iter().any(|a| a.deleted)),
        )
    });
    let reported = signal((None, None::<String>));
    effect(move || {
        let (receipt, error, unknown, deleted) = feedback.get();
        let (last_receipt, last_error) = untracked(|| reported.get());
        let m = untracked(|| model.get());
        if receipt.is_some() && receipt != last_receipt {
            ui::notify(
                ui::Tone::Success,
                format!(
                    "{} · commit {}",
                    m.word(if deleted {
                        Msg::ReviewDeleted
                    } else {
                        Msg::ReviewApplied
                    }),
                    receipt.unwrap_or_default()
                ),
            );
        }
        if let Some(e) = &error
            && error != last_error
            && !unknown
        {
            let wsl = m.connection.target == precept_ui_core::connection::Target::Wsl;
            ui::notify(
                ui::Tone::Error,
                format!("{e}: {}", tr(m.locale, error_msg(e, wsl))),
            );
        }
        reported.set((receipt, error));
    });
    // The list loads itself when the page is entered, its scope changes, a
    // change was applied (a receipt never leaves a stale page), a snapshot read
    // completed, or the Profile observation changed (writes turned on in
    // Profile change the binding: an old observation would refuse the list as
    // a stale generation). On first mount only when nothing was read yet.
    let entry = computed(move || {
        let m = model.get();
        let o = &m.operator;
        (
            o.open,
            o.mode,
            o.global,
            o.receipt.as_ref().map(|r| r.commit_sequence),
            m.epoch,
            m.phase == precept_ui_core::Phase::Loading,
            o.info
                .as_ref()
                .map(|i| (i.selected, i.write_enabled, i.binding.clone())),
        )
    });
    let last_entry = signal(None);
    let pending = signal(false);
    effect(move || {
        let next = entry.get();
        let previous = untracked(|| last_entry.get());
        last_entry.set(Some(next.clone()));
        let first_unread = previous.is_none() && untracked(|| model.get().operator.info.is_none());
        if next.0 && (first_unread || previous.is_some_and(|p| p != next)) {
            pending.set(true);
        }
    });
    let stage = computed(move || {
        let m = model.get();
        let o = &m.operator;
        // Never while a snapshot read is loading: its completion changes the
        // read guard, so a reply sent now would be discarded (and stay busy).
        let idle = o.open
            && o.pane == Pane::Preferences
            && m.phase != precept_ui_core::Phase::Loading
            && !o.busy
            && !o.outcome_unknown
            && o.review.is_none();
        (idle, o.info.as_ref().map(|i| i.selected), o.error.is_none())
    });
    effect(move || {
        let (idle, selected, clean) = stage.get();
        if !idle {
            return;
        }
        untracked(|| {
            let method = match selected {
                // Observe the Profile first (no retry loop over an error).
                None if clean => "profile.info",
                Some(true) if pending.get() => {
                    if model.get().operator.mode == ReviewMode::Inbox {
                        "profile.review.queue"
                    } else {
                        "profile.preferences.list"
                    }
                }
                _ => return,
            };
            let mut request = None;
            model.update(|m| request = m.operator_read(method));
            if let Some(request) = request {
                if method != "profile.info" {
                    pending.set(false);
                }
                send(model, request);
            }
        });
    });
    // The native modals belong to the visible workspace root, never to the
    // clipped/scrolled document that may place its insertion point off-screen.
    view()
        .flex_1()
        .style_when(
            move || !model.get().operator.open,
            |s| s.hidden().invisible(),
        )
        .child(scroll().flex_1().child(content))
        .child(record_sheet(model, delete_open))
        .child(apply_dialog(model, false))
        .child(apply_dialog(model, true))
        .child(add_dialog(model))
        .child(import_dialog(model))
        .child(delete_dialog(model, delete_open, retype_ref))
}

/// The unified hub tabs: pending review queue, personal preferences, and
/// current project's preferences.
fn mode_tabs(model: Signal<Model>) -> impl IntoElement {
    ui::tabs(model)
        .add_fn(
            move || model.get().word(Msg::ProfileReview).into(),
            move || model.get().operator.mode == ReviewMode::Inbox,
            move || open_mode(model, ReviewMode::Inbox),
        )
        .add_fn(
            move || model.get().word(Msg::ReviewTabPersonal).into(),
            move || model.get().operator.mode == ReviewMode::Personal,
            move || open_mode(model, ReviewMode::Personal),
        )
        .add_fn(
            move || {
                let m = model.get();
                match m.selected_project.as_deref() {
                    Some(name) => format!(
                        "{} · {}",
                        m.word(Msg::ReviewTabConventions),
                        sanitize(name, 32)
                    )
                    .into(),
                    None => m.word(Msg::ReviewTabConventions).into(),
                }
            },
            move || model.get().operator.mode == ReviewMode::Project,
            move || open_mode(model, ReviewMode::Project),
        )
}

/// Writes and Profile live in Runtime checks; here only a short notice when
/// changes cannot be applied, with the one-click path there.
fn writes_notice(model: Signal<Model>) -> impl IntoElement {
    let state = computed(move || {
        let m = model.get();
        (
            m.locale,
            m.operator
                .info
                .as_ref()
                .map(|i| (i.selected, i.write_enabled)),
        )
    });
    dynamic(move || {
        let (_, info) = state.get();
        let m = model.get_untracked();
        let key = match info {
            Some((false, _)) => Msg::OperatorUnselected,
            Some((true, false)) => Msg::WritesOffNotice,
            _ => return view().into_element(),
        };
        view()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(ui::badge(model, m.word(key)).variant(ui::BadgeVariant::Warning))
            .child(
                ui::button(model, "")
                    .label_fn(move || model.get().word(Msg::OpenProfileStep).into())
                    .size(ui::ButtonSize::Small)
                    .on_press(move || shell::go_setup(model, precept_ui_core::SetupTab::Profile)),
            )
            .into_element()
    })
}

/// The page's commands: the scope switch (To review), Add preference (Personal
/// and Project tabs), and Refresh.
fn page_toolbar(model: Signal<Model>) -> impl IntoElement {
    let scope = |global: bool| {
        ui::button(model, "")
            .label_fn(move || {
                let m = model.get();
                if global {
                    m.word(Msg::ScopePersonal).into()
                } else {
                    match m.selected_project.as_deref() {
                        Some(name) => {
                            format!("{} · {}", m.word(Msg::ScopeThisProject), sanitize(name, 32))
                                .into()
                        }
                        None => m.word(Msg::ScopeThisProject).into(),
                    }
                }
            })
            .size(ui::ButtonSize::Small)
            .selected(move || model.get().operator.global == global)
            .disabled(move || {
                !available(model) || (!global && model.get().selected_project.is_none())
            })
            .on_press(move || {
                model.update(|m| {
                    if m.operator.global != global {
                        m.operator.invalidate();
                        m.operator.batch.clear();
                        m.operator.global = global;
                        m.operator.after_id = 0;
                        m.operator.queue_after_id = 0;
                    }
                })
            })
    };
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(
            view()
                .flex_row()
                .gap_1()
                .style_when(
                    move || model.get().operator.mode != ReviewMode::Inbox,
                    |s| s.hidden(),
                )
                .child(scope(true))
                .child(scope(false)),
        )
        .child(
            view()
                .style_when(
                    move || model.get().operator.mode == ReviewMode::Inbox,
                    |s| s.hidden(),
                )
                .child(
                    ui::button(model, "")
                        .label_fn(move || {
                            let m = model.get();
                            m.word(match m.operator.mode {
                                ReviewMode::Project => Msg::AddConvention,
                                _ => Msg::AddPreference,
                            })
                            .into()
                        })
                        .icon(icons::PLUS)
                        .variant(ui::ButtonVariant::Primary)
                        .disabled(move || {
                            let m = model.get();
                            !available(model)
                                || !write_grant(model)
                                || (m.operator.mode == ReviewMode::Project
                                    && m.selected_project.is_none())
                        })
                        .on_press(move || {
                            model.update(|m| {
                                m.operator.start_draft();
                                m.operator.adding = true;
                            });
                        }),
                ),
        )
        .child(
            ui::button(model, "")
                .label_fn(move || model.get().word(Msg::ImportOpen).into())
                .disabled(move || {
                    !available(model)
                        || !write_grant(model)
                        || model.get().selected_project.is_none()
                })
                .on_press(move || open_import(model)),
        )
        .child(
            view()
                .style_when(
                    move || model.get().operator.mode == ReviewMode::Inbox,
                    |s| s.hidden(),
                )
                .child(ui::switch(
                    model,
                    move || model.get().word(Msg::ShowDeprecated).into(),
                    move || model.get().operator.show_deprecated,
                    || false,
                    move |show| {
                        model.update(|m| {
                            let close = !show
                                && m.operator
                                    .record()
                                    .is_some_and(|r| r.status == "deprecated");
                            m.operator.show_deprecated = show;
                            if close {
                                m.operator.selected_id = None;
                                m.operator.inspect = None;
                                m.operator.inspect_revision = 0;
                            }
                            m.operator.review = None;
                            m.operator.delete_arm = None;
                        });
                    },
                )),
        )
        .child(view().flex_1())
        .child(
            ui::button(model, "")
                .label_fn(move || model.get().word(Msg::RefreshList).into())
                .size(ui::ButtonSize::Small)
                .variant(ui::ButtonVariant::Ghost)
                .icon(icons::REFRESH)
                .disabled(move || !available(model))
                .on_press(move || {
                    model.update(|m| {
                        m.operator.page = None;
                        m.operator.queue = None;
                        m.operator.error = None;
                        // The list is read again after the Profile, even when
                        // the Profile did not change (else it stays loading).
                        m.operator.follow_up = Some(if m.operator.mode == ReviewMode::Inbox {
                            "profile.review.queue"
                        } else {
                            "profile.preferences.list"
                        });
                    });
                    refresh(model, "profile.info");
                }),
        )
}

/// The scoped pages' list: records matching the status filter; a row opens the
/// record's sheet (and reads the exact record for it).
fn records_list(model: Signal<Model>, delete_open: Signal<bool>) -> AnyElement {
    let m = model.get();
    let body = view().gap_1().flex_shrink_0();
    let Some(page) = &m.operator.page else {
        return body
            .child(ui::empty(model, m.word(Msg::RecordsLoading)))
            .into_element();
    };
    let records = std::rc::Rc::new(
        page.records
            .iter()
            .filter(|r| m.operator.show_deprecated || r.status != "deprecated")
            .cloned()
            .collect::<Vec<_>>(),
    );
    let mut body = body;
    if records.is_empty() {
        let unfiltered_empty = page.records.is_empty();
        body = body.child(
            ui::empty(
                model,
                m.word(if unfiltered_empty {
                    Msg::OperatorEmpty
                } else {
                    Msg::RecordsFilteredEmpty
                }),
            )
            .description(m.word(if unfiltered_empty {
                Msg::RecordsEmptyHelp
            } else {
                Msg::DeprecatedFilterHelp
            })),
        );
        if page.next_after_id.is_none() {
            return body.into_element();
        }
    }
    let list = loaded_list(
        model,
        records.iter().map(|r| r.id).collect(),
        {
            let records = records.clone();
            move |index| record_row(model, delete_open, &records[index], index)
        },
        page.next_after_id.is_some(),
        move || {
            let mut go = false;
            model.update(|m| {
                if let Some(id) = m.operator.page.as_ref().and_then(|p| p.next_after_id) {
                    m.operator.after_id = id;
                    go = true;
                }
                m.operator.review = None;
            });
            if go {
                refresh(model, "profile.preferences.list");
            }
        },
    );
    body.child(list).into_element()
}
/// One record of the scope: id, key, state and revision (pressing it opens the
/// exact record), the first line of its value, its tags and origin.
fn record_row(
    model: Signal<Model>,
    delete_open: Signal<bool>,
    r: &PreferenceRecord,
    index: usize,
) -> AnyElement {
    let m = model.get();
    let id = r.id;
    let arrive = ui::motion::arrive(((index % 12) as f32) * 50.0);
    let variant = match r.status.as_str() {
        "confirmed" => ui::BadgeVariant::Success,
        "suggested" => ui::BadgeVariant::Warning,
        _ => ui::BadgeVariant::Neutral,
    };
    let label = format!(
        "#{} · {} · {} · r{}",
        id,
        sanitize(&r.key, 120),
        row_status(&m, &r.status),
        r.revision
    );
    view()
        .py_1()
        .bind_motion(&arrive)
        .child(record_line(
            model,
            r,
            label,
            &format!("#{id} · r{}", r.revision),
            move || open_record(model, delete_open, id),
            ui::badge(model, status_word(&m, &r.status))
                .variant(variant)
                .into_element(),
        ))
        .into_element()
}
fn status_word(m: &Model, status: &str) -> String {
    match status {
        "confirmed" => m.word(Msg::StatusConfirmed).into(),
        "suggested" => m.word(Msg::StatusSuggested).into(),
        "deprecated" => m.word(Msg::StatusDeprecated).into(),
        "rejected" => m.word(Msg::StatusRejected).into(),
        other => sanitize(other, 40),
    }
}
/// The status inside a row's label (lowercase in English).
fn row_status(m: &Model, status: &str) -> String {
    match status {
        "confirmed" => m.word(Msg::RowConfirmed).into(),
        "suggested" => m.word(Msg::RowSuggested).into(),
        "rejected" => m.word(Msg::RowRejected).into(),
        "deprecated" => m.word(Msg::RowDeprecated).into(),
        other => sanitize(other, 40),
    }
}
/// Selects a record, opens its sheet and reads the exact record for it.
fn open_record(model: Signal<Model>, delete_open: Signal<bool>, id: i64) {
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
}

/// The selected record beside the list: its exact detail and decision history,
/// and its decisions (each prepares, then the Apply dialog commits it).
/// A confirmed rule's switch: off keeps the reviewed record but agents stop
/// receiving it. Applied at once (revision-checked), no new review.
fn delivered_switch(model: Signal<Model>) -> impl IntoElement {
    let record = move || model.get().operator.record().cloned();
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .p_3()
        .rounded(px(ui::scale::radius::CONTAINER))
        .bg(move || palette(model).inset)
        .style_when(
            move || record().is_none_or(|r| r.status != "confirmed"),
            |s| s.hidden(),
        )
        .child(
            view()
                .flex_1()
                .gap_0p5()
                .child(
                    text(move || model.get().word(Msg::DeliveredSwitch))
                        .font_weight(FontWeight::MEDIUM),
                )
                .child(
                    text(move || model.get().word(Msg::DeliveredHelp))
                        .text_size(px(ui::scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                ),
        )
        .child(ui::switch(
            model,
            move || model.get().word(Msg::DeliveredSwitch).into(),
            move || record().is_some_and(|r| r.enabled),
            move || {
                let m = model.get();
                m.operator.busy || !m.operator_inspected() || !write_grant(model)
            },
            move |on| {
                review(model, if on { "switch_on" } else { "switch_off" });
                submit(model);
            },
        ))
}
/// The note the next decision on this record keeps (outside the rebuilt
/// detail, so typing keeps its focus).
fn decision_note(model: Signal<Model>) -> impl IntoElement {
    view()
        .style_when(
            move || {
                model
                    .get()
                    .operator
                    .record()
                    .is_none_or(|r| !matches!(r.status.as_str(), "suggested" | "confirmed"))
            },
            |s| s.hidden(),
        )
        .gap_1()
        .child(ui::field(
            model,
            move || model.get().word(Msg::DecisionNote).into(),
            ui::input(model, "")
                .label_fn(move || model.get().word(Msg::DecisionNote).into())
                .value(move || model.get().operator.note.clone())
                .disabled(move || model.get().operator.busy || model.get().operator.outcome_unknown)
                .on_change(move |v: &str| {
                    model.update(|m| {
                        m.operator.review = None;
                        m.operator.note = v.chars().take(600).collect();
                    })
                }),
        ))
        .child(
            text(move || model.get().word(Msg::DecisionNoteHelp))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
}
fn record_sheet(model: Signal<Model>, delete_open: Signal<bool>) -> impl IntoElement {
    ui::sheet(model)
        .title_fn(move || {
            let m = model.get();
            m.operator
                .record()
                .map(|r| format!("#{} · {}", r.id, sanitize(&r.key, 80)))
                .unwrap_or_default()
                .into()
        })
        .close_fn(move || model.get().word(Msg::CloseDetail).into())
        .open(move || {
            let m = model.get();
            m.operator.open && m.operator.selected_id.is_some() && m.operator.record().is_some()
        })
        .on_close(move || {
            disarm(model, delete_open);
            model.update(|m| {
                m.operator.selected_id = None;
                m.operator.inspect = None;
                m.operator.inspect_revision = 0;
                m.operator.review = None;
            })
        })
        .body(move || {
            view()
                .gap_4()
                .child(delivered_switch(model))
                .child(decision_note(model))
                .child(dynamic(move || detail_tab(model, delete_open)))
                .child(dynamic(move || history_tab(model)))
                .into_element()
        })
}

/// The prepared operation, committed only by Apply: what it does, to which
/// exact record, the disclosures, and the request itself behind "Show request".
/// `destructive` builds the delete tier's (danger) variant.
fn apply_dialog(model: Signal<Model>, destructive: bool) -> impl IntoElement {
    let raw = signal(false);
    let is_delete = move |m: &Model| {
        matches!(
            m.operator
                .review
                .as_ref()
                .and_then(|r| r.mutation.as_ref())
                .map(|m| &m.command),
            Some(Command::DeletePreference { .. })
        )
    };
    let dialog = if destructive {
        ui::alert_dialog(model, "").icon(icons::ALERT, ui::DialogTone::Danger)
    } else {
        ui::dialog(model, "").icon(icons::CHECK, ui::DialogTone::Accent)
    };
    dialog
        .size(ui::DialogSize::Medium)
        .title_fn(move || model.get().word(Msg::OperatorReview).into())
        .description_fn(move || model.get().word(Msg::OperatorPending).into())
        .content_fn(move || {
            view()
                .gap_2()
                .child(dynamic(move || {
                    let m = model.get();
                    let Some(review) = &m.operator.review else {
                        return view().into_element();
                    };
                    let mut lines = vec![review.method.clone()];
                    if let Some(r) = &review.record {
                        lines.push(format!(
                            "#{} · {} · {} · r{} · {}…",
                            r.id,
                            sanitize(&r.key, 80),
                            scope_label(m.locale, &r.scope),
                            r.revision,
                            r.proposal_digest.chars().take(16).collect::<String>()
                        ));
                    }
                    if let Some(mutation) = &review.mutation {
                        match &mutation.command {
                            Command::ConfirmPreference { .. } => {
                                lines.push(m.word(Msg::OperatorPeerWarning).into())
                            }
                            Command::DeprecatePreference { .. } => {
                                lines.push(m.word(Msg::ReviewDeprecateImpact).into())
                            }
                            Command::DeletePreference { .. } => {
                                lines.push(m.word(Msg::ReviewDeleteBody).into())
                            }
                            Command::SuggestPreference { key, value, .. } => {
                                lines.push(sanitize(key, 120));
                                lines.extend(literal(value).into_iter().take(6));
                            }
                            Command::SuggestBatch { items } => {
                                lines.push(
                                    m.word(Msg::ImportLine)
                                        .replace("{n}", &items.len().to_string()),
                                );
                                for i in items {
                                    lines.push(format!(
                                        "{} · {}",
                                        sanitize(&i.key, 120),
                                        scope_label(m.locale, &i.scope)
                                    ));
                                }
                            }
                            Command::DecideBatch {
                                decision, items, ..
                            } => {
                                let confirm =
                                    *decision == precept_ui_core::operator::BatchDecision::Confirm;
                                lines.push(
                                    m.word(if confirm {
                                        Msg::BatchConfirmLine
                                    } else {
                                        Msg::BatchRejectLine
                                    })
                                    .replace("{n}", &items.len().to_string()),
                                );
                                let (listed, _) = m.operator.batch_records();
                                for i in items {
                                    let key = listed
                                        .iter()
                                        .find(|r| r.id == i.id)
                                        .map(|r| sanitize(&r.key, 80))
                                        .unwrap_or_default();
                                    lines.push(format!(
                                        "#{} · {} · {}…",
                                        i.id,
                                        key,
                                        i.proposal_digest.chars().take(12).collect::<String>()
                                    ));
                                }
                                if confirm {
                                    lines.push(m.word(Msg::OperatorPeerWarning).into());
                                }
                            }
                            Command::RevisePreference {
                                id,
                                key,
                                value,
                                tags,
                                ..
                            } => {
                                lines.push(
                                    m.word(if m.operator.revising_confirmed {
                                        Msg::EditRuleHelp
                                    } else {
                                        Msg::EditSuggestionHelp
                                    })
                                    .replace("{id}", &id.to_string()),
                                );
                                lines.push(format!(
                                    "{} · {}",
                                    sanitize(key, 120),
                                    precept_ui_core::presentation::hashtags(tags)
                                ));
                                lines.extend(literal(value).into_iter().take(6));
                            }
                            _ => {}
                        }
                        if let Some(note) = mutation.command.note() {
                            lines.push(format!(
                                "{}: {}",
                                m.word(Msg::DecisionNote),
                                sanitize(note, 500)
                            ));
                        }
                    }
                    if raw.get() {
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
                    }
                    card(model, m.word(Msg::OperatorReview), lines).into_element()
                }))
                .child(
                    ui::button(model, "")
                        .label_fn(move || {
                            model
                                .get()
                                .word(if raw.get() {
                                    Msg::HideRequest
                                } else {
                                    Msg::ShowRequest
                                })
                                .into()
                        })
                        .size(ui::ButtonSize::Small)
                        .variant(ui::ButtonVariant::Ghost)
                        .on_press(move || raw.update(|r| *r = !*r)),
                )
                .into_element()
        })
        .cancel_fn(move || model.get().word(Msg::OperatorCancel).into())
        .confirm_fn(move || model.get().word(Msg::OperatorCommit).into())
        .confirm_disabled(move || !available(model) || model.get().operator.busy)
        .open(move || {
            let m = model.get();
            m.operator.open && m.operator.review.is_some() && is_delete(&m) == destructive
        })
        .on_cancel(move || {
            raw.set(false);
            model.update(|m| {
                m.operator.review = None;
                m.operator.delete_arm = None;
            })
        })
        .on_confirm(move || {
            raw.set(false);
            submit(model)
        })
}

/// Add preference: key, a Markdown value, its tags and where it comes from.
/// Saving prepares a suggestion (never a confirmation); Apply commits it, then
/// the new exact record opens to confirm.
/// One comma-separated `applies_to` field of the draft.
fn applies_field(
    model: Signal<Model>,
    label: Msg,
    get: fn(&precept_ui_core::operator::Operator) -> &String,
    set: fn(&mut precept_ui_core::operator::Operator, String),
) -> AnyElement {
    ui::field(
        model,
        move || model.get().word(label).into(),
        ui::input(model, "")
            .label_fn(move || model.get().word(label).into())
            .value(move || get(&model.get().operator).clone())
            .disabled(move || model.get().operator.busy || model.get().operator.outcome_unknown)
            .on_change(move |v: &str| {
                let v: String = v.chars().take(512).collect();
                model.update(|m| {
                    set(&mut m.operator, v);
                    m.operator.review = None;
                    m.operator.serial += 1;
                })
            }),
    )
    .into_element()
}
/// The draft's rule options: where it applies, its priority and its declared
/// enforcement.
fn rule_options(model: Signal<Model>) -> AnyElement {
    let pick = |e: precept_ui_core::operator::Enforcement| {
        move || model.update(|m| m.operator.enforcement = e)
    };
    let is =
        |e: precept_ui_core::operator::Enforcement| move || model.get().operator.enforcement == e;
    view()
        .gap_3()
        .child(text(move || model.get().word(Msg::AppliesTitle)).font_weight(FontWeight::MEDIUM))
        .child(
            text(move || model.get().word(Msg::AppliesHelp))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_3()
                .child(view().flex_1().min_w(px(200.0)).child(applies_field(
                    model,
                    Msg::AppliesLanguages,
                    |o| &o.applies_languages,
                    |o, v| o.applies_languages = v,
                )))
                .child(view().flex_1().min_w(px(200.0)).child(applies_field(
                    model,
                    Msg::AppliesPaths,
                    |o| &o.applies_paths,
                    |o, v| o.applies_paths = v,
                )))
                .child(view().flex_1().min_w(px(200.0)).child(applies_field(
                    model,
                    Msg::AppliesAgents,
                    |o| &o.applies_agents,
                    |o, v| o.applies_agents = v,
                )))
                .child(view().flex_1().min_w(px(200.0)).child(applies_field(
                    model,
                    Msg::AppliesTaskTags,
                    |o| &o.applies_tags,
                    |o, v| o.applies_tags = v,
                ))),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_3()
                .child(view().w(px(200.0)).child(applies_field(
                    model,
                    Msg::PriorityLabel,
                    |o| &o.priority,
                    |o, v| o.priority = v,
                )))
                .child(
                    view().flex_1().child(ui::field(
                        model,
                        move || model.get().word(Msg::EnforcementLabel).into(),
                        ui::radio_group(model, move || {
                            model.get().word(Msg::EnforcementLabel).into()
                        })
                        .option(
                            model.get().word(Msg::EnforcementAdvisory),
                            is(precept_ui_core::operator::Enforcement::Advisory),
                            pick(precept_ui_core::operator::Enforcement::Advisory),
                        )
                        .option(
                            model.get().word(Msg::EnforcementVerify),
                            is(precept_ui_core::operator::Enforcement::Verify),
                            pick(precept_ui_core::operator::Enforcement::Verify),
                        )
                        .option(
                            model.get().word(Msg::EnforcementHard),
                            is(precept_ui_core::operator::Enforcement::Hard),
                            pick(precept_ui_core::operator::Enforcement::Hard),
                        ),
                    )),
                ),
        )
        .child(
            text(move || model.get().word(Msg::EnforcementHelp))
                .text_size(px(ui::scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
        .into_element()
}
fn add_dialog(model: Signal<Model>) -> impl IntoElement {
    let ready = move || {
        let o = model.get().operator;
        !o.key.trim().is_empty()
            && !o.value.trim().is_empty()
            && precept_ui_core::operator::canonical_tags(&o.tags).is_some()
    };
    let tags_ref = element_ref();
    focus_shortcut(model, "review.field.tags", Pane::Preferences, tags_ref);
    ui::dialog(model, "")
        .size(ui::DialogSize::Large)
        .icon(icons::PLUS, ui::DialogTone::Accent)
        .title_fn(move || {
            let m = model.get();
            if let Some(id) = m.operator.revising {
                return m
                    .word(if m.operator.revising_confirmed {
                        Msg::EditRuleTitle
                    } else {
                        Msg::EditSuggestionTitle
                    })
                    .replace("{id}", &id.to_string())
                    .into();
            }
            m.word(if m.operator.global {
                Msg::AddPersonalPreference
            } else {
                Msg::AddProjectConvention
            })
            .into()
        })
        .description_fn(move || {
            let m = model.get();
            match m.operator.revising {
                Some(id) => m
                    .word(if m.operator.revising_confirmed {
                        Msg::EditRuleHelp
                    } else {
                        Msg::EditSuggestionHelp
                    })
                    .replace("{id}", &id.to_string())
                    .into(),
                None => m.word(Msg::AddPreferenceHelp).into(),
            }
        })
        .content_fn(move || {
            view()
                .w_full()
                .gap_4()
                .child(field(
                    model,
                    Msg::OperatorKey,
                    |o| &o.key,
                    |o, v| o.key = v,
                    120,
                ))
                .child(ui::field(
                    model,
                    move || model.get().word(Msg::OperatorValue).into(),
                    ui::markdown_input(
                        model,
                        "preference-value",
                        move || model.get().word(Msg::OperatorValue).into(),
                        move || model.get().operator.value.clone(),
                        move |v| model.update(|m| m.operator.value = v),
                    ),
                ))
                .child(
                    ui::tag_editor(
                        model,
                        move || model.get().operator.tags,
                        move || model.get().operator.tag_draft,
                    )
                    .element_ref(&tags_ref)
                    .suggestions(move || {
                        let m = model.get();
                        m.tag_suggestions(&m.operator.tags, &m.operator.tag_draft)
                    })
                    .on_draft(move |v| {
                        let v: String = v.chars().take(64).collect();
                        model.update(|m| m.operator.tag_draft = v)
                    })
                    .on_add(move |v| {
                        let mut taken = false;
                        model.update(|m| {
                            taken = m.operator.add_tag(v);
                            m.operator.review = None;
                            m.operator.serial += 1;
                        });
                        taken
                    })
                    .on_remove(move |v| {
                        model.update(|m| {
                            m.operator.remove_tag(v);
                            m.operator.review = None;
                            m.operator.serial += 1;
                        })
                    })
                    .disabled(move || {
                        model.get().operator.busy || model.get().operator.outcome_unknown
                    }),
                )
                .child(
                    view()
                        .style_when(
                            move || model.get().operator.revising.is_some(),
                            |s| s.hidden(),
                        )
                        .child(ui::field(
                            model,
                            move || model.get().word(Msg::OriginQuestion).into(),
                            ui::radio_group(model, move || {
                                model.get().word(Msg::OriginQuestion).into()
                            })
                            .option(
                                model.get().word(Msg::OriginOperator),
                                move || model.get().operator.origin == Origin::Operator,
                                move || model.update(|m| m.operator.origin = Origin::Operator),
                            )
                            .option(
                                model.get().word(Msg::OriginAgent),
                                move || model.get().operator.origin == Origin::Agent,
                                move || model.update(|m| m.operator.origin = Origin::Agent),
                            ),
                        )),
                )
                .child(rule_options(model))
                .into_element()
        })
        .cancel_fn(move || model.get().word(Msg::Cancel).into())
        .confirm_fn(move || {
            let m = model.get();
            m.word(if m.operator.revising_confirmed {
                Msg::SaveRuleEdit
            } else if m.operator.revising.is_some() {
                Msg::SaveEdit
            } else {
                Msg::SaveAsSuggestion
            })
            .into()
        })
        .confirm_disabled(move || !ready() || !available(model) || !write_grant(model))
        .open(move || {
            let m = model.get();
            (m.operator.adding || m.operator.revising.is_some()) && m.operator.open
        })
        .on_cancel(move || {
            model.update(|m| {
                m.operator.adding = false;
                m.operator.revising = None;
            });
        })
        .on_confirm(move || {
            model.update(|m| m.operator.adding = false);
            if model.get().operator.revising.is_some() {
                review(model, "revise");
            } else {
                review(model, "suggest");
            }
        })
}

/// Reads the selected project's AGENTS.md for sections to import (a read; the
/// dialog opens at once and fills in).
fn open_import(model: Signal<Model>) {
    let Some(project) = model.get().selected_project.clone() else {
        return;
    };
    model.update(|m| {
        m.operator.import = Some(precept_ui_core::operator::ImportScan {
            project: String::new(),
            candidates: Vec::new(),
            skipped: Vec::new(),
        });
        m.operator.import_picked.clear();
        m.operator.error = None;
    });
    super::profile::call(
        model,
        "import.scan",
        serde_json::json!({ "project": project }),
        move |_, m, r| {
            if m.operator.import.is_none() {
                return; // Closed meanwhile.
            }
            match r
                .ok()
                .and_then(|v| precept_ui_core::operator::ImportScan::parse(&project, v))
            {
                Some((scan, picked)) => {
                    m.operator.import = Some(scan);
                    m.operator.import_picked = picked;
                }
                None => {
                    m.operator.import = None;
                    m.operator.error = Some("Unavailable".into());
                }
            }
        },
    );
}
fn import_reason(m: &Model, reason: &str) -> String {
    m.word(match reason {
        "too_long" => Msg::ImportReasonTooLong,
        "empty" => Msg::ImportReasonEmpty,
        _ => Msg::ImportReasonUnreadable,
    })
    .into()
}
/// One found rule: its checkbox, where it came from, its key and scope, and
/// the first line of its text.
fn import_row(
    model: Signal<Model>,
    index: usize,
    c: &precept_ui_core::operator::ImportCandidate,
) -> AnyElement {
    let m = model.get_untracked();
    let key = c.key.clone();
    let exists = c.exists;
    let first: String = c
        .value
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or_default()
        .chars()
        .take(100)
        .collect();
    let scope = m.word(if matches!(c.scope, Scope::Global) {
        Msg::ImportPersonal
    } else {
        Msg::ImportProject
    });
    view()
        .flex_row()
        .items_start()
        .gap_2()
        .py_1p5()
        .style_when(move || index > 0, |s| s.border_t_1())
        .border_color(move || palette(model).hairline)
        .child(ui::checkbox(
            model,
            move || {
                model
                    .get()
                    .word(Msg::ImportPick)
                    .replace("{key}", &key)
                    .into()
            },
            move || model.get().operator.import_picked.contains(&index),
            move || exists || model.get().operator.busy,
            move |on| {
                model.update(|m| {
                    let picked = &mut m.operator.import_picked;
                    if on
                        && !picked.contains(&index)
                        && picked.len() < precept_ui_core::operator::MAX_BATCH_UI
                    {
                        picked.push(index);
                        picked.sort_unstable();
                    } else if !on {
                        picked.retain(|i| *i != index);
                    }
                    m.operator.review = None;
                })
            },
        ))
        .child(
            view()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(text(sanitize(&c.key, 120)).font_weight(FontWeight::MEDIUM))
                        .child(ui::badge(model, scope).variant(ui::BadgeVariant::Neutral))
                        .child(if exists {
                            ui::badge(model, m.word(Msg::ImportExists))
                                .variant(ui::BadgeVariant::Success)
                                .into_element()
                        } else {
                            view().into_element()
                        }),
                )
                .child(
                    text(format!(
                        "{} · {}",
                        sanitize(&c.source, 120),
                        sanitize(&c.title, 120)
                    ))
                    .text_size(px(ui::scale::text::CAPTION))
                    .text_color(move || palette(model).muted),
                )
                .child(
                    text(sanitize(&first, 100))
                        .text_size(px(ui::scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                ),
        )
        .into_element()
}
/// What the project's files hold, to pick from; saving prepares one write
/// that files the picks as suggestions.
fn import_dialog(model: Signal<Model>) -> impl IntoElement {
    let count = move || model.get().operator.import_picked.len();
    ui::dialog(model, "")
        .size(ui::DialogSize::Large)
        .icon(icons::FILE, ui::DialogTone::Accent)
        .title_fn(move || {
            let m = model.get();
            m.word(Msg::ImportTitle)
                .replace(
                    "{name}",
                    &sanitize(m.selected_project.as_deref().unwrap_or(""), 64),
                )
                .into()
        })
        .description_fn(move || model.get().word(Msg::ImportHelp).into())
        .content_fn(move || {
            view()
                .w_full()
                .gap_2()
                .child(dynamic(move || {
                    let m = model.get();
                    let Some(scan) = &m.operator.import else {
                        return view().into_element();
                    };
                    if scan.project.is_empty() {
                        return text(m.word(Msg::ImportScanning))
                            .text_color(move || palette(model).muted)
                            .into_element();
                    }
                    let mut list = view();
                    if scan.candidates.is_empty() {
                        list = list.child(ui::empty(model, m.word(Msg::ImportNone)));
                    }
                    for (i, c) in scan.candidates.iter().enumerate() {
                        list = list.child(import_row(model, i, c));
                    }
                    for s in scan.skipped.iter().take(20) {
                        list = list.child(
                            text(
                                m.word(Msg::ImportSkippedLine)
                                    .replace("{source}", &sanitize(&s.source, 120))
                                    .replace("{title}", &sanitize(&s.title, 120))
                                    .replace("{reason}", &import_reason(&m, &s.reason)),
                            )
                            .text_size(px(ui::scale::text::CAPTION))
                            .text_color(move || palette(model).warning),
                        );
                    }
                    scroll().max_h(px(420.0)).child(list).into_element()
                }))
                .into_element()
        })
        .cancel_fn(move || model.get().word(Msg::Cancel).into())
        .confirm_fn(move || {
            model
                .get()
                .word(Msg::ImportSubmit)
                .replace("{n}", &count().to_string())
                .into()
        })
        .confirm_disabled(move || count() == 0 || !available(model) || !write_grant(model))
        .open(move || {
            let m = model.get();
            m.operator.open && m.operator.import.is_some() && m.operator.review.is_none()
        })
        .on_cancel(move || {
            model.update(|m| {
                m.operator.import = None;
                m.operator.import_picked.clear();
            })
        })
        .on_confirm(move || review(model, "import"))
}

/// A write whose reply was lost: the outcome is unknown until the identical
/// request is retried (or abandoned). Inline, not a toast: it needs a decision.
fn unknown_banner(model: Signal<Model>) -> impl IntoElement {
    view()
        .gap_2()
        .style_when(
            move || !model.get().operator.outcome_unknown,
            |s| s.hidden(),
        )
        .child(
            text(move || model.get().word(Msg::OperatorUnknown))
                .text_color(move || palette(model).warning),
        )
        .child(
            view()
                .flex_row()
                .gap_2()
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
                )),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsh_engine::{EngineConfig, Root};
    use precept_ui_core::operator::{
        Binding, PeerDemotion, PreferencePage, PreferenceRecord, ProfileInfo, ReviewInspect,
        ReviewQueue, digest_hex, proposal_digest,
    };
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
            rule: Default::default(),
            enabled: true,
            id,
            scope: Scope::Global,
            key: key.clone(),
            value: "synthetic-value".into(),
            tags: vec!["synthetic".into()],
            status: status.into(),
            revision,
            origin: Origin::Agent,
            proposal_digest: digest_hex(&proposal_digest(
                &Scope::Global,
                None,
                &key,
                "synthetic-value",
                Origin::Agent,
                &["synthetic".to_owned()],
                &Default::default(),
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
            ..Default::default()
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
        m.operator.mode = ReviewMode::Personal;
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
        m.operator.inspect.as_mut().unwrap().record.value =
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
    fn a_selected_record_opens_one_sheet_with_detail_and_history() {
        let mut app = start::<QueueConsole>();
        // Detail and history together in the record's sheet (no tabs).
        app.wait_for("Show full digest").unwrap();
        app.wait_for("No decisions recorded for this record")
            .unwrap();
        app.press("Close detail").unwrap();
        app.wait_for_gone("Show full digest", Duration::from_secs(5))
            .unwrap();
        // The list stays; the row reopens it with a fresh exact read (no
        // daemon in this fixture, so that read is covered by the real-process
        // tests).
        app.wait_for("#7 · synthetic-key-7 · r1").unwrap();
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
        app.wait_for("Nothing is written until you apply it")
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
        assert!(!app.screen().shows("Nothing is written until you apply it"));
        // The exact id arms the tier; Enter prepares the delete (no submit here).
        app.type_text("Retype the exact ID", "7").unwrap();
        app.wait_for("Deletion armed; apply to commit").unwrap();
        app.press_key("Enter").unwrap();
        app.wait_for("Nothing is written until you apply it")
            .unwrap();
        assert!(app.screen().shows(
            "This permanently removes the record's content and its decision history. Receipts remain as identity-only proof; the database file is not securely erased."
        ));
        // Cancelling the prepared delete disarms; a fresh tier refuses a wrong id.
        app.press("Cancel").unwrap();
        app.wait_for("#7 · synthetic-key-7 · r1").unwrap();
        app.press_key("alt+shift+q").unwrap();
        app.wait_for("Delete global preference #7?").unwrap();
        app.type_text("Retype the exact ID", "8").unwrap();
        app.wait_for("The typed ID does not match").unwrap();
        app.press_key("Enter").unwrap();
        assert!(!app.screen().shows("Nothing is written until you apply it"));
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
        assert!(!app.screen().shows("Nothing is written until you apply it"));
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
        app.wait_for("Personal · all projects").unwrap();
        app.wait_for("Show full digest").unwrap();
        app.press_key("mod+l").unwrap();
        app.wait_for("个人 · 所有项目").unwrap();
        app.wait_for("显示完整摘要").unwrap();
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
        assert!(!app.screen().shows("Nothing is written until you apply it"));
        app.press_key("alt+shift+q").unwrap();
        app.wait_for("Delete global preference #7?").unwrap();
        assert_eq!(
            app.screen().input_text("Retype the exact ID").unwrap(),
            Some("")
        );
        app.press_key("Enter").unwrap();
        assert!(!app.screen().shows("Nothing is written until you apply it"));
    }

    #[test]
    fn unloaded_queue_does_not_claim_no_pending_suggestions() {
        let mut app = start::<UnobservedQueueConsole>();
        app.wait_for("Loading pending suggestions…").unwrap();
        assert!(!app.screen().shows("No pending suggestions"));
    }

    #[test]
    fn empty_later_queue_page_still_discloses_drift() {
        let mut app = start::<DriftedEmptyQueueConsole>();
        app.wait_for("No pending suggestions").unwrap();
        app.wait_for("The Profile changed 3 time(s) since this list loaded; refresh to see it.")
            .unwrap();
        // Implementation detail stays out of the page.
        assert!(!app.screen().texts().iter().any(|t| t.contains("watermark")));
        app.press_key("mod+l").unwrap();
        app.wait_for("此列表加载后 Profile 又变化了 3 次；请刷新查看。")
            .unwrap();
    }

    #[test]
    fn history_never_renders_the_previous_selection() {
        let mut app = start::<SwitchedHistoryConsole>();
        app.wait_for("#7 · synthetic-key-7 · r1").unwrap();
        // Selection 8 is not the inspected record: no sheet, and never #7's
        // detail or history in its place.
        assert!(!app.screen().shows("No decisions recorded for this record"));
        assert!(!app.screen().shows("Show full digest"));
    }

    #[test]
    fn exact_detail_preserves_value_characters_and_explains_authority() {
        let mut app = start::<LiteralSourceConsole>();
        app.wait_for("Show full digest").unwrap();
        app.wait_until("escaped source tail", Duration::from_secs(5), |s| {
            s.texts()
                .iter()
                .any(|t| t.contains("\\n\\u{202e}source-tail"))
        })
        .unwrap();
        // The authority notes sit behind the record's info button.
        assert!(
            app.screen()
                .count("role:button[About this record]")
                .unwrap()
                > 0
        );
        app.wait_for("synthetic").unwrap();
        app.wait_until("origin named", Duration::from_secs(5), |s| {
            s.texts().iter().any(|t| t.ends_with("· Agent"))
        })
        .unwrap();
        app.press_key("mod+l").unwrap();
        app.wait_until("localized revision", Duration::from_secs(5), |s| {
            s.texts().iter().any(|t| t.contains("修订 1"))
        })
        .unwrap();
    }

    #[test]
    fn focus_shortcut_disarms_delete_before_apply() {
        let mut app = start::<QueueConsole>();
        app.wait_for("#7 · synthetic-key-7 · r1").unwrap();
        app.press_key("alt+shift+q").unwrap();
        app.wait_for("Delete global preference #7?").unwrap();
        app.type_text("Retype the exact ID", "7").unwrap();
        app.wait_for("Deletion armed; apply to commit").unwrap();
        app.press_key("Escape").unwrap();
        app.wait_for_gone("Delete global preference #7?", Duration::from_secs(5))
            .unwrap();
        app.press_key("alt+shift+v").unwrap();
        assert!(!app.screen().shows("Nothing is written until you apply it"));
        app.press_key("alt+shift+q").unwrap();
        assert!(!app.screen().shows("Nothing is written until you apply it"));
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
        app.wait_for("Nothing is written until you apply it")
            .unwrap();
        assert!(!app.screen().texts().join("").contains(&full));
        app.press("Cancel").unwrap();
        app.wait_for_gone(
            "Nothing is written until you apply it",
            Duration::from_secs(5),
        )
        .unwrap();
        app.press("Show full digest").unwrap();
        app.press_key("alt+shift+y").unwrap();
        app.wait_for("Nothing is written until you apply it")
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
        assert!(!app.screen().shows("Check before writing"));
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
        app.wait_for("Nothing is written until you apply it")
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
        assert!(!app.screen().shows("Nothing is written until you apply it"));
        app.press_key("alt+shift+v").unwrap();
        assert!(!app.screen().shows("Nothing is written until you apply it"));
        app.press_key("Escape").unwrap();
        app.wait_for_gone("Delete global preference #7?", Duration::from_secs(5))
            .unwrap();
    }
}
