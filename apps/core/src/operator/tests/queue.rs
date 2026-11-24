use super::*;

#[test]
fn queue_empty_paging_watermark_drift_and_malformed_pages() {
    let mut m = model();
    // An empty queue is a valid pending-review state, never an error.
    assert!(complete_queue(&mut m, queue_of(vec![], 3, None)));
    assert!(m.operator.error.is_none());
    assert!(m.operator.queue.as_ref().unwrap().records.is_empty());
    assert_eq!(m.operator.queue_watermark, 3);
    // The first page establishes the drift baseline.
    let page1: Vec<_> = (1..=12)
        .map(|id| synthetic_record(id, &Scope::Global, "suggested", 1))
        .collect();
    assert!(complete_queue(&mut m, queue_of(page1.clone(), 5, Some(12))));
    assert_eq!(m.operator.queue_watermark, 5);
    // A later page discloses drift against the baseline, never rebases it.
    m.operator.queue_after_id = 12;
    assert!(complete_queue(
        &mut m,
        queue_of(
            vec![synthetic_record(13, &Scope::Global, "suggested", 1)],
            9,
            None
        )
    ));
    assert_eq!(m.operator.queue_watermark, 5);
    assert_eq!(m.operator.queue.as_ref().unwrap().commit_sequence, 9);
    // A later page below the disclosed baseline fails closed.
    m.operator.queue_after_id = 13;
    assert!(complete_queue(&mut m, queue_of(vec![], 4, None)));
    assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
    assert!(m.operator.queue.is_none());
    // A negative cursor never prepares a read.
    let mut m = model();
    m.operator.queue_after_id = -1;
    assert!(m.operator_read("profile.review.queue").is_none());
    let oversized: Vec<_> = (1..=13)
        .map(|id| synthetic_record(id, &Scope::Global, "suggested", 1))
        .collect();
    let mut descending = page1[..2].to_vec();
    descending.reverse();
    for bad in [
        queue_of(oversized, 5, Some(12)),
        queue_of(
            vec![synthetic_record(1, &Scope::Global, "confirmed", 1)],
            5,
            None,
        ),
        queue_of(descending, 5, None),
        queue_of(page1.clone(), 5, Some(11)),
        queue_of(
            vec![synthetic_record(
                1,
                &Scope::Project {
                    name: "alpha".into(),
                },
                "suggested",
                1,
            )],
            5,
            None,
        ),
    ] {
        let mut m = model();
        assert!(complete_queue(&mut m, bad));
        assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
        assert!(m.operator.queue.is_none());
    }
    // Rows at or below the paging cursor are refused.
    let mut m = model();
    m.operator.queue_after_id = 5;
    assert!(complete_queue(
        &mut m,
        queue_of(
            vec![synthetic_record(3, &Scope::Global, "suggested", 1)],
            5,
            None
        )
    ));
    assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
    // A moved generation or owner epoch is stale, never applied.
    for moved in [
        Binding {
            generation: "c".repeat(32),
            ..binding()
        },
        Binding {
            owner_epoch: 2,
            ..binding()
        },
    ] {
        let mut m = model();
        let mut queue = queue_of(vec![], 5, None);
        queue.binding = moved;
        assert!(complete_queue(&mut m, queue));
        assert_eq!(m.operator.error.as_deref(), Some("StaleGeneration"));
        assert!(m.operator.queue.is_none());
    }
}
#[test]
fn inspect_exact_revision_history_bound_and_peer_preview() {
    let mut m = inspect_model();
    let request = m.operator_read("profile.review.inspect").unwrap();
    assert_eq!(request.params["id"], json!(7));
    assert_eq!(request.params["revision"], json!(1));
    assert_eq!(request.params["scope"], json!({"kind":"global"}));
    let record = synthetic_record(7, &Scope::Global, "suggested", 1);
    let decisions: Vec<_> = (1..=8)
        .rev()
        .map(|seq| decision(7, DecisionAction::Confirm, seq))
        .collect();
    let peers: Vec<_> = (1..=12)
        .map(|i| PeerDemotion {
            id: 100 + i,
            revision: 1,
        })
        .collect();
    assert!(m.complete_operator(
        &request,
        Ok(serde_json::to_value(inspect_of(record, decisions, peers, 20)).unwrap())
    ));
    assert!(m.operator.error.is_none());
    assert_eq!(m.operator.inspect_revision, 1);
    assert!(!m.operator.digest_expanded);
    assert_eq!(m.operator.record().unwrap().id, 7);
    // A truncated preview must disclose the full total; exact totals match.
    let mut m = inspect_model();
    assert!(complete_inspect(
        &mut m,
        inspect_of(
            synthetic_record(7, &Scope::Global, "suggested", 1),
            vec![],
            vec![PeerDemotion {
                id: 100,
                revision: 1
            }],
            1
        )
    ));
    assert!(m.operator.error.is_none());
    let record = synthetic_record(7, &Scope::Global, "suggested", 1);
    let nine: Vec<_> = (1..=9)
        .map(|seq| decision(7, DecisionAction::Reject, seq))
        .collect();
    let mut wrong_id = decision(8, DecisionAction::Confirm, 1);
    wrong_id.preference_id = 8;
    for bad in [
        // A moved revision is a conflict, never a silently different preview.
        inspect_of(
            synthetic_record(7, &Scope::Global, "suggested", 2),
            vec![],
            vec![],
            0,
        ),
        inspect_of(record.clone(), nine, vec![], 0),
        inspect_of(record.clone(), vec![wrong_id], vec![], 0),
        inspect_of(
            record.clone(),
            vec![],
            vec![PeerDemotion {
                id: 100,
                revision: 1,
            }],
            20,
        ),
        inspect_of(
            record.clone(),
            vec![],
            vec![PeerDemotion { id: 7, revision: 1 }],
            1,
        ),
        inspect_of(record.clone(), vec![], vec![], -1),
        inspect_of(
            synthetic_record(8, &Scope::Global, "suggested", 1),
            vec![],
            vec![],
            0,
        ),
    ] {
        let mut m = inspect_model();
        assert!(complete_inspect(&mut m, bad));
        assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
        assert!(m.operator.inspect.is_none());
    }
}
#[test]
fn project_and_global_scope_isolation() {
    let mut m = model();
    // A global queue never accepts project rows.
    assert!(complete_queue(
        &mut m,
        queue_of(
            vec![synthetic_record(
                1,
                &Scope::Project {
                    name: "alpha".into()
                },
                "suggested",
                1
            )],
            5,
            None
        )
    ));
    assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
    // Project scope names the exact project in both read params.
    let mut m = model();
    m.operator.global = false;
    m.operator.page = None;
    let scope = Scope::Project {
        name: "alpha".into(),
    };
    let request = m.operator_read("profile.review.queue").unwrap();
    assert_eq!(
        request.params["scope"],
        json!({"kind":"project","name":"alpha"})
    );
    assert!(
        m.complete_operator(
            &request,
            Ok(serde_json::to_value(queue_of(
                vec![synthetic_record(1, &Scope::Global, "suggested", 1)],
                5,
                None
            ))
            .unwrap())
        )
    );
    assert_eq!(m.operator.error.as_deref(), Some("InvalidRequest"));
    // With a fresh info read, project rows are accepted in project scope.
    let mut m = model();
    m.operator.global = false;
    m.operator.page = None;
    assert!(complete_queue(
        &mut m,
        queue_of(vec![synthetic_record(1, &scope, "suggested", 1)], 5, None)
    ));
    m.operator.selected_id = Some(1);
    let request = m.operator_read("profile.review.inspect").unwrap();
    assert_eq!(
        request.params["scope"],
        json!({"kind":"project","name":"alpha"})
    );
    assert!(
        m.complete_operator(
            &request,
            Ok(serde_json::to_value(inspect_of(
                synthetic_record(1, &scope, "suggested", 1),
                vec![],
                vec![],
                0
            ))
            .unwrap())
        )
    );
    assert!(m.operator.error.is_none());
    // A cross-scope record can never prepare a decision.
    m.operator.page = Some(PreferencePage {
        binding: binding(),
        records: vec![synthetic_record(9, &Scope::Global, "suggested", 1)],
        next_after_id: None,
    });
    m.operator.selected_id = Some(9);
    assert!(!m.operator_review("confirm", "cross"));
    assert!(!m.operator_review("delete", "cross"));
}

