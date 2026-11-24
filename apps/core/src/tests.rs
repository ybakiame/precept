use super::*;
#[test]
fn live_context_reads_tags_and_tolerates_rows_without_them() {
    // The daemon's effective context carries tags and adds an id.
    let context: Context = serde_json::from_value(serde_json::json!({
            "project":"alpha",
            "preferences":[
                {"id":1,"key":"editor","value":"v","scope":"project","status":"confirmed","tags":["style"]},
                {"id":2,"key":"shell","value":"v","scope":"project","status":"confirmed"}
            ],
            "repository":{},
            "documents":[],
            "warnings":[]
        }))
        .unwrap();
    assert_eq!(context.preferences[0].tags, ["style"]);
    assert!(context.preferences[1].tags.is_empty());
}
#[test]
fn cancel_invalidates_pending_results_and_detail_follows_visible_scope() {
    let mut m = Model::fixture();
    let pending = m.begin_read().unwrap();
    m.cancel_read();
    assert_eq!(m.phase, Phase::Unavailable);
    assert!(!m.complete_read(&pending, Ok(Snapshot::fixture()), 1));
    assert!(m.project().is_none());
    let mut snapshot = Snapshot::fixture();
    snapshot.projects[0].context.preferences[0].value = "中文\u{1b}".repeat(600);
    m.receive(Ok(snapshot));
    m.select_preference(0);
    assert!(m.detail_chunk().chars().count() <= 512);
    assert!(!m.detail_chunk().contains('\u{1b}'));
    m.detail_by(1000);
    assert_eq!(m.detail_page + 1, m.detail_pages());
    m.set_filter("absent");
    assert!(m.preference_detail().is_none());
    m.set_filter("");
    m.select_preference(0);
    m.select(1);
    assert!(m.preference_detail().is_none());
    assert_eq!(m.detail_page, 0);
}
#[test]
fn delayed_responses_cannot_restore_scope_or_revoked_authority() {
    let mut fixture = Snapshot::fixture();
    for p in &mut fixture.projects {
        p.handoff = None;
    }
    let mut m = Model::fixture();
    m.live = true;
    m.select(0);
    let alpha = m.begin_read().unwrap();
    m.select(1);
    let beta = m.begin_read().unwrap();
    assert!(m.project().is_none());
    assert!(!m.complete_read(&alpha, Ok(fixture.clone()), 10));
    assert!(m.complete_read(&beta, Ok(fixture.clone()), 20));
    assert_eq!(m.project().unwrap().name, "beta");
    assert_eq!(m.last_read_ms, Some(20));
    let old = m.begin_read().unwrap();
    let newest = m.begin_read().unwrap();
    assert!(m.complete_read(&newest, Err("Permission denied"), 30));
    assert!(!m.complete_read(&old, Ok(fixture.clone()), 40));
    assert!(m.permission_denied && m.project().is_none());
    assert_eq!(m.last_read_ms, Some(20));
    let restarting = m.begin_read().unwrap();
    m.task_id = "invalid\n".into();
    assert!(m.begin_read().is_err());
    assert!(!m.complete_read(&restarting, Ok(fixture.clone()), 50));
    assert_eq!(m.phase, Phase::Error);
    m.task_id.clear();
    let restarted = m.begin_read().unwrap();
    assert!(m.complete_read(&restarted, Ok(fixture.clone()), 60));
    assert_eq!(m.project().unwrap().name, "beta");
}
#[test]
fn rapid_exact_task_lookups_ignore_older_success_and_error() {
    let mut m = Model::fixture();
    m.select(0);
    m.live = true;
    m.task_id = "older".into();
    let older = m.begin_read().unwrap();
    m.task_id = "newest".into();
    let newest = m.begin_read().unwrap();
    let mut snapshot = Snapshot::fixture();
    snapshot.projects[0].handoff.as_mut().unwrap().task_id = "newest".into();
    assert!(m.complete_read(&newest, Ok(snapshot), 1));
    assert!(!m.complete_read(&older, Err("Permission denied"), 2));
    assert!(!m.complete_read(&older, Ok(Snapshot::fixture()), 3));
    assert_eq!(m.queried_task, "newest");
    assert_eq!(
        m.project().unwrap().handoff.as_ref().unwrap().task_id,
        "newest"
    );
    assert!(!m.permission_denied);
}
#[test]
fn unexpected_scope_task_and_not_ready_fail_closed() {
    let mut m = Model::fixture();
    m.live = true;
    m.select(0);
    m.task_id = "missing".into();
    let request = m.begin_read().unwrap();
    assert!(m.complete_read(&request, Ok(Snapshot::fixture()), 1));
    assert!(m.project().is_none());
    let request = m.begin_read().unwrap();
    let mut snapshot = Snapshot::fixture();
    snapshot.projects.remove(0);
    m.complete_read(&request, Ok(snapshot), 2);
    assert!(m.permission_denied && m.data.is_none());
    let mut snapshot = Snapshot::fixture();
    snapshot.health.ready = false;
    assert!(snapshot.validate().is_err());
}
#[test]
fn pages_bound_historical_evidence_and_sanitize_display_and_copy() {
    let mut m = Model::fixture();
    let mut snapshot = Snapshot::fixture();
    let h = snapshot.projects[0].handoff.as_mut().unwrap();
    h.validation = (0..100)
        .map(|i| serde_json::json!({"index":i,"text":"x".repeat(1000)}))
        .collect();
    h.objective = "unsafe\u{1b}[2J".into();
    m.receive(Ok(snapshot));
    m.set_section(Section::Handoff);
    assert!(!m.visible_lines().join("\n").contains("\u{1b}"));
    assert!(!m.visible_lines().join("\n").contains("\"index\""));
    // At most 20 checks, each one bounded line, never raw JSON.
    assert!(m.visible_lines().len() <= 48);
    assert!(
        m.visible_lines()
            .iter()
            .all(|line| line.chars().count() < 550)
    );
    m.set_section(Section::Diagnostics);
    for section in Section::ALL {
        m.set_section(section);
        assert!(m.visible_lines()[0].contains("alpha"));
    }
}
#[test]
fn states_selection_filter_and_unknown_evidence() {
    let mut m = Model::fixture();
    assert_eq!(m.phase, Phase::Ready);
    assert!(
        m.detail_lines()
            .iter()
            .any(|s| s.contains("Fingerprint: Unavailable"))
    );
    m.next();
    assert_eq!(m.project().unwrap().name, "beta");
    m.select(99);
    assert_eq!(m.selected, 1);
    m.filter = "never-match".into();
    assert!(m.rows().is_empty());
    m.receive(Err("synthetic"));
    assert_eq!(m.phase, Phase::Unavailable);
    assert!(m.project().is_none());
    let mut empty = Snapshot::fixture();
    empty.projects.clear();
    m.receive(Ok(empty));
    assert_eq!(m.phase, Phase::Empty);
    m.phase = Phase::Error;
    assert!(m.status().contains("Error"));
    assert!(Model::loading().status().contains("Loading"));
}
#[test]
fn rejects_cross_project_and_unconfirmed_rows() {
    let mut s = Snapshot::fixture();
    s.projects[0].context.project = "beta".into();
    assert!(s.validate().is_err());
    for status in ["suggested", "rejected", "deprecated"] {
        let mut s = Snapshot::fixture();
        s.projects[0].context.preferences[0].status = status.into();
        assert!(s.validate().is_err());
    }
}
#[test]
fn ten_thousand_rows_filter_and_selection() {
    let mut s = Snapshot::fixture();
    let row = s.projects[0].context.preferences[0].clone();
    s.projects[0].context.preferences = (0..10_000)
        .map(|i| Preference {
            key: format!("event-{i}"),
            ..row.clone()
        })
        .collect();
    let mut m = Model::loading();
    m.receive(Ok(s));
    m.filter = "event-9999".into();
    assert_eq!(m.rows().len(), 1);
    m.next();
    assert_eq!(m.project().unwrap().name, "beta");
}
#[test]
fn input_bounds_pages_and_permission_fail_closed() {
    for invalid in ["", " ", "task\nnext", "\u{1b}[2J"] {
        assert!(!valid_task(invalid));
    }
    assert!(!valid_task(&"x".repeat(121)));
    assert!(valid_task("task / 'quoted' 中文"));
    let mut m = Model::fixture();
    m.page_by(isize::MAX);
    assert_eq!(m.page, 0);
    m.set_filter(&"x".repeat(1000));
    assert_eq!(m.filter.len(), 120);
    m.receive(Err("Permission denied"));
    assert!(m.data.is_none());
    assert!(m.status().contains("Permission denied"));
    m.receive(Ok(Snapshot::fixture()));
    assert!(!m.permission_denied);
    m.phase = Phase::Loading;
    assert!(m.project().is_none());
    assert!(m.rows().is_empty());
    let mut s = Snapshot::fixture();
    let row = s.projects[0].context.preferences[0].clone();
    s.projects[0].context.preferences = vec![row; MAX_ROWS + 1];
    assert!(s.validate().is_err());
}
