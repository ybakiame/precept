use super::*;

#[test]
fn wsl_status_names_the_cause_without_touching_phase_data_or_guards() {
    use crate::connection::{Connection, Target, wsl_status};
    let mut m = Model::fixture();
    m.connection = Connection {
        target: Target::Wsl,
        distribution: "Synthetic".into(),
        user: "operator".into(),
        helper: "/s/precept-bridge".into(),
        wsl_runtime: "/s/runtime".into(),
        wsl_endpoint: "/s/runtime/control.sock".into(),
        ..Default::default()
    };
    m.receive(Err(wsl_status::DISTRIBUTION_NOT_FOUND));
    assert_eq!(m.phase, Phase::Unavailable);
    assert_eq!(m.read_label(), m.word(i18n::Msg::WslDistributionNotFound));
    assert!(
        m.status()
            .contains(m.word(i18n::Msg::WslDistributionNotFound))
    );
    // A live link problem is display-only: phase, data and the Operator
    // guard are unchanged, so an uncertain request can still be retried.
    m.receive(Ok(Snapshot::fixture()));
    assert_eq!(m.phase, Phase::Ready);
    m.operator.open = true;
    let guard = m.operator_guard();
    assert!(guard.is_some());
    m.wsl_link = Some(wsl_status::BRIDGE_DISCONNECTED);
    assert_eq!(m.phase, Phase::Ready);
    assert!(m.data.is_some());
    assert!(m.operator_guard() == guard);
    assert_eq!(m.read_label(), m.word(i18n::Msg::WslBridgeDisconnected));
    // A successful read clears both; local targets never show WSL text.
    m.receive(Ok(Snapshot::fixture()));
    assert_eq!((m.wsl_link, m.read_failure), (None, None));
    m.connection.target = Target::Local;
    m.wsl_link = Some(wsl_status::BRIDGE_DISCONNECTED);
    assert_eq!(m.wsl_reason(), None);
}

#[test]
fn target_switch_invalidates_delayed_scope_data_and_timestamps() {
    let mut m = Model::fixture();
    m.live = true;
    m.last_read_ms = Some(123);
    m.last_duration_ms = Some(8);
    m.task_id = "shared".into();
    m.selected_project = Some("alpha".into());
    let old = m.begin_read().unwrap();
    m.connection_draft = connection::Connection::default();
    assert!(m.apply_connection());
    assert!(m.data.is_none() && m.project_names.is_empty() && m.selected_project.is_none());
    assert!(
        m.task_id.is_empty()
            && m.queried_task.is_empty()
            && m.last_read_ms.is_none()
            && m.last_duration_ms.is_none()
    );
    assert!(!m.complete_read(&old, Ok(Snapshot::fixture()), 999));
    let next = m.begin_read().unwrap();
    let mut snapshot = Snapshot::fixture();
    for p in &mut snapshot.projects {
        p.handoff = None;
    }
    assert!(m.complete_read(&next, Ok(snapshot), 1000));
    assert!(!m.live && m.last_read_ms.is_none());
    m.connection_draft.target = connection::Target::Local;
    let epoch = m.epoch;
    assert!(!m.apply_connection());
    assert_eq!(m.epoch, epoch);
    assert!(m.data.is_some());
}
#[test]
fn first_read_binds_project_and_empty_allowlist_still_records_success() {
    let mut m = Model::loading();
    m.live = true;
    m.task_id = "shared".into();
    let request = m.begin_read().unwrap();
    assert!(m.complete_read(&request, Ok(Snapshot::fixture()), 1));
    assert_eq!(m.selected_project.as_deref(), Some("alpha"));
    let mut reversed = Snapshot::fixture();
    reversed.projects.reverse();
    let request = m.begin_read().unwrap();
    assert!(m.complete_read(&request, Ok(reversed), 2));
    assert_eq!(m.project().unwrap().name, "alpha");
    m.selected_project = None;
    let mut empty = Snapshot::fixture();
    empty.projects.clear();
    let request = m.begin_read().unwrap();
    assert!(m.complete_read(&request, Ok(empty), 3));
    assert_eq!(m.phase, Phase::Empty);
    assert_eq!(m.last_read_ms, Some(3));
}
#[test]
fn initial_unqueried_handoff_is_refused() {
    let mut m = Model::loading();
    let request = m.begin_read().unwrap();
    assert!(m.complete_read(&request, Ok(Snapshot::fixture()), 1));
    assert_eq!(m.phase, Phase::Unavailable);
    assert!(m.data.is_none());
}
#[test]
fn chinese_page_copy_retains_git_handoff_and_disclosed_value() {
    let mut m = Model::fixture();
    m.locale = i18n::Locale::ZhCn;
    let lines = m.visible_lines().join("\n");
    let p = m.project().unwrap();
    assert!(lines.contains(p.context.repository.branch.as_deref().unwrap()));
    assert!(lines.contains(&p.handoff.as_ref().unwrap().objective));
    assert!(lines.contains(&p.context.warnings[0]));
    m.section = Section::Rules;
    let mut snapshot = Snapshot::fixture();
    snapshot.projects[0].context.preferences[0].value = "中文".repeat(600);
    m.receive(Ok(snapshot));
    m.select_preference(0);
    m.detail_by(1);
    assert!(m.visible_lines().join("\n").contains(&m.detail_chunk()));
    assert!(m.visible_lines().iter().all(|s| s.chars().count() < 550));
}
#[test]
fn chinese_ui_preserves_original_values_and_evidence() {
    let mut m = Model::fixture();
    m.locale = i18n::Locale::ZhCn;
    m.section = Section::Handoff;
    let lines = m.visible_lines().join("\n");
    let h = m.project().unwrap().handoff.as_ref().unwrap();
    assert!(lines.contains(&h.objective));
    let check = h.validation[0]["check"].as_str().unwrap();
    assert!(lines.contains(check));
    assert!(lines.contains("并非当前会话验证"));
    m.section = Section::Rules;
    assert!(m.visible_lines().join("\n").contains(&m.rows()[0].value));
}
