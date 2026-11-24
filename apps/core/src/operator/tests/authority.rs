use super::*;

#[test]
fn no_implicit_grant_or_cross_target_writes() {
    let mut m = model();
    let local = m.connection.clone();
    // Synthetic data and an incomplete WSL connection never get Operator access.
    for target in [Target::Synthetic, Target::Wsl] {
        m.connection.target = target;
        assert!(!m.operator_review("confirm", "synthetic-op"));
        assert!(m.operator_read("profile.info").is_none());
    }
    // An explicit, valid WSL connection gets the same guarded Operator surface.
    m.connection = crate::connection::Connection {
        target: Target::Wsl,
        distribution: "Synthetic Linux".into(),
        user: "operator".into(),
        helper: "/synthetic/precept-bridge".into(),
        wsl_runtime: "/synthetic/runtime".into(),
        wsl_endpoint: "/synthetic/runtime/control.sock".into(),
        ..Default::default()
    };
    assert!(m.operator_guard().is_some());
    assert!(m.operator_read("profile.info").is_some());
    m.operator.busy = false;
    m.connection = local;
    m.operator.info.as_mut().unwrap().write_enabled = false;
    assert!(!m.operator_review("confirm", "synthetic-op"));
    assert!(m.operator_review("enable", "synthetic-op"));
    assert!(!m.operator.info.as_ref().unwrap().write_enabled);
    let request = m.operator_confirm().unwrap();
    assert_eq!(request.method, "profile.write.enable");
    assert!(request.mutation.is_none());
}
#[test]
fn an_unsubmitted_retry_keeps_the_uncertainty_and_the_exact_request() {
    let mut m = model();
    assert!(m.operator_review("confirm", "kept-op"));
    let request = m.operator_confirm().unwrap();
    assert!(m.complete_operator(&request, Err("OutcomeUnknown")));
    // The retry could not be submitted (lost WSL bridge, stopped daemon).
    let retry = m.operator_retry().unwrap();
    assert!(m.complete_operator(&retry, Err("Unavailable")));
    assert!(m.operator.outcome_unknown);
    assert!(m.operator.pending.as_ref() == Some(&request));
    // A later identical retry is still possible and settles it once.
    let again = m.operator_retry().unwrap();
    assert_eq!(
        serde_json::to_vec(&again.mutation).unwrap(),
        serde_json::to_vec(&request.mutation).unwrap()
    );
    let receipt = json!({"generation":"b".repeat(32),"operation_id":"kept-op","commit_sequence":2,"affected":[]});
    assert!(m.complete_operator(&again, Ok(receipt)));
    assert!(!m.operator.outcome_unknown && m.operator.pending.is_none());
    // A fresh mutation that was never submitted is not uncertain.
    let mut fresh = model();
    assert!(fresh.operator_review("confirm", "fresh-op"));
    let first = fresh.operator_confirm().unwrap();
    assert!(fresh.complete_operator(&first, Err("Unavailable")));
    assert!(!fresh.operator.outcome_unknown && fresh.operator.pending.is_none());
}
#[test]
fn exact_review_revision_and_same_request_only_uncertain_retry() {
    let mut m = model();
    assert!(m.operator_review("confirm", "keep-this-id"));
    let request = m.operator_confirm().unwrap();
    assert!(matches!(
        request.mutation.as_ref().unwrap().command,
        Command::ConfirmPreference {
            id: 1,
            expected: Expected::Revision(1),
            ..
        }
    ));
    assert!(m.operator_confirm().is_none());
    assert!(m.complete_operator(&request, Err("OutcomeUnknown")));
    assert!(m.operator.info.is_none());
    assert!(!m.operator_review("confirm", "new-id"));
    assert!(m.operator_read("profile.info").is_none());
    let retry = m.operator_retry().unwrap();
    assert_eq!(
        serde_json::to_vec(&request.mutation).unwrap(),
        serde_json::to_vec(&retry.mutation).unwrap()
    );
    let receipt = json!({"generation":"b".repeat(32),"operation_id":"keep-this-id","commit_sequence":2,"affected":[]});
    assert!(m.complete_operator(&retry, Ok(receipt)));
    assert!(!m.operator.outcome_unknown);
    assert!(m.operator.pending.is_none());
    assert_eq!(
        m.operator.receipt.as_ref().unwrap().operation_id,
        "keep-this-id"
    );
}
#[test]
fn scope_changes_reject_stale_success_failure_review_and_retry() {
    for change in 0..5 {
        let mut m = model();
        assert!(m.operator_review("confirm", "stable"));
        let request = m.operator_confirm().unwrap();
        assert!(m.complete_operator(&request, Err("OutcomeUnknown")));
        match change {
            0 => m.epoch += 1,
            1 => m.connection.endpoint = "different".into(),
            2 => m.selected_project = Some("beta".into()),
            3 => m.operator.task = "different-task".into(),
            _ => {
                m.operator.serial += 1;
                m.operator.selected_id = None;
            }
        }
        assert!(m.operator_retry().is_none());
        assert!(!m.complete_operator(&request, Err("Unauthorized")));
        assert!(!m.complete_operator(&request, Ok(json!({}))));
        assert_eq!(
            m.operator
                .pending
                .as_ref()
                .unwrap()
                .mutation
                .as_ref()
                .unwrap()
                .operation_id,
            "stable"
        );
    }
}
#[test]
fn malformed_preview_and_wrong_scope_fail_closed() {
    let mut m = model();
    let r = m.operator_read("profile.preferences.list").unwrap();
    let mut page = m.operator.page.clone().unwrap();
    page.records[0].scope = Scope::Project {
        name: "beta".into(),
    };
    assert!(m.complete_operator(&r, Ok(serde_json::to_value(page).unwrap())));
    assert!(m.operator.page.is_none());
    assert!(m.operator.info.is_none());
    assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
}
#[test]
fn disable_ack_and_explicit_denial_never_report_success() {
    let mut m = model();
    assert!(m.operator_review("disable", "unused"));
    let r = m.operator_confirm().unwrap();
    assert!(
        m.operator.info.as_ref().unwrap().write_enabled,
        "not disabled until ack"
    );
    assert!(m.complete_operator(&r, Ok(json!({"write_enabled":false}))));
    assert!(!m.operator.info.as_ref().unwrap().write_enabled);
    for error in [
        "Unauthorized",
        "Busy",
        "Conflict",
        "StaleGeneration",
        "RestartRequired",
        "Unavailable",
    ] {
        let mut m = model();
        assert!(m.operator_review("confirm", "once"));
        let r = m.operator_confirm().unwrap();
        assert!(m.complete_operator(&r, Err(error)));
        assert!(m.operator.receipt.is_none());
        assert!(m.operator.info.is_none());
        assert!(!m.operator.busy);
        assert_eq!(m.operator.error.as_deref(), Some(error));
        assert!(!m.operator.outcome_unknown);
    }
}
#[test]
fn changed_generation_or_review_state_cannot_be_confirmed() {
    let mut m = model();
    m.operator.inspect.as_mut().unwrap().binding.owner_epoch = 2;
    assert!(!m.operator_review("confirm", "once"));
    m.operator.inspect.as_mut().unwrap().binding.owner_epoch = 1;
    m.operator.inspect.as_mut().unwrap().record.status = "confirmed".into();
    assert!(!m.operator_review("confirm", "once"));
    assert!(!m.operator_review("reject", "once"));
}

#[test]
fn set_up_profile_reads_never_fence_an_in_flight_workspace_request() {
    let mut m = model();
    m.operator.busy = false;
    let read = m.operator_read("profile.info").unwrap();
    // The Set-up pages record their own Profile state while it is in flight.
    *m.profile_info = m.operator.info.clone();
    m.profile_info.as_mut().unwrap().write_enabled =
        !m.operator.info.as_ref().unwrap().write_enabled;
    assert!(
        m.operator_current(&read),
        "the workspace reply still settles"
    );
    // Whereas changing the workspace's own observation does fence it.
    m.operator.info.as_mut().unwrap().write_enabled ^= true;
    assert!(!m.operator_current(&read));
}
