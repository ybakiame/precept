use super::*;
use nsh_engine::{EngineConfig, Root};
use precept_ui_core::{
    Model, connection::Target, rule_lint::LintReport, rule_templates::RuleTemplate,
};
use serde_json::json;

fn issue_report(count: usize) -> LintReport {
    let mut files = vec![json!({
        "file":".precept/rules/a.md","status":"issues","diagnostics":[{
            "code":"key.invalid","severity":"error","message":"The rule key is invalid.",
            "line":2,"column":1,"end_line":2,"end_column":4
        }]
    })];
    for start in (0..count).step_by(31) {
        let end = (start + 31).min(count);
        let diagnostics: Vec<_> = (start..end)
            .map(|index| {
                json!({
                    "code":"rule.duplicate_key", "severity":"warning",
                    "message":"Another rule file uses the same key.",
                    "line":index + 1,"column":1,"end_line":index + 1,"end_column":4,
                    "related_file":".precept/rules/a.md"
                })
            })
            .collect();
        let file = format!(
            ".precept/rules/{}.md",
            (b'b' + files.len() as u8 - 1) as char
        );
        files.push(json!({"file":file,"status":"issues","diagnostics":diagnostics}));
    }
    let file_count = files.len();
    LintReport::parse(json!({
        "format":"precept-rules-lint", "version":1, "status":"issues",
        "summary":{"files":file_count,"errors":1,"warnings":count},
        "files":files
    }))
    .unwrap()
}

fn model(report: Option<LintReport>, live: bool) -> Model {
    let mut model = Model::fixture();
    model.section = precept_ui_core::Section::Rules;
    model.live = live;
    if live {
        model.connection.target = Target::Local;
        model.connection.runtime = std::env::temp_dir().to_string_lossy().into_owned();
        model.connection.endpoint = "synthetic".into();
        model.select(0);
    }
    model.rule_lint.report = report;
    model.rule_lint.project = model.selected_project.clone();
    model
}

fn windowed(content: impl Fn() -> AnyElement + 'static) -> impl IntoElement {
    nsh_core::element::desktop::window()
        .id("main")
        .content(content)
        .into_element()
}

#[component]
fn DiagnosticsRules() -> impl IntoElement {
    let model = signal(model(Some(issue_report(3)), true));
    windowed(move || {
        view()
            .flex_1()
            .child(features::preferences::preferences(model))
            .child(templates_dialog(model))
            .into_element()
    })
}

#[component]
fn OfflineRules() -> impl IntoElement {
    let model = signal(model(None, false));
    windowed(move || {
        view()
            .flex_1()
            .child(features::preferences::preferences(model))
            .child(templates_dialog(model))
            .into_element()
    })
}

#[component]
fn TemplateRules() -> impl IntoElement {
    let model = signal(model(None, false));
    windowed(move || {
        view()
            .flex_1()
            .child(panel(model))
            .child(templates_dialog(model))
            .into_element()
    })
}

#[component]
fn CappedRules() -> impl IntoElement {
    let model = signal(model(Some(issue_report(45)), true));
    windowed(move || {
        view()
            .flex_1()
            .child(scroll().flex_1().child(panel(model)))
            .into_element()
    })
}

fn simple_report(status: &str) -> LintReport {
    let value = match status {
        "ok" => json!({
            "format":"precept-rules-lint","version":1,"status":"ok",
            "summary":{"files":1,"errors":0,"warnings":0},
            "files":[{"file":".precept/rules/rust.md","status":"ok","diagnostics":[]}]
        }),
        "not_configured" => json!({
            "format":"precept-rules-lint","version":1,"status":"not_configured",
            "summary":{"files":0,"errors":0,"warnings":0},"files":[]
        }),
        _ => json!({
            "format":"precept-rules-lint","version":1,"status":"unavailable",
            "summary":{"files":1,"errors":1,"warnings":0},
            "files":[{"file":".precept/rules","status":"unavailable","diagnostics":[{
                "code":"source.unavailable","severity":"error",
                "message":"The rule source is unavailable.",
                "line":1,"column":1,"end_line":1,"end_column":1
            }]}]
        }),
    };
    LintReport::parse(value).unwrap()
}

