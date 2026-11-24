use super::*;
use ui::ButtonSize;

/// One event: time, what happened, who asked and the outcome.
pub(crate) fn activity_row(
    model: Signal<Model>,
    m: &Model,
    e: &precept_ui_core::activity::Event,
) -> AnyElement {
    let ok = e.outcome == "ok";
    let refused = e.kind == "refused";
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x_3()
        .gap_y_1()
        .py_1p5()
        .border_t_1()
        .border_color(move || palette(model).hairline)
        .child(
            text(precept_ui_core::activity::clock(e.at_ms))
                .font_family("monospace")
                .text_size(px(scale::text::CAPTION))
                .text_color(move || palette(model).muted)
                .flex_shrink_0(),
        )
        .child(
            view()
                .flex_1()
                .min_w(px(200.0))
                .gap_0p5()
                .child(text(m.activity_summary(e)))
                .child(
                    text(m.activity_who(e))
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).muted),
                ),
        )
        .child(
            badge(
                model,
                match (ok, &e.rule) {
                    (true, _) => "OK".to_owned(),
                    (false, Some(rule)) => format!(
                        "{} · {}",
                        e.outcome,
                        precept_ui_core::presentation::sanitize(rule, 64)
                    ),
                    (false, None) => e.outcome.clone(),
                },
            )
            // A gate's answer is a decision, not a failure: a denial is the
            // one worth a look; allow and off are routine.
            .variant(match (e.kind.as_str(), e.outcome.as_str()) {
                (_, "ok") | ("gate", "allow") => BadgeVariant::Success,
                ("gate", "off") => BadgeVariant::Neutral,
                ("gate", "deny") => BadgeVariant::Warning,
                _ if refused || e.outcome == "Unauthorized" => BadgeVariant::Danger,
                _ => BadgeVariant::Warning,
            }),
        )
        .into_element()
}

/// The most events listed (the daemon keeps more; Refresh reads the newest).
const SHOWN: usize = 50;

/// Recent agent activity of the selected project, with its chain status
/// (shown on the Sessions page).
pub(crate) fn activity(model: Signal<Model>) -> impl IntoElement {
    let state = computed(move || {
        let m = model.get();
        (m.agents.activity.clone(), m.locale)
    });
    surface(model)
        .title_fn(word(model, Msg::ActivityTitle))
        .child(
            view()
                .flex_row()
                .items_start()
                .gap_3()
                .child(
                    text(move || {
                        let m = model.get();
                        let chain = m
                            .agents
                            .activity
                            .page
                            .as_ref()
                            .and_then(|p| p.chain.clone());
                        let status = match chain {
                            Some(c) if !c.ok => m
                                .word(Msg::ChainBroken)
                                .replace("{n}", &c.broken_at.unwrap_or(0).to_string()),
                            Some(c) => m
                                .word(if c.dropped > 0 {
                                    Msg::ChainOkDropped
                                } else {
                                    Msg::ChainOk
                                })
                                .replace("{n}", &c.entries.to_string()),
                            None => String::new(),
                        };
                        format!("{} {status}", m.word(Msg::ActivityHelp))
                    })
                    .flex_1()
                    .text_size(px(scale::text::CAPTION))
                    .text_color(move || palette(model).muted),
                )
                .child(
                    button(model, "")
                        .label_fn(word(model, Msg::RefreshList))
                        .spoken(word(model, Msg::ActivityRefresh))
                        .size(ButtonSize::Small)
                        .variant(ButtonVariant::Ghost)
                        .icon(ui::icons::RESET)
                        .loading(move || model.get().agents.activity.busy)
                        .on_press(move || {
                            if let Some(name) = model.get().selected_project.clone() {
                                load_activity(model, name);
                            }
                        }),
                ),
        )
        .child(dynamic(move || {
            let (a, _) = state.get();
            let m = model.get_untracked();
            if let Some(code) = a.error {
                return text(code)
                    .text_size(px(scale::text::CAPTION))
                    .text_color(move || palette(model).error)
                    .into_element();
            }
            let Some(page) = a.page else {
                return view().into_element();
            };
            if page.events.is_empty() {
                return ui::empty(model, m.word(Msg::ActivityEmpty))
                    .icon(ui::icons::AGENTS)
                    .into_element();
            }
            let headline = match page.last_read() {
                Some(e) => m
                    .word(Msg::ActivityLastRead)
                    .replace("{time}", &precept_ui_core::activity::clock(e.at_ms))
                    .replace("{client}", &m.activity_who(e)),
                None => m.word(Msg::ActivityNeverRead).to_owned(),
            };
            let mut list = view().child(text(headline).font_weight(FontWeight::MEDIUM).pb_2());
            for e in page.events.iter().take(SHOWN) {
                list = list.child(activity_row(model, &m, e));
            }
            list.into_element()
        }))
}
