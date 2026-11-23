//! The operator's presentation and interaction surface for the NSH desktop.
use nsh_base::button;
use nsh_core::prelude::*;
use precept_ui_core::{Model, Phase};
mod features;
mod shell;
mod shortcuts;
mod ui;

pub use features::brand_preview;
use features::review;
use ui::{icons, tokens};
pub static DESKTOP_RUNTIME: std::sync::OnceLock<(String, String)> = std::sync::OnceLock::new();
fn runtime_lines(locale: precept_ui_core::i18n::Locale) -> Vec<String> {
    use precept_ui_core::i18n::{Msg, tr};
    if let Some((backend, renderer)) = DESKTOP_RUNTIME.get() {
        return vec![
            format!("Actual GTK backend: {backend} · renderer: {renderer}"),
            tr(locale, Msg::RendererHelp).into(),
        ];
    }
    vec![
        if cfg!(target_os = "windows") {
            "Backend: NSH native Windows"
        } else {
            tr(locale, Msg::BackendUnobserved)
        }
        .into(),
    ]
}
pub static PROBE_STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
/// Set by the desktop binary: the operator's settings are read
/// from and written to the private `settings.toml`. Tests and previews never
/// touch the developer's settings.
pub static PERSIST_SETTINGS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(all(feature = "ipc", not(test)))]
fn persist_settings() -> bool {
    PERSIST_SETTINGS.load(std::sync::atomic::Ordering::Relaxed)
}
pub fn probe_elapsed() -> f64 {
    PROBE_STARTED
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs_f64()
}

pub fn ephemeral_storage() -> std::sync::Arc<dyn nsh_core::storage::StorageBackend> {
    struct Empty;
    impl nsh_core::storage::StorageBackend for Empty {
        fn load(&self) -> Result<std::collections::BTreeMap<String, String>, String> {
            Ok(Default::default())
        }
        fn write(&self, _: nsh_core::storage::StorageChange) {}
    }
    std::sync::Arc::new(Empty)
}

#[native_module("precept-ui/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

#[cfg(feature = "ipc")]
pub static INPUTS: std::sync::OnceLock<precept_ui_client::native::Inputs> =
    std::sync::OnceLock::new();

