use super::*;

#[test]
fn stale_identity_changes_fence_reads_and_armed_actions() {
    // In-flight queue reads are refused after any identity change.
    for change in 0..10 {
        let mut m = model();
        let request = m.operator_read("profile.review.queue").unwrap();
        match change {
            0 => m.epoch += 1,
            1 => m.connection.endpoint = "different".into(),
            2 => m.selected_project = Some("beta".into()),
            3 => m.operator.task = "different-task".into(),
            4 => m.operator.serial += 1,
            5 => m.operator.queue_after_id = 9,
            6 => m.operator.queue_watermark = 9,
            7 => m.operator.inspect_revision = 9,
            8 => m.operator.global = false,
            _ => m.operator.invalidate(),
        }
        assert!(!m.complete_operator(
            &request,
            Ok(serde_json::to_value(queue_of(vec![], 5, None)).unwrap())
        ));
        assert!(m.operator.queue.is_none());
    }
    // An armed delete cannot apply after scope, watermark, revision or
    // digest drift; a revoked write grant refuses every decision action.
    for change in 0..6 {
        let mut m = armed_delete_model();
        assert!(m.operator_review("delete", "fenced"));
        match change {
            0 => m.epoch += 1,
            1 => m.operator.global = false,
            2 => m.operator.queue_watermark = 99,
            3 => m.operator.inspect_revision = 99,
            4 => {
                let mut drifted = m.operator.inspect.clone().unwrap();
                drifted.record.proposal_digest = "d".repeat(64);
                m.operator.inspect = Some(drifted);
            }
            _ => m.operator.info.as_mut().unwrap().write_enabled = false,
        }
        assert!(m.operator_confirm().is_none(), "change {change}");
    }
    // A revoked grant refuses new review preparations outright.
    let mut m = armed_delete_model();
    m.operator.info.as_mut().unwrap().write_enabled = false;
    assert!(!m.operator_review("delete", "revoked"));
    assert!(!m.operator_review("confirm", "revoked"));
    assert!(!m.operator_review("deprecate", "revoked"));
}
#[test]
fn delayed_switch_duplicate_apply_and_cancel_refused() {
    // A project switch invalidates an in-flight queue read.
    let mut m = model();
    let request = m.operator_read("profile.review.queue").unwrap();
    m.select(1);
    assert!(!m.complete_operator(
        &request,
        Ok(serde_json::to_value(queue_of(vec![], 5, None)).unwrap())
    ));
    assert!(m.operator.queue.is_none());
    // An explicit cancel invalidates an in-flight inspect read.
    let mut m = inspect_model();
    let request = m.operator_read("profile.review.inspect").unwrap();
    m.operator.invalidate();
    assert!(
        !m.complete_operator(
            &request,
            Ok(serde_json::to_value(inspect_of(
                synthetic_record(7, &Scope::Global, "suggested", 1),
                vec![],
                vec![],
                0
            ))
            .unwrap())
        )
    );
    assert!(m.operator.inspect.is_none());
    // A settled read can never apply twice.
    let mut m = model();
    let request = m.operator_read("profile.review.queue").unwrap();
    let value = serde_json::to_value(queue_of(vec![], 5, None)).unwrap();
    assert!(m.complete_operator(&request, Ok(value.clone())));
    assert!(!m.complete_operator(&request, Ok(value)));
    // A settled mutation can never apply twice; the uncertain retry keeps
    // its identity and settles exactly once.
    let mut m = model();
    assert!(m.operator_review("confirm", "once"));
    let request = m.operator_confirm().unwrap();
    assert!(m.complete_operator(&request, Err("OutcomeUnknown")));
    let retry = m.operator_retry().unwrap();
    let receipt = receipt_json("once", 2, json!([]));
    assert!(m.complete_operator(&retry, Ok(receipt.clone())));
    assert!(!m.complete_operator(&retry, Ok(receipt.clone())));
    assert!(!m.complete_operator(&request, Ok(receipt)));
}

