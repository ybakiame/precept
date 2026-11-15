//! Minimal real embedding gate: a Rust NSH surface inside the Kit host.
use gpui_kit::*;
use nsh_platform_gpui::{App, AppOptions, AppView, Root};
pub fn surface(window: &mut Window, cx: &mut Context<super::Console>) -> Entity<AppView> {
    cx.new(|cx| {
        let options = AppOptions {
            storage: Some(acp_operator_ui::ephemeral_storage()),
            cache: nsh_platform_gpui::CacheOptions::off(),
            ..Default::default()
        };
        let view = AppView::new(options, window, cx);
        view.load(App::Rust(Root::of::<acp_operator_ui::App>()));
        view
    })
}
