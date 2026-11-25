use super::*;
use crate::ui::gallery::Gallery;
use nsh_engine::{EngineConfig, Root};
#[test]
fn the_top_bar_shows_the_version_and_hides_the_sidebar() {
    // A desktop window (the least nsh.json allows is wider than the
    // narrow layout, where the sidebar becomes a top row).
    let mut cfg = EngineConfig::new(Root::of::<App>());
    cfg.viewport = Size::new(px(1180.0), px(780.0));
    let mut app = nsh_testing::Headless::start(cfg).unwrap();
    app.wait_for("Project: alpha").unwrap();
    assert!(
        app.screen()
            .shows(&format!("v{}", env!("CARGO_PKG_VERSION")))
    );
    assert!(app.screen().shows("Rules"));
    app.press("Hide the sidebar").unwrap();
    app.wait_until("sidebar hidden", std::time::Duration::from_secs(5), |s| {
        !s.shows("Rules")
    })
    .unwrap();
    app.press("Show the sidebar").unwrap();
    app.wait_for("Rules").unwrap();
    app.press_key("mod+b").unwrap();
    app.wait_until(
        "hidden by keyboard",
        std::time::Duration::from_secs(5),
        |s| !s.shows("Rules"),
    )
    .unwrap();
}

#[test]
fn repository_rules_show_their_checks_and_open_in_the_dialog() {
    let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<App>())).unwrap();
    app.wait_for("Project: alpha").unwrap();
    app.press_key("alt+3").unwrap();
    app.wait_for("Repository rules").unwrap();
    assert!(app.screen().shows(".precept/rules/formatter.md"));
    assert!(app.screen().shows("Checks pass 1/1"));
    assert!(
        app.screen()
            .shows("1 rule file(s) not committed yet; they apply once committed.")
    );
    app.press("Details · formatter").unwrap();
    app.wait_for("Rust formatting").unwrap();
    app.press("Copy text").unwrap();
    assert_eq!(
        app.screen().clipboard(),
        Some("# Rust formatting\n\nRun `cargo fmt` before committing.")
    );
    app.press("Close").unwrap();
    app.wait_until("dialog closed", std::time::Duration::from_secs(5), |s| {
        !s.shows("Copy text")
    })
    .unwrap();
}

