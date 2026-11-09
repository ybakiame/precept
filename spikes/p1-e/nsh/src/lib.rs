//! One read-only presentation and interaction surface for NSH desktop and TUI.
use acp_ui_spike_model::{Model, Phase, valid_task};
use nsh_base::{button, theme};
use nsh_core::element::input;
use nsh_core::prelude::*;

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
    let current = model.get();
    let task = current.task_id.clone();
    if !task.is_empty() && !valid_task(&task) {
        model.update(|m| {
            m.phase = Phase::Error;
            m.data = None;
        });
        return;
    }
    #[cfg(feature = "ipc")]
    let inputs = INPUTS.get().cloned().unwrap_or_default();
    let project = current
        .data
        .as_ref()
        .and_then(|s| s.projects.get(current.selected))
        .map(|p| p.name.clone())
        .or(current.selected_project.clone());
    #[cfg(feature = "ipc")]
    let project = project.or_else(|| inputs.project.clone());
    model.update(|m| {
        m.phase = Phase::Loading;
        m.queried_task = task.clone();
    });
    spawn(async move {
        #[cfg(feature = "ipc")]
        let selected = project.clone();
        #[cfg(feature = "ipc")]
        let result = blocking(move || inputs.fetch_for(selected.as_deref(), &task))
            .await
            .unwrap_or(Err("Client unavailable"));
        #[cfg(not(feature = "ipc"))]
        let result = {
            timer(std::time::Duration::from_millis(20)).await;
            let mut s = acp_ui_spike_model::Snapshot::fixture();
            for p in &mut s.projects {
                if p.handoff.as_ref().is_some_and(|h| h.task_id != task) {
                    p.handoff = None;
                }
            }
            Ok(s)
        };
        model.update(|m| {
            m.receive(result);
            if let Some(name) = project
                && let Some(index) = m
                    .data
                    .as_ref()
                    .and_then(|s| s.projects.iter().position(|p| p.name == name))
            {
                m.select(index);
            }
        });
    });
}
fn reconnect(model: Signal<Model>) {
    if model.get().phase != Phase::Loading {
        load(model);
    }
}
fn next_project(model: Signal<Model>) {
    if model.get().phase == Phase::Loading {
        return;
    }
    model.update(|m| {
        m.next();
        m.task_id.clear();
        m.queried_task.clear();
    });
    if model.get().live {
        load(model);
    }
}
fn switch_project(model: Signal<Model>, index: usize) {
    if model.get().phase == Phase::Loading {
        return;
    }
    model.update(|m| {
        m.select(index);
        m.task_id.clear();
        m.queried_task.clear();
    });
    if model.get().live {
        load(model);
    }
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
        set_clipboard(model.get().detail_lines().join("\n"));
    }
}
fn control(label: &'static str, callback: impl Fn() + 'static) -> impl IntoElement {
    button()
        .accessibility_label(label)
        .px_2()
        .py_1()
        .rounded(px(5.0))
        .bg(move || theme().muted)
        .focus(|s| s.border_2().border_color(theme().primary))
        .on_press(callback)
        .child(text(label))
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
    on_key("mod+f", move |_| filter_ref.focus());
    on_key("mod+k", move |_| task_ref.focus());
    on_key("mod+d", move |_| toggle_theme(model));
    on_key("mod+shift+c", move |_| copy_page(model));
    on_key("alt+ArrowRight", move |_| model.update(|m| m.page_by(1)));
    on_key("alt+ArrowLeft", move |_| model.update(|m| m.page_by(-1)));
    #[cfg(not(target_arch = "wasm32"))]
    effect(move || {
        if std::env::var_os("ACP_UI_PROBE").is_some() {
            let current = model.get();
            eprintln!(
                "ACP_NSH_STATE phase={:?} selected={} rows={}",
                current.phase,
                current.selected,
                current.rows().len()
            );
        }
    });
    view().flex_1().bg(move || theme().background).text_color(move || theme().foreground)
        .p_3().gap_2()
        .child(text("Agent Control Plane").text_size(px(22.0)))
        .child(text("Read-only operator console · Tab moves focus · Ctrl/Cmd R reconnect · P project · F filter · K task · D theme"))
        .child(view().flex_row().flex_wrap().gap_2()
            .child(control("Reconnect", move || reconnect(model)))
            .child(control("Next project", move || next_project(model)))
            .child(control("Light / dark", move || toggle_theme(model)))
            .child(control("Copy visible page", move || copy_page(model))))
        .child(view().accessible(true).label("Daemon connection state").child(text(move || model.get().status())))
        .child(view().flex_row().flex_1().gap_3()
            .child(scroll().w(px(160.0)).child(view().gap_2()
                .child(text("PROJECTS"))
                .child(dynamic(move || {
                    let current = model.get();
                    let names = current.data.as_ref().map(|s| s.projects.iter().map(|p| p.name.clone()).collect::<Vec<_>>()).unwrap_or_default();
                    view().gap_2().children(names.into_iter().enumerate().map(move |(index, name)| {
                        let label = if index == current.selected { format!("> {name}") } else { name.clone() };
                        button().accessibility_label(name).py_2()
                            .focus(|s| s.border_2().border_color(theme().primary))
                            .on_press(move || switch_project(model, index)).child(text(label))
                    }))
                }))))
            .child(view().flex_1().gap_2()
                .child(text("Task ID · exact project-scoped lookup (max 120 characters)"))
                .child(view().flex_row().gap_2()
                    .child(input().element_ref(&task_ref).label("Task ID").flex_1().h(px(30.0))
                        .value(move || model.get().task_id).placeholder("Enter a task ID")
                        .on_change(move |v: &str| model.update(|m| m.task_id = v.chars().take(121).collect()))
                        .on_submit(move |_: &str| reconnect(model)))
                    .child(control("Lookup", move || reconnect(model))))
                .child(text("Filter confirmed preferences · key, value, scope or source"))
                .child(input().element_ref(&filter_ref).label("Filter preferences").h(px(30.0))
                    .value(move || model.get().filter).placeholder("Filter preferences")
                    .on_change(move |v: &str| model.update(|m| m.set_filter(v))))
                .child(view().flex_row().gap_2()
                    .child(control("Previous page", move || model.update(|m| m.page_by(-1))))
                    .child(control("Next page", move || model.update(|m| m.page_by(1))))
                    .child(text(move || model.get().page_label())))
                .child(scroll().flex_1().child(dynamic(move || {
                    view().gap_1().on_layout(move |_: &Bounds<Points>| {
                        #[cfg(not(target_arch = "wasm32"))]
                        if model.get().phase == Phase::Ready && std::env::var_os("ACP_UI_PROBE").is_some() {
                            eprintln!("ACP_NSH_LAYOUT_READY");
                        }
                    }).children(model.get().detail_lines().into_iter().map(text))
                })))))
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
        app.type_text("Filter preferences", "no-match").unwrap();
        app.wait_for("Empty — no confirmed preferences match")
            .unwrap();
        app.press_key("mod+f").unwrap();
        app.press_key("mod+d").unwrap();
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
        app.press("beta").unwrap();
        app.wait_for("Project: beta").unwrap();
        println!(
            "RENDERED_WORKLOAD: 10000 rows, 834 mounted pages, <100 text nodes, {:?}",
            started.elapsed()
        );
    }
}