#[component]
fn StateRules() -> impl IntoElement {
    let model = signal(model(Some(simple_report("ok")), true));
    let action = move |label: &'static str, status: &'static str, error: bool| {
        ui::button(model, label)
            .size(ui::ButtonSize::Small)
            .on_press(move || {
                model.update(|m| {
                    m.phase = match status {
                        "waiting" => Phase::Loading,
                        "connection" => Phase::Unavailable,
                        _ => Phase::Ready,
                    };
                    m.rule_lint.busy = status == "busy";
                    m.rule_lint.error = error.then_some("Invalid rule lint report");
                    m.rule_lint.report = match status {
                        "busy" | "error" | "waiting" | "connection" => None,
                        _ => Some(simple_report(status)),
                    };
                })
            })
    };
    windowed(move || {
        view()
            .flex_1()
            .child(panel(model))
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .child(action("Show loading", "busy", false))
                    .child(action("Show waiting", "waiting", false))
                    .child(action("Show connection error", "connection", false))
                    .child(action("Show clean", "ok", false))
                    .child(action("Show not configured", "not_configured", false))
                    .child(action("Show unavailable", "unavailable", false))
                    .child(action("Show error", "error", true)),
            )
            .into_element()
    })
}

fn start(root: Root) -> nsh_testing::Headless {
    let mut config = EngineConfig::new(root);
    config.viewport = Size::new(px(1100.0), px(800.0));
    config.environment.os = nsh_core::platform::Os::Windows;
    config.environment.idiom = nsh_core::platform::Idiom::Desktop;
    nsh_testing::Headless::start(config).unwrap()
}

#[test]
fn diagnostics_render_as_named_plain_text_and_copy_the_validated_report() {
    let mut app = start(Root::of::<DiagnosticsRules>());
    app.wait_for("Rule diagnostics").unwrap();
    for text in [
        ".precept/rules/a.md",
        "Error",
        "key.invalid",
        "2:1-2:4",
        "The rule key is invalid.",
        "Warning",
        "rule.duplicate_key",
        "Related: .precept/rules/a.md",
    ] {
        assert!(
            app.screen().shows(text),
            "missing {text}: {:?}",
            app.screen().texts()
        );
    }
    assert_eq!(app.screen().count("role:button[Check rules]").unwrap(), 1);
    assert_eq!(app.screen().count("role:button[Templates…]").unwrap(), 1);
    app.press("role:button[Copy report]").unwrap();
    let copied = app.screen().clipboard().unwrap();
    assert!(copied.starts_with("Precept rule diagnostics\nStatus: issues found"));
    assert!(copied.contains("The rule key is invalid."));
    assert!(!copied.contains("<") && !copied.contains("**"));
}

#[test]
fn check_rules_is_named_and_disabled_offline_while_templates_remain_available() {
    let mut app = start(Root::of::<OfflineRules>());
    app.wait_for("Rule diagnostics").unwrap();
    assert!(!app.screen().enabled("role:button[Check rules]").unwrap());
    assert!(app.screen().enabled("role:button[Templates…]").unwrap());
    app.wait_for("Connect to a local or WSL daemon to check this project's rules.")
        .unwrap();
}

#[test]
fn loading_clean_not_configured_unavailable_and_error_states_are_distinct() {
    let mut app = start(Root::of::<StateRules>());
    app.wait_for("No rule issues found").unwrap();
    app.press("role:button[Show loading]").unwrap();
    app.wait_for("Checking repository rules…").unwrap();
    app.press("role:button[Show waiting]").unwrap();
    app.wait_for("Waiting for the selected project").unwrap();
    app.wait_for("Wait for the project read to finish, then check its repository rules.")
        .unwrap();
    assert!(!app.screen().enabled("role:button[Check rules]").unwrap());
    app.press("role:button[Show connection error]").unwrap();
    app.wait_for(
        "Connect to a local or WSL daemon and reload the project before checking its rules.",
    )
    .unwrap();
    app.press("role:button[Show clean]").unwrap();
    app.wait_for("No rule issues found").unwrap();
    app.press("role:button[Show not configured]").unwrap();
    app.wait_for("No repository rules configured").unwrap();
    app.press("role:button[Show unavailable]").unwrap();
    app.wait_for("Rule diagnostics unavailable").unwrap();
    app.press("role:button[Show error]").unwrap();
    app.wait_for("Rule check failed").unwrap();
    app.wait_for(
        "The daemon refused the check or returned an invalid report. Reconnect and try again.",
    )
    .unwrap();
}