#[test]
fn preference_details_open_in_a_dialog_with_copy() {
    let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<App>())).unwrap();
    app.wait_for("Project: alpha").unwrap();
    app.press_key("alt+3").unwrap();
    app.wait_for("Confirmed preferences and conventions")
        .unwrap();
    app.press("Details · commit.style").unwrap();
    app.wait_for("Copy text").unwrap();
    assert!(app.screen().shows("Close"));
    app.press("Copy text").unwrap();
    assert_eq!(app.screen().clipboard(), Some("newest"));
    app.press("Close").unwrap();
    app.wait_until("dialog closed", std::time::Duration::from_secs(5), |s| {
        !s.shows("Copy text")
    })
    .unwrap();
}
#[test]
fn doctor_lists_what_it_saw_and_copies_a_report_without_paths() {
    let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<App>())).unwrap();
    app.wait_for("Project: alpha").unwrap();
    app.press("Diagnostics").unwrap();
    app.wait_for("Doctor").unwrap();
    // Demo data: nothing is claimed as working that was not read.
    app.wait_for("Demo data: no daemon is connected").unwrap();
    assert!(app.screen().shows("Not read"));
    assert!(app.screen().shows("Codex · MCP"));
    assert!(app.screen().shows("Codex · hook"));
    app.press("Copy report").unwrap();
    let report = app.screen().clipboard().unwrap();
    assert!(report.starts_with("Precept doctor · alpha\n"), "{report}");
    assert!(report.contains("[Not read] Daemon: Demo data"));
    assert!(!report.contains('/'), "no paths: {report}");
}
#[test]
fn component_inputs_selection_empty_scope_and_explicit_copy() {
    let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<App>())).unwrap();
    app.wait_for("Project: alpha").unwrap();
    assert!(app.screen().clipboard().is_none());
    app.press_key("mod+k").unwrap();
    app.type_text("Task ID", "shared").unwrap();
    app.press("Lookup").unwrap();
    app.wait_for("Recorded checks").unwrap();
    app.wait_for("Before you continue").unwrap();
    app.press("Copy visible page").unwrap();
    assert!(app.screen().clipboard().unwrap().contains("Handoff shared"));
    // The project menu: its trigger shows the current project.
    app.press("alpha").unwrap();
    app.press("beta").unwrap();
    app.wait_for("Project: beta").unwrap();
    app.wait_for("Handoff — enter a task ID and choose Lookup")
        .unwrap();
    app.type_text("Task ID", "missing").unwrap();
    app.press("Lookup").unwrap();
    app.wait_for("Empty — no handoff for task missing in this project")
        .unwrap();
    app.press_key("mod+f").unwrap();
    app.type_text("Filter preferences", "no-match").unwrap();
    app.wait_for("Empty — no confirmed preferences match")
        .unwrap();
    app.press_key("alt+2").unwrap();
    app.wait_for("Registered workspaces").unwrap();
    // Workspaces manages registrations only: no legacy summary card and no
    // click-to-switch button on workspace rows.
    assert!(
        !app.screen()
            .shows("Only the daemon's projects; switching clears the task lookup.")
    );
    app.press_key("mod+f").unwrap();
    app.press_key("mod+d").unwrap();
    app.press_key("mod+shift+h").unwrap();
    app.press_key("Escape").unwrap();
    for (key, label) in [
        ("alt+2", "Registered workspaces"),
        ("alt+3", "Confirmed preferences and conventions"),
        (
            "alt+4",
            "Empty — no handoff for task missing in this project",
        ),
        ("alt+5", "Doctor"),
    ] {
        app.press_key(key).unwrap();
        app.wait_for(label).unwrap();
    }
    app.set_viewport(Size::new(px(640.0), px(480.0))).unwrap();
    app.wait_for("Project: beta").unwrap();
    app.set_viewport(Size::new(px(1080.0), px(760.0))).unwrap();
}
#[test]
fn real_console_edits_connection_without_implicit_reads_and_localizes() {
    let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<App>())).unwrap();
    app.wait_for("Project: alpha").unwrap();
    app.press("Runtime checks").unwrap();
    // Connection has no bottom explanatory clutter text.
    assert!(!app.screen().shows("Checks only read status."));
    // Choosing this computer runs the read-only check; nothing connects yet.
    app.press("This computer").unwrap();
    if cfg!(unix) {
        app.wait_for("Checks").unwrap();
        app.wait_for("Daemon service").unwrap();
    } else {
        // Windows-local discovery checks the daemon beside the Desktop.
        app.wait_for("Daemon executable").unwrap();
    }
    app.press("Advanced").unwrap();
    // The check may already have filled the fields; replace them by hand.
    app.clear("Private runtime directory").unwrap();
    app.type_text("Private runtime directory", "relative/runtime")
        .unwrap();
    app.clear("IPC endpoint").unwrap();
    app.type_text("IPC endpoint", "synthetic").unwrap();
    assert_eq!(
        app.screen()
            .input_text("Private runtime directory")
            .unwrap(),
        Some("relative/runtime")
    );
    assert_eq!(
        app.screen().input_text("IPC endpoint").unwrap(),
        Some("synthetic")
    );
    app.press("Connect").unwrap();
    app.wait_for("Invalid connection fields; the active target is unchanged.")
        .unwrap();
    // Invalid drafts cannot clear a usable snapshot or replace its source.
    app.wait_for("Preview · Demo data").unwrap();
    assert!(!app.screen().shows("Demo data · Synthetic snapshot ready"));
    app.press("Demo data").unwrap();
    app.press("Use demo data").unwrap();
    app.press_key("Escape").unwrap();
    app.wait_for("Project: alpha").unwrap();
    app.press("Settings").unwrap();
    app.press("Ledger").unwrap();
    app.press("Light").unwrap();
    app.press("简体中文").unwrap();
    app.wait_for("主题集").unwrap();
    app.press_key("alt+3").unwrap();
    app.wait_for("已确认的偏好与约定").unwrap();
    app.wait_for("2 条已确认").unwrap();
    // Tags filter the list; several narrow to rows carrying all of them.
    app.press("#language · 1").unwrap();
    app.wait_for("1 条已确认").unwrap();
    assert!(app.screen().shows("zh-CN"));
    assert!(!app.screen().shows("newest"));
    app.press("#git · 1").unwrap();
    app.wait_for("0 条已确认").unwrap();
    app.wait_for("选择多个标签时，只显示同时带有这些标签的偏好。")
        .unwrap();
    app.press("全部").unwrap();
    app.wait_for("2 条已确认").unwrap();
    assert!(app.screen().clipboard().is_none());
}
#[component]
fn Workload() -> impl IntoElement {
    let mut data = precept_ui_core::Snapshot::fixture();
    let row = data.projects[0].context.preferences[0].clone();
    data.projects[0].context.preferences = (0..10_000)
        .map(|i| precept_ui_core::Preference {
            key: format!("event-{i:05}"),
            ..row.clone()
        })
        .collect();
    let mut model = Model::loading();
    model.section = precept_ui_core::Section::Rules;
    model.receive(Ok(data));
    console(signal(model))
}
#[test]
fn ten_thousand_rows_stay_virtual_while_scrolled_filtered_and_tagged() {
    let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<Workload>())).unwrap();
    app.wait_for("10000 confirmed").unwrap();
    let started = std::time::Instant::now();
    let shown = |app: &nsh_testing::Headless, key: &str| {
        app.screen().texts().iter().any(|t| t.contains(key))
    };
    assert!(shown(&app, "event-00000"));
    assert!(!shown(&app, "event-09999"));
    assert!(
        app.screen().texts().len() < 400,
        "screen tree grew with dataset"
    );
    // One continuous list: scrolling reveals later rows and keeps the tree bounded.
    app.scroll("event-00000", px(4000.0)).unwrap();
    app.wait_until("later rows", std::time::Duration::from_secs(5), |s| {
        s.texts()
            .iter()
            .any(|t| t.starts_with("event-000") && *t != "event-00000")
            && !s.texts().contains(&"event-00000")
    })
    .unwrap();
    assert!(app.screen().texts().len() < 400);
    app.type_text("Filter preferences", "event-09999").unwrap();
    app.wait_for("1 confirmed").unwrap();
    assert!(shown(&app, "event-09999"));
    // A tag the rows carry narrows nothing away; `#` matches tags only.
    app.clear("Filter preferences").unwrap();
    app.type_text("Filter preferences", "#git").unwrap();
    app.wait_for("10000 confirmed").unwrap();
    app.clear("Filter preferences").unwrap();
    app.type_text("Filter preferences", "#event").unwrap();
    app.wait_for("0 confirmed").unwrap();
    app.clear("Filter preferences").unwrap();
    app.type_text("Filter preferences", "event").unwrap();
    app.wait_for("10000 confirmed").unwrap();
    // Switch projects through the single sidebar workspace switcher.
    app.press("alpha").unwrap();
    app.press("beta").unwrap();
    app.press_key("alt+1").unwrap();
    app.wait_for("Project: beta").unwrap();
    println!(
        "RENDERED_WORKLOAD: 10000 rows in one virtual list, <400 text nodes, {:?}",
        started.elapsed()
    );
}

