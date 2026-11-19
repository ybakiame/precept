//! One read-only presentation and interaction surface for NSH desktop and TUI.
use acp_ui_core::{Model, Phase};
use nsh_base::button;
use nsh_core::prelude::*;
mod features;
mod shell;
mod shortcuts;
mod ui;

pub use features::brand_preview;
use features::{review, sessions};
use ui::{icons, tokens};
pub static DESKTOP_RUNTIME: std::sync::OnceLock<(String, String)> = std::sync::OnceLock::new();
fn runtime_lines(locale: acp_ui_core::i18n::Locale) -> Vec<String> {
    use acp_ui_core::i18n::{Msg, tr};
    if TERMINAL_SURFACE.load(std::sync::atomic::Ordering::Relaxed) {
        return vec!["Backend: NSH / ratatui / crossterm terminal".into()];
    }
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
pub static TERMINAL_SURFACE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
pub static PROBE_STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
/// Set by the desktop and terminal binaries: the operator's settings are read
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

#[native_module("acp-ui/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

#[cfg(target_arch = "wasm32")]
nsh_platform_web::rust_app!(
    vec![nsh_base::registry(), registry()],
    nsh_core::element::Root::of::<App>()
);

#[cfg(feature = "ipc")]
pub static INPUTS: std::sync::OnceLock<acp_ui_client::native::Inputs> = std::sync::OnceLock::new();

fn load(model: Signal<Model>) {
    #[cfg(feature = "ipc")]
    let inputs = acp_ui_client::native::Inputs::from_connection(&model.get().connection);
    // NSH's clock uses the host monotonic timer on WASM as well as native;
    // std::time::Instant panics on wasm32-unknown-unknown.
    let started = nsh_core::time::now();
    let mut request = None;
    model.update(|m| {
        request = m.begin_read().ok();
    });
    let Some(request) = request else { return };
    spawn(async move {
        #[cfg(feature = "ipc")]
        let selected = request.clone();
        #[cfg(feature = "ipc")]
        let result = blocking(move || {
            inputs.and_then(|inputs| inputs.fetch_for(selected.project.as_deref(), &selected.task))
        })
        .await
        .unwrap_or(Err("Client unavailable"));
        #[cfg(not(feature = "ipc"))]
        let result = {
            timer(std::time::Duration::from_millis(20)).await;
            if model.get().live {
                Err("Client unavailable")
            } else {
                let mut s = acp_ui_core::Snapshot::fixture();
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
        #[cfg(target_arch = "wasm32")]
        let now = 0;
        #[cfg(not(target_arch = "wasm32"))]
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
    });
}
fn reconnect(model: Signal<Model>) {
    load(model);
}
fn next_project(model: Signal<Model>) {
    let next = (model.get().selected + 1) % model.get().project_names.len().max(1);
    switch_project(model, next);
}
fn switch_project(model: Signal<Model>, index: usize) {
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
        acp_ui_core::appearance::ColorMode::System => dark(),
        acp_ui_core::appearance::ColorMode::Dark => true,
        acp_ui_core::appearance::ColorMode::Light => false,
    };
    model.update(|m| {
        m.dark = !actual_dark;
        m.color_mode = if m.dark {
            acp_ui_core::appearance::ColorMode::Dark
        } else {
            acp_ui_core::appearance::ColorMode::Light
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
        && !model.get().connection_editor
        && !model.get().sessions
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
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var_os("ACP_UI_GALLERY").is_some() {
        return ui::gallery::gallery().into_element();
    }
    #[cfg(feature = "ipc")]
    let inputs = INPUTS.get().cloned().unwrap_or_default();
    let mut initial = Model::loading();
    initial.operator.channel = if TERMINAL_SURFACE.load(std::sync::atomic::Ordering::Relaxed) {
        acp_ui_core::operator::Channel::Tui
    } else {
        acp_ui_core::operator::Channel::Desktop
    };
    #[cfg(feature = "ipc")]
    {
        // Saved settings first; explicit launch arguments win over them.
        #[cfg(not(test))]
        if persist_settings()
            && let Some(saved) = acp_ui_client::settings::load_settings()
        {
            saved.apply(
                &mut initial,
                acp_ui_core::settings::Explicit {
                    theme: inputs.theme.is_some(),
                    mode: inputs.appearance.is_some(),
                    language: inputs.locale.is_some(),
                },
            );
        }
        if let Some(locale) = inputs
            .locale
            .as_deref()
            .and_then(acp_ui_core::i18n::Locale::parse)
        {
            initial.locale = locale;
        }
        if let Some(theme) = inputs
            .theme
            .as_deref()
            .and_then(acp_ui_core::appearance::ThemeId::from_id)
        {
            initial.theme = theme;
        }
        if let Some(mode) = inputs
            .appearance
            .as_deref()
            .and_then(acp_ui_core::appearance::ColorMode::from_id)
        {
            initial.color_mode = mode;
        }
        initial.sessions = inputs.sessions;
        initial.operator.open = inputs.operator;
        initial.settings = inputs.settings;
        initial.connection_editor = inputs.connection_settings;
        initial.section = match inputs.section.as_str() {
            "projects" => acp_ui_core::Section::Projects,
            "preferences" => acp_ui_core::Section::Preferences,
            "handoff" => acp_ui_core::Section::Handoff,
            "diagnostics" => acp_ui_core::Section::Diagnostics,
            _ => acp_ui_core::Section::Dashboard,
        };
        initial.connection = inputs.connection();
        initial.connection_draft = initial.connection.clone();
        initial.live = inputs.is_live();
        // Explicit launch arguments win; otherwise the saved connection, and on a
        // first desktop launch the connection page. Never in unit tests, which
        // must not read or write the developer's settings.
        #[cfg(not(test))]
        if !inputs.is_live() && inputs.synthetic_rows.is_none() {
            match acp_ui_client::settings::load() {
                Some(saved) => {
                    initial.live = saved.target != acp_ui_core::connection::Target::Synthetic;
                    initial.connection = saved.clone();
                    initial.connection_draft = saved;
                }
                None if !TERMINAL_SURFACE.load(std::sync::atomic::Ordering::Relaxed) => {
                    initial.connection_editor = true;
                    // The primary workflow: a daemon inside WSL2.
                    if cfg!(target_os = "windows") {
                        initial.connection_draft.target = acp_ui_core::connection::Target::Wsl;
                    }
                }
                None => {}
            }
        }
        initial.task_id = inputs.task_id.unwrap_or_default();
        initial.selected_project = inputs.project;
    }
    initial.dark = initial.color_mode != acp_ui_core::appearance::ColorMode::Light;
    set_color_scheme(match initial.color_mode {
        acp_ui_core::appearance::ColorMode::System => Appearance::System,
        acp_ui_core::appearance::ColorMode::Light => Appearance::Light,
        _ => Appearance::Dark,
    });
    let model = signal(initial);
    // Save the settings (appearance, language, shortcuts) when they change,
    // never the values the app started with.
    #[cfg(all(feature = "ipc", not(test)))]
    if persist_settings() {
        let chosen = computed(move || acp_ui_core::settings::Settings::of(&model.get()));
        let first = std::cell::Cell::new(true);
        effect(move || {
            let settings = chosen.get();
            if first.replace(false) {
                return;
            }
            // A refused or failed save leaves this session's choice as it is.
            let _ = acp_ui_client::settings::save_settings(&settings);
        });
    }
    if model.get_untracked().connection_editor {
        features::connection::opened(model);
    }
    load(model);
    sessions::poll(model);
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
    use acp_ui_core::connection::Target;
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
                acp_ui_client::native::Inputs::from_connection(&probe)
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
    let to_section = move |section: acp_ui_core::Section| {
        model.update(|m| {
            m.operator.open = false;
            m.operator.review = None;
            m.settings = false;
            m.sessions = false;
            m.session.confirm_start = false;
            m.connection_editor = false;
            m.profile_page = false;
            m.agents_page = false;
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
            to_section(acp_ui_core::Section::Preferences);
            filter_ref.focus();
        }),
    );
    bind(
        "handoff.task",
        auto,
        Box::new(move || {
            to_section(acp_ui_core::Section::Handoff);
            task_ref.focus();
        }),
    );
    bind(
        "language.toggle",
        auto,
        Box::new(move || {
            model.update(|m| {
                m.locale = match m.locale {
                    acp_ui_core::i18n::Locale::En => acp_ui_core::i18n::Locale::ZhCn,
                    _ => acp_ui_core::i18n::Locale::En,
                }
            })
        }),
    );
    for (id, theme) in [
        (
            "theme.instrument",
            acp_ui_core::appearance::ThemeId::Instrument,
        ),
        ("theme.ledger", acp_ui_core::appearance::ThemeId::Ledger),
        ("theme.harbor", acp_ui_core::appearance::ThemeId::Harbor),
        ("theme.graphite", acp_ui_core::appearance::ThemeId::Graphite),
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
                m.sessions = false;
                m.session.confirm_start = false;
                m.settings = !m.settings;
                m.connection_editor = false;
                m.profile_page = false;
                m.agents_page = false;
            })
        }),
    );
    bind(
        "contrast.toggle",
        auto,
        Box::new(move || toggle_contrast(model)),
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
                m.sessions = false;
                m.session.confirm_start = false;
                m.connection_editor = false;
                m.profile_page = false;
                m.agents_page = false;
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
    .zip(acp_ui_core::Section::ALL)
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
    bind(
        "go.sessions",
        global,
        Box::new(move || sessions::open(model)),
    );
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
    // The TUI platform names arrows "ArrowLeft/ArrowRight"; the GTK desktop names
    // them "Left/Right", so both spellings are the defaults.
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
    #[cfg(not(target_arch = "wasm32"))]
    effect(move || {
        if std::env::var_os("ACP_UI_PROBE").is_some() {
            let current = model.get();
            eprintln!(
                "ACP_UI_WSL link={:?} failure={:?}",
                current.wsl_link, current.read_failure
            );
            eprintln!(
                "ACP_UI_SESSION target={:?} draft={:?} invalid={} settings={} locale={:?} theme={:?} mode={:?} denied={}",
                current.connection.target,
                current.connection_draft.target,
                current.connection_invalid,
                current.settings,
                current.locale,
                current.theme,
                current.color_mode,
                current.permission_denied
            );
            if current.sessions {
                eprintln!(
                    "ACP_NSH_SESSION state={:?} alive={} pending={} revision={} busy={} configured={} confirm={}",
                    current.session.view.as_ref().map(|v| &v.state),
                    current
                        .session
                        .view
                        .as_ref()
                        .is_some_and(|v| v.process_alive),
                    current
                        .session
                        .view
                        .as_ref()
                        .map_or(0, |v| v.permissions.len()),
                    current.session.view.as_ref().map_or(0, |v| v.revision),
                    current.session.busy,
                    current
                        .session
                        .capabilities
                        .as_ref()
                        .map_or(0, |c| c.executables.len()),
                    current.session.confirm_start
                );
                let lease = current.session.context.unwrap_or_default();
                eprintln!(
                    "ACP_NSH_CONTEXT exe={} share={} enabled={} active={} context_calls={} handoff_calls={} stop_reason={:?}",
                    current.session.executable,
                    current.session.share_context,
                    lease.enabled,
                    lease.active,
                    lease.context_calls,
                    lease.handoff_calls,
                    current
                        .session
                        .view
                        .as_ref()
                        .and_then(|v| v.stop_reason.as_deref())
                );
            }
            eprintln!(
                "ACP_NSH_STATE phase={:?} selected={} rows={} queried_chars={} filter_chars={} section={:?} page={} detail={} dark={} contrast={} elapsed={:.6}",
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
    fn session_surface_keeps_synthetic_and_wsl_execution_disabled() {
        let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<App>())).unwrap();
        app.wait_for("Project: alpha").unwrap();
        app.press_key("alt+6").unwrap();
        app.wait_for("Choose a native local or WSL Daemon target and an allowlisted project.")
            .unwrap();
        app.press_key("mod+shift+s").unwrap();
        assert!(!app.screen().shows("Confirm start on this checkout"));
        app.press_key("mod+shift+c").unwrap();
        assert!(app.screen().clipboard().is_none());
        app.press_key("Escape").unwrap();
        app.press_key("alt+3").unwrap();
        app.wait_for("Confirmed effective preferences · key / value / scope / source")
            .unwrap();
    }
    #[test]
    fn component_inputs_selection_empty_scope_and_explicit_copy() {
        let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<App>())).unwrap();
        app.wait_for("Project: alpha").unwrap();
        assert!(app.screen().clipboard().is_none());
        app.press_key("mod+k").unwrap();
        app.type_text("Task ID", "shared").unwrap();
        app.press("Lookup").unwrap();
        app.wait_for("Recorded validation evidence (not current session verification)")
            .unwrap();
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
        app.wait_for("Only the daemon's projects; switching clears the task lookup.")
            .unwrap();
        app.press_key("mod+f").unwrap();
        app.press_key("mod+d").unwrap();
        app.press_key("mod+shift+h").unwrap();
        app.press_key("Escape").unwrap();
        for (key, label) in [
            (
                "alt+2",
                "Only the daemon's projects; switching clears the task lookup.",
            ),
            (
                "alt+3",
                "Confirmed effective preferences · key / value / scope / source",
            ),
            (
                "alt+4",
                "Empty — no handoff for task missing in this project",
            ),
            (
                "alt+5",
                "Read-only context IPC · API v1 · no daemon autostart",
            ),
        ] {
            app.press_key(key).unwrap();
            app.wait_for(label).unwrap();
        }
        app.press("Expand / collapse").unwrap();
        app.set_viewport(Size::new(px(640.0), px(480.0))).unwrap();
        app.wait_for("Project: beta").unwrap();
        app.set_viewport(Size::new(px(1080.0), px(760.0))).unwrap();
    }
    #[test]
    fn real_console_edits_connection_without_implicit_reads_and_localizes() {
        let mut app = nsh_testing::Headless::start(EngineConfig::new(Root::of::<App>())).unwrap();
        app.wait_for("Project: alpha").unwrap();
        app.press("Step 1: Connection").unwrap();
        // Choosing this computer runs the read-only check; nothing connects yet.
        app.press("This computer").unwrap();
        if cfg!(unix) {
            app.wait_for("Checks").unwrap();
            app.wait_for("Daemon service").unwrap();
        } else {
            // A Windows-local daemon is not discovered: manual coordinates only.
            app.wait_for(
                "Enter the private runtime directory and endpoint of a daemon on this computer.",
            )
            .unwrap();
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
        app.wait_for("Demo data · Synthetic snapshot ready")
            .unwrap();
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
        app.wait_for("已生效的确认偏好 · 键 / 值 / 范围 / 来源")
            .unwrap();
        assert!(app.screen().clipboard().is_none());
    }
    #[component]
    fn Workload() -> impl IntoElement {
        let mut data = acp_ui_core::Snapshot::fixture();
        let row = data.projects[0].context.preferences[0].clone();
        data.projects[0].context.preferences = (0..10_000)
            .map(|i| acp_ui_core::Preference {
                key: format!("event-{i:05}"),
                ..row.clone()
            })
            .collect();
        let mut model = Model::loading();
        model.section = acp_ui_core::Section::Preferences;
        model.receive(Ok(data));
        console(signal(model))
    }
    #[test]
    fn ten_thousand_rows_mount_every_page_then_filter_and_select() {
        let mut app =
            nsh_testing::Headless::start(EngineConfig::new(Root::of::<Workload>())).unwrap();
        app.wait_for("10000 confirmed rows · page 1/834").unwrap();
        let started = std::time::Instant::now();
        for page in 0..834 {
            for row in
                (page * acp_ui_core::PAGE_SIZE)..((page + 1) * acp_ui_core::PAGE_SIZE).min(10_000)
            {
                let key = format!("event-{row:05}");
                assert!(app.screen().texts().iter().any(|t| t.contains(&key)));
            }
            assert!(
                app.screen().texts().len() < 100,
                "screen tree grew with dataset"
            );
            if page < 833 {
                app.press("Next page").unwrap();
            }
        }
        app.type_text("Filter preferences", "event-09999").unwrap();
        app.wait_for("1 confirmed rows · page 1/1").unwrap();
        assert!(
            app.screen()
                .texts()
                .iter()
                .any(|t| t.contains("event-09999"))
        );
        app.press("Step 3: Workspaces").unwrap();
        app.press("beta").unwrap();
        app.wait_for("Project: beta").unwrap();
        println!(
            "RENDERED_WORKLOAD: 10000 rows, 834 mounted pages, <100 text nodes, {:?}",
            started.elapsed()
        );
    }
}
