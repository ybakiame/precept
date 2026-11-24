use super::{
    chrome,
    header::{header, preference_input, task_input, top_bar},
    navigation::{Destination, copyable_destination, current},
    sidebar::{brand_bar, sidebar},
};
use crate::*;
use features::{
    agents, doctor, memory, page, profile, projects, review, sessions, settings, setup,
};
use tokens::{compact, palette};

pub(crate) fn shell(
    model: Signal<Model>,
    task_ref: ElementRef,
    filter_ref: ElementRef,
) -> impl IntoElement {
    profile::watch(model);
    profile::watch_count(model);
    agents::watch(model);
    sessions::watch(model);
    memory::watch(model);
    features::handoff::watch(model);
    let on = move |d: Destination| move || current(&model.get()) != d;
    // A new destination starts at its top and its content settles in with a
    // short rise (`fast`); nothing remounts, so inputs keep state. Movement
    // only: an opacity cross-fade left content that changed mid-fade drawn
    // dimmed on Windows until the next fade.
    let page_rise = animated_value(0.0);
    let page_scroll = element_ref();
    let destination = computed(move || current(&model.get()));
    effect(move || {
        let _ = destination.get();
        page_scroll.scroll_to_offset(Points(0.0), false);
        if !reduce_motion() {
            page_rise.set(ui::motion::RISE);
            page_rise.timing(0.0, ui::motion::FAST, Easing::EaseOut);
        }
    });
    view()
        .flex_1()
        .bg(move || palette(model).background)
        .child(
            view()
                .flex_1()
                .flex_row()
                .style_when(compact, |s| s.flex_col())
                .font_family("sans-serif")
                .bg(move || palette(model).background)
                .text_color(move || palette(model).text)
                .text_size(px(ui::scale::text::BODY))
                // The sidebar runs from the window's top edge to its bottom:
                // its brand bar is level with the window's buttons.
                .child(
                    view()
                        .flex_shrink_0()
                        .bg(move || palette(model).sidebar)
                        .border_color(move || palette(model).hairline)
                        .style_when(move || !compact(), |s| s.border_r_1())
                        .style_when(compact, |s| s.border_b_1())
                        .style_when(
                            move || !compact() && model.get().sidebar_collapsed,
                            |s| s.hidden(),
                        )
                        .child(brand_bar(model))
                        .child(sidebar(model)),
                )
                .child(
                    view()
                        .flex_1()
                        .min_w_0()
                        .child(view().child(top_bar(model)))
                        .child(
                            view()
                                .flex_1()
                                .min_w_0()
                                .px_6()
                                .pt_2()
                                .gap_4()
                                .style_when(compact, |s| s.px_2().pt_2().gap_2())
                                .child(header(model))
                                .child(task_input(model, task_ref))
                                .child(preference_input(model, filter_ref))
                                .child(review::panel(model))
                                .child(projects::remove_dialog(model))
                                .child(features::preferences::detail_dialog(model))
                                .child(features::rule_lint::templates_dialog(model))
                                .child(features::review::export_dialog(model))
                                .child(agents::pointer_dialog(model))
                                .child(
                                    scroll()
                                        .flex_1()
                                        .element_ref(&page_scroll)
                                        .style_when(
                                            move || model.get().operator.open,
                                            |s| s.hidden(),
                                        )
                                        .child(
                                            view()
                                                .bind_translate_y(&page_rise)
                                                .gap_4()
                                                .pb_6()
                                                .flex_shrink_0()
                                                .child(
                                                    view()
                                                        .style_when(
                                                            on(Destination::Settings),
                                                            |s| s.hidden(),
                                                        )
                                                        .child(settings::appearance(model)),
                                                )
                                                .child(
                                                    view()
                                                        .style_when(
                                                            on(Destination::RuntimeChecks),
                                                            |s| s.hidden(),
                                                        )
                                                        .child(setup::page(model)),
                                                )
                                                .child(
                                                    view()
                                                        .style_when(
                                                            on(Destination::Workspaces),
                                                            |s| s.hidden().invisible(),
                                                        )
                                                        .child(projects::page(model)),
                                                )
                                                .child(
                                                    view()
                                                        .style_when(on(Destination::Agents), |s| {
                                                            s.hidden()
                                                        })
                                                        .child(agents::page(model)),
                                                )
                                                .child(
                                                    view()
                                                        .style_when(
                                                            on(Destination::Sessions),
                                                            |s| s.hidden(),
                                                        )
                                                        .child(sessions::page(model)),
                                                )
                                                .child(
                                                    view()
                                                        .style_when(on(Destination::Memory), |s| {
                                                            s.hidden()
                                                        })
                                                        .child(memory::page(model)),
                                                )
                                                .child(
                                                    view()
                                                        .style_when(
                                                            move || {
                                                                !copyable_destination(current(
                                                                    &model.get(),
                                                                ))
                                                            },
                                                            |s| s.hidden(),
                                                        )
                                                        .child(dynamic(move || page(model))),
                                                )
                                                .child(
                                                    view()
                                                        .style_when(
                                                            on(Destination::Diagnostics),
                                                            |s| s.hidden(),
                                                        )
                                                        .child(doctor::panel(model)),
                                                ),
                                        ),
                                ),
                        ),
                ),
        )
        // The resize grip sits in the window's corner over the page, so the
        // sidebar runs to the bottom edge.
        .child(
            view()
                .absolute()
                .bottom_0()
                .right_0()
                .child(chrome::grip(model)),
        )
        .child(crate::shortcuts::panel(model))
        .child(crate::features::tags::panel(model))
        // Last, so toasts float above the page; they never take its space.
        .child(ui::toaster(model))
}
