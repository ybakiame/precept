use super::*;

#[test]
fn deprecate_requires_confirmed_exact_identity() {
    let mut m = model();
    assert!(!m.operator_review("deprecate", "op"));
    m.operator.inspect.as_mut().unwrap().record.status = "confirmed".into();
    m.operator.inspect.as_mut().unwrap().record.revision = 2;
    m.operator.inspect_revision = 2;
    m.operator.channel = Channel::Desktop;
    let record = m.operator.record().unwrap().clone();
    assert!(complete_inspect(
        &mut m,
        inspect_of(record, vec![], vec![], 0)
    ));
    assert!(m.operator_review("deprecate", "dep-op"));
    let request = m.operator_confirm().unwrap();
    let digest = m.operator.record().unwrap().proposal_digest.clone();
    match &request.mutation.as_ref().unwrap().command {
        Command::DeprecatePreference {
            id,
            scope,
            proposal_digest,
            channel,
            expected,
            note,
        } => {
            assert_eq!(*note, None);
            assert_eq!(*id, 1);
            assert!(*scope == Scope::Global);
            assert_eq!(*proposal_digest, digest);
            assert_eq!(*channel, Channel::Desktop);
            assert!(*expected == Expected::Revision(2));
        }
        _ => panic!("expected DeprecatePreference"),
    }
    assert!(m.complete_operator(
        &request,
        Ok(receipt_json(
            "dep-op",
            3,
            json!([{"kind":"preference","project":"","record_id":"1","revision":3,"deleted":false}])
        ))
    ));
    assert!(m.operator.receipt.is_some());
    assert!(m.operator.page.is_none());
    // A suggested queue row can never be deprecated.
    let mut m = inspect_model();
    assert!(!m.operator_review("deprecate", "op"));
    // Digest or revision drift between prepare and apply refuses the submit.
    for drift in 0..2 {
        let mut m = model();
        m.operator.inspect.as_mut().unwrap().record.status = "confirmed".into();
        let record = m.operator.record().unwrap().clone();
        assert!(complete_inspect(
            &mut m,
            inspect_of(record, vec![], vec![], 0)
        ));
        assert!(m.operator_review("deprecate", "drift"));
        let record = &mut m.operator.inspect.as_mut().unwrap().record;
        if drift == 0 {
            record.proposal_digest = "d".repeat(64);
        } else {
            record.revision += 1;
        }
        assert!(m.operator_confirm().is_none());
    }
}
#[test]
fn delete_requires_exact_retype_and_armed_identity() {
    let mut m = armed_delete_model();
    let digest = m.operator.record().unwrap().proposal_digest.clone();
    // Unarmed delete never prepares.
    m.operator.delete_arm = None;
    assert!(!m.operator_review("delete", "op"));
    // Wrong, padded or stale retype text never arms the destructive tier.
    for retype in ["", "6", "07", " 7", "7 ", "seven"] {
        m.operator.delete_arm = Some(DeleteArm {
            retype: retype.into(),
            id: 7,
            revision: 1,
            proposal_digest: digest.clone(),
        });
        assert!(!m.operator_review("delete", "op"), "retype {retype:?}");
    }
    // A stale armed revision or digest refuses even with the exact text.
    m.operator.delete_arm = Some(DeleteArm {
        retype: "7".into(),
        id: 7,
        revision: 2,
        proposal_digest: digest.clone(),
    });
    assert!(!m.operator_review("delete", "op"));
    m.operator.delete_arm = Some(DeleteArm {
        retype: "7".into(),
        id: 7,
        revision: 1,
        proposal_digest: "d".repeat(64),
    });
    assert!(!m.operator_review("delete", "op"));
    // The exact arm prepares the destructive command on the reviewed tuple.
    m.operator.delete_arm = Some(DeleteArm {
        retype: "7".into(),
        id: 7,
        revision: 1,
        proposal_digest: digest.clone(),
    });
    assert!(m.operator_review("delete", "del-op"));
    let request = m.operator_confirm().unwrap();
    assert!(matches!(
        request.mutation.as_ref().unwrap().command,
        Command::DeletePreference {
            id: 7,
            expected: Expected::Revision(1),
            ..
        }
    ));
    // A cleared or edited arm between prepare and apply refuses the submit.
    for clear in 0..2 {
        let mut m = armed_delete_model();
        assert!(m.operator_review("delete", "del-op"));
        if clear == 0 {
            m.operator.delete_arm = None;
        } else {
            m.operator.delete_arm.as_mut().unwrap().retype = "8".into();
        }
        assert!(m.operator_confirm().is_none());
    }
}
#[test]
fn delete_receipt_tombstone_clears_content_and_history() {
    let mut m = armed_delete_model();
    assert!(m.operator_review("delete", "del-op"));
    let request = m.operator_confirm().unwrap();
    let tombstone =
        json!([{"kind":"preference","project":"","record_id":"7","revision":2,"deleted":true}]);
    // A receipt without the identity-only tombstone is not a delete proof.
    assert!(m.complete_operator(&request, Ok(receipt_json("del-op", 7, json!([])))));
    assert!(m.operator.outcome_unknown);
    assert!(m.operator.receipt.is_none());
    // The identical same-epoch retry settles once with the real tombstone.
    let retry = m.operator_retry().unwrap();
    assert!(m.complete_operator(&retry, Ok(receipt_json("del-op", 7, tombstone))));
    assert!(!m.operator.outcome_unknown);
    assert_eq!(m.operator.receipt.as_ref().unwrap().operation_id, "del-op");
    // Content and decision-history snapshots never survive the deletion.
    assert!(m.operator.queue.is_none());
    assert!(m.operator.inspect.is_none());
    assert!(m.operator.page.is_none());
    assert!(m.operator.selected_id.is_none());
    assert!(m.operator.delete_arm.is_none());
    assert_eq!(m.operator.inspect_revision, 0);
    // A duplicate settlement of the same delete is refused.
    assert!(!m.complete_operator(&retry, Ok(receipt_json("del-op", 7, json!([])))));
}