fn load(model: Signal<Model>) {
    #[cfg(feature = "ipc")]
    let inputs = precept_ui_client::native::Inputs::from_connection(&model.get().connection);
    let started = nsh_core::time::now();
    let mut request = None;
    model.update(|m| {
        request = m.begin_read().ok();
    });
    let Some(request) = request else { return };
    // Detached: a switch from the sidebar's project menu starts this read,
    // and the menu's owner is disposed when it closes. The epoch fences a
    // late completion instead.
    Owner::detached().run(|| {
        spawn(async move {
            #[cfg(feature = "ipc")]
            let selected = request.clone();
            #[cfg(feature = "ipc")]
            let result = blocking(move || {
                inputs.and_then(|inputs| {
                    inputs.fetch_for(selected.project.as_deref(), &selected.task)
                })
            })
            .await
            .unwrap_or(Err("Client unavailable"));
            #[cfg(not(feature = "ipc"))]
            let result = {
                timer(std::time::Duration::from_millis(20)).await;
                if model.get().live {
                    Err("Client unavailable")
                } else {
                    let mut s = precept_ui_core::Snapshot::fixture();
                    for p in &mut s.projects {
                        if p.handoff
                            .as_ref()
                            .is_some_and(|h| h.task_id != request.task)
                        {
                            p.handoff = None;
                        }
                    }
                    Ok(s)
                }
            };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|t| t.as_millis() as u64)
                .unwrap_or_default();
            model.update(|m| {
                if m.complete_read(&request, result, now) {
                    m.last_duration_ms = Some(started.elapsed().as_millis() as u64);
                }
            });
            // Re-open in the entry the operator chose (a read finishing after a
            // click must not turn Personal preferences into To review).
            let (open, mode) = {
                let m = model.get();
                (m.operator.open, m.operator.mode)
            };
            if open {
                review::open_mode(model, mode);
            }
        })
    });
}
pub(crate) fn reconnect(model: Signal<Model>) {
    load(model);
}
fn next_project(model: Signal<Model>) {
    let next = (model.get().selected + 1) % model.get().project_names.len().max(1);
    switch_project(model, next);
}
pub(crate) fn switch_project(model: Signal<Model>, index: usize) {
    model.update(|m| {
        m.select(index);
        m.task_id.clear();
        m.queried_task.clear();
        if m.live {
            m.phase = Phase::Loading;
        }
    });
    if model.get().live || model.get().phase == Phase::Loading {
        load(model);
    } else {
        model.update(|m| m.epoch += 1);
    }
}
fn toggle_contrast(model: Signal<Model>) {
    model.update(|m| m.high_contrast = !m.high_contrast);
}
fn review_or_page(model: Signal<Model>, delta: i32) {
    // The review workspace has no tabs (a list and a record sheet); the
    // chords page the read-only console preferences.
    model.update(|m| m.page_by(delta as isize));
}
fn toggle_theme(model: Signal<Model>) {
    let actual_dark = match model.get().color_mode {
        precept_ui_core::appearance::ColorMode::System => dark(),
        precept_ui_core::appearance::ColorMode::Dark => true,
        precept_ui_core::appearance::ColorMode::Light => false,
    };
    model.update(|m| {
        m.dark = !actual_dark;
        m.color_mode = if m.dark {
            precept_ui_core::appearance::ColorMode::Dark
        } else {
            precept_ui_core::appearance::ColorMode::Light
        };
    });
    set_color_scheme(if model.get().dark {
        Appearance::Dark
    } else {
        Appearance::Light
    });
}
/// Copies the visible page; whether it copied.
fn copy_page(model: Signal<Model>) -> bool {
    if model.get().phase == Phase::Ready
        && !model.get().settings
        && !model.get().setup_open
        && !model.get().agents_page
        && !model.get().operator.open
    {
        set_clipboard(model.get().visible_lines().join("\n"));
        model.update(|m| m.copied = true);
        return true;
    }
    false
}

