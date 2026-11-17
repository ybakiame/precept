//! A real child process must not see any inherited ACP_* control variable,
//! whatever its case, while the explicitly re-added test variable survives.
//! Its own test binary with one test: setting process variables is race-free here.
use acp_native_acceptance::environment::clear_inherited_acp;
use std::process::Command;

#[test]
fn mixed_case_inherited_acp_controls_never_reach_a_child() {
    // SAFETY: the only test in this binary; no other thread reads the environment.
    unsafe {
        std::env::set_var("Acp_Ui_Gallery", "leaked");
        std::env::set_var("acp_ui_probe_id", "leaked");
        std::env::set_var("ACP_PREVIEW_SYNTHETIC", "leaked");
        std::env::set_var("ACPX_NOT_CONTROL", "kept");
    }
    let mut child = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/d", "/c", "set"]);
        c
    } else {
        Command::new("env")
    };
    clear_inherited_acp(&mut child).env("ACP_UI_PROBE", "1");
    let output = child.output().expect("environment listing child");
    assert!(output.status.success());
    let listing = String::from_utf8_lossy(&output.stdout);
    let acp: Vec<_> = listing
        .lines()
        .filter_map(|line| line.split_once('='))
        // Independent oracle: never the helper under test.
        .filter(|(key, _)| key.to_ascii_lowercase().starts_with("acp_"))
        .collect();
    assert_eq!(acp, [("ACP_UI_PROBE", "1")], "{listing}");
    assert!(listing.lines().any(|l| l == "ACPX_NOT_CONTROL=kept"));
}
