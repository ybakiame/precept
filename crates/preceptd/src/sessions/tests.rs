use super::{model::*, store::Sessions};
use precept_ipc::Error;
use serde_json::{Value, json};

fn state() -> (tempfile::TempDir, crate::state::StateDir) {
    let dir = tempfile::tempdir().unwrap();
    let state = crate::state::StateDir::open(&dir.path().join("state")).unwrap();
    (dir, state)
}

fn update(id: &str, rev: u64) -> Update {
    Update {
        id: Some(id.into()),
        expected_rev: Some(rev),
        ..Default::default()
    }
}

fn item(id: &str, text: &str, state: ItemState, deps: &[&str]) -> NewItem {
    NewItem {
        id: Some(id.into()),
        text: text.into(),
        kind: ItemKind::Task,
        state,
        depends_on: deps.iter().map(|d| (*d).into()).collect(),
        owner: None,
        blocked_reason: None,
    }
}

#[test]
fn start_resume_update_and_close_keep_one_session() {
    let (_dir, state) = state();
    let sessions = Sessions::new();
    let start = || Update {
        goal: Some("Ship structured work".into()),
        conversation: Some("conv-1".into()),
        client: Some("agent-a 1.0".into()),
        items: Some(vec![item("design", "Design", ItemState::Done, &[])]),
        ..Default::default()
    };
    let first = sessions.start(&state, "alpha", start()).unwrap();
    let resumed = sessions
        .start(
            &state,
            "alpha",
            Update {
                goal: Some("Ship structured work".into()),
                conversation: Some("conv-1".into()),
                client: Some("agent-a 1.0".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(first.id, resumed.id);
    assert_eq!(resumed.rev, 2);
    let mut change = update(&first.id, resumed.rev);
    change.operations = Some(vec![Operation::Add {
        item: item("implement", "Implement", ItemState::Todo, &["design"]),
    }]);
    let changed = sessions.update(&state, "alpha", change, false).unwrap();
    assert_eq!(changed.rev, 3);
    assert_eq!(changed.items[1].depends_on, ["design"]);
    let mut close = update(&first.id, changed.rev);
    close.summary = Some("Design complete; implementation remains.".into());
    let closed = sessions.update(&state, "alpha", close, true).unwrap();
    assert_eq!(closed.status, "closed");
    assert_eq!(
        sessions.update(&state, "alpha", update(&first.id, closed.rev), false),
        Err(Error::InvalidTransition)
    );
    assert!(sessions.list(&state, "beta").unwrap().is_empty());
}

#[test]
fn single_item_operations_allow_cross_state_dependencies_and_claims() {
    let (_dir, state) = state();
    let sessions = Sessions::new();
    let started = sessions
        .start(
            &state,
            "alpha",
            Update {
                goal: Some("Coordinate agents".into()),
                client: Some("agent-a 1.0".into()),
                items: Some(vec![
                    item("api", "API", ItemState::Done, &[]),
                    item("ui", "UI", ItemState::Todo, &["api"]),
                ]),
                ..Default::default()
            },
        )
        .unwrap();
    let mut change = update(&started.id, started.rev);
    change.client = Some("agent-b 1.0".into());
    change.operations = Some(vec![
        Operation::Update {
            id: "ui".into(),
            text: None,
            kind: None,
            depends_on: None,
            owner: Some("agent-b 1.0".into()),
            blocked_reason: None,
        },
        Operation::Start { id: "ui".into() },
        Operation::Block {
            id: "ui".into(),
            reason: "Waiting for review".into(),
        },
    ]);
    let changed = sessions.update(&state, "alpha", change, false).unwrap();
    let ui = changed.items.iter().find(|item| item.id == "ui").unwrap();
    assert_eq!(ui.state, ItemState::Blocked);
    assert_eq!(ui.owner.as_deref(), Some("agent-b 1.0"));
    assert_eq!(ui.blocked_reason.as_deref(), Some("Waiting for review"));
}

#[test]
fn dependencies_are_session_wide_unique_and_acyclic() {
    let (_dir, state) = state();
    let sessions = Sessions::new();
    let started = sessions
        .start(
            &state,
            "alpha",
            Update {
                goal: Some("Graph".into()),
                items: Some(vec![
                    item("a", "A", ItemState::Todo, &[]),
                    item("b", "B", ItemState::Todo, &["a"]),
                ]),
                ..Default::default()
            },
        )
        .unwrap();
    let mut cycle = update(&started.id, started.rev);
    cycle.operations = Some(vec![Operation::Update {
        id: "a".into(),
        text: None,
        kind: None,
        depends_on: Some(vec!["b".into()]),
        owner: None,
        blocked_reason: None,
    }]);
    let error = sessions.update(&state, "alpha", cycle, false).unwrap_err();
    match error {
        Error::InvalidWorkState { field, reason } => {
            assert!(field.contains("depends_on"));
            assert_eq!(reason, "dependency cycle");
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(sessions.list(&state, "alpha").unwrap()[0].rev, started.rev);
}

#[test]
fn cas_and_bounds_refuse_without_writing() {
    let (_dir, state) = state();
    let sessions = Sessions::new();
    let started = sessions
        .start(
            &state,
            "alpha",
            Update {
                goal: Some("Bounds".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let mut valid = update(&started.id, started.rev);
    valid.goal = Some("Renamed".into());
    let changed = sessions.update(&state, "alpha", valid, false).unwrap();
    let mut stale = update(&started.id, started.rev);
    stale.goal = Some("Clobber".into());
    assert_eq!(
        sessions.update(&state, "alpha", stale, false),
        Err(Error::Conflict)
    );
    let mut missing_rev = update(&started.id, changed.rev);
    missing_rev.expected_rev = None;
    missing_rev.goal = Some("No revision".into());
    assert!(matches!(
        sessions.update(&state, "alpha", missing_rev, false),
        Err(Error::InvalidWorkState { .. })
    ));
    let mut too_long = update(&started.id, changed.rev);
    too_long.operations = Some(vec![Operation::Add {
        item: NewItem {
            text: "x".repeat(MAX_ITEM + 1),
            ..item("long", "", ItemState::Todo, &[])
        },
    }]);
    assert!(matches!(
        sessions.update(&state, "alpha", too_long, false),
        Err(Error::InvalidWorkState { .. })
    ));
    assert_eq!(sessions.list(&state, "alpha").unwrap()[0].goal, "Renamed");
}

#[test]
fn capacity_keeps_the_commit_and_evicts_the_oldest_other_open_session() {
    for (tied, removed, retained) in [(true, "s19", "s00"), (false, "s00", "s19")] {
        let (_dir, state) = state();
        let old: Vec<Value> = (0..MAX_SESSIONS)
            .map(|index| {
                let updated = if tied { u64::MAX } else { index as u64 + 1 };
                json!({"id":format!("s{index:02}"),"agent":null,"conversation":null,
                    "status":"open","started_ms":updated,"updated_ms":updated,"closed_ms":null,
                    "goal":"Old","items":[],"summary":null,"rev":1})
            })
            .collect();
        state
            .write(
                "sessions-alpha.json",
                &serde_json::to_vec(&json!({"format":2,"sessions":old})).unwrap(),
            )
            .unwrap();
        let sessions = Sessions::new();
        let started = sessions
            .start(
                &state,
                "alpha",
                Update {
                    goal: Some("New".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let listed = sessions.list(&state, "alpha").unwrap();
        assert_eq!(listed.len(), MAX_SESSIONS);
        assert!(listed.iter().any(|session| session.id == started.id));
        assert!(!listed.iter().any(|session| session.id == removed));
        assert!(listed.iter().any(|session| session.id == retained));
    }
}

#[test]
fn legacy_duplicate_ids_and_started_dependencies_normalize_without_blocking_reads() {
    let (_dir, state) = state();
    let legacy = json!({
        "format": 1,
        "sessions": [{
            "id": "old", "agent": null, "conversation": null, "status": "open",
            "started_ms": 1, "updated_ms": 2, "closed_ms": null, "goal": "Legacy",
            "doing": [
                {"id":"shared","text":"Doing shared","depends_on":["dep"]},
                {"id":"dep","text":"Dependency","depends_on":["shared"]}
            ],
            "todo": [
                {"id":"shared","text":"Todo shared"},
                {"id":"child","text":"Child","depends_on":["shared","shared","missing"]}
            ],
            "done": [{"id":"shared","text":"Done shared"}],
            "summary": null, "rev": 4
        }]
    });
    state
        .write("sessions-alpha.json", &serde_json::to_vec(&legacy).unwrap())
        .unwrap();
    let sessions = Sessions::new();
    let listed = sessions.list(&state, "alpha").unwrap();
    let items = &listed[0].items;
    assert_eq!(items.len(), 5);
    assert_eq!(
        items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        5
    );
    assert_eq!(
        items[0].state,
        ItemState::Todo,
        "invalid legacy start is deferred"
    );
    assert_eq!(items[0].depends_on, ["dep"]);
    assert!(items[1].depends_on.is_empty(), "legacy cycle is broken");
    assert_eq!(items[2].id, "shared-todo");
    assert_eq!(items[3].depends_on, ["shared-todo"]);
    assert_eq!(items[4].id, "shared-done");
}

#[test]
fn method_inappropriate_fields_are_refused_instead_of_discarded() {
    let (_dir, state) = state();
    let sessions = Sessions::new();
    let started = sessions
        .start(
            &state,
            "alpha",
            Update {
                goal: Some("Strict methods".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let start = Update {
        goal: Some("Bad start".into()),
        operations: Some(vec![Operation::Add {
            item: item("x", "X", ItemState::Todo, &[]),
        }]),
        ..Default::default()
    };
    assert!(matches!(
        sessions.start(&state, "alpha", start),
        Err(Error::InvalidWorkState { .. })
    ));
    let mut update_items = update(&started.id, started.rev);
    update_items.items = Some(vec![]);
    assert!(matches!(
        sessions.update(&state, "alpha", update_items, false),
        Err(Error::InvalidWorkState { .. })
    ));
    let mut update_summary = update(&started.id, started.rev);
    update_summary.summary = Some("Not closing".into());
    assert!(matches!(
        sessions.update(&state, "alpha", update_summary, false),
        Err(Error::InvalidWorkState { .. })
    ));
    let start_summary = Update {
        goal: Some("Bad start".into()),
        summary: Some("Not starting".into()),
        ..Default::default()
    };
    assert!(matches!(
        sessions.start(&state, "alpha", start_summary),
        Err(Error::InvalidWorkState { .. })
    ));
}

#[test]
fn dependencies_gate_start_and_completion_and_blank_owners_are_claimed() {
    let (_dir, state) = state();
    let sessions = Sessions::new();
    assert!(matches!(
        sessions.start(
            &state,
            "alpha",
            Update {
                goal: Some("Invalid initial order".into()),
                items: Some(vec![
                    item("a", "A", ItemState::Todo, &[]),
                    item("b", "B", ItemState::Doing, &["a"]),
                ]),
                ..Default::default()
            },
        ),
        Err(Error::InvalidWorkState { .. })
    ));
    let started = sessions
        .start(
            &state,
            "alpha",
            Update {
                goal: Some("Ordered".into()),
                client: Some("agent-a 1.0".into()),
                items: Some(vec![
                    item("a", "A", ItemState::Todo, &[]),
                    NewItem {
                        owner: Some("  ".into()),
                        ..item("b", "B", ItemState::Todo, &["a"])
                    },
                ]),
                ..Default::default()
            },
        )
        .unwrap();
    let mut early = update(&started.id, started.rev);
    early.operations = Some(vec![Operation::Start { id: "b".into() }]);
    assert!(matches!(
        sessions.update(&state, "alpha", early, false),
        Err(Error::InvalidWorkState { .. })
    ));
    let mut early_complete = update(&started.id, started.rev);
    early_complete.operations = Some(vec![Operation::Complete { id: "b".into() }]);
    assert!(matches!(
        sessions.update(&state, "alpha", early_complete, false),
        Err(Error::InvalidWorkState { .. })
    ));
    let mut finish_a = update(&started.id, started.rev);
    finish_a.operations = Some(vec![Operation::Complete { id: "a".into() }]);
    let next = sessions.update(&state, "alpha", finish_a, false).unwrap();
    let mut start_b = update(&started.id, next.rev);
    start_b.operations = Some(vec![Operation::Start { id: "b".into() }]);
    let changed = sessions.update(&state, "alpha", start_b, false).unwrap();
    let b = changed.items.iter().find(|item| item.id == "b").unwrap();
    assert_eq!(b.state, ItemState::Doing);
    assert_eq!(b.owner.as_deref(), Some("agent-a 1.0"));
}

#[test]
fn persisted_blank_owners_normalize_to_none() {
    let (_dir, state) = state();
    let session = Session {
        id: "blank-owner".into(),
        agent: None,
        conversation: None,
        status: "open".into(),
        started_ms: 1,
        updated_ms: 2,
        closed_ms: None,
        goal: "Blank owner".into(),
        items: vec![Item {
            id: "task".into(),
            text: "Task".into(),
            kind: ItemKind::Task,
            state: ItemState::Todo,
            depends_on: vec![],
            owner: Some("   ".into()),
            blocked_reason: None,
        }],
        summary: None,
        rev: 1,
    };
    state
        .write(
            "sessions-alpha.json",
            &serde_json::to_vec(&json!({"format":2,"sessions":[session]})).unwrap(),
        )
        .unwrap();
    let listed = Sessions::new().list(&state, "alpha").unwrap();
    assert_eq!(listed[0].items[0].owner, None);
}

#[test]
fn legacy_lists_normalize_to_structured_items_on_read() {
    let (_dir, state) = state();
    let legacy = json!({
        "format": 1,
        "sessions": [{
            "id": "old",
            "agent": null,
            "conversation": null,
            "status": "open",
            "started_ms": 1,
            "updated_ms": 2,
            "closed_ms": null,
            "goal": "Legacy",
            "done": [{"id":"done","text":"Done"}],
            "doing": [{"id":"doing","text":"Doing"}],
            "todo": [{"id":"todo","text":"Todo","depends_on":["doing"]}],
            "summary": null,
            "rev": 4
        }]
    });
    state
        .write("sessions-alpha.json", &serde_json::to_vec(&legacy).unwrap())
        .unwrap();
    let sessions = Sessions::new();
    let listed = sessions.list(&state, "alpha").unwrap();
    assert_eq!(listed[0].items.len(), 3);
    assert_eq!(listed[0].rev, 4);
    assert!(
        listed[0]
            .items
            .iter()
            .any(|item| item.id == "todo" && item.state == ItemState::Todo)
    );
}
