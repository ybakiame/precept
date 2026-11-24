use super::detail::fetch_update_of;
use super::*;

pub(super) fn available(model: Signal<Model>) -> bool {
    let m = model.get();
    m.operator.open
        && matches!(m.connection.target, Target::Local | Target::Wsl)
        && m.connection.validate()
        && !m.operator.busy
        && !m.operator.outcome_unknown
}
pub(super) fn write_grant(model: Signal<Model>) -> bool {
    model
        .get()
        .operator
        .info
        .as_ref()
        .is_some_and(|i| i.write_enabled)
}
pub(super) fn status_is(model: Signal<Model>, status: &str) -> bool {
    model
        .get()
        .operator
        .record()
        .is_some_and(|r| r.status == status)
}
pub(super) fn refresh(model: Signal<Model>, method: &str) {
    let mut request = None;
    model.update(|m| request = m.operator_read(method));
    if let Some(request) = request {
        send(model, request);
    }
}
pub(super) fn send(model: Signal<Model>, request: Review) {
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
pub(super) fn ensure_inspected(model: Signal<Model>) -> bool {
    let exact = model.with(Model::operator_inspected);
    if !exact {
        model.update(|m| m.operator.review_tab = ReviewTab::Detail);
        refresh(model, "profile.review.inspect");
    }
    exact
}
pub(super) fn review(model: Signal<Model>, action: &str) {
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
pub(super) fn submit(model: Signal<Model>) {
    let mut request = None;
    model.update(|m| request = m.operator_confirm());
    if let Some(request) = request {
        send(model, request);
    }
}
/// The delete tier is armed only against the exact reviewed id+revision+digest
/// with the id retyped verbatim; mirrors the shared model's submit-time guard.
pub(super) fn delete_armed(model: Signal<Model>) -> bool {
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
pub(super) fn retype_mismatch(model: Signal<Model>) -> bool {
    let m = model.get();
    m.operator
        .delete_arm
        .as_ref()
        .zip(m.operator.record())
        .is_some_and(|(a, r)| !a.retype.is_empty() && a.retype != r.id.to_string())
}
/// Open the destructive tier for the exact selected record (never auto-armed).
pub(super) fn open_delete(model: Signal<Model>, delete_open: Signal<bool>) {
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
pub(super) fn disarm(model: Signal<Model>, delete_open: Signal<bool>) {
    delete_open.set(false);
    model.update(|m| m.operator.delete_arm = None);
}
/// Prepare the destructive delete only when exactly armed; the shared model
/// rechecks the arm again at submit. A blind activation prepares nothing.
pub(super) fn commit_delete(model: Signal<Model>, delete_open: Signal<bool>) {
    if delete_open.get() && delete_armed(model) && available(model) && write_grant(model) {
        review(model, "delete");
        delete_open.set(false);
    }
}
pub(super) fn error_msg(code: &str, wsl: bool) -> Msg {
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
