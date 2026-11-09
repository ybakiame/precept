#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
use nsh_platform_linux as platform;
#[cfg(target_os = "macos")]
use nsh_platform_macos as platform;
#[cfg(target_os = "windows")]
use nsh_platform_windows as platform;
fn main() -> impl std::process::Termination {
    acp_nsh_spike::INPUTS
        .set(acp_ui_spike_model::native::Inputs::parse())
        .ok();
    platform::run_app(
        platform::AppSource::Rust(platform::Root::of::<acp_nsh_spike::App>()),
        platform::AppOptions {
            title: "Agent Control Plane — read-only console".into(),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            app_id: "org.example.acp.spike".into(),
            size: (1080, 760),
            storage: Some(acp_nsh_spike::ephemeral_storage()),
            cache: platform::CacheOptions::off(),
            ..Default::default()
        },
    )
}