#[test]
fn queue_sourced_selection_drives_detail_and_confirm() {
    let mut m = model();
    m.operator.page = None;
    m.operator.inspect = None;
    m.operator.inspect_revision = 0;
    m.operator.selected_id = Some(1);
    assert!(m.operator.record().is_none());
    assert!(m.operator_read("profile.review.inspect").is_none());
    assert!(complete_queue(
        &mut m,
        queue_of(
            vec![synthetic_record(1, &Scope::Global, "suggested", 1)],
            5,
            None
        )
    ));
    // Detail resolves the queue-sourced record without a preferences page.
    assert_eq!(m.operator.record().unwrap().id, 1);
    let request = m.operator_read("profile.review.inspect").unwrap();
    assert_eq!(request.params["id"], json!(1));
    assert_eq!(request.params["revision"], json!(1));
    assert!(
        m.complete_operator(
            &request,
            Ok(serde_json::to_value(inspect_of(
                synthetic_record(1, &Scope::Global, "suggested", 1),
                vec![],
                vec![],
                0
            ))
            .unwrap())
        )
    );
    // Confirm binds the exact inspected snapshot and its digest.
    assert!(m.operator_review("confirm", "q-op"));
    let request = m.operator_confirm().unwrap();
    assert!(matches!(
        request.mutation.as_ref().unwrap().command,
        Command::ConfirmPreference {
            id: 1,
            expected: Expected::Revision(1),
            ..
        }
    ));
    assert!(m.complete_operator(&request, Ok(receipt_json("q-op", 6, json!([])))));
    assert!(m.operator.queue.is_none());
    // A queue refresh that drops the selected record clears the selection
    // when no other snapshot still sources it.
    let mut m = model();
    m.operator.page = None;
    assert!(complete_queue(
        &mut m,
        queue_of(
            vec![synthetic_record(7, &Scope::Global, "suggested", 1)],
            5,
            None
        )
    ));
    m.operator.selected_id = Some(7);
    assert!(complete_queue(&mut m, queue_of(vec![], 6, None)));
    assert!(m.operator.selected_id.is_none());
    assert!(m.operator.record().is_none());
}
