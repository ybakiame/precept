use super::chrome::compact;
use super::config::Config;
use nsh_base::button;
use nsh_core::prelude::*;
use precept_ui_core::{
    appearance::{ColorMode, Colors},
    i18n::{Msg, tr},
    presentation::sanitize,
};

pub(super) fn color(value: u32) -> AdaptiveColor {
    AdaptiveColor::rgb(value, value)
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Ui {
    pub(super) config: Config,
    pub(super) expanded: bool,
}
impl Ui {
    pub(super) fn word(self, key: Msg) -> &'static str {
        tr(self.config.locale, key)
    }
    pub(super) fn colors(self) -> Colors {
        self.config.theme.colors(
            match self.config.mode {
                ColorMode::System => dark(),
                ColorMode::Dark => true,
                ColorMode::Light => false,
            },
            self.config.contrast,
        )
    }
}
pub(super) fn set_mode(ui: Signal<Ui>, mode: ColorMode) {
    ui.update(|s| s.config.mode = mode);
    set_color_scheme(match mode {
        ColorMode::System => Appearance::System,
        ColorMode::Light => Appearance::Light,
        ColorMode::Dark => Appearance::Dark,
    });
}
pub(super) fn action(
    ui: Signal<Ui>,
    key: Msg,
    selected: impl Fn() -> bool + 'static,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    let chosen = computed(selected);
    button()
        .accessibility_label(ui.get().word(key))
        .selected(move || chosen.get())
        .px_2()
        .py_1()
        .rounded(px(6.0))
        .border_2()
        .bg(move || {
            let p = ui.get().colors();
            color(if chosen.get() { p.accent } else { p.inset })
        })
        .text_color(move || {
            let p = ui.get().colors();
            color(if chosen.get() { p.on_accent } else { p.text })
        })
        .border_color(move || color(ui.get().colors().border))
        .hover(|s| s.border_color(color(ui.get().colors().accent)))
        .focus(|s| s.border_color(color(ui.get().colors().text)))
        .on_press(callback)
        .child(text(ui.get().word(key)))
}
pub(super) fn muted(ui: Signal<Ui>, value: impl Into<String>) -> impl IntoElement + Styled {
    text(value.into())
        .text_size(px(12.0))
        .text_color(move || color(ui.get().colors().muted))
}
pub(super) fn panel(ui: Signal<Ui>, title: Msg, lines: Vec<String>) -> impl IntoElement {
    view()
        .p_3()
        .gap_2()
        .style_when(compact, |s| s.p_2().gap_1())
        .rounded(px(8.0))
        .border_1()
        .border_color(move || color(ui.get().colors().border))
        .bg(move || color(ui.get().colors().panel))
        .child(text(ui.get().word(title)).font_weight(FontWeight::SEMIBOLD))
        .children(lines.into_iter().map(|s| text(sanitize(&s, 512))))
}
pub(super) fn title(ui: Signal<Ui>, key: Msg) -> impl IntoElement {
    text(ui.get().word(key))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(22.0))
}