#[component]
pub fn App() -> impl IntoElement {
    // Debug-only design-system gallery; never reached from normal navigation.
    if std::env::var_os("PRECEPT_UI_GALLERY").is_some() {
        return ui::gallery::gallery().into_element();
    }
    #[cfg(feature = "ipc")]
    let inputs = INPUTS.get().cloned().unwrap_or_default();
    let mut initial = Model::loading();
    initial.operator.channel = precept_ui_core::operator::Channel::Desktop;
    #[cfg(feature = "ipc")]
    {
        // Saved settings first; explicit launch arguments win over them.
        #[cfg(not(test))]
        if persist_settings()
            && let Some(saved) = precept_ui_client::settings::load_settings()
        {
            saved.apply(
                &mut initial,
                precept_ui_core::settings::Explicit {
                    theme: inputs.theme.is_some(),
                    mode: inputs.appearance.is_some(),
                    language: inputs.locale.is_some(),
                },
            );
        }
        if let Some(locale) = inputs
            .locale
            .as_deref()
            .and_then(precept_ui_core::i18n::Locale::parse)
        {
            initial.locale = locale;
        }
        if let Some(theme) = inputs
            .theme
            .as_deref()
            .and_then(precept_ui_core::appearance::ThemeId::from_id)
        {
            initial.theme = theme;
        }
        if let Some(mode) = inputs
            .appearance
            .as_deref()
            .and_then(precept_ui_core::appearance::ColorMode::from_id)
        {
            initial.color_mode = mode;
        }
        initial.operator.open = inputs.operator;
        initial.settings = inputs.settings;
        if inputs.connection_settings {
            initial.setup_open = true;
            initial.setup_tab = precept_ui_core::SetupTab::Connection;
        }
        initial.section = match inputs.section.as_str() {
            "projects" => {
                initial.setup_open = true;
                initial.setup_tab = precept_ui_core::SetupTab::Workspaces;
                precept_ui_core::Section::Dashboard
            }
            "rules" | "preferences" => precept_ui_core::Section::Rules,
            "handoff" => precept_ui_core::Section::Handoff,
            "diagnostics" => precept_ui_core::Section::Diagnostics,
            _ => precept_ui_core::Section::Dashboard,
        };
        initial.connection = inputs.connection();
        initial.connection_draft = initial.connection.clone();
        initial.live = inputs.is_live();
        // Explicit launch arguments win; otherwise the saved connection, and on a
        // first desktop launch Runtime checks (Connection tab). Never in unit
        // tests, which must not read or write the developer's settings.
        #[cfg(not(test))]
        if !inputs.is_live() && inputs.synthetic_rows.is_none() {
            match precept_ui_client::settings::load() {
                Some(saved) => {
                    initial.live = saved.target != precept_ui_core::connection::Target::Synthetic;
                    initial.connection = saved.clone();
                    initial.connection_draft = saved;
                }
                None => {
                    initial.setup_open = true;
                    initial.setup_tab = precept_ui_core::SetupTab::Connection;
                    // The primary workflow: a daemon inside WSL2.
                    if cfg!(target_os = "windows") {
                        initial.connection_draft.target = precept_ui_core::connection::Target::Wsl;
                    }
                }
            }
        }
        initial.task_id = inputs.task_id.unwrap_or_default();
        initial.selected_project = inputs.project;
    }
    initial.dark = initial.color_mode != precept_ui_core::appearance::ColorMode::Light;
    set_color_scheme(match initial.color_mode {
        precept_ui_core::appearance::ColorMode::System => Appearance::System,
        precept_ui_core::appearance::ColorMode::Light => Appearance::Light,
        _ => Appearance::Dark,
    });
    let model = signal(initial);
    // Save the settings (appearance, language, shortcuts) when they change,
    // never the values the app started with.
    #[cfg(all(feature = "ipc", not(test)))]
    if persist_settings() {
        let chosen = computed(move || precept_ui_core::settings::Settings::of(&model.get()));
        let first = std::cell::Cell::new(true);
        effect(move || {
            let settings = chosen.get();
            if first.replace(false) {
                return;
            }
            // A refused or failed save leaves this session's choice as it is.
            let _ = precept_ui_client::settings::save_settings(&settings);
        });
    }
    if model.get_untracked().setup_open
        && model.get_untracked().setup_tab == precept_ui_core::SetupTab::Connection
    {
        features::connection::opened(model);
    }
    load(model);
    #[cfg(feature = "ipc")]
    wsl_watch(model);
    console(model).into_element()
}

/// Live WSL link status every 2 s for a WSL target: a lost bridge (distribution
/// stopped, helper exited) or an unreachable daemon is shown without waiting
/// for a user action. It never starts a bridge or a process, and only updates
/// the display-only `wsl_link` field (page phase, data and guards are untouched).
#[cfg(feature = "ipc")]
fn wsl_watch(model: Signal<Model>) {
    use precept_ui_core::connection::Target;
    spawn(async move {
        loop {
            timer(std::time::Duration::from_secs(2)).await;
            let connection = model.get_untracked().connection.clone();
            if connection.target != Target::Wsl {
                if model.get_untracked().wsl_link.is_some() {
                    model.update(|m| m.wsl_link = None);
                }
                continue;
            }
            let probe = connection.clone();
            let status = blocking(move || {
                precept_ui_client::native::Inputs::from_connection(&probe)
                    .ok()
                    .and_then(|inputs| inputs.wsl_link_status())
            })
            .await
            .unwrap_or(None);
            if model.get_untracked().connection == connection
                && model.get_untracked().wsl_link != status
            {
                model.update(|m| m.wsl_link = status);
            }
        }
    });
}

