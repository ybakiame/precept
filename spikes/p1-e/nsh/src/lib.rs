//! The same Rust component runs in native NSH, the terminal and offline Web.
use acp_ui_spike_model::{Model, Phase};
use nsh_base::{button, theme};
use nsh_core::prelude::*;

/// Disables framework profile persistence as well as ACP data writes.
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

fn reconnect(model: Signal<Model>) {
    if model.get().phase == Phase::Loading {
        return;
    }
    load(model);
}
fn load(model: Signal<Model>) {
    model.update(|m| m.phase = Phase::Loading);
    spawn(async move {
        #[cfg(feature = "ipc")]
        let result = blocking(|| INPUTS.get().cloned().unwrap_or_default().fetch())
            .await
            .unwrap_or(Err("Client unavailable"));
        #[cfg(not(feature = "ipc"))]
        let result = {
            timer(std::time::Duration::from_millis(150)).await;
            Ok(acp_ui_spike_model::Snapshot::fixture())
        };
        model.update(|m| m.receive(result));
    });
}
fn control(label: &'static str, callback: impl Fn() + 'static) -> impl IntoElement {
    button()
        .accessibility_label(label)
        .px_3()
        .py_2()
        .rounded(px(6.0))
        .bg(move || theme().muted)
        .on_press(callback)
        .child(text(label))
}
#[component]
pub fn App() -> impl IntoElement {
    #[cfg(feature = "ipc")]
    let live = INPUTS.get().is_some_and(|i| i.runtime_dir.is_some());
    #[cfg(not(feature = "ipc"))]
    let live = false;
    let model = signal(Model {
        live,
        ..Model::loading()
    });
    load(model);
    #[cfg(not(target_arch = "wasm32"))]
    effect(move || {
        if model.get().phase == Phase::Ready && std::env::var_os("ACP_UI_PROBE").is_some() {
            eprintln!("ACP_NSH_READY");
        }
    });
    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .p_3()
        .gap_2()
        .child(text("Agent Control Plane · NSH Rust experiment").text_size(px(22.0)))
        .child(text(
            "Synthetic/offline unless explicit private IPC flags supplied · no writes",
        ))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(control("Reconnect", move || reconnect(model)))
                .child(control("Next project", move || model.update(Model::next)))
                .child(control("Light / dark", move || {
                    model.update(|m| m.dark = !m.dark);
                    set_color_scheme(if model.get().dark {
                        Appearance::Dark
                    } else {
                        Appearance::Light
                    });
                }))
                .child(control("Filter commit / all", move || {
                    model.update(|m| {
                        m.filter = if m.filter.is_empty() {
                            "commit".into()
                        } else {
                            String::new()
                        }
                    })
                }))
                .child(control("Demo state", move || {
                    model.update(|m| {
                        m.phase = match m.phase {
                            Phase::Ready => Phase::Loading,
                            Phase::Loading => Phase::Empty,
                            Phase::Empty => Phase::Error,
                            Phase::Error => Phase::Unavailable,
                            Phase::Unavailable => Phase::Ready,
                        }
                    })
                })),
        )
        .child(
            view()
                .accessible(true)
                .label("Daemon connection state")
                .child(text(move || model.get().status())),
        )
        .child(
            view()
                .flex_row()
                .flex_1()
                .gap_3()
                .child(
                    view()
                        .w(px(180.0))
                        .gap_2()
                        .child(text("Allowlisted projects"))
                        .child(dynamic(move || {
                            let names = model
                                .get()
                                .data
                                .map(|s| s.projects.into_iter().map(|p| p.name).collect::<Vec<_>>())
                                .unwrap_or_default();
                            view().gap_2().children(names.into_iter().enumerate().map(
                                move |(index, name)| {
                                    button()
                                        .accessibility_label(name.clone())
                                        .py_2()
                                        .on_press(move || model.update(|m| m.select(index)))
                                        .child(text(name))
                                },
                            ))
                        })),
                )
                .child(scroll().flex_1().child(dynamic(move || {
                    view()
                        .gap_2()
                        .on_layout(move |_: &Bounds<Points>| {
                            #[cfg(not(target_arch = "wasm32"))]
                            if model.get().phase == Phase::Ready
                                && std::env::var_os("ACP_UI_PROBE").is_some()
                            {
                                eprintln!("ACP_NSH_LAYOUT_READY");
                            }
                        })
                        .children(model.get().detail_lines().into_iter().map(text))
                }))),
        )
}

#[cfg(test)]
mod tests {
    use nsh_engine::{EngineConfig, Root};
    #[test]
    fn actual_component_project_selection_and_states() {
        let mut app =
            nsh_testing::Headless::start(EngineConfig::new(Root::of::<super::App>())).unwrap();
        app.wait_for("Project: alpha").unwrap();
        app.press("Next project").unwrap();
        app.wait_for("Project: beta").unwrap();
        app.press("Demo state").unwrap();
        app.wait_for("Loading — read-only query in progress")
            .unwrap();
        app.press("Demo state").unwrap();
        app.wait_for("Empty — no allowlisted projects").unwrap();
        app.press("Demo state").unwrap();
        app.wait_for("Error — synthetic error state; reconnect to retry")
            .unwrap();
        app.press("Reconnect").unwrap();
        app.wait_for("Project: beta").unwrap();
    }
}
