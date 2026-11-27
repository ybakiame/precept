use super::actions::error_msg;
use super::delete::delete_dialog;
use super::*;
use nsh_engine::{EngineConfig, Root};
use precept_ui_core::operator::{
    Binding, PeerDemotion, PreferencePage, PreferenceRecord, ProfileInfo, ReviewEvidence,
    ReviewInspect, ReviewQueue, ReviewRelated, ReviewRelation, digest_hex, proposal_digest,
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
        related: vec![],
        related_total: 0,
        evidence: None,
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
        related: vec![],
        related_total: 0,
        evidence: None,
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
fn DuplicateQueueConsole() -> impl IntoElement {
    let mut m = queue_model();
    let mut duplicate = record(8, "suggested", 1);
    duplicate.key = record(7, "suggested", 1).key;
    duplicate.value = record(7, "suggested", 1).value;
    m.operator.queue.as_mut().unwrap().records.push(duplicate);
    crate::console(signal(m))
}
#[component]
fn EvidenceConsole() -> impl IntoElement {
    let mut m = queue_model();
    m.selected_project = Some("alpha".into());
    m.operator.global = false;
    m.operator.mode = ReviewMode::Project;
    m.operator.review_tab = ReviewTab::Detail;
    let scope = Scope::Project {
        name: "alpha".into(),
    };
    let mut inspected = record(7, "suggested", 1);
    inspected.scope = scope.clone();
    inspected.rule.applies_to.paths = vec!["src/".into(), "Cargo.toml".into()];
    m.operator.queue.as_mut().unwrap().records[0] = inspected.clone();
    let inspect = m.operator.inspect.as_mut().unwrap();
    inspect.record = inspected;
    inspect.related = vec![ReviewRelated {
        id: 8,
        revision: 1,
        key: "synthetic-key-7".into(),
        relation: ReviewRelation::Duplicate,
    }];
    inspect.related_total = 1;
    inspect.evidence = Some(ReviewEvidence {
        gate_denials: 2,
        last_gate_denial_ms: Some(1_763_640_000_000),
    });
    observe_inspect(&mut m);
    crate::console(signal(m))
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
fn duplicate_queue_rows_are_marked_without_implicit_selection() {
    let mut app = start::<DuplicateQueueConsole>();
    app.wait_for("Duplicates ×2").unwrap();
    app.wait_for("Possible duplicates in the loaded queue are marked; each record still requires its own exact review.")
        .unwrap();
    assert!(app.screen().shows("#7 · synthetic-key-7"));
    assert!(!app.screen().shows("Confirm 2…"));
}

#[test]
fn detail_shows_scope_related_records_and_retained_gate_evidence() {
    let mut app = start::<EvidenceConsole>();
    app.wait_for("Affected files").unwrap();
    app.wait_for("src/\nCargo.toml").unwrap();
    app.wait_for("Duplicate content · #8 · synthetic-key-7 · r1")
        .unwrap();
    app.wait_until("gate evidence", Duration::from_secs(5), |s| {
        s.texts().iter().any(|t| {
            t.contains("2 gate denial(s) exist in retained project evidence.")
                && t.contains("Last gate denial: 2025-11-20 12:00 UTC")
        })
    })
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
        s.texts().iter().any(|t| t.ends_with("· Source Agent"))
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

#[test]
fn duplicates_are_counted_by_exact_content() {
    let same = |id: i64| {
        let mut r = record(id, "suggested", 1);
        r.key = "shared".into();
        r
    };
    let mut other_tags = same(4);
    other_tags.tags = vec!["other".into()];
    let records = vec![
        same(1),
        record(2, "suggested", 1),
        same(3),
        other_tags,
        same(5),
    ];
    assert_eq!(
        super::queue::duplicate_counts(&records),
        vec![2, 0, 2, 0, 2]
    );
    assert!(super::queue::duplicate_counts(&[]).is_empty());
}
