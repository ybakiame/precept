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
    precept_ui::PROBE_STARTED
        .set(std::time::Instant::now())
        .ok();
    let preview = match precept_ui::brand_preview::configure() {
        Ok(value) => value,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    if !preview {
        precept_ui::PERSIST_SETTINGS.store(true, std::sync::atomic::Ordering::Relaxed);
        precept_ui::INPUTS
            .set(precept_ui_client::native::Inputs::parse())
            .ok();
    }
    #[cfg(target_os = "linux")]
    linux_probe::install();
    let mut title = String::from("Precept — local console");
    if std::env::var_os("PRECEPT_UI_PROBE").is_some()
        && let Ok(id) = std::env::var("PRECEPT_UI_PROBE_ID")
        && id.len() == 16
        && id.bytes().all(|c| c.is_ascii_hexdigit())
    {
        title.push_str(&format!(" [{id}]"));
    }
    platform::run_app(
        platform::AppSource::Rust(if preview {
            platform::Root::of::<precept_ui::brand_preview::BrandPreview>()
                .main_window(precept_ui::brand_preview::window_options())
        } else {
            platform::Root::of::<precept_ui::App>()
                .main_window(precept_ui::brand_preview::window_options())
        }),
        platform::AppOptions {
            title,
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            app_id: "org.precept.operator".into(),
            size: (1080, 760),
            storage: Some(precept_ui::ephemeral_storage()),
            cache: platform::CacheOptions::off(),
            ..Default::default()
        },
    )
}
