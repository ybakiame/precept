//! B0 revision: one information architecture, theme collection, bilingual UI.
//! This root only consumes a built-in synthetic snapshot. Native IPC is never initialized.
mod appearance;
mod chrome;
mod config;
mod controls;
mod dashboard;
mod handoff;

use chrome::compact;
use config::CONFIG;
pub use config::{Config, Page, configure, default_size, parse, window_options};
use controls::{Ui, action, color, set_mode};
use dashboard::dashboard;
use handoff::handoff_page;
use nsh_core::{desktop::ResizeEdge, prelude::*};
use precept_ui_core::{
    Model,
    appearance::ThemeId,
    i18n::{Locale, Msg},
};

#[cfg(test)]
mod tests;

fn toolbar(ui: Signal<Ui>) -> impl IntoElement {
    view().flex_row().flex_wrap().gap_1().children(
        [
            (Page::Dashboard, Msg::Dashboard),
            (Page::Handoff, Msg::Handoff),
            (Page::Appearance, Msg::Appearance),
        ]
        .into_iter()
        .map(move |(page, key)| {
            action(
                ui,
                key,
                move || ui.get().config.page == page,
                move || ui.update(|s| s.config.page = page),
            )
        }),
    )
}
#[component]
pub fn BrandPreview() -> impl IntoElement {
    preview(CONFIG.get().copied().unwrap_or_default())
}
fn preview(config: Config) -> impl IntoElement {
    let ui = signal(Ui {
        config,
        expanded: false,
    });
    let model = signal(Model::fixture());
    set_mode(ui, config.mode);
    effect(move || {
        if std::env::var_os("PRECEPT_UI_PROBE").is_some() {
            let s = ui.get().config;
            eprintln!(
                "PRECEPT_BRAND_UI theme={:?} page={:?} locale={:?} mode={:?} contrast={}",
                s.theme, s.page, s.locale, s.mode, s.contrast
            );
        }
    });
    let always = KeyOptions {
        when: nsh_core::keymap::When::Always,
        ..Default::default()
    };
    for (key, theme) in [
        ("alt+a", ThemeId::Instrument),
        ("alt+b", ThemeId::Ledger),
        ("alt+c", ThemeId::Harbor),
    ] {
        on_key_with(key, always, move |_| ui.update(|s| s.config.theme = theme));
    }
    for (key, page) in [
        ("alt+1", Page::Dashboard),
        ("alt+4", Page::Handoff),
        ("alt+0", Page::Appearance),
        ("Escape", Page::Dashboard),
    ] {
        on_key_with(key, always, move |_| ui.update(|s| s.config.page = page));
    }
    on_key_with("mod+l", always, move |_| {
        ui.update(|s| {
            s.config.locale = if s.config.locale == Locale::En {
                Locale::ZhCn
            } else {
                Locale::En
            }
        })
    });
    view()
        .flex_1()
        .font_family("sans-serif")
        .text_size(px(14.0))
        .bg(move || color(ui.get().colors().background))
        .text_color(move || color(ui.get().colors().text))
        .child(view().child(dynamic(move || chrome::titlebar(ui))))
        .child(
            view()
                .flex_1()
                .p_4()
                .gap_2()
                .style_when(compact, |s| s.p_2().gap_1())
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .items_center()
                        .child(
                            text("Precept")
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(18.0)),
                        )
                        .child(dynamic(move || toolbar(ui))),
                )
                .child(
                    text(move || {
                        format!("OFFLINE_SYNTHETIC · {}", ui.get().word(Msg::PreviewNote))
                    })
                    .text_size(px(12.0))
                    .text_color(move || color(ui.get().colors().muted)),
                )
                .child(scroll().flex_1().child(dynamic(move || {
                    let s = ui.get();
                    let mut body = view().gap_2().child(controls::title(
                        ui,
                        match s.config.page {
                            Page::Dashboard => Msg::Dashboard,
                            Page::Handoff => Msg::Handoff,
                            Page::Appearance => Msg::ThemeCollection,
                        },
                    ));
                    body = match s.config.page {
                        Page::Dashboard => body.child(dashboard(ui, model)),
                        Page::Handoff => body.child(handoff_page(ui, model)),
                        Page::Appearance => body.child(appearance::appearance(ui)),
                    };
                    body.on_layout(move |_: &Bounds<Points>| {
                        if std::env::var_os("PRECEPT_UI_PROBE").is_some() {
                            eprintln!(
                                "PRECEPT_NSH_LAYOUT_READY brand_preview=true elapsed={:.6}",
                                crate::probe_elapsed()
                            );
                        }
                    })
                })))
                .child(
                    text(move || ui.get().word(Msg::Shortcuts))
                        .text_size(px(12.0))
                        .text_color(move || color(ui.get().colors().muted)),
                ),
        )
        .child(
            view().flex_row().justify_end().child(
                view()
                    .w(px(18.0))
                    .h(px(12.0))
                    .window_resize(ResizeEdge::BottomRight)
                    .accessible(true)
                    .label(move || ui.get().word(Msg::Resize))
                    .child(
                        text("⋱")
                            .text_size(px(12.0))
                            .text_color(move || color(ui.get().colors().muted)),
                    ),
            ),
        )
}
