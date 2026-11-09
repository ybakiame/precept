//! Shared responsive shell and small, scoped, read-only page components.
use super::*;
use acp_ui_spike_model::{PAGE_SIZE, Section, presentation::sanitize};
use nsh_core::element::input;
use tokens::{compact, palette};

pub(super) fn action(
    model: Signal<Model>,
    label: impl Into<String>,
    enabled: impl Fn() -> bool + 'static,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    let label = label.into();
    button()
        .accessibility_label(label.clone())
        .px_2()
        .py_1()
        .rounded(px(5.0))
        .border_2()
        .bg(move || palette(model).inset)
        .text_color(move || palette(model).text)
        .border_color(move || palette(model).inset)
        .hover(|s| s.border_color(palette(model).border))
        .focus(|s| s.border_color(palette(model).primary))
        .disabled(move || !enabled())
        .disabled_style(|s| s.opacity(0.45))
        .on_press(callback)
        .child(text(label))
}
fn control(
    model: Signal<Model>,
    label: &'static str,
    callback: impl Fn() + 'static,
) -> impl IntoElement {
    action(model, label, || true, callback)
}
fn card(model: Signal<Model>, title: &'static str, lines: Vec<String>) -> impl IntoElement {
    view()
        .p_3()
        .gap_2()
        .rounded(px(8.0))
        .border_1()
        .border_color(move || palette(model).border)
        .bg(move || palette(model).panel)
        .child(
            text(title)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(14.0)),
        )
        .children(lines.into_iter().map(|line| text(sanitize(&line, 512))))
}
fn navigation(model: Signal<Model>) -> impl IntoElement {
    view()
        .gap_1()
        .style_when(compact, |s| s.flex_row().flex_wrap())
        .children(
            Section::ALL
                .into_iter()
                .enumerate()
                .map(move |(index, section)| {
                    button()
                        .accessibility_label(section.label())
                        .px_2()
                        .py_1()
                        .border_2()
                        .rounded(px(5.0))
                        .selected(move || model.get().section == section)
                        .bg(move || {
                            if model.get().section == section {
                                palette(model).primary
                            } else {
                                palette(model).panel
                            }
                        })
                        .text_color(move || {
                            if model.get().section == section {
                                palette(model).on_primary
                            } else {
                                palette(model).text
                            }
                        })
                        .border_color(move || {
                            if model.get().section == section {
                                palette(model).primary
                            } else {
                                palette(model).panel
                            }
                        })
                        .hover(|s| s.border_color(palette(model).border))
                        .focus(|s| s.border_color(palette(model).text))
                        .on_press(move || model.update(|m| m.set_section(section)))
                        .child(text(format!("{}  {}", index + 1, section.label())))
                }),
        )
}
pub(super) fn shell(
    model: Signal<Model>,
    task_ref: ElementRef,
    filter_ref: ElementRef,
) -> impl IntoElement {
    view().flex_1().p_4().gap_3().style_when(compact,|s|s.p_2().gap_1())
        .bg(move||palette(model).background).text_color(move||palette(model).text).text_size(px(14.0))
        .child(view().flex_row().flex_wrap().gap_2()
            .child(text("Agent Control Plane").font_weight(FontWeight::SEMIBOLD).text_size(px(20.0)))
            .child(text(move||format!(" / {}",model.get().selected_project.or_else(||model.get().project().map(|p|p.name.clone())).unwrap_or_else(||"Choose a project".into()))).text_color(move||palette(model).muted))
            .child(control(model,"Next project",move||next_project(model))))
        .child(view().flex_row().gap_3().flex_1().style_when(compact,|s|s.flex_col().gap_1())
            .child(view().w(move||px(if compact(){(window().width.0-16.0).max(1.0)}else{176.0})).gap_3()
                .child(navigation(model))
                .child(view().gap_2().style_when(compact,|s|s.hidden())
                    .child(text("OPERATOR CONSOLE").text_size(px(12.0)).text_color(move||palette(model).muted))
                    .child(text("Read-only by design. Repository facts are observations; earlier evidence is historical.").text_color(move||palette(model).muted))
                    .child(text("Alt 1–5 pages\nCtrl P project\nCtrl F filter\nCtrl K exact task\nCtrl R reconnect").text_size(px(12.0)))))
            .child(view().flex_1().gap_2()
                .child(view().flex_row().flex_wrap().gap_2()
                    .child(text(move||model.get().section.label()).font_weight(FontWeight::SEMIBOLD).text_size(px(22.0)))
                    .child(control(model,"Reconnect",move||reconnect(model)))
                    .child(control(model,"Light / dark",move||toggle_theme(model)))
                    .child(control(model,"High contrast",move||toggle_contrast(model)))
                    .child(action(model,"Copy visible page",move||model.get().phase==Phase::Ready,move||copy_page(model))))
                .child(view().accessible(true).label("Daemon connection state").gap_1()
                    .child(text(move||format!("{} · {}",model.get().source_label(),model.get().read_label()))
                        .text_color(move||match model.get().phase {Phase::Error|Phase::Unavailable=>palette(model).error,Phase::Loading=>palette(model).warning,_=>palette(model).muted}))
                    .child(text(move||model.get().read_time()).text_size(px(12.0)).text_color(move||palette(model).muted)))
                .child(view().gap_1().style_when(move||model.get().section!=Section::Handoff,|s|s.hidden())
                    .child(text("Task ID · exact project-scoped lookup (max 120 characters)").text_size(px(12.0)))
                    .child(view().flex_row().gap_2()
                        .child(input().element_ref(&task_ref).label("Task ID").h(px(30.0)).flex_1().border_2().border_color(move||palette(model).border)
                            .bg(move||palette(model).panel).text_color(move||palette(model).text).focus(|s|s.border_color(palette(model).primary))
                            .value(move||model.get().task_id).placeholder("Enter a task ID")
                            .on_change(move|v:&str|model.update(|m|m.task_id=v.chars().take(121).collect()))
                            .on_submit(move|_:&str|reconnect(model)))
                        .child(control(model,"Lookup",move||reconnect(model)))))
                .child(view().gap_1().style_when(move||model.get().section!=Section::Preferences,|s|s.hidden())
                    .child(text("Filter confirmed preferences · key, value, scope or source").text_size(px(12.0)))
                    .child(input().element_ref(&filter_ref).label("Filter preferences").h(px(30.0)).border_2().border_color(move||palette(model).border)
                        .bg(move||palette(model).panel).text_color(move||palette(model).text).focus(|s|s.border_color(palette(model).primary))
                        .value(move||model.get().filter).placeholder("Filter preferences")
                        .on_change(move|v:&str|model.update(|m|m.set_filter(v))))
                    .child(view().flex_row().flex_wrap().gap_1()
                        .child(action(model,"Previous page",move||model.get().page>0,move||model.update(|m|m.page_by(-1))))
                        .child(action(model,"Next page",move||(model.get().page+1)*PAGE_SIZE<model.get().rows().len(),move||model.update(|m|m.page_by(1))))
                        .child(text(move||model.get().page_label()))))
                .child(scroll().flex_1().child(dynamic(move||page(model))))
                .child(text(move||if model.get().copied {"Copied the visible sanitized content"}else{"Ctrl F filter · K task · D theme · Shift H contrast · Escape blur/cancel · Ctrl Q close"}).text_size(px(12.0)).text_color(move||palette(model).muted))))
}
fn preferences(model: Signal<Model>) -> impl IntoElement {
    let current = model.get();
    let rows = current.rows();
    let page = current.page;
    let mut body = view().gap_2().child(
        text("Confirmed effective preferences · key / value / scope / source")
            .font_weight(FontWeight::SEMIBOLD),
    );
    if rows.is_empty() {
        return body.child(card(
            model,
            "No matching preferences",
            vec![
                "Empty — no confirmed preferences match".into(),
                "Try a shorter filter. Only confirmed records in this scope are shown.".into(),
            ],
        ));
    }
    if let Some(row) = current.preference_detail() {
        body = body
            .child(card(
                model,
                "Selected preference",
                vec![
                    sanitize(&row.key, 120),
                    current.detail_chunk(),
                    format!(
                        "Value detail {}/{} · {} / {}",
                        current.detail_page + 1,
                        current.detail_pages(),
                        sanitize(&row.scope, 64),
                        sanitize(&row.source, 80)
                    ),
                ],
            ))
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_1()
                    .child(action(
                        model,
                        "Earlier value",
                        move || model.get().detail_page > 0,
                        move || model.update(|m| m.detail_by(-1)),
                    ))
                    .child(action(
                        model,
                        "Later value",
                        move || model.get().detail_page + 1 < model.get().detail_pages(),
                        move || model.update(|m| m.detail_by(1)),
                    ))
                    .child(control(model, "Copy value detail", move || {
                        set_clipboard(model.get().detail_chunk());
                        model.update(|m| m.copied = true);
                    }))
                    .child(control(model, "Close detail", move || {
                        model.update(|m| m.selected_preference = None)
                    })),
            );
    }
    for (offset, row) in rows
        .iter()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .enumerate()
    {
        let index = page * PAGE_SIZE + offset;
        body = body.child(
            view()
                .p_2()
                .gap_1()
                .rounded(px(6.0))
                .bg(move || palette(model).panel)
                .border_1()
                .border_color(move || palette(model).border)
                .child(
                    view()
                        .flex_row()
                        .gap_2()
                        .child(
                            text(sanitize(&row.key, 64))
                                .font_weight(FontWeight::SEMIBOLD)
                                .flex_1(),
                        )
                        .child(action(
                            model,
                            format!("Details {}", offset + 1),
                            || true,
                            move || model.update(|m| m.select_preference(index)),
                        )),
                )
                .child(text(sanitize(&row.value, 160)))
                .child(
                    text(format!(
                        "{} / {}",
                        sanitize(&row.scope, 64),
                        sanitize(&row.source, 80)
                    ))
                    .text_size(px(12.0))
                    .text_color(move || palette(model).muted),
                ),
        );
    }
    body
}
fn page(model: Signal<Model>) -> impl IntoElement {
    let current = model.get();
    let mut body =
        view()
            .gap_3()
            .style_when(compact, |s| s.gap_2())
            .on_layout(move |_: &Bounds<Points>| {
                #[cfg(not(target_arch = "wasm32"))]
                if model.get().phase == Phase::Ready && std::env::var_os("ACP_UI_PROBE").is_some() {
                    eprintln!("ACP_NSH_LAYOUT_READY elapsed={:.6}", probe_elapsed());
                }
            });
    if current.phase != Phase::Ready {
        return body.child(card(model,current.read_label(),vec![current.status(),"Reconnect after checking the local daemon and private operator access. The console never starts the daemon.".into()]));
    }
    if current.section == Section::Preferences {
        return body.child(preferences(model));
    }
    if current.section == Section::Projects {
        body = body.child(
            view().flex_row().flex_wrap().gap_2().children(
                current
                    .project_names
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(move |(index, name)| {
                        action(
                            model,
                            sanitize(&name, 120),
                            || true,
                            move || switch_project(model, index),
                        )
                    }),
            ),
        );
    }
    if matches!(current.section, Section::Handoff | Section::Diagnostics) {
        body = body.child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_1()
                .child(control(model, "Expand / collapse", move || {
                    model.update(|m| m.expanded = !m.expanded)
                }))
                .child(action(
                    model,
                    "Earlier evidence",
                    move || model.get().evidence_page > 0,
                    move || model.update(|m| m.evidence_by(-1)),
                ))
                .child(action(
                    model,
                    "Later evidence",
                    move || model.get().evidence_page + 1 < model.get().evidence_pages(),
                    move || model.update(|m| m.evidence_by(1)),
                ))
                .child(text(format!(
                    "Evidence page {}/{}",
                    current.evidence_page + 1,
                    current.evidence_pages()
                ))),
        );
    }
    let mut lines = current.visible_lines();
    if current.section == Section::Diagnostics {
        lines.extend(super::runtime_lines());
    }
    let title = match current.section {
        Section::Dashboard => "Selected project / repository snapshot",
        Section::Projects => "Allowlisted project scope",
        Section::Handoff => "Handoff / resume evidence",
        _ => "Connection / runtime limitations",
    };
    body.child(card(model, title, lines))
}
