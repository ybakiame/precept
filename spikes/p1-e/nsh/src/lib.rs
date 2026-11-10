//! One read-only presentation and interaction surface for NSH desktop and TUI.
use acp_ui_spike_model::{Model, Phase};
use nsh_base::button;
use nsh_core::prelude::*;
pub mod brand_preview;
mod components;
mod tokens;
pub static DESKTOP_RUNTIME: std::sync::OnceLock<(String, String)> = std::sync::OnceLock::new();
fn runtime_lines() -> Vec<String> {
    if TERMINAL_SURFACE.load(std::sync::atomic::Ordering::Relaxed) {
        return vec!["Backend: NSH / ratatui / crossterm terminal".into()];
    }
    if let Some((backend, renderer)) = DESKTOP_RUNTIME.get() {
        return vec![
            format!("Actual GTK backend: {backend} · renderer: {renderer}"),
            "Renderer class does not attest GPU acceleration or compositor identity.".into(),
        ];
    }
    vec![
        if cfg!(target_os = "windows") {
            "Backend: NSH native Windows"
        } else {
            "Runtime backend not observed yet; inspect local diagnostics."
        }
        .into(),
    ]
}
pub static TERMINAL_SURFACE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
pub static PROBE_STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
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

#[native_module("acp-spike/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

#[cfg(target_arch = "wasm32")]
nsh_platform_web::rust_app!(
    vec![nsh_base::registry(), registry()],
    nsh_core::element::Root::of::<App>()
);

#[cfg(feature = "ipc")]
pub static INPUTS: std::sync::OnceLock<acp_ui_spike_model::native::Inputs> =
    std::sync::OnceLock::new();

fn load(model: Signal<Model>) {
    #[cfg(feature = "ipc")]
    let inputs = INPUTS.get().cloned().unwrap_or_default();
    let mut request = None;
    model.update(|m| {
        request = m.begin_read().ok();
    });
    let Some(request) = request else { return };
    spawn(async move {
        #[cfg(feature = "ipc")]
        let selected = request.clone();
        #[cfg(feature = "ipc")]
        let result =
            blocking(move || inputs.fetch_for(selected.project.as_deref(), &selected.task))
                .await
                .unwrap_or(Err("Client unavailable"));
        #[cfg(not(feature = "ipc"))]
        let result = {
            timer(std::time::Duration::from_millis(20)).await;
            let mut s = acp_ui_spike_model::Snapshot::fixture();
            for p in &mut s.projects {
                if p.handoff
                    .as_ref()
                    .is_some_and(|h| h.task_id != request.task)
                {
                    p.handoff = None;
                }
            }
            Ok(s)
        };
        #[cfg(target_arch = "wasm32")]
        let now = 0;
        #[cfg(not(target_arch = "wasm32"))]
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|t| t.as_millis() as u64)
            .unwrap_or_default();
        model.update(|m| {
            m.complete_read(&request, result, now);
        });
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
fn toggle_theme(model: Signal<Model>) {
    model.update(|m| m.dark = !m.dark);
    set_color_scheme(if model.get().dark {
        Appearance::Dark
    } else {
        Appearance::Light
    });
}
fn copy_page(model: Signal<Model>) {
    if model.get().phase == Phase::Ready {
        set_clipboard(model.get().visible_lines().join("\n"));
        model.update(|m| m.copied = true);
    }
}

#[component]
pub fn App() -> impl IntoElement {
    #[cfg(feature = "ipc")]
    let inputs = INPUTS.get().cloned().unwrap_or_default();
    let mut initial = Model::loading();
    #[cfg(feature = "ipc")]
    {
        initial.live = inputs.is_live();
        initial.task_id = inputs.task_id.unwrap_or_default();
        initial.selected_project = inputs.project;
    }
    initial.dark = true;
    set_color_scheme(Appearance::Dark);
    let model = signal(initial);
    load(model);
    console(model)
}

fn console(model: Signal<Model>) -> impl IntoElement {
    let filter_ref = element_ref();
    let task_ref = element_ref();
    on_key("mod+r", move |_| reconnect(model));
    on_key("mod+p", move |_| next_project(model));
    on_key("mod+f", move |_| {
        model.update(|m| m.set_section(acp_ui_spike_model::Section::Preferences));
        filter_ref.focus();
    });
    on_key("mod+k", move |_| {
        model.update(|m| m.set_section(acp_ui_spike_model::Section::Handoff));
        task_ref.focus();
    });
    on_key("mod+shift+h", move |_| toggle_contrast(model));
    let global = KeyOptions {
        when: nsh_core::keymap::When::Always,
        ..Default::default()
    };
    on_key_with("Escape", global, move |_| {
        filter_ref.blur();
        task_ref.blur();
        model.update(|m| m.cancel_read());
    });
    for (key, section) in ["alt+1", "alt+2", "alt+3", "alt+4", "alt+5"]
        .into_iter()
        .zip(acp_ui_spike_model::Section::ALL)
    {
        on_key_with(key, global, move |_| {
            filter_ref.blur();
            task_ref.blur();
            model.update(|m| m.set_section(section));
        });
    }
    on_key("mod+d", move |_| toggle_theme(model));
    on_key("mod+shift+c", move |_| copy_page(model));
    on_key_with("alt+ArrowRight", global, move |_| {
        model.update(|m| m.page_by(1))
    });
    on_key_with("alt+ArrowLeft", global, move |_| {
        model.update(|m| m.page_by(-1))
    });
    #[cfg(not(target_arch = "wasm32"))]
    effect(move || {
        if std::env::var_os("ACP_UI_PROBE").is_some() {
            let current = model.get();
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
    components::shell(model, task_ref, filter_ref)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsh_engine::{EngineConfig, Root};
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
        app.press("Next project").unwrap();
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
        app.wait_for("Only daemon-allowlisted projects; switching clears the task lookup.")
            .unwrap();
        app.press_key("mod+f").unwrap();
        app.press_key("mod+d").unwrap();
        app.press_key("mod+shift+h").unwrap();
        app.press_key("Escape").unwrap();
        for (key, label) in [
            (
                "alt+2",
                "Only daemon-allowlisted projects; switching clears the task lookup.",
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
                "Read-only operator IPC · API v1 · no daemon autostart",
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
    #[component]
    fn Workload() -> impl IntoElement {
        let mut data = acp_ui_spike_model::Snapshot::fixture();
        let row = data.projects[0].context.preferences[0].clone();
        data.projects[0].context.preferences = (0..10_000)
            .map(|i| acp_ui_spike_model::Preference {
                key: format!("event-{i:05}"),
                ..row.clone()
            })
            .collect();
        let mut model = Model::loading();
        model.section = acp_ui_spike_model::Section::Preferences;
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
            for row in (page * acp_ui_spike_model::PAGE_SIZE)
                ..((page + 1) * acp_ui_spike_model::PAGE_SIZE).min(10_000)
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
        app.press("Projects").unwrap();
        app.press("beta").unwrap();
        app.wait_for("Project: beta").unwrap();
        println!(
            "RENDERED_WORKLOAD: 10000 rows, 834 mounted pages, <100 text nodes, {:?}",
            started.elapsed()
        );
    }
}
