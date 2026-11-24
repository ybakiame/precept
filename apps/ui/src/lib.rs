//! The operator's presentation and interaction surface for the NSH desktop.
use nsh_base::button;
use nsh_core::prelude::*;
use precept_ui_core::{Model, Phase};
mod app;
mod data;
mod features;
mod shell;
mod shortcuts;
mod ui;

pub use app::App;
use data::next_project;
pub(crate) use data::{reconnect, switch_project};
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
mod tests;