#[test]
fn visible_diagnostics_are_capped_with_an_explicit_complete_copy_path() {
    let mut app = start(Root::of::<CappedRules>());
    app.wait_for("1 error(s) · 45 warning(s) · 3 file(s)")
        .unwrap();
    app.scroll("Rule diagnostics", px(5000.0)).unwrap();
    app.wait_for("Showing 40 of 46 diagnostics. Copy the report for more detail.")
        .unwrap();
    app.press("role:button[Copy report]").unwrap();
    let copied = app.screen().clipboard().unwrap();
    assert_eq!(copied.matches("[Warning] rule.duplicate_key").count(), 45);
    assert!(copied.chars().count() <= 24 * 1024);
}

#[test]
fn template_choices_preview_exact_text_copy_and_dismiss_without_writes() {
    let mut app = start(Root::of::<TemplateRules>());
    app.wait_for("role:button[Templates…]").unwrap();
    app.press("role:button[Templates…]").unwrap();
    app.wait_for("Rule templates").unwrap();
    app.wait_for("Precept does not write these files. Copy a template, review it, then commit it in the repository.")
        .unwrap();
    for name in ["Rust", "Git", "Security", "Testing"] {
        assert_eq!(app.screen().count(&format!("role:tab[{name}]")).unwrap(), 1);
    }
    app.wait_for(".precept/rules/rust.md").unwrap();
    assert!(
        app.screen()
            .texts()
            .iter()
            .any(|text| *text == RuleTemplate::Rust.text())
    );
    app.press("role:tab[Git]").unwrap();
    app.wait_for(".precept/rules/git.md").unwrap();
    assert!(
        app.screen()
            .texts()
            .iter()
            .any(|text| *text == RuleTemplate::Git.text())
    );
    assert!(!app.screen().shows(".precept/rules/rust.md"));
    app.press("role:button[Copy template]").unwrap();
    assert_eq!(app.screen().clipboard(), Some(RuleTemplate::Git.text()));
    app.press("role:button[Done]").unwrap();
    app.wait_for_gone("Rule templates", std::time::Duration::from_secs(5))
        .unwrap();
    app.press("role:button[Templates…]").unwrap();
    app.wait_for("Rule templates").unwrap();
    app.press("role:tab[Security]").unwrap();
    app.wait_for(".precept/rules/security.md").unwrap();
    app.press_key("Escape").unwrap();
    app.wait_for_gone("Rule templates", std::time::Duration::from_secs(5))
        .unwrap();
}

#[test]
fn template_entry_and_choices_are_keyboard_reachable_with_accessible_names() {
    let mut app = start(Root::of::<TemplateRules>());
    app.wait_for("role:button[Templates…]").unwrap();
    app.press_key("Tab").unwrap();
    app.press_key("Enter").unwrap();
    app.wait_for("Rule templates").unwrap();
    assert!(
        app.screen()
            .accessibility_state("role:tab[Rust]")
            .unwrap()
            .selected
    );
    app.press_key("Tab").unwrap();
    app.press_key("ArrowRight").unwrap();
    app.press_key("Enter").unwrap();
    app.wait_for(".precept/rules/git.md").unwrap();
    assert!(
        app.screen()
            .accessibility_state("role:tab[Git]")
            .unwrap()
            .selected
    );
    assert!(
        !app.screen()
            .accessibility_state("role:tab[Rust]")
            .unwrap()
            .selected
    );
    app.press_key("Escape").unwrap();
    app.wait_for_gone("Rule templates", std::time::Duration::from_secs(5))
        .unwrap();
}
