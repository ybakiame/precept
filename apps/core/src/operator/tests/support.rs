use super::*;

pub(in crate::operator) fn model() -> Model {
    let mut m = Model::fixture();
    m.select(0);
    m.connection.target = Target::Local;
    m.connection.runtime = std::env::temp_dir().to_str().unwrap().into();
    m.connection.endpoint = "synthetic".into();
    m.operator.open = true;
    m.operator.global = true;
    m.operator.info = Some(ProfileInfo {
        selected: true,
        binding: Some(Binding {
            profile_id: "a".repeat(32),
            generation: "b".repeat(32),
            owner_epoch: 1,
        }),
        write_enabled: true,
        ..Default::default()
    });
    let binding = m.operator.info.as_ref().unwrap().binding.clone().unwrap();
    m.operator.page = Some(PreferencePage {
        binding,
        records: vec![PreferenceRecord {
            rule: Default::default(),
            enabled: true,
            id: 1,
            scope: Scope::Global,
            key: "synthetic-key".into(),
            value: "synthetic-value".into(),
            tags: vec!["test".into()],
            status: "suggested".into(),
            revision: 1,
            origin: Origin::Agent,
            proposal_digest: digest_hex(&proposal_digest(
                &Scope::Global,
                None,
                "synthetic-key",
                "synthetic-value",
                Origin::Agent,
                &["test".to_owned()],
                &Default::default(),
            )),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }],
        next_after_id: None,
    });
    m.operator.selected_id = Some(1);
    let record = m.operator.page.as_ref().unwrap().records[0].clone();
    assert!(complete_inspect(
        &mut m,
        inspect_of(record, vec![], vec![], 0)
    ));
    m
}

pub(super) fn binding() -> Binding {
    Binding {
        profile_id: "a".repeat(32),
        generation: "b".repeat(32),
        owner_epoch: 1,
    }
}
pub(super) fn synthetic_record(
    id: i64,
    scope: &Scope,
    status: &str,
    revision: i64,
) -> PreferenceRecord {
    let key = format!("synthetic-key-{id}");
    let project = scope.project();
    PreferenceRecord {
        rule: Default::default(),
        enabled: true,
        id,
        scope: scope.clone(),
        key: key.clone(),
        value: "synthetic-value".into(),
        tags: vec!["test".into()],
        status: status.into(),
        revision,
        origin: Origin::Agent,
        proposal_digest: digest_hex(&proposal_digest(
            scope,
            project,
            &key,
            "synthetic-value",
            Origin::Agent,
            &["test".to_owned()],
            &Default::default(),
        )),
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }
}
pub(super) fn queue_of(
    records: Vec<PreferenceRecord>,
    commit_sequence: i64,
    next_after_id: Option<i64>,
) -> ReviewQueue {
    ReviewQueue {
        binding: binding(),
        commit_sequence,
        records,
        next_after_id,
    }
}
pub(super) fn complete_queue(m: &mut Model, queue: ReviewQueue) -> bool {
    let request = m.operator_read("profile.review.queue").unwrap();
    m.complete_operator(&request, Ok(serde_json::to_value(queue).unwrap()))
}
pub(super) fn decision(
    preference_id: i64,
    action: DecisionAction,
    seq: i64,
) -> ReviewDecisionRecord {
    ReviewDecisionRecord {
        preference_id,
        action,
        from_revision: 1,
        to_revision: 2,
        proposal_digest: synthetic_record(preference_id, &Scope::Global, "suggested", 1)
            .proposal_digest,
        receipt_commit_sequence: seq,
        channel: Channel::Desktop,
        decided_at: "2026-01-01T00:00:00Z".into(),
        note: None,
        replaced_by: None,
    }
}
pub(super) fn inspect_of(
    record: PreferenceRecord,
    decisions: Vec<ReviewDecisionRecord>,
    peer_demotions: Vec<PeerDemotion>,
    peer_demotion_total: i64,
) -> ReviewInspect {
    let evidence = match record.scope {
        Scope::Project { .. } => Some(ReviewEvidence {
            gate_denials: 0,
            last_gate_denial_ms: None,
        }),
        Scope::Global => None,
    };
    ReviewInspect {
        binding: binding(),
        commit_sequence: 8,
        record,
        decisions,
        peer_demotions,
        peer_demotion_total,
        related: vec![],
        related_total: 0,
        evidence,
    }
}
pub(super) fn complete_inspect(m: &mut Model, inspect: ReviewInspect) -> bool {
    let request = m.operator_read("profile.review.inspect").unwrap();
    m.complete_operator(&request, Ok(serde_json::to_value(inspect).unwrap()))
}
/// Queue-sourced selection with an exact inspect snapshot, no preferences page.
pub(super) fn inspect_model() -> Model {
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
    m
}
pub(super) fn armed_delete_model() -> Model {
    let mut m = inspect_model();
    assert!(complete_inspect(
        &mut m,
        inspect_of(
            synthetic_record(7, &Scope::Global, "suggested", 1),
            vec![],
            vec![],
            0
        )
    ));
    m.operator.delete_arm = Some(DeleteArm {
        retype: "7".into(),
        id: 7,
        revision: 1,
        proposal_digest: m.operator.record().unwrap().proposal_digest.clone(),
    });
    m
}
pub(super) fn receipt_json(operation_id: &str, commit_sequence: i64, affected: Value) -> Value {
    json!({"generation":"b".repeat(32),"operation_id":operation_id,"commit_sequence":commit_sequence,"affected":affected})
}

pub(super) fn change_review_context(m: &mut Model, change: usize) {
    match change {
        0 => {
            m.operator
                .info
                .as_mut()
                .unwrap()
                .binding
                .as_mut()
                .unwrap()
                .profile_id = "d".repeat(32)
        }
        1 => {
            m.operator
                .info
                .as_mut()
                .unwrap()
                .binding
                .as_mut()
                .unwrap()
                .generation = "d".repeat(32)
        }
        2 => {
            m.operator
                .info
                .as_mut()
                .unwrap()
                .binding
                .as_mut()
                .unwrap()
                .owner_epoch += 1
        }
        3 => m.operator.info.as_mut().unwrap().write_enabled = false,
        4 => m.operator.selected_id = Some(8),
        5 => m.task_id = "new-context-task".into(),
        6 => m.queried_task = "new-queried-task".into(),
        7 => m.operator.after_id += 1,
        8 => m.operator.queue.as_mut().unwrap().commit_sequence += 1,
        9 => m.phase = crate::Phase::Unavailable,
        10 => {
            let snapshot = std::sync::Arc::make_mut(m.data.as_mut().unwrap());
            snapshot.projects[0].context.documents[0]["status"] = json!("unavailable");
        }
        11 => {
            let snapshot = std::sync::Arc::make_mut(m.data.as_mut().unwrap());
            snapshot.projects[0].context.documents[0]["sha256"] = json!("d".repeat(64));
        }
        12 => {
            let snapshot = std::sync::Arc::make_mut(m.data.as_mut().unwrap());
            snapshot.projects[0]
                .context
                .warnings
                .push("Source changed".into());
        }
        13 => m.operator.open = false,
        14 => m.permission_denied = true,
        15 => m.live = !m.live,
        _ => m.data = None,
    }
}