#[test]
fn shared_controls_expose_stable_ids_to_headless_interaction() {
    let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<Gallery>())).unwrap();
    app.wait_for("UI Gallery").unwrap();
    assert_eq!(app.screen().count("id:gallery.nav.overview").unwrap(), 1);
    app.press("id:gallery.nav.preferences").unwrap();
    app.wait_until(
        "gallery selection",
        std::time::Duration::from_secs(5),
        |s| {
            s.accessibility_state("id:gallery.nav.preferences")
                .unwrap()
                .selected
        },
    )
    .unwrap();
    app.press("id:gallery.nav.settings").unwrap();
    app.wait_until(
        "gallery selection changed",
        std::time::Duration::from_secs(5),
        |s| {
            s.accessibility_state("id:gallery.nav.settings")
                .unwrap()
                .selected
        },
    )
    .unwrap();
}

#[test]
fn navigation_separates_workspaces_review_rules_and_diagnostics() {
    let mut config = EngineConfig::new(Root::of::<App>());
    config.viewport = Size::new(px(1180.0), px(780.0));
    let mut app = nsh_testing::Headless::start(config).unwrap();
    app.wait_for("Project: alpha").unwrap();
    let texts = app.screen().texts();
    let position = |name: &str| texts.iter().position(|text| *text == name).unwrap();
    assert!(position("Workspace") < position("Set up"));
    assert!(position("Set up") < position("Workspaces"));
    assert!(position("Workspaces") < position("Agents"));
    assert!(position("Agents") < position("Personal sync"));
    assert_eq!(app.screen().count("id:nav.runtime-checks").unwrap(), 1);
    assert_eq!(app.screen().count("id:nav.personal-sync").unwrap(), 1);
    app.press("id:nav.runtime-checks").unwrap();
    app.wait_for("Connection").unwrap();
    assert_eq!(app.screen().count("id:setup.profile").unwrap(), 1);
    app.wait_for("1/2").unwrap();
    assert!(!app.screen().shows("Checks only read status."));
    app.press("id:setup.profile").unwrap();
    app.wait_for("Connect to a daemon first").unwrap();
    app.press("Go to Connection").unwrap();
    app.wait_for("Use demo data").unwrap();
    app.press("id:nav.workspaces").unwrap();
    app.wait_for("Registered workspaces").unwrap();
    assert!(
        !app.screen()
            .shows("Only the daemon's projects; switching clears the task lookup.")
    );
    assert_eq!(app.screen().count("role:button[beta]").unwrap(), 0);
    // Pending review and reviewed records are separate Context destinations.
    app.press("id:nav.preferences").unwrap();
    app.wait_for("Confirmed preferences and conventions")
        .unwrap();
    app.press("id:nav.review").unwrap();
    app.wait_for("Live daemon required").unwrap();
    assert!(!app.screen().shows("Review & rules"));
    // The two diagnostic cards precede Doctor.
    app.press("id:nav.diagnostics").unwrap();
    let texts = app.screen().texts();
    let daemon = texts.iter().position(|t| *t == "Daemon status").unwrap();
    let doctor = texts.iter().position(|t| *t == "Doctor").unwrap();
    assert!(daemon < doctor);
    // Agent access is a Set up destination and shows offline guidance on Demo data.
    app.press("id:nav.agents").unwrap();
    app.wait_for("Connect to a daemon first").unwrap();
    // The same automation id drives navigation after the visible language changes.
    app.press_key("mod+l").unwrap();
    app.wait_until(
        "localized dashboard navigation id",
        std::time::Duration::from_secs(5),
        |s| s.count("id:nav.dashboard").unwrap_or(0) == 1,
    )
    .unwrap();
    app.press("id:nav.dashboard").unwrap();
    app.wait_for("概览").unwrap();
    assert_eq!(app.screen().count("id:nav.dashboard").unwrap(), 1);
}

#[test]
fn personal_sync_is_a_separate_setup_destination_with_honest_offline_state() {
    let mut config = EngineConfig::new(Root::of::<App>());
    config.viewport = Size::new(px(1180.0), px(780.0));
    let mut app = nsh_testing::Headless::start(config).unwrap();
    app.wait_for("Project: alpha").unwrap();
    assert_eq!(app.screen().count("id:nav.personal-sync").unwrap(), 1);
    app.press("Personal sync").unwrap();
    app.wait_for("Move confirmed global preferences between paired devices through manual encrypted files. Nothing syncs automatically.")
        .unwrap();
    assert!(app.screen().shows("Status not read"));
    assert!(
        app.screen()
            .shows("Connect to a live daemon before using personal sync.")
    );
    assert!(app.screen().shows("Personal sync requires a live daemon."));
    app.press("Runtime checks").unwrap();
    app.wait_for("Connection").unwrap();
}
