#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
use nsh_platform_linux as platform;
#[cfg(target_os = "macos")]
use nsh_platform_macos as platform;
#[cfg(target_os = "windows")]
use nsh_platform_windows as platform;
#[cfg(target_os = "linux")]
mod linux_probe;
fn main() -> impl std::process::Termination {
    acp_nsh_spike::PROBE_STARTED
        .set(std::time::Instant::now())
        .ok();
    let preview = match acp_nsh_spike::brand_preview::configure() {
        Ok(value) => value,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    if !preview {
        acp_nsh_spike::INPUTS
            .set(acp_ui_spike_model::native::Inputs::parse())
            .ok();
    }
    #[cfg(target_os = "linux")]
    linux_probe::install();
    let mut title = String::from("Agent Control Plane — read-only console");
    if std::env::var_os("ACP_UI_PROBE").is_some()
        && let Ok(id) = std::env::var("ACP_UI_PROBE_ID")
        && id.len() == 16
        && id.bytes().all(|c| c.is_ascii_hexdigit())
    {
        title.push_str(&format!(" [{id}]"));
    }
    platform::run_app(
        platform::AppSource::Rust(if preview {
            platform::Root::of::<acp_nsh_spike::brand_preview::BrandPreview>()
                .main_window(acp_nsh_spike::brand_preview::window_options())
        } else {
            platform::Root::of::<acp_nsh_spike::App>()
        }),
        platform::AppOptions {
            title,
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            app_id: "org.example.acp.spike".into(),
            size: (1080, 760),
            storage: Some(acp_nsh_spike::ephemeral_storage()),
            cache: platform::CacheOptions::off(),
            ..Default::default()
        },
    )
}