#[test]
fn delete_tombstone_must_match_preference_identity_and_resulting_revision() {
    for (project, revision) in [("other-project", 2), ("", 1), ("", 3)] {
        let mut m = armed_delete_model();
        assert!(m.operator_review("delete", "exact-delete"));
        let request = m.operator_confirm().unwrap();
        assert!(m.complete_operator(
            &request,
            Ok(receipt_json(
                "exact-delete",
                9,
                json!([{
                    "kind":"preference", "project":project, "record_id":"7",
                    "revision":revision, "deleted":true
                }])
            ))
        ));
        assert!(m.operator.outcome_unknown);
        assert!(m.operator.receipt.is_none());
        let replay = m.operator_retry().unwrap();
        assert_eq!(
            serde_json::to_vec(&request.mutation).unwrap(),
            serde_json::to_vec(&replay.mutation).unwrap()
        );
        assert!(m.complete_operator(
            &replay,
            Ok(receipt_json(
                "exact-delete",
                9,
                json!([{
                    "kind":"preference", "project":"", "record_id":"7",
                    "revision":2, "deleted":true
                }])
            ))
        ));
        assert!(m.operator.receipt.is_some());
    }
}
#[test]
fn project_delete_accepts_writer_global_id_receipt_without_losing_request_scope() {
    let mut m = model();
    let scope = Scope::Project {
        name: "alpha".into(),
    };
    let record = synthetic_record(7, &scope, "suggested", 1);
    m.operator.global = false;
    m.operator.page = Some(PreferencePage {
        binding: binding(),
        records: vec![record.clone()],
        next_after_id: None,
    });
    m.operator.selected_id = Some(7);
    m.operator.inspect = None;
    m.operator.inspect_revision = 0;
    assert!(complete_inspect(
        &mut m,
        inspect_of(record.clone(), vec![], vec![], 0)
    ));
    m.operator.delete_arm = Some(DeleteArm {
        retype: "7".into(),
        id: 7,
        revision: 1,
        proposal_digest: record.proposal_digest.clone(),
    });
    assert!(m.operator_review("delete", "project-delete"));
    let request = m.operator_confirm().unwrap();
    match &request.mutation.as_ref().unwrap().command {
        Command::DeletePreference {
            id,
            scope: actual_scope,
            expected,
            proposal_digest,
            ..
        } => {
            assert_eq!(*id, 7);
            assert!(*actual_scope == scope);
            assert!(*expected == Expected::Revision(1));
            assert_eq!(proposal_digest, &record.proposal_digest);
        }
        _ => panic!("expected exact project delete"),
    }
    assert!(m.complete_operator(
        &request,
        Ok(receipt_json(
            "project-delete",
            9,
            json!([{
                "kind":"preference", "project":"", "record_id":"7",
                "revision":2, "deleted":true
            }])
        ))
    ));
    assert!(!m.operator.outcome_unknown);
    assert!(m.operator.receipt.is_some());
    assert!(m.operator.pending.is_none());
    assert!(m.operator.record().is_none());
    assert!(m.operator.inspect.is_none());
    assert!(m.operator.delete_arm.is_none());
    assert!(m.operator_retry().is_none());
}
#[test]
fn precise_review_refusals_are_settled_without_replay_or_success() {
    for error in [
        "ReviewMismatch",
        "Conflict",
        "InvalidTransition",
        "Unauthorized",
        "Busy",
        "Capacity",
        "Unavailable",
        "IdempotencyConflict",
    ] {
        let mut m = armed_delete_model();
        assert!(m.operator_review("confirm", "refused"));
        let request = m.operator_confirm().unwrap();
        assert!(m.complete_operator(&request, Err(error)));
        assert_eq!(m.operator.error.as_deref(), Some(error));
        assert!(m.operator.receipt.is_none());
        assert!(m.operator.pending.is_none());
        assert!(m.operator.review.is_none());
        assert!(m.operator.inspect.is_none());
        assert!(m.operator_retry().is_none());
    }
}

