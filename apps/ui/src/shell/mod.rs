//! The window shell: a task-oriented sidebar with one persistent workspace
//! switcher beside one page with its header and primary scroll region (see
//! docs/product-ui.md). Also the shared action helpers the feature pages use.
use crate::*;
use precept_ui_core::i18n::Msg;

pub(crate) mod chrome;
mod content;
mod header;
mod navigation;
mod sidebar;

pub(crate) use content::shell;
pub(crate) use navigation::{Destination, current, go, go_review, go_setup, workspace_scoped};
pub(crate) use sidebar::target_words;

pub(crate) fn word_action(
    model: Signal<Model>,
    key: Msg,
    enabled: impl Fn() -> bool + 'static,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    ui::button(model, "")
        .label_fn(move || model.get().word(key).into())
        .probe_key(move || format!("{key:?}"))
        .disabled(move || !enabled())
        .on_press(move || {
            if std::env::var_os("PRECEPT_UI_PROBE").is_some() {
                eprintln!("PRECEPT_UI_ACTION key={key:?}");
            }
            callback();
        })
}
pub(crate) fn control(
    model: Signal<Model>,
    key: Msg,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    word_action(model, key, || true, callback)
}
pub(crate) fn card(
    model: Signal<Model>,
    title: &'static str,
    lines: Vec<String>,
) -> impl IntoElement {
    ui::card(model, title, lines)
}
