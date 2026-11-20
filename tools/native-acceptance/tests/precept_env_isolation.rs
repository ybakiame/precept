//! A real child process must not see any inherited PRECEPT_* control variable,
//! whatever its case, while the explicitly re-added test variable survives.
//! Its own test binary with one test: setting process variables is race-free here.
use precept_native_acceptance::environment::clear_inherited_precept;
use std::process::Command;

#[test]
fn mixed_case_inherited_precept_controls_never_reach_a_child() {
    // SAFETY: the only test in this binary; no other thread reads the environment.
    unsafe {
        std::env::set_var("Precept_Ui_Gallery", "leaked");
        std::env::set_var("precept_ui_probe_id", "leaked");
        std::env::set_var("PRECEPT_PREVIEW_SYNTHETIC", "leaked");
        std::env::set_var("PRECEPTX_NOT_CONTROL", "kept");
    }
    let mut child = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/d", "/c", "set"]);
        c
    } else {
        Command::new("env")
    };
    clear_inherited_precept(&mut child).env("PRECEPT_UI_PROBE", "1");
    let output = child.output().expect("environment listing child");
    assert!(output.status.success());
    let listing = String::from_utf8_lossy(&output.stdout);
    let precept: Vec<_> = listing
        .lines()
        .filter_map(|line| line.split_once('='))
        // Independent oracle: never the helper under test.
        .filter(|(key, _)| key.to_ascii_lowercase().starts_with("precept_"))
        .collect();
    assert_eq!(precept, [("PRECEPT_UI_PROBE", "1")], "{listing}");
    assert!(listing.lines().any(|l| l == "PRECEPTX_NOT_CONTROL=kept"));
}
