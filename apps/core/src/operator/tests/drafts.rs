use super::*;

#[test]
fn a_saved_preference_opens_as_the_exact_record_to_confirm() {
    let mut m = model();
    m.operator.key = "editor".into();
    m.operator.value = "synthetic".into();
    m.operator.tags = vec!["style".into()];
    m.operator.origin = Origin::Agent;
    assert!(m.operator_review("suggest", "s-op"));
    // The suggestion carries its tags and origin; a new tag keeps an oklch() color.
    assert!(matches!(
        &m.operator.review.as_ref().unwrap().mutation.as_ref().unwrap().command,
        Command::SuggestPreference { tags, origin: Origin::Agent, .. } if tags == &["style"]
    ));
    assert_eq!(m.tag_colors["style"], crate::tag_colors::generated("style"));
    let request = m.operator_confirm().unwrap();
    let affected =
        json!([{"kind":"preference","project":"","record_id":"9","revision":1,"deleted":false}]);
    assert!(m.complete_operator(&request, Ok(receipt_json("s-op", 4, affected))));
    // Step one is done (saved as suggested); nothing is confirmed.
    assert_eq!(m.operator.follow, Some(9));
    // The inbox (default) finds it in the queue; a scoped page in its list.
    assert_eq!(m.operator.follow_up, Some("profile.review.queue"));
    let next = m.operator.follow_up.take().unwrap();
    let read = m.operator_read(next).unwrap();
    assert_eq!(read.method, "profile.review.queue");
    let queue = queue_of(
        vec![
            synthetic_record(3, &Scope::Global, "suggested", 1),
            synthetic_record(9, &Scope::Global, "suggested", 1),
        ],
        5,
        None,
    );
    assert!(m.complete_operator(&read, Ok(serde_json::to_value(queue).unwrap())));
    // Step two is offered: the exact record, in Detail, to inspect and confirm.
    assert_eq!(m.operator.selected_id, Some(9));
    assert_eq!(m.operator.review_tab, ReviewTab::Detail);
    assert_eq!(m.operator.follow_up, Some("profile.review.inspect"));
    assert!(m.operator.follow.is_none());
    // The scope's record list opens it the same way (a scoped page reloads
    // its list after the receipt, possibly before the queue read).
    let mut m = model();
    m.operator.page = None;
    m.operator.follow = Some(7);
    let read = m.operator_read("profile.preferences.list").unwrap();
    let page = PreferencePage {
        binding: binding(),
        records: vec![synthetic_record(7, &Scope::Global, "suggested", 1)],
        next_after_id: None,
    };
    assert!(m.complete_operator(&read, Ok(serde_json::to_value(page).unwrap())));
    assert_eq!(m.operator.selected_id, Some(7));
    assert_eq!(m.operator.follow_up, Some("profile.review.inspect"));
    // A refresh of the same exact record keeps the selection and its
    // inspection; a changed revision clears both.
    m.operator.inspect = None;
    let read = m.operator_read("profile.preferences.list").unwrap();
    let same = PreferencePage {
        binding: binding(),
        records: vec![synthetic_record(7, &Scope::Global, "suggested", 1)],
        next_after_id: None,
    };
    assert!(m.complete_operator(&read, Ok(serde_json::to_value(same).unwrap())));
    assert_eq!(m.operator.selected_id, Some(7));
    let read = m.operator_read("profile.preferences.list").unwrap();
    let changed = PreferencePage {
        binding: binding(),
        records: vec![synthetic_record(7, &Scope::Global, "confirmed", 2)],
        next_after_id: None,
    };
    assert!(m.complete_operator(&read, Ok(serde_json::to_value(changed).unwrap())));
    assert_eq!(m.operator.selected_id, None);
    // A refresh that does not show it never selects something else.
    let mut m = model();
    m.operator.follow = Some(42);
    assert!(complete_queue(&mut m, queue_of(vec![], 5, None)));
    assert!(m.operator.follow.is_none() && m.operator.follow_up.is_none());
}
