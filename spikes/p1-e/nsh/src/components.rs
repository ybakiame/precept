//! Stable input/focus controls around small reactive page bodies.
use super::*;
use acp_ui_spike_model::Section;
use nsh_core::element::input;

pub(super) fn shell(
    model: Signal<Model>,
    task_ref: ElementRef,
    filter_ref: ElementRef,
) -> impl IntoElement {
    view()
        .flex_1()
        .bg(move || {
            if model.get().high_contrast {
                AdaptiveColor::from(black())
            } else {
                theme().background
            }
        })
        .text_color(move || {
            if model.get().high_contrast {
                AdaptiveColor::from(white())
            } else {
                theme().foreground
            }
        })
        .p_2()
        .gap_1()
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(text("Agent Control Plane").text_size(px(20.0)))
                .child(text("Read-only · Ctrl 1–5 pages · Q close")),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_1()
                .child(control("Reconnect", move || reconnect(model)))
                .child(control("Next project", move || next_project(model)))
                .child(control("Light / dark", move || toggle_theme(model)))
                .child(control("High contrast", move || toggle_contrast(model)))
                .child(control("Copy visible page", move || copy_page(model))),
        )
        .child(
            view()
                .accessible(true)
                .label("Daemon connection state")
                .child(text(move || model.get().status()).text_size(px(12.0))),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_1()
                .children(Section::ALL.into_iter().map(move |section| {
                    button()
                        .accessibility_label(section.label())
                        .px_2()
                        .py_1()
                        .bg(move || {
                            if model.get().section == section {
                                theme().primary
                            } else {
                                theme().muted
                            }
                        })
                        .text_color(move || {
                            if model.get().section == section {
                                theme().primary_foreground
                            } else {
                                theme().foreground
                            }
                        })
                        .focus(|s| s.border_2().border_color(theme().foreground))
                        .on_press(move || model.update(|m| m.set_section(section)))
                        .child(text(section.label()))
                })),
        )
        .child(
            text("Task ID · exact project-scoped lookup (max 120 characters)").text_size(px(12.0)),
        )
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(
                    input()
                        .element_ref(&task_ref)
                        .label("Task ID")
                        .flex_1()
                        .h(px(28.0))
                        .value(move || model.get().task_id)
                        .placeholder("Enter a task ID")
                        .focus(|s| s.border_2().border_color(theme().primary))
                        .on_change(move |v: &str| {
                            model.update(|m| m.task_id = v.chars().take(121).collect())
                        })
                        .on_submit(move |_: &str| reconnect(model)),
                )
                .child(control("Lookup", move || reconnect(model))),
        )
        .child(
            text("Filter confirmed preferences · key, value, scope or source").text_size(px(12.0)),
        )
        .child(
            input()
                .element_ref(&filter_ref)
                .label("Filter preferences")
                .h(px(28.0))
                .value(move || model.get().filter)
                .placeholder("Filter preferences")
                .focus(|s| s.border_2().border_color(theme().primary))
                .on_change(move |v: &str| model.update(|m| m.set_filter(v))),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(control("Previous page", move || {
                    model.update(|m| m.page_by(-1))
                }))
                .child(control("Next page", move || model.update(|m| m.page_by(1))))
                .child(text(move || model.get().page_label())),
        )
        .child(scroll().flex_1().child(dynamic(move || page(model))))
}
fn page(model: Signal<Model>) -> impl IntoElement {
    let current = model.get();
    let mut body = view().gap_1().on_layout(move |_: &Bounds<Points>| {
        #[cfg(not(target_arch = "wasm32"))]
        if model.get().phase == Phase::Ready && std::env::var_os("ACP_UI_PROBE").is_some() {
            eprintln!("ACP_NSH_LAYOUT_READY elapsed={:.6}", probe_elapsed());
        }
    });
    if current.section == Section::Projects {
        body =
            body.child(
                view().flex_row().flex_wrap().gap_2().children(
                    current.project_names.clone().into_iter().enumerate().map(
                        move |(index, name)| {
                            button()
                                .accessibility_label(acp_ui_spike_model::presentation::sanitize(
                                    &name, 120,
                                ))
                                .px_2()
                                .py_1()
                                .focus(|s| s.border_2().border_color(theme().primary))
                                .on_press(move || switch_project(model, index))
                                .child(text(acp_ui_spike_model::presentation::sanitize(&name, 120)))
                        },
                    ),
                ),
            );
    }
    if matches!(current.section, Section::Handoff | Section::Diagnostics) {
        body = body.child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(control("Expand / collapse", move || {
                    model.update(|m| m.expanded = !m.expanded)
                }))
                .child(control("Earlier evidence", move || {
                    model.update(|m| m.evidence_by(-1))
                }))
                .child(control("Later evidence", move || {
                    model.update(|m| m.evidence_by(1))
                }))
                .child(text(format!(
                    "Evidence page {}/{}",
                    current.evidence_page + 1,
                    current.evidence_pages()
                ))),
        );
    }
    body.children(model.get().visible_lines().into_iter().map(text))
}
