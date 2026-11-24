use precept_ui_core::{
    appearance::{ColorMode, ThemeId},
    i18n::Locale,
};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Dashboard,
    Handoff,
    Appearance,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub theme: ThemeId,
    pub page: Page,
    pub locale: Locale,
    pub mode: ColorMode,
    pub contrast: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeId::Harbor,
            page: Page::Dashboard,
            locale: Locale::En,
            mode: ColorMode::Dark,
            contrast: false,
        }
    }
}
pub(super) static CONFIG: OnceLock<Config> = OnceLock::new();
pub fn parse(args: &[String]) -> Result<Option<Config>, &'static str> {
    if !args.iter().any(|a| a == "--brand-preview") {
        return Ok(None);
    }
    let mut config = Config::default();
    let mut seen = std::collections::HashSet::new();
    for pair in args.chunks(2) {
        if pair.len() != 2 || !seen.insert(pair[0].as_str()) {
            return Err("Preview requires unique flag/value pairs");
        }
        match pair[0].as_str() {
            "--brand-preview" => {
                config.theme = match pair[1].as_str() {
                    "a" => ThemeId::Instrument,
                    "b" => ThemeId::Ledger,
                    "c" => ThemeId::Harbor,
                    "d" => ThemeId::Graphite,
                    _ => return Err("Choose theme a, b, c or d"),
                }
            }
            "--preview-page" => {
                config.page = match pair[1].as_str() {
                    "dashboard" => Page::Dashboard,
                    "handoff" => Page::Handoff,
                    "appearance" => Page::Appearance,
                    _ => return Err("Unknown preview page"),
                }
            }
            "--locale" => {
                config.locale = Locale::parse(&pair[1]).ok_or("Supported UI locales: en, zh-CN")?
            }
            "--appearance" => {
                config.mode = match pair[1].as_str() {
                    "system" => ColorMode::System,
                    "light" => ColorMode::Light,
                    "dark" => ColorMode::Dark,
                    _ => return Err("Unknown appearance"),
                }
            }
            "--preview-contrast" => {
                config.contrast = match pair[1].as_str() {
                    "normal" => false,
                    "high" => true,
                    _ => return Err("Unknown contrast mode"),
                }
            }
            _ => return Err("Brand preview permits synthetic data only; no other inputs"),
        }
    }
    Ok(Some(config))
}
pub fn configure() -> Result<bool, &'static str> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let Some(config) = parse(&args)? else {
        return Ok(false);
    };
    CONFIG
        .set(config)
        .map_err(|_| "Preview already configured")?;
    Ok(true)
}
/// The main window's launch settings, from `nsh.json` `"window"` (the one
/// place they are set): no system frame, its default and least size.
pub fn window_options() -> nsh_core::desktop::MainWindowOptions {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../nsh.json")).expect("nsh.json is JSON");
    nsh_core::desktop::MainWindowOptions::from_json(&manifest["window"])
        .expect("nsh.json \"window\" is valid")
}
/// The default window size from `nsh.json`, in points.
pub fn default_size() -> (i32, i32) {
    let size = window_options().default_size;
    size.map_or((1180, 780), |s| (s.width.0 as i32, s.height.0 as i32))
}