#[test]
fn profile_grant_selection_and_live_sources_fence_prepared_and_delayed_actions() {
    for change in 0..17 {
        let mut m = armed_delete_model();
        assert!(m.operator_review("delete", "stale"));
        change_review_context(&mut m, change);
        assert!(m.operator_confirm().is_none(), "prepared change {change}");
        assert!(m.operator.pending.is_none());

        let mut m = armed_delete_model();
        let request = m.operator_read("profile.review.inspect").unwrap();
        change_review_context(&mut m, change);
        assert!(
            !m.complete_operator(&request, Err("Conflict")),
            "late change {change}"
        );
        assert!(m.operator.error.is_none());
    }
    // Losing observed authority is not equivalent to a current write grant.
    let mut m = armed_delete_model();
    assert!(m.operator_review("delete", "no-info"));
    m.operator.info = None;
    assert!(m.operator_confirm().is_none());
}
#[test]
fn unknown_replay_never_crosses_observed_authority_or_source_changes() {
    for change in 0..17 {
        let mut m = armed_delete_model();
        assert!(m.operator_retry().is_none());
        assert!(m.operator_review("delete", "immutable-delete"));
        let request = m.operator_confirm().unwrap();
        let info = m.operator.info.clone();
        let bytes = serde_json::to_vec(&request.mutation).unwrap();
        assert!(m.complete_operator(&request, Err("OutcomeUnknown")));
        assert!(!m.operator.busy, "no automatic retry");
        assert_eq!(m.operator.selected_id, Some(7));
        m.operator.info = info;
        change_review_context(&mut m, change);
        assert!(m.operator_retry().is_none(), "replay change {change}");
        assert!(!m.complete_operator(&request, Ok(receipt_json("immutable-delete", 9, json!([])))));
        assert_eq!(
            serde_json::to_vec(&m.operator.pending.as_ref().unwrap().mutation).unwrap(),
            bytes,
            "request identity is retained, never rebound"
        );
    }
}
#[test]
fn refreshed_queue_drops_stale_inspect_content_and_delete_consent() {
    let mut m = armed_delete_model();
    assert!(m.operator_review("delete", "old-delete"));
    let request = m.operator_read("profile.review.queue").unwrap();
    assert!(m.operator.review.is_none());
    assert!(m.operator.delete_arm.is_none());
    let mut record = synthetic_record(7, &Scope::Global, "suggested", 2);
    record.tags = vec!["changed".into()];
    record.value = "new proposal".into();
    record.proposal_digest = "d".repeat(64);
    assert!(m.complete_operator(
        &request,
        Ok(serde_json::to_value(queue_of(vec![record.clone()], 9, None)).unwrap())
    ));
    assert!(m.operator.inspect.is_none());
    assert_eq!(m.operator.inspect_revision, 0);
    assert!(m.operator.record() == Some(&record));
    assert!(!m.operator_review("delete", "stale-consent"));
    assert!(complete_queue(&mut m, queue_of(vec![], 10, None)));
    assert!(m.operator.record().is_none());
    assert!(m.operator.selected_id.is_none());
}
#[test]
fn changed_immutable_content_tags_or_origin_refuses_prepared_decision() {
    for change in 0..5 {
        let mut m = armed_delete_model();
        assert!(m.operator_review("confirm", "exact-content"));
        let record = &mut m.operator.inspect.as_mut().unwrap().record;
        match change {
            0 => record.key = "different-key".into(),
            1 => record.value = "different-value".into(),
            2 => record.tags = vec!["different".into()],
            3 => record.origin = Origin::Operator,
            _ => record.status = "rejected".into(),
        }
        assert!(m.operator_confirm().is_none(), "content change {change}");
    }
}
#[test]
fn history_order_future_receipts_and_nonconsecutive_revisions_fail_closed() {
    for change in 0..4 {
        let mut m = inspect_model();
        let mut inspect = inspect_of(
            synthetic_record(7, &Scope::Global, "suggested", 1),
            vec![
                decision(7, DecisionAction::Demote, 8),
                decision(7, DecisionAction::Confirm, 6),
            ],
            vec![],
            0,
        );
        match change {
            0 => inspect.decisions.reverse(),
            1 => inspect.decisions[0].receipt_commit_sequence = 9,
            2 => inspect.decisions[0].to_revision = 4,
            _ => inspect.decisions[1].receipt_commit_sequence = 8,
        }
        assert!(complete_inspect(&mut m, inspect));
        assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
        assert!(m.operator.inspect.is_none());
    }
}

