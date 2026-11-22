//! B0 revision: one information architecture, theme collection, bilingual UI.
//! This root only consumes a built-in synthetic snapshot. Native IPC is never initialized.
use nsh_base::button;
use nsh_core::{
    desktop::{MainWindowOptions, ResizeEdge, WindowMode},
    element::desktop::current_window,
    prelude::*,
};
use precept_ui_core::{
    Model,
    appearance::{ColorMode, Colors, ThemeId},
    i18n::{Locale, Msg, tr},
    presentation::sanitize,
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
static CONFIG: OnceLock<Config> = OnceLock::new();
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
pub fn window_options() -> MainWindowOptions {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../nsh.json")).expect("nsh.json is JSON");
    MainWindowOptions::from_json(&manifest["window"]).expect("nsh.json \"window\" is valid")
}
/// The default window size from `nsh.json`, in points.
pub fn default_size() -> (i32, i32) {
    let size = window_options().default_size;
    size.map_or((1180, 780), |s| (s.width.0 as i32, s.height.0 as i32))
}
fn compact() -> bool {
    crate::ui::tokens::compact()
}
fn stack_cards() -> bool {
    window().width.0 < 580.0
}
fn color(value: u32) -> AdaptiveColor {
    AdaptiveColor::rgb(value, value)
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Ui {
    config: Config,
    expanded: bool,
}
impl Ui {
    fn word(self, key: Msg) -> &'static str {
        tr(self.config.locale, key)
    }
    fn colors(self) -> Colors {
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
fn set_mode(ui: Signal<Ui>, mode: ColorMode) {
    ui.update(|s| s.config.mode = mode);
    set_color_scheme(match mode {
        ColorMode::System => Appearance::System,
        ColorMode::Light => Appearance::Light,
        ColorMode::Dark => Appearance::Dark,
    });
}
fn action(
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
fn muted(ui: Signal<Ui>, value: impl Into<String>) -> impl IntoElement + Styled {
    text(value.into())
        .text_size(px(12.0))
        .text_color(move || color(ui.get().colors().muted))
}
fn panel(ui: Signal<Ui>, title: Msg, lines: Vec<String>) -> impl IntoElement {
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

// Paths are centered in a 24×24 viewBox. No font glyph metrics or baseline offset.
fn caption(
    ui: Signal<Ui>,
    key: Msg,
    path: &'static str,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    let markup = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path d='{path}' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'/></svg>"
    );
    button()
        .accessibility_label(ui.get().word(key))
        .w(px(40.0))
        .h(px(36.0))
        .p_0()
        .flex_row()
        .items_center()
        .justify_center()
        .border_2()
        .bg(move || color(ui.get().colors().panel))
        .text_color(move || color(ui.get().colors().text))
        .border_color(move || color(ui.get().colors().panel))
        .hover(|s| s.bg(color(ui.get().colors().inset)))
        .focus(|s| s.border_color(color(ui.get().colors().accent)))
        .on_press(callback)
        .child(
            svg(nsh_core::element::ImageSrc::svg_markup(&markup))
                .w(px(14.0))
                .h(px(14.0)),
        )
}
fn titlebar(ui: Signal<Ui>) -> impl IntoElement {
    let minimize = current_window();
    let maximize = current_window();
    let close = current_window();
    let maximized = maximize
        .state()
        .is_some_and(|s| s.mode == WindowMode::Maximized);
    view()
        .flex_row()
        .h(px(36.0))
        .items_center()
        .bg(move || color(ui.get().colors().panel))
        .text_color(move || color(ui.get().colors().text))
        .child(
            view()
                .flex_1()
                .h_full()
                .px_3()
                .justify_center()
                .window_drag(true)
                .child(
                    text(format!("Precept / {}", ui.get().word(Msg::Preview))).text_size(px(12.0)),
                ),
        )
        .child(caption(ui, Msg::Minimize, "M5 12H19", move || {
            minimize.minimize()
        }))
        .child(caption(
            ui,
            if maximized {
                Msg::Restore
            } else {
                Msg::Maximize
            },
            if maximized {
                "M8 5H19V16M5 8H16V19H5Z"
            } else {
                "M5 5H19V19H5Z"
            },
            move || {
                maximize.set_mode(
                    if maximize
                        .state()
                        .is_some_and(|s| s.mode == WindowMode::Maximized)
                    {
                        WindowMode::Normal
                    } else {
                        WindowMode::Maximized
                    },
                );
            },
        ))
        .child(caption(ui, Msg::Close, "M6 6L18 18M18 6L6 18", move || {
            close.close()
        }))
}
fn title(ui: Signal<Ui>, key: Msg) -> impl IntoElement {
    text(ui.get().word(key))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(22.0))
}
fn workspace_cards(ui: Signal<Ui>, model: Signal<Model>) -> impl IntoElement {
    let s = ui.get();
    let m = model.get();
    view()
        .flex_row()
        .gap_2()
        .style_when(stack_cards, |s| s.flex_col())
        .child(
            view()
                .flex_1()
                .p_3()
                .gap_2()
                .style_when(compact, |s| s.p_2().gap_1())
                .rounded(px(8.0))
                .border_1()
                .border_color(color(s.colors().border))
                .bg(color(s.colors().panel))
                .child(text(s.word(Msg::Projects)).font_weight(FontWeight::SEMIBOLD))
                .child(
                    view().flex_row().flex_wrap().gap_2().children(
                        m.project_names
                            .iter()
                            .cloned()
                            .enumerate()
                            .map(move |(index, name)| {
                                button()
                                    .px_2()
                                    .py_1()
                                    .rounded(px(6.0))
                                    .border_2()
                                    .accessibility_label(format!(
                                        "{} {name}",
                                        s.word(Msg::CurrentWorkspace)
                                    ))
                                    .selected(move || model.get().selected == index)
                                    .bg(move || {
                                        let p = ui.get().colors();
                                        color(if model.get().selected == index {
                                            p.accent
                                        } else {
                                            p.inset
                                        })
                                    })
                                    .text_color(move || {
                                        let p = ui.get().colors();
                                        color(if model.get().selected == index {
                                            p.on_accent
                                        } else {
                                            p.text
                                        })
                                    })
                                    .border_color(move || color(ui.get().colors().border))
                                    .focus(|s| s.border_color(color(ui.get().colors().text)))
                                    .on_press(move || {
                                        model.update(|m| {
                                            m.select(index);
                                            m.task_id.clear();
                                            m.queried_task.clear();
                                        })
                                    })
                                    .child(text(name))
                            }),
                    ),
                )
                .child(muted(ui, s.word(Msg::WorkspaceHelp)).style_when(compact, |s| s.hidden())),
        )
        .child(view().flex_1().child(panel(
            ui,
            Msg::ClientAccess,
            vec![
                s.word(Msg::ClientPresence).into(),
                s.word(Msg::Tools).into(),
                s.word(Msg::Transport).into(),
            ],
        )))
}
fn dashboard(ui: Signal<Ui>, model: Signal<Model>) -> impl IntoElement {
    let s = ui.get();
    let m = model.get();
    let p = m.project().expect("built-in preview snapshot");
    let r = &p.context.repository;
    let h = p.handoff.as_ref().filter(|h| h.task_id == m.queried_task);
    view()
        .gap_2()
        .child(workspace_cards(ui, model))
        .child(panel(
            ui,
            Msg::EffectiveContext,
            vec![
                format!(
                    "{} · {}",
                    s.word(Msg::CurrentWorkspace),
                    sanitize(&p.name, 120)
                ),
                format!(
                    "{}: {} · HEAD: {}",
                    s.word(Msg::Branch),
                    r.branch.as_deref().unwrap_or(s.word(Msg::Unknown)),
                    r.head.as_deref().unwrap_or(s.word(Msg::Unknown))
                ),
                format!(
                    "{}: {}",
                    s.word(Msg::Worktree),
                    s.word(match r.dirty {
                        Some(true) => Msg::HasChanges,
                        Some(false) => Msg::NoChanges,
                        None => Msg::Unknown,
                    })
                ),
                format!(
                    "{}: {} · {}",
                    s.word(Msg::ContractReferences),
                    p.context.documents.len(),
                    s.word(Msg::MetadataOnly)
                ),
                format!(
                    "{}: {} · {}",
                    s.word(Msg::ConfirmedPreferences),
                    p.context.preferences.len(),
                    s.word(Msg::ConfirmedOnly)
                ),
            ],
        ))
        .child(panel(
            ui,
            Msg::ResumeSummary,
            vec![
                h.map_or_else(
                    || s.word(Msg::NoHandoff).into(),
                    |h| {
                        format!(
                            "{}: {} · {}",
                            s.word(Msg::ExactTask),
                            sanitize(&h.task_id, 120),
                            sanitize(&h.objective, 160)
                        )
                    },
                ),
                s.word(Msg::EarlierNotCurrent).into(),
            ],
        ))
        .child(action(
            ui,
            Msg::OpenHandoff,
            || false,
            move || ui.update(|s| s.config.page = Page::Handoff),
        ))
        .child(panel(
            ui,
            Msg::ReviewSuggestions,
            vec![
                s.word(Msg::ReviewDeferred).into(),
                s.word(Msg::ReviewHelp).into(),
            ],
        ))
        .child(muted(ui, s.word(Msg::ClientHelp)))
}
fn handoff_page(ui: Signal<Ui>, model: Signal<Model>) -> impl IntoElement {
    let s = ui.get();
    let m = model.get();
    let p = m.project().expect("built-in preview snapshot");
    let Some(h) = p.handoff.as_ref().filter(|h| h.task_id == m.queried_task) else {
        return view().child(panel(ui, Msg::Handoff, vec![s.word(Msg::NoHandoff).into()]));
    };
    let mut body = view()
        .gap_2()
        .child(muted(
            ui,
            format!(
                "{} / {} / {}",
                s.word(Msg::ExactTask),
                sanitize(&p.name, 120),
                sanitize(&h.task_id, 120)
            ),
        ))
        .child(panel(ui, Msg::Objective, vec![h.objective.clone()]))
        .child(panel(ui, Msg::NextAction, vec![h.next_action.clone()]))
        .child(
            view()
                .p_3()
                .gap_1()
                .style_when(compact, |s| s.p_2())
                .rounded(px(8.0))
                .border_2()
                .border_color(color(s.colors().warning))
                .child(
                    text(s.word(Msg::CurrentUncertainty))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(color(s.colors().warning)),
                )
                .child(text(format!(
                    "HEAD / {}: {}",
                    s.word(Msg::Fingerprint),
                    s.word(Msg::Unknown)
                )))
                .child(muted(ui, s.word(Msg::EarlierNotCurrent))),
        )
        .child(panel(
            ui,
            Msg::RecordedEvidence,
            vec![
                format!("{}: {}", s.word(Msg::RecordedEvidence), h.validation.len()),
                s.word(Msg::CurrentValidation).into(),
                s.word(Msg::EvidenceHelp).into(),
            ],
        ))
        .child(action(
            ui,
            if s.expanded {
                Msg::CollapseEvidence
            } else {
                Msg::ExpandEvidence
            },
            || false,
            move || ui.update(|s| s.expanded = !s.expanded),
        ));
    if s.expanded {
        body = body.child(
            view().p_2().gap_1().bg(color(s.colors().inset)).children(
                h.validation
                    .iter()
                    .take(6)
                    .map(|entry| text(sanitize(&entry.to_string(), 512))),
            ),
        );
    }
    body.child(muted(ui, s.word(Msg::PlainData)))
}
fn theme_card(ui: Signal<Ui>, theme: ThemeId) -> impl IntoElement {
    let s = ui.get();
    let p = theme.colors(
        match s.config.mode {
            ColorMode::Dark => true,
            ColorMode::Light => false,
            ColorMode::System => dark(),
        },
        false,
    );
    button()
        .w(px(176.0))
        .p_2()
        .rounded(px(8.0))
        .border_2()
        .accessibility_label(format!("{} {}", s.word(Msg::ThemeCollection), theme.name()))
        .selected(move || ui.get().config.theme == theme)
        .bg(move || color(ui.get().colors().panel))
        .text_color(move || color(ui.get().colors().text))
        .border_color(move || {
            color(if ui.get().config.theme == theme {
                ui.get().colors().accent
            } else {
                ui.get().colors().border
            })
        })
        .focus(|s| s.border_color(color(ui.get().colors().text)))
        .on_press(move || ui.update(|s| s.config.theme = theme))
        .child(
            view()
                .gap_1()
                .child(
                    view()
                        .p_2()
                        .gap_2()
                        .rounded(px(6.0))
                        .bg(color(p.background))
                        .child(text(s.word(Msg::Sample)).text_color(color(p.text)))
                        .child(
                            view().flex_row().gap_1().children(
                                [p.text, p.accent, p.success, p.warning, p.error, p.muted]
                                    .into_iter()
                                    .map(|c| {
                                        view().w(px(10.0)).h(px(10.0)).rounded(px(5.0)).bg(color(c))
                                    }),
                            ),
                        ),
                )
                .child(text(move || {
                    format!(
                        "{}{}",
                        if ui.get().config.theme == theme {
                            "> "
                        } else {
                            ""
                        },
                        theme.name()
                    )
                })),
        )
}
fn appearance(ui: Signal<Ui>) -> impl IntoElement {
    let s = ui.get();
    view()
        .gap_3()
        .style_when(compact, |s| s.gap_2())
        .child(muted(ui, s.word(Msg::ThemeHelp)))
        .child(
            view().flex_row().flex_wrap().gap_2().children(
                ThemeId::ALL
                    .into_iter()
                    .map(move |theme| theme_card(ui, theme)),
            ),
        )
        .child(text(s.word(Msg::ColorMode)).font_weight(FontWeight::SEMIBOLD))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_1()
                .children(
                    [
                        (Msg::System, ColorMode::System),
                        (Msg::Light, ColorMode::Light),
                        (Msg::Dark, ColorMode::Dark),
                    ]
                    .into_iter()
                    .map(move |(key, mode)| {
                        action(
                            ui,
                            key,
                            move || ui.get().config.mode == mode,
                            move || set_mode(ui, mode),
                        )
                    }),
                )
                .child(action(
                    ui,
                    if s.config.contrast {
                        Msg::ContrastOn
                    } else {
                        Msg::ContrastOff
                    },
                    move || ui.get().config.contrast,
                    move || ui.update(|s| s.config.contrast = !s.config.contrast),
                )),
        )
        .child(muted(ui, s.word(Msg::SystemHelp)))
        .child(text(s.word(Msg::Language)).font_weight(FontWeight::SEMIBOLD))
        .child(
            button()
                .px_3()
                .py_1()
                .rounded(px(6.0))
                .border_2()
                .border_color(color(s.colors().border))
                .bg(color(s.colors().inset))
                .text_color(color(s.colors().text))
                .accessibility_label("English / 简体中文")
                .focus(|s| s.border_color(color(ui.get().colors().text)))
                .on_press(move || {
                    ui.update(|s| {
                        s.config.locale = if s.config.locale == Locale::En {
                            Locale::ZhCn
                        } else {
                            Locale::En
                        }
                    })
                })
                .child(text(if s.config.locale == Locale::En {
                    "English → 简体中文"
                } else {
                    "简体中文 → English"
                })),
        )
        .child(muted(ui, s.word(Msg::FontHelp)))
        .child(muted(ui, s.word(Msg::SessionOnly)))
        .child(muted(ui, s.word(Msg::PlainData)))
}
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
        .child(view().child(dynamic(move || titlebar(ui))))
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
                    let mut body = view().gap_2().child(title(
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
                        Page::Appearance => body.child(appearance(ui)),
                    };
                    body.on_layout(move |_: &Bounds<Points>| {
                        if std::env::var_os("PRECEPT_UI_PROBE").is_some() {
                            eprintln!(
                                "PRECEPT_NSH_LAYOUT_READY brand_preview=true elapsed={:.6}",
                                super::probe_elapsed()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_settings_come_from_nsh_json() {
        let o = window_options();
        assert_eq!(
            o.attributes.frame,
            Some(nsh_core::desktop::WindowFrameStyle::None)
        );
        let min = o.attributes.min_size.flatten().unwrap();
        assert_eq!((min.width.0, min.height.0), (960.0, 600.0));
        assert_eq!(default_size(), (1180, 780));
    }
    #[test]
    fn preview_cannot_accept_live_or_arbitrary_inputs() {
        let args = |s: &[&str]| s.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let c = parse(&args(&[
            "--brand-preview",
            "b",
            "--preview-page",
            "appearance",
            "--locale",
            "zh-CN",
            "--appearance",
            "light",
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(c.locale, Locale::ZhCn);
        assert_eq!(c.page, Page::Appearance);
        assert_eq!(c.theme, ThemeId::Ledger);
        for flag in [
            "--runtime-dir",
            "--endpoint",
            "--wsl-distribution",
            "--project",
            "--synthetic-rows",
            "--brand-preview",
            "--unknown",
        ] {
            assert!(parse(&args(&["--brand-preview", "a", flag, "x"])).is_err());
        }
        assert!(parse(&args(&["--brand-preview", "a", "--locale", "unsupported"])).is_err());
        assert!(parse(&args(&["--brand-preview"])).is_err());
    }
    #[component]
    fn TestPreview() -> impl IntoElement {
        preview(Config::default())
    }
    #[test]
    fn themes_and_language_preserve_scope_and_untranslated_evidence() {
        let mut app =
            nsh_testing::Headless::start(nsh_engine::EngineConfig::new(nsh_engine::Root::of::<
                TestPreview,
            >()))
            .unwrap();
        app.wait_for("Agent access").unwrap();
        app.press_key("alt+0").unwrap();
        app.press("Theme collection Ledger").unwrap();
        app.press("Light").unwrap();
        app.press("English / 简体中文").unwrap();
        app.wait_for("主题集").unwrap();
        app.press_key("alt+1").unwrap();
        app.wait_for("有效上下文").unwrap();
        app.press_key("alt+4").unwrap();
        app.wait_for("synthetic-alpha").unwrap();
        app.press("展开证据").unwrap();
        assert!(app.screen().texts().iter().any(|s| s.contains("record-id")));
        app.press_key("mod+l").unwrap();
        app.wait_for("Recorded objective").unwrap();
        app.wait_for("synthetic-alpha").unwrap();
        app.press_key("alt+1").unwrap();
        app.press("Current workspace beta").unwrap();
        app.press_key("alt+4").unwrap();
        app.wait_for("No task selected. Open the live Handoff page for an exact lookup.")
            .unwrap();
        assert!(
            !app.screen()
                .texts()
                .iter()
                .any(|s| s.contains("synthetic-alpha"))
        );
        assert!(app.screen().clipboard().is_none());
    }
}
