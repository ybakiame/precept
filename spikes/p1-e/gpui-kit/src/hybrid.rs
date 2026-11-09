//! Minimal real embedding gate: a Rust NSH surface inside the Kit host.
use gpui_kit::*;
use nsh_platform_gpui::{App, AppOptions, AppView, Root};
pub fn surface(window: &mut Window, cx: &mut Context<super::Console>) -> Entity<AppView> {
    cx.new(|cx| {
        let view = AppView::new(AppOptions::default(), window, cx);
        view.load(App::Rust(Root::of::<acp_nsh_spike::App>()));
        view
    })
}