#[test]
fn queue_and_inspect_watermarks_never_regress_behind_latest_observed_page() {
    let mut m = inspect_model();
    m.operator.queue.as_mut().unwrap().commit_sequence = 9;
    m.operator.queue_after_id = 7;
    assert!(complete_queue(&mut m, queue_of(vec![], 8, None)));
    assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
    let mut m = inspect_model();
    m.operator.queue.as_mut().unwrap().commit_sequence = 9;
    assert!(complete_inspect(
        &mut m,
        inspect_of(
            synthetic_record(7, &Scope::Global, "suggested", 1),
            vec![],
            vec![],
            0
        )
    ));
    assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
    assert!(m.operator.inspect.is_none());
}
#[test]
fn changed_history_digest_and_duplicate_peer_preview_are_not_displayed_as_exact() {
    for change in 0..2 {
        let mut m = inspect_model();
        let mut inspect = inspect_of(
            synthetic_record(7, &Scope::Global, "suggested", 1),
            vec![decision(7, DecisionAction::Confirm, 6)],
            vec![PeerDemotion {
                id: 100,
                revision: 1,
            }],
            1,
        );
        if change == 0 {
            inspect.decisions[0].proposal_digest = "d".repeat(64);
        } else {
            inspect.peer_demotions.push(PeerDemotion {
                id: 100,
                revision: 1,
            });
            inspect.peer_demotion_total = 2;
        }
        assert!(complete_inspect(&mut m, inspect));
        assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
        assert!(m.operator.inspect.is_none());
    }
}
#[test]
fn unsafe_root_refusal_discards_inspection_and_requires_fresh_consent_after_restore() {
    // Native IPC sanitizes UnsafeRuntime to Unauthorized. Either spelling
    // must settle as a refusal, never preserve an arm for a restored root.
    for error in ["UnsafeRuntime", "Unauthorized"] {
        for action in ["confirm", "reject", "deprecate", "delete"] {
            let mut m = armed_delete_model();
            if action == "deprecate" {
                m.operator.inspect.as_mut().unwrap().record.status = "confirmed".into();
                let record = m.operator.record().unwrap().clone();
                assert!(complete_inspect(
                    &mut m,
                    inspect_of(record, vec![], vec![], 0)
                ));
            }
            let info = m.operator.info.clone();
            let record = m.operator.record().unwrap().clone();
            let arm = m.operator.delete_arm.clone();
            assert!(m.operator_review(action, "unsafe-root"));
            let request = m.operator_confirm().unwrap();
            assert!(m.complete_operator(&request, Err(error)));
            assert_eq!(m.operator.error.as_deref(), Some(error));
            assert!(m.operator.inspection.is_none());
            assert!(m.operator.inspect.is_none());
            assert!(m.operator.delete_arm.is_none());
            assert!(m.operator.review.is_none());
            assert!(m.operator.pending.is_none());
            assert!(m.operator.receipt.is_none());
            assert!(m.operator_retry().is_none());

            // Restoring availability or even old content/typed consent does
            // not restore the discarded inspection authority.
            m.operator.info = info;
            m.operator.page = Some(PreferencePage {
                binding: binding(),
                records: vec![record.clone()],
                next_after_id: None,
            });
            m.operator.selected_id = Some(record.id);
            m.operator.delete_arm = arm;
            assert!(!m.operator_inspected());
            assert!(!m.operator_review(action, "restored-root"));
            assert!(m.operator_confirm().is_none());
            assert!(complete_inspect(
                &mut m,
                inspect_of(record.clone(), vec![], vec![], 0)
            ));
            assert!(m.operator_inspected());
            assert!(
                m.operator.delete_arm.is_none(),
                "inspection never restores typed consent"
            );
            if action == "delete" {
                m.operator.delete_arm = Some(DeleteArm {
                    retype: record.id.to_string(),
                    id: record.id,
                    revision: record.revision,
                    proposal_digest: record.proposal_digest,
                });
            }
            assert!(m.operator_review(action, "fresh-explicit-review"));
            assert!(m.operator.pending.is_none(), "preparation does not submit");
        }
    }
}