fn console(model: Signal<Model>) -> impl IntoElement {
    let filter_ref = element_ref();
    let task_ref = element_ref();
    let global = KeyOptions {
        when: nsh_core::keymap::When::Always,
        ..Default::default()
    };
    let auto = KeyOptions::default();
    let bind = |id: &'static str, options: KeyOptions, handler: Box<dyn Fn()>| {
        shortcuts::bind(model, id, options, handler)
    };
    // Leaves every workspace for a section (and focuses nothing).
    let to_section = move |section: precept_ui_core::Section| {
        if section == precept_ui_core::Section::Projects {
            shell::go_setup(model, precept_ui_core::SetupTab::Workspaces);
            return;
        }
        model.update(|m| {
            m.operator.open = false;
            m.operator.review = None;
            m.settings = false;
            m.setup_open = false;
            m.agents_page = false;
            m.sessions_page = false;
            m.memory_page = false;
            m.set_section(section)
        });
    };
    bind(
        "shortcuts.open",
        global,
        Box::new(move || shortcuts::open(model)),
    );
    bind("reconnect", auto, Box::new(move || reconnect(model)));
    bind("project.next", auto, Box::new(move || next_project(model)));
    bind(
        "preferences.filter",
        auto,
        Box::new(move || {
            to_section(precept_ui_core::Section::Rules);
            filter_ref.focus();
        }),
    );
    bind(
        "handoff.task",
        auto,
        Box::new(move || {
            to_section(precept_ui_core::Section::Handoff);
            task_ref.focus();
        }),
    );
    bind(
        "language.toggle",
        auto,
        Box::new(move || {
            model.update(|m| {
                m.locale = match m.locale {
                    precept_ui_core::i18n::Locale::En => precept_ui_core::i18n::Locale::ZhCn,
                    _ => precept_ui_core::i18n::Locale::En,
                }
            })
        }),
    );
    for (id, theme) in [
        (
            "theme.instrument",
            precept_ui_core::appearance::ThemeId::Instrument,
        ),
        ("theme.ledger", precept_ui_core::appearance::ThemeId::Ledger),
        ("theme.harbor", precept_ui_core::appearance::ThemeId::Harbor),
        (
            "theme.graphite",
            precept_ui_core::appearance::ThemeId::Graphite,
        ),
    ] {
        bind(
            id,
            auto,
            Box::new(move || model.update(|m| m.theme = theme)),
        );
    }
    bind(
        "go.settings",
        auto,
        Box::new(move || {
            model.update(|m| {
                m.operator.open = false;
                m.operator.review = None;
                m.operator.delete_arm = None;
                m.settings = !m.settings;
                m.setup_open = false;
                m.agents_page = false;
                m.sessions_page = false;
                m.memory_page = false;
            })
        }),
    );
    bind(
        "contrast.toggle",
        auto,
        Box::new(move || toggle_contrast(model)),
    );
    bind(
        "sidebar.toggle",
        global,
        Box::new(move || model.update(|m| m.sidebar_collapsed = !m.sidebar_collapsed)),
    );
    // Escape is not rebindable. It steps aside while the Shortcuts panel is
    // open (the panel's sheet hears it) or a keystroke is being recorded.
    let escape_free = computed(move || {
        let m = model.get();
        !m.shortcuts_open && m.shortcut_recording.is_none()
    });
    effect(move || {
        if !escape_free.get() {
            return;
        }
        on_key_with("Escape", global, move |_| {
            if model.with(|m| m.operator.open && m.operator.delete_arm.is_some()) {
                model.update(|m| {
                    m.operator.delete_arm = None;
                    m.operator.review = None;
                });
                return;
            }
            filter_ref.blur();
            task_ref.blur();
            model.update(|m| {
                m.cancel_read();
                m.operator.open = false;
                m.operator.review = None;
                m.operator.delete_arm = None;
                m.settings = false;
                m.setup_open = false;
                m.agents_page = false;
                m.sessions_page = false;
                m.memory_page = false;
            });
        });
    });
    for (id, section) in [
        "go.dashboard",
        "go.workspaces",
        "go.preferences",
        "go.handoff",
        "go.diagnostics",
    ]
    .into_iter()
    .zip(precept_ui_core::Section::ALL)
    {
        bind(
            id,
            global,
            Box::new(move || {
                filter_ref.blur();
                task_ref.blur();
                to_section(section);
            }),
        );
    }
    bind("go.review", global, Box::new(move || review::open(model)));
    bind(
        "appearance.toggle",
        auto,
        Box::new(move || toggle_theme(model)),
    );
    bind(
        "page.copy",
        auto,
        Box::new(move || {
            copy_page(model);
        }),
    );
    // Alt+left/right navigate the review tabs while the operator review workspace
    // is active; otherwise they keep paging the read-only console preferences.
    // Platforms name arrows "ArrowLeft/ArrowRight" or "Left/Right", so both
    // spellings are the defaults.
    bind(
        "page.next",
        global,
        Box::new(move || review_or_page(model, 1)),
    );
    bind(
        "page.previous",
        global,
        Box::new(move || review_or_page(model, -1)),
    );
    effect(move || {
        if std::env::var_os("PRECEPT_UI_PROBE").is_some() {
            let current = model.get();
            eprintln!(
                "PRECEPT_UI_WSL link={:?} failure={:?}",
                current.wsl_link, current.read_failure
            );
            eprintln!(
                "PRECEPT_UI_SESSION target={:?} draft={:?} invalid={} settings={} locale={:?} theme={:?} mode={:?} denied={}",
                current.connection.target,
                current.connection_draft.target,
                current.connection_invalid,
                current.settings,
                current.locale,
                current.theme,
                current.color_mode,
                current.permission_denied
            );
            eprintln!(
                "PRECEPT_NSH_STATE phase={:?} selected={} rows={} queried_chars={} filter_chars={} section={:?} page={} detail={} dark={} contrast={} elapsed={:.6}",
                current.phase,
                current.selected,
                current.rows().len(),
                current.queried_task.chars().count(),
                current.filter.chars().count(),
                current.section,
                current.page,
                current.selected_preference.is_some(),
                current.dark,
                current.high_contrast,
                probe_elapsed()
            );
        }
    });
    shell::shell(model, task_ref, filter_ref)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsh_engine::{EngineConfig, Root};
    #[test]
    fn the_top_bar_shows_the_version_and_hides_the_sidebar() {
        // A desktop window (the least nsh.json allows is wider than the
        // narrow layout, where the sidebar becomes a top row).
        let mut cfg = EngineConfig::new(Root::of::<App>());
        cfg.viewport = Size::new(px(1180.0), px(780.0));
        let mut app = nsh_testing::Headless::start(cfg).unwrap();
        app.wait_for("Project: alpha").unwrap();
        assert!(
            app.screen()
                .shows(&format!("v{}", env!("CARGO_PKG_VERSION")))
        );
        assert!(app.screen().shows("Review & rules"));
        app.press("Hide the sidebar").unwrap();
        app.wait_until("sidebar hidden", std::time::Duration::from_secs(5), |s| {
            !s.shows("Review & rules")
        })
        .unwrap();
        app.press("Show the sidebar").unwrap();
        app.wait_for("Review & rules").unwrap();
        app.press_key("mod+b").unwrap();
        app.wait_until(
            "hidden by keyboard",
            std::time::Duration::from_secs(5),
            |s| !s.shows("Review & rules"),
        )
        .unwrap();
    }

    #[test]
    fn repository_rules_show_their_checks_and_open_in_the_dialog() {
        let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<App>())).unwrap();
        app.wait_for("Project: alpha").unwrap();
        app.press_key("alt+3").unwrap();
        app.wait_for("Repository rules").unwrap();
        assert!(app.screen().shows(".precept/rules/formatter.md"));
        assert!(app.screen().shows("Checks pass 1/1"));
        assert!(
            app.screen()
                .shows("1 rule file(s) not committed yet; they apply once committed.")
        );
        app.press("Details · formatter").unwrap();
        app.wait_for("Rust formatting").unwrap();
        app.press("Copy text").unwrap();
        assert_eq!(
            app.screen().clipboard(),
            Some("# Rust formatting\n\nRun `cargo fmt` before committing.")
        );
        app.press("Close").unwrap();
        app.wait_until("dialog closed", std::time::Duration::from_secs(5), |s| {
            !s.shows("Copy text")
        })
        .unwrap();
    }

    #[test]
    fn preference_details_open_in_a_dialog_with_copy() {
        let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<App>())).unwrap();
        app.wait_for("Project: alpha").unwrap();
        app.press_key("alt+3").unwrap();
        app.wait_for("Confirmed preferences and conventions")
            .unwrap();
        app.press("Details · commit.style").unwrap();
        app.wait_for("Copy text").unwrap();
        assert!(app.screen().shows("Close"));
        app.press("Copy text").unwrap();
        assert_eq!(app.screen().clipboard(), Some("newest"));
        app.press("Close").unwrap();
        app.wait_until("dialog closed", std::time::Duration::from_secs(5), |s| {
            !s.shows("Copy text")
        })
        .unwrap();
    }
    #[test]
    fn doctor_lists_what_it_saw_and_copies_a_report_without_paths() {
        let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<App>())).unwrap();
        app.wait_for("Project: alpha").unwrap();
        app.press("Diagnostics").unwrap();
        app.wait_for("Doctor").unwrap();
        // Demo data: nothing is claimed as working that was not read.
        app.wait_for("Demo data: no daemon is connected").unwrap();
        assert!(app.screen().shows("Not read"));
        assert!(app.screen().shows("Codex · MCP"));
        assert!(app.screen().shows("Codex · hook"));
        app.press("Copy report").unwrap();
        let report = app.screen().clipboard().unwrap();
        assert!(report.starts_with("Precept doctor · alpha\n"), "{report}");
        assert!(report.contains("[Not read] Daemon: Demo data"));
        assert!(!report.contains('/'), "no paths: {report}");
    }
    #[test]
    fn component_inputs_selection_empty_scope_and_explicit_copy() {
        let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<App>())).unwrap();
        app.wait_for("Project: alpha").unwrap();
        assert!(app.screen().clipboard().is_none());
        app.press_key("mod+k").unwrap();
        app.type_text("Task ID", "shared").unwrap();
        app.press("Lookup").unwrap();
        app.wait_for("Recorded checks").unwrap();
        app.wait_for("Before you continue").unwrap();
        app.press("Copy visible page").unwrap();
        assert!(app.screen().clipboard().unwrap().contains("Handoff shared"));
        // The project menu: its trigger shows the current project.
        app.press("alpha").unwrap();
        app.press("beta").unwrap();
        app.wait_for("Project: beta").unwrap();
        app.wait_for("Handoff — enter a task ID and choose Lookup")
            .unwrap();
        app.type_text("Task ID", "missing").unwrap();
        app.press("Lookup").unwrap();
        app.wait_for("Empty — no handoff for task missing in this project")
            .unwrap();
        app.press_key("mod+f").unwrap();
        app.type_text("Filter preferences", "no-match").unwrap();
        app.wait_for("Empty — no confirmed preferences match")
            .unwrap();
        app.press_key("alt+2").unwrap();
        app.wait_for("Registered workspaces").unwrap();
        // Workspaces manages registrations only: no legacy summary card and no
        // click-to-switch button on workspace rows.
        assert!(
            !app.screen()
                .shows("Only the daemon's projects; switching clears the task lookup.")
        );
        app.press_key("mod+f").unwrap();
        app.press_key("mod+d").unwrap();
        app.press_key("mod+shift+h").unwrap();
        app.press_key("Escape").unwrap();
        for (key, label) in [
            ("alt+2", "Registered workspaces"),
            ("alt+3", "Confirmed preferences and conventions"),
            (
                "alt+4",
                "Empty — no handoff for task missing in this project",
            ),
            ("alt+5", "Doctor"),
        ] {
            app.press_key(key).unwrap();
            app.wait_for(label).unwrap();
        }
        app.set_viewport(Size::new(px(640.0), px(480.0))).unwrap();
        app.wait_for("Project: beta").unwrap();
        app.set_viewport(Size::new(px(1080.0), px(760.0))).unwrap();
    }
    #[test]
    fn real_console_edits_connection_without_implicit_reads_and_localizes() {
        let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<App>())).unwrap();
        app.wait_for("Project: alpha").unwrap();
        app.press("Runtime checks").unwrap();
        // Connection has no bottom explanatory clutter text.
        assert!(!app.screen().shows("Checks only read status."));
        // Choosing this computer runs the read-only check; nothing connects yet.
        app.press("This computer").unwrap();
        if cfg!(unix) {
            app.wait_for("Checks").unwrap();
            app.wait_for("Daemon service").unwrap();
        } else {
            // Windows-local discovery checks the daemon beside the Desktop.
            app.wait_for("Daemon executable").unwrap();
        }
        app.press("Advanced").unwrap();
        // The check may already have filled the fields; replace them by hand.
        app.clear("Private runtime directory").unwrap();
        app.type_text("Private runtime directory", "relative/runtime")
            .unwrap();
        app.clear("IPC endpoint").unwrap();
        app.type_text("IPC endpoint", "synthetic").unwrap();
        assert_eq!(
            app.screen()
                .input_text("Private runtime directory")
                .unwrap(),
            Some("relative/runtime")
        );
        assert_eq!(
            app.screen().input_text("IPC endpoint").unwrap(),
            Some("synthetic")
        );
        app.press("Connect").unwrap();
        app.wait_for("Invalid connection fields; the active target is unchanged.")
            .unwrap();
        // Invalid drafts cannot clear a usable snapshot or replace its source.
        app.wait_for("Preview · Demo data").unwrap();
        assert!(!app.screen().shows("Demo data · Synthetic snapshot ready"));
        app.press("Demo data").unwrap();
        app.press("Use demo data").unwrap();
        app.press_key("Escape").unwrap();
        app.wait_for("Project: alpha").unwrap();
        app.press("Settings").unwrap();
        app.press("Ledger").unwrap();
        app.press("Light").unwrap();
        app.press("简体中文").unwrap();
        app.wait_for("主题集").unwrap();
        app.press_key("alt+3").unwrap();
        app.wait_for("已确认的偏好与约定").unwrap();
        app.wait_for("2 条已确认").unwrap();
        // Tags filter the list; several narrow to rows carrying all of them.
        app.press("#language · 1").unwrap();
        app.wait_for("1 条已确认").unwrap();
        assert!(app.screen().shows("zh-CN"));
        assert!(!app.screen().shows("newest"));
        app.press("#git · 1").unwrap();
        app.wait_for("0 条已确认").unwrap();
        app.wait_for("选择多个标签时，只显示同时带有这些标签的偏好。")
            .unwrap();
        app.press("全部").unwrap();
        app.wait_for("2 条已确认").unwrap();
        assert!(app.screen().clipboard().is_none());
    }
    #[component]
    fn Workload() -> impl IntoElement {
        let mut data = precept_ui_core::Snapshot::fixture();
        let row = data.projects[0].context.preferences[0].clone();
        data.projects[0].context.preferences = (0..10_000)
            .map(|i| precept_ui_core::Preference {
                key: format!("event-{i:05}"),
                ..row.clone()
            })
            .collect();
        let mut model = Model::loading();
        model.section = precept_ui_core::Section::Rules;
        model.receive(Ok(data));
        console(signal(model))
    }
    #[test]
    fn ten_thousand_rows_stay_virtual_while_scrolled_filtered_and_tagged() {
        let mut app =
            nsh_testing::Headless::start(EngineConfig::new(Root::of::<Workload>())).unwrap();
        app.wait_for("10000 confirmed").unwrap();
        let started = std::time::Instant::now();
        let shown = |app: &nsh_testing::Headless, key: &str| {
            app.screen().texts().iter().any(|t| t.contains(key))
        };
        assert!(shown(&app, "event-00000"));
        assert!(!shown(&app, "event-09999"));
        assert!(
            app.screen().texts().len() < 400,
            "screen tree grew with dataset"
        );
        // One continuous list: scrolling reveals later rows and keeps the tree bounded.
        app.scroll("event-00000", px(4000.0)).unwrap();
        app.wait_until("later rows", std::time::Duration::from_secs(5), |s| {
            s.texts()
                .iter()
                .any(|t| t.starts_with("event-000") && *t != "event-00000")
                && !s.texts().contains(&"event-00000")
        })
        .unwrap();
        assert!(app.screen().texts().len() < 400);
        app.type_text("Filter preferences", "event-09999").unwrap();
        app.wait_for("1 confirmed").unwrap();
        assert!(shown(&app, "event-09999"));
        // A tag the rows carry narrows nothing away; `#` matches tags only.
        app.clear("Filter preferences").unwrap();
        app.type_text("Filter preferences", "#git").unwrap();
        app.wait_for("10000 confirmed").unwrap();
        app.clear("Filter preferences").unwrap();
        app.type_text("Filter preferences", "#event").unwrap();
        app.wait_for("0 confirmed").unwrap();
        app.clear("Filter preferences").unwrap();
        app.type_text("Filter preferences", "event").unwrap();
        app.wait_for("10000 confirmed").unwrap();
        // Switch projects through the single sidebar workspace switcher.
        app.press("alpha").unwrap();
        app.press("beta").unwrap();
        app.press_key("alt+1").unwrap();
        app.wait_for("Project: beta").unwrap();
        println!(
            "RENDERED_WORKLOAD: 10000 rows in one virtual list, <400 text nodes, {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn runtime_checks_tabs_and_merged_review_hub_provide_clear_offline_guidance() {
        let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<App>())).unwrap();
        app.wait_for("Project: alpha").unwrap();
        // Runtime checks unifies Connection, Profile and Workspaces in one destination.
        app.press("Runtime checks").unwrap();
        app.wait_for("Connection").unwrap();
        app.wait_for("Profile").unwrap();
        app.wait_for("Workspaces").unwrap();
        assert!(!app.screen().shows("Checks only read status."));
        // Profile tab shows a designed offline empty state guiding back to Connection.
        app.press("Profile").unwrap();
        app.wait_for("Connect to a daemon first").unwrap();
        app.press("Go to Connection").unwrap();
        app.wait_for("Use demo data").unwrap();
        // Workspaces tab manages workspaces only: no bottom card, no click-to-switch.
        app.press("Workspaces").unwrap();
        app.wait_for("Registered workspaces").unwrap();
        assert!(
            !app.screen()
                .shows("Only the daemon's projects; switching clears the task lookup.")
        );
        assert_eq!(app.screen().count("role:button[beta]").unwrap(), 0);
        // Merged Review & rules hub shows offline guidance on Demo data.
        app.press("Review & rules").unwrap();
        app.wait_for("Live daemon required").unwrap();
        // Agents page lives under Workspace and shows offline guidance on Demo data.
        app.press("Agents").unwrap();
        app.wait_for("Connect to a daemon first").unwrap();
    }
}