#[test]
fn decisions_require_current_exact_inspection_and_failed_prepare_cancels() {
    for action in ["confirm", "reject", "deprecate", "delete"] {
        let mut m = model();
        m.operator.inspect = None;
        m.operator.inspect_revision = 0;
        if action == "deprecate" {
            m.operator.page.as_mut().unwrap().records[0].status = "confirmed".into();
        }
        assert!(!m.operator_review(action, "uninspected"), "{action}");
        assert!(m.operator_confirm().is_none());
        let record = m.operator.record().unwrap().clone();
        assert!(complete_inspect(
            &mut m,
            inspect_of(record.clone(), vec![], vec![], 0)
        ));
        if action == "delete" {
            m.operator.delete_arm = Some(DeleteArm {
                retype: record.id.to_string(),
                id: record.id,
                revision: record.revision,
                proposal_digest: record.proposal_digest,
            });
        }
        assert!(m.operator_review(action, "inspected"), "{action}");
        assert!(m.operator_confirm().is_some(), "{action}");
    }
    let mut m = model();
    assert!(m.operator_review("confirm", "old-confirm"));
    assert!(!m.operator_review("deprecate", "invalid-action"));
    assert!(m.operator.review.is_none());
    assert!(m.operator_confirm().is_none());
    for change in 0..17 {
        let mut m = armed_delete_model();
        change_review_context(&mut m, change);
        assert!(
            !m.operator_review("delete", "obsolete-inspect"),
            "change {change}"
        );
        assert!(
            !m.operator_review("confirm", "obsolete-inspect"),
            "change {change}"
        );
    }
}
