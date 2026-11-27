//! The section page is rebuilt only when what it shows changes: a long Rules
//! list must not be rebuilt by unrelated updates (a status, a read finishing),
//! which is what made switching context slow with many rules.
use super::*;
use nsh_engine::{EngineConfig, Root};
use std::sync::Arc;

fn many_rules(n: usize) -> Model {
    let mut model = Model::fixture();
    model.section = Section::Rules;
    let mut snapshot = (**model.data.as_ref().unwrap()).clone();
    let project = &mut snapshot.projects[0];
    let base = project.context.preferences[0].clone();
    project.context.preferences = (0..n)
        .map(|i| {
            let mut p = base.clone();
            p.key = format!("rule-{i:04}");
            p
        })
        .collect();
    model.data = Some(Arc::new(snapshot));
    model
}

/// The engine thread the test app renders on.
static UI_THREAD: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
fn builds() -> usize {
    let thread = UI_THREAD.lock().unwrap().clone().unwrap_or_default();
    PAGE_BUILDS
        .lock()
        .unwrap()
        .get(&thread)
        .copied()
        .unwrap_or(0)
}

#[component]
fn LongRules() -> impl IntoElement {
    *UI_THREAD.lock().unwrap() = Some(format!("{:?}", std::thread::current().id()));
    let model = signal(many_rules(500));
    nsh_core::element::desktop::window()
        .id("main")
        .content(move || {
            view()
                .flex_1()
                .child(
                    ui::button(model, "Unrelated update")
                        .on_press(move || model.update(|m| m.agents.busy = !m.agents.busy)),
                )
                .child(
                    ui::button(model, "Filter")
                        .on_press(move || model.update(|m| m.filter = "rule-01".into())),
                )
                .child(
                    view()
                        .h(px(600.0))
                        .child(scroll().child(section_page(model))),
                )
                .into_element()
        })
        .into_element()
}

#[test]
fn unrelated_updates_do_not_rebuild_a_long_rules_page() {
    let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<LongRules>())).unwrap();
    app.wait_for("rule-0000").unwrap();
    let built = builds();
    for _ in 0..20 {
        app.press("Unrelated update").unwrap();
    }
    assert_eq!(builds(), built, "an unrelated update rebuilt the page");
    // What the page shows still rebuilds it.
    app.press("Filter").unwrap();
    app.wait_until("filtered", std::time::Duration::from_secs(5), |s| {
        !s.texts().contains(&"rule-0000")
    })
    .unwrap();
    assert!(builds() > built);
}
