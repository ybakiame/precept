use super::state::{active_entry, set_propose, set_shared, word};
use super::*;

/// Access toggle for the currently selected workspace (no second project list).
pub(super) fn project_access(model: Signal<Model>) -> impl IntoElement {
    let state = computed(move || {
        let m = model.get();
        (active_entry(&m), m.locale)
    });
    dynamic(move || {
        let (Some(entry), _) = state.get() else {
            return view().into_element();
        };
        let m = model.get_untracked();
        let name = entry.name.clone();
        let shared = entry.agents;
        let toggle_name = name.clone();
        let spoken_name = entry.label.clone().unwrap_or_else(|| name.clone());
        surface(model)
            .title_fn(word(model, Msg::AgentsProjectStatus))
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .child(
                        view()
                            .flex_1()
                            .min_w(px(160.0))
                            .gap_0p5()
                            .child(
                                text(precept_ui_core::presentation::sanitize(&spoken_name, 64))
                                    .font_weight(FontWeight::SEMIBOLD),
                            )
                            .child(
                                text(precept_ui_core::presentation::sanitize(&entry.root, 512))
                                    .font_family("monospace")
                                    .text_size(px(scale::text::CAPTION))
                                    .text_color(move || palette(model).muted),
                            ),
                    )
                    .child(
                        text(m.word(if shared {
                            Msg::AgentsShared
                        } else {
                            Msg::AgentsNotShared
                        }))
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                    )
                    .child(ui::switch(
                        model,
                        move || {
                            model
                                .get()
                                .word(Msg::ShareWithAgents)
                                .replace("{name}", &spoken_name)
                                .into()
                        },
                        move || shared,
                        move || model.get().agents.busy,
                        move |on| set_shared(model, toggle_name.clone(), on),
                    )),
            )
            .child(suggest_row(model, &entry))
            .into_element()
    })
}

/// Whether the project's agents may suggest rules (`suggest_rule`).
pub(super) fn suggest_row(model: Signal<Model>, entry: &ProjectEntry) -> impl IntoElement {
    let m = model.get_untracked();
    let on = entry.propose;
    let name = entry.name.clone();
    let spoken = entry.label.clone().unwrap_or_else(|| entry.name.clone());
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_3()
        .pt_3()
        .border_t_1()
        .border_color(move || palette(model).hairline)
        .child(
            view()
                .flex_1()
                .min_w(px(160.0))
                .gap_0p5()
                .child(
                    text(m.word(if on { Msg::SuggestOn } else { Msg::SuggestOff }))
                        .font_weight(FontWeight::MEDIUM),
                )
                .child(
                    text(m.word(Msg::SuggestHelp))
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                ),
        )
        .child(ui::switch(
            model,
            move || {
                model
                    .get()
                    .word(Msg::SuggestSwitch)
                    .replace("{name}", &spoken)
                    .into()
            },
            move || on,
            move || model.get().agents.busy || !entry_shared(&model.get()),
            move |enabled| set_propose(model, name.clone(), enabled),
        ))
}
pub(super) fn entry_shared(m: &Model) -> bool {
    active_entry(m).is_some_and(|e| e.agents)
}
