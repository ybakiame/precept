use super::*;

#[test]
fn the_window_settings_come_from_nsh_json() {
    let o = window_options();
    assert_eq!(
        o.attributes.frame,
        Some(nsh_core::desktop::WindowFrameStyle::None)
    );
    let min = o.attributes.min_size.flatten().unwrap();
    assert_eq!((min.width.0, min.height.0), (960.0, 600.0));
    assert_eq!(default_size(), (1180, 780));
}
#[test]
fn preview_cannot_accept_live_or_arbitrary_inputs() {
    let args = |s: &[&str]| s.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let c = parse(&args(&[
        "--brand-preview",
        "b",
        "--preview-page",
        "appearance",
        "--locale",
        "zh-CN",
        "--appearance",
        "light",
    ]))
    .unwrap()
    .unwrap();
    assert_eq!(c.locale, Locale::ZhCn);
    assert_eq!(c.page, Page::Appearance);
    assert_eq!(c.theme, ThemeId::Ledger);
    for flag in [
        "--runtime-dir",
        "--endpoint",
        "--wsl-distribution",
        "--project",
        "--synthetic-rows",
        "--brand-preview",
        "--unknown",
    ] {
        assert!(parse(&args(&["--brand-preview", "a", flag, "x"])).is_err());
    }
    assert!(parse(&args(&["--brand-preview", "a", "--locale", "unsupported"])).is_err());
    assert!(parse(&args(&["--brand-preview"])).is_err());
}
#[component]
fn TestPreview() -> impl IntoElement {
    preview(Config::default())
}
#[test]
fn themes_and_language_preserve_scope_and_untranslated_evidence() {
    let mut app =
        nsh_testing::Headless::start(nsh_engine::EngineConfig::new(nsh_engine::Root::of::<
            TestPreview,
        >()))
        .unwrap();
    app.wait_for("Agent access").unwrap();
    app.press_key("alt+0").unwrap();
    app.press("Theme collection Ledger").unwrap();
    app.press("Light").unwrap();
    app.press("English / 简体中文").unwrap();
    app.wait_for("主题集").unwrap();
    app.press_key("alt+1").unwrap();
    app.wait_for("有效上下文").unwrap();
    app.press_key("alt+4").unwrap();
    app.wait_for("synthetic-alpha").unwrap();
    app.press("展开证据").unwrap();
    assert!(app.screen().texts().iter().any(|s| s.contains("record-id")));
    app.press_key("mod+l").unwrap();
    app.wait_for("Recorded objective").unwrap();
    app.wait_for("synthetic-alpha").unwrap();
    app.press_key("alt+1").unwrap();
    app.press("Current workspace beta").unwrap();
    app.press_key("alt+4").unwrap();
    app.wait_for("No task selected. Open the live Handoff page for an exact lookup.")
        .unwrap();
    assert!(
        !app.screen()
            .texts()
            .iter()
            .any(|s| s.contains("synthetic-alpha"))
    );
    assert!(app.screen().clipboard().is_none());
}
