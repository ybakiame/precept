//! The independent first-launch guide. It reuses the normal setup workflows
//! and adds only navigation, observed progress and completion presentation.
use crate::*;
use precept_ui_core::{OnboardingStep, SetupTab, i18n::Msg};
use tokens::{compact, palette};
use ui::{ButtonSize, ButtonVariant, scale};

fn required_done(model: Signal<Model>, step: OnboardingStep) -> bool {
    let m = model.get();
    if !m.live {
        return false;
    }
    match step {
        OnboardingStep::Connection => super::setup::step_done(&m, SetupTab::Connection),
        OnboardingStep::Profile => super::setup::step_done(&m, SetupTab::Profile),
        OnboardingStep::Workspace => super::setup::step_done(&m, SetupTab::Workspaces),
        OnboardingStep::Agents => super::agents::active_entry(&m).is_some_and(|entry| entry.agents),
        OnboardingStep::Complete => true,
    }
}

fn required_complete(model: Signal<Model>) -> u8 {
    [
        OnboardingStep::Connection,
        OnboardingStep::Profile,
        OnboardingStep::Workspace,
    ]
    .into_iter()
    .filter(|step| required_done(model, *step))
    .count() as u8
}

fn unlocked(model: Signal<Model>, step: OnboardingStep) -> bool {
    match step {
        OnboardingStep::Connection => true,
        OnboardingStep::Profile => required_done(model, OnboardingStep::Connection),
        OnboardingStep::Workspace => required_done(model, OnboardingStep::Profile),
        OnboardingStep::Agents | OnboardingStep::Complete => [
            OnboardingStep::Connection,
            OnboardingStep::Profile,
            OnboardingStep::Workspace,
        ]
        .into_iter()
        .all(|step| required_done(model, step)),
    }
}

fn position(step: OnboardingStep) -> u8 {
    match step {
        OnboardingStep::Connection => 1,
        OnboardingStep::Profile => 2,
        OnboardingStep::Workspace => 3,
        OnboardingStep::Agents => 4,
        OnboardingStep::Complete => 4,
    }
}

fn title(step: OnboardingStep) -> Msg {
    match step {
        OnboardingStep::Connection => Msg::LaunchGuideConnectionTitle,
        OnboardingStep::Profile => Msg::LaunchGuideProfileTitle,
        OnboardingStep::Workspace => Msg::LaunchGuideWorkspaceTitle,
        OnboardingStep::Agents => Msg::LaunchGuideAgentsTitle,
        OnboardingStep::Complete => Msg::LaunchGuideCompleteTitle,
    }
}

fn body(step: OnboardingStep) -> Msg {
    match step {
        OnboardingStep::Connection => Msg::LaunchGuideConnectionBody,
        OnboardingStep::Profile => Msg::LaunchGuideProfileBody,
        OnboardingStep::Workspace => Msg::LaunchGuideWorkspaceBody,
        OnboardingStep::Agents => Msg::LaunchGuideAgentsBody,
        OnboardingStep::Complete => Msg::LaunchGuideCompleteBody,
    }
}

fn icon(step: OnboardingStep) -> &'static str {
    match step {
        OnboardingStep::Connection => icons::CONNECTION,
        OnboardingStep::Profile => icons::PROFILE,
        OnboardingStep::Workspace => icons::section_path(precept_ui_core::Section::Projects),
        OnboardingStep::Agents => icons::AGENTS,
        OnboardingStep::Complete => icons::CHECK,
    }
}

fn rail_step(
    model: Signal<Model>,
    slide: &ui::Slide<OnboardingStep>,
    step: OnboardingStep,
    number: u8,
    label: Msg,
    description: Msg,
) -> AnyElement {
    slide.row(
        step,
        ui::guide_step(
            model,
            number,
            move || model.get().word(label).into(),
            move || model.get().word(description).into(),
        )
        .selected(move || model.get().onboarding_step == step)
        .done(move || {
            model.get().onboarding_step == OnboardingStep::Complete || required_done(model, step)
        })
        .disabled(move || !unlocked(model, step))
        .quiet_selection(|| true)
        .on_press(move || shell::go_onboarding(model, step)),
    )
}

fn rail(model: Signal<Model>) -> impl IntoElement {
    let slide = ui::Slide::<OnboardingStep>::default();
    let container = element_ref();
    let indicator = slide.indicator(model, container, move || model.get().onboarding_step);
    view()
        .element_ref(&container)
        .w(px(scale::layout::GUIDE_RAIL))
        .flex_1()
        .h_full()
        .flex_shrink_0()
        .p_3()
        .gap_6()
        .bg(move || palette(model).sidebar)
        .border_r_1()
        .border_color(move || palette(model).hairline)
        .child(indicator)
        .child(
            view()
                .gap_1()
                .child(
                    text(move || model.get().word(Msg::LaunchGuide))
                        .role(AccessibilityRole::Header)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(scale::text::SUBTITLE)),
                )
                .child(
                    text(move || {
                        format!(
                            "{}/3 · {}",
                            required_complete(model),
                            model.get().word(Msg::SetupStepComplete)
                        )
                    })
                    .text_size(px(scale::text::CAPTION))
                    .text_color(move || palette(model).muted),
                ),
        )
        .child(
            view()
                .gap_1()
                .child(rail_step(
                    model,
                    &slide,
                    OnboardingStep::Connection,
                    1,
                    Msg::Connection,
                    Msg::LaunchGuideConnectionBody,
                ))
                .child(rail_step(
                    model,
                    &slide,
                    OnboardingStep::Profile,
                    2,
                    Msg::ProfileSection,
                    Msg::LaunchGuideProfileBody,
                ))
                .child(rail_step(
                    model,
                    &slide,
                    OnboardingStep::Workspace,
                    3,
                    Msg::Projects,
                    Msg::LaunchGuideWorkspaceBody,
                ))
                .child(rail_step(
                    model,
                    &slide,
                    OnboardingStep::Agents,
                    4,
                    Msg::AgentsDestination,
                    Msg::LaunchGuideAgentsOptional,
                )),
        )
        .child(view().flex_1())
        .child(
            text(move || model.get().word(Msg::LaunchGuideIntro))
                .text_size(px(scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
}

fn top_bar(model: Signal<Model>) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .h(px(scale::layout::TITLE_BAR))
        .px_3()
        .flex_shrink_0()
        .border_b_1()
        .border_color(move || palette(model).hairline)
        .child(
            view()
                .text_color(move || palette(model).primary)
                .child(ui::icon::glyph(icons::BRAND, scale::icon::MD)),
        )
        .child(
            text(move || {
                let m = model.get();
                m.word(Msg::LaunchGuideStepOf)
                    .replace("{current}", &position(m.onboarding_step).to_string())
            })
            .text_size(px(scale::text::CAPTION))
            .text_color(move || palette(model).muted),
        )
        .child(view().flex_1().min_w_0().h_full().window_drag(true))
        .child(
            view()
                .style_when(
                    move || model.get().onboarding_step == OnboardingStep::Complete,
                    |s| s.hidden(),
                )
                .child(
                    ui::button(model, "")
                        .test_id("guide.skip")
                        .label_fn(move || model.get().word(Msg::LaunchGuideSkip).into())
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .on_press(move || shell::close_onboarding(model)),
                ),
        )
        .child(crate::shell::chrome::window_buttons(model))
}

fn heading(model: Signal<Model>, step: OnboardingStep) -> impl IntoElement {
    view()
        .flex_row()
        .flex_wrap()
        .items_start()
        .gap_4()
        .child(
            view()
                .size(px(scale::disc::LG))
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .rounded(px(scale::radius::PILL))
                .bg(move || {
                    if step == OnboardingStep::Complete {
                        palette(model).success.opacity(ui::tokens::tint::REST)
                    } else {
                        palette(model).primary.opacity(ui::tokens::tint::REST)
                    }
                })
                .text_color(move || {
                    if step == OnboardingStep::Complete {
                        palette(model).success
                    } else {
                        palette(model).primary
                    }
                })
                .child(ui::icon::glyph(icon(step), scale::icon::XL)),
        )
        .child(
            view()
                .flex_1()
                .min_w(px(240.0))
                .gap_1()
                .child(
                    text(model.get_untracked().word(title(step)))
                        .role(AccessibilityRole::Header)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(scale::text::TITLE)),
                )
                .child(
                    text(model.get_untracked().word(body(step)))
                        .text_size(px(scale::text::BODY))
                        .text_color(move || palette(model).muted),
                ),
        )
}

fn completion(model: Signal<Model>) -> impl IntoElement {
    dynamic(move || {
        let m = model.get();
        let project = m
            .selected_project
            .clone()
            .or_else(|| m.project().map(|p| p.name.clone()))
            .unwrap_or_else(|| m.word(Msg::Unknown).into());
        let mut facts = ui::properties(model)
            .row(m.word(Msg::Project), project)
            .row(m.word(Msg::TargetLabel), crate::shell::target_words(&m))
            .row(m.word(Msg::ProfileSection), m.word(Msg::SetupStepComplete));
        if let Some(entry) = super::agents::active_entry(&m) {
            facts = facts.row(
                m.word(Msg::AgentsProjectStatus),
                m.word(if entry.agents {
                    Msg::AgentsShared
                } else {
                    Msg::AgentsNotShared
                }),
            );
        }
        ui::surface(model).child(facts).into_element()
    })
}

fn step_content(model: Signal<Model>, step: OnboardingStep) -> AnyElement {
    match step {
        OnboardingStep::Connection => view()
            .gap_6()
            .child(heading(model, step))
            .child(super::connection::connection(model, false))
            .into_element(),
        OnboardingStep::Profile => view()
            .gap_6()
            .child(heading(model, step))
            .child(super::profile::section(model, false))
            .into_element(),
        OnboardingStep::Workspace => view()
            .gap_6()
            .child(heading(model, step))
            .child(super::projects::page(model))
            .into_element(),
        OnboardingStep::Agents => view()
            .gap_6()
            .child(heading(model, step))
            .child(super::agents::onboarding(model))
            .into_element(),
        OnboardingStep::Complete => view()
            .gap_6()
            .child(heading(model, step))
            .child(completion(model))
            .into_element(),
    }
}

fn previous(step: OnboardingStep) -> Option<OnboardingStep> {
    match step {
        OnboardingStep::Connection | OnboardingStep::Complete => None,
        OnboardingStep::Profile => Some(OnboardingStep::Connection),
        OnboardingStep::Workspace => Some(OnboardingStep::Profile),
        OnboardingStep::Agents => Some(OnboardingStep::Workspace),
    }
}

fn footer(model: Signal<Model>) -> impl IntoElement {
    let back = computed(move || previous(model.get().onboarding_step));
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .px_6()
        .py_3()
        .flex_shrink_0()
        .border_t_1()
        .border_color(move || palette(model).hairline)
        .bg(move || palette(model).background)
        .child(
            view()
                .style_when(move || back.get().is_none(), |s| s.hidden())
                .child(
                    ui::button(model, "")
                        .test_id("guide.back")
                        .label_fn(move || model.get().word(Msg::LaunchGuideBack).into())
                        .variant(ButtonVariant::Ghost)
                        .on_press(move || {
                            if let Some(step) = back.get() {
                                shell::go_onboarding(model, step);
                            }
                        }),
                ),
        )
        .child(view().flex_1())
        .child(
            ui::button(model, "")
                .test_id("guide.primary")
                .label_fn(move || {
                    let m = model.get();
                    let key = match m.onboarding_step {
                        OnboardingStep::Agents => Msg::LaunchGuideFinish,
                        OnboardingStep::Complete => Msg::LaunchGuideOpenDashboard,
                        _ => Msg::LaunchGuideContinue,
                    };
                    m.word(key).into()
                })
                .variant(ButtonVariant::Primary)
                .disabled(move || {
                    let m = model.get();
                    let step = m.onboarding_step;
                    (matches!(
                        step,
                        OnboardingStep::Connection
                            | OnboardingStep::Profile
                            | OnboardingStep::Workspace
                    ) && !required_done(model, step))
                        || (step == OnboardingStep::Agents && m.agents.busy)
                })
                .on_press(move || {
                    let step = model.get_untracked().onboarding_step;
                    match step {
                        OnboardingStep::Connection => {
                            shell::go_onboarding(model, OnboardingStep::Profile)
                        }
                        OnboardingStep::Profile => {
                            shell::go_onboarding(model, OnboardingStep::Workspace)
                        }
                        OnboardingStep::Workspace => {
                            shell::go_onboarding(model, OnboardingStep::Agents)
                        }
                        OnboardingStep::Agents => {
                            shell::go_onboarding(model, OnboardingStep::Complete)
                        }
                        OnboardingStep::Complete => shell::close_onboarding(model),
                    }
                }),
        )
}

/// The launch guide replaces the workspace shell until it is completed or
/// skipped. Its actions are the same daemon operations used by Runtime checks.
pub(crate) fn page(model: Signal<Model>) -> impl IntoElement {
    let step = computed(move || model.get().onboarding_step);
    let page_scroll = element_ref();
    let page_rise = animated_value(0.0);
    effect(move || {
        let _ = step.get();
        page_scroll.scroll_to_offset(Points(0.0), false);
        if !reduce_motion() {
            page_rise.set(ui::motion::RISE);
            page_rise.timing(0.0, ui::motion::FAST, Easing::EaseOut);
        }
    });
    view()
        .flex_1()
        .bg(move || palette(model).background)
        .text_color(move || palette(model).text)
        .text_size(px(scale::text::BODY))
        .font_family("sans-serif")
        .child(
            view()
                .flex_1()
                .flex_row()
                .style_when(compact, |s| s.flex_col())
                .child(
                    view()
                        .h_full()
                        .style_when(compact, |s| s.hidden())
                        .child(rail(model)),
                )
                .child(
                    view()
                        .flex_1()
                        .min_w_0()
                        .child(top_bar(model))
                        .child(
                            view()
                                .px_6()
                                .py_2()
                                .flex_shrink_0()
                                .style_when(compact, |s| s.px_2())
                                .child(
                                    text(move || {
                                        let m = model.get();
                                        format!(
                                            "{}/3 · {}",
                                            required_complete(model),
                                            m.word(Msg::SetupStepComplete)
                                        )
                                    })
                                    .text_size(px(scale::text::CAPTION))
                                    .text_color(move || palette(model).muted),
                                ),
                        )
                        .child(
                            scroll().flex_1().element_ref(&page_scroll).child(
                                view()
                                    .bind_translate_y(&page_rise)
                                    .w_full()
                                    .max_w(px(scale::layout::CONTENT))
                                    .px_6()
                                    .pb_8()
                                    .gap_6()
                                    .flex_shrink_0()
                                    .style_when(compact, |s| s.px_2().pb_4())
                                    .child(dynamic(move || {
                                        let current = step.get();
                                        untracked(|| step_content(model, current))
                                    })),
                            ),
                        )
                        .child(footer(model)),
                ),
        )
        .child(
            view()
                .absolute()
                .bottom_0()
                .right_0()
                .child(crate::shell::chrome::grip(model)),
        )
        .child(super::projects::remove_dialog(model))
        .child(crate::shortcuts::panel(model))
        .child(ui::toaster(model))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsh_engine::{EngineConfig, Root};

    fn ready_model() -> Model {
        let mut model = Model::fixture();
        model.live = true;
        model.onboarding_open = true;
        model.onboarding_step = OnboardingStep::Connection;
        *model.profile_info = Some(precept_ui_core::operator::ProfileInfo {
            selected: true,
            write_enabled: true,
            ..Default::default()
        });
        std::sync::Arc::make_mut(model.data.as_mut().unwrap()).directory =
            vec![precept_ui_core::ProjectEntry {
                name: "alpha".into(),
                root: String::new(),
                state: "ready".into(),
                agents: true,
                propose: false,
                gate: false,
            }];
        model
    }

    #[component]
    fn ReadyGuide() -> impl IntoElement {
        console(signal(ready_model()))
    }

    #[component]
    fn LockedGuide() -> impl IntoElement {
        let mut model = Model::loading();
        model.onboarding_open = true;
        console(signal(model))
    }

    #[component]
    fn CompletingGuide() -> impl IntoElement {
        let mut model = Model::loading();
        model.onboarding_open = true;
        let model = signal(model);
        set_timeout(std::time::Duration::from_millis(50), move || {
            model.set(ready_model());
        });
        console(model)
    }

    #[component]
    fn AgentStatusGuide() -> impl IntoElement {
        let mut model = ready_model();
        model.onboarding_step = OnboardingStep::Complete;
        std::sync::Arc::make_mut(model.data.as_mut().unwrap()).directory[0].agents = false;
        let model = signal(model);
        set_timeout(std::time::Duration::from_millis(50), move || {
            model.update(|m| {
                std::sync::Arc::make_mut(m.data.as_mut().unwrap()).directory[0].agents = true;
            });
        });
        console(model)
    }

    #[component]
    fn BusyAgentsGuide() -> impl IntoElement {
        let mut model = ready_model();
        model.onboarding_step = OnboardingStep::Agents;
        model.agents.busy = true;
        std::sync::Arc::make_mut(model.data.as_mut().unwrap()).directory[0].agents = false;
        let model = signal(model);
        set_timeout(std::time::Duration::from_millis(50), move || {
            model.update(|m| {
                m.agents.busy = false;
                std::sync::Arc::make_mut(m.data.as_mut().unwrap()).directory[0].agents = true;
            });
        });
        console(model)
    }

    #[test]
    fn the_guide_advances_through_required_steps_and_optional_agents() {
        let mut cfg = EngineConfig::new(Root::of::<ReadyGuide>());
        cfg.viewport = Size::new(px(1180.0), px(780.0));
        let mut app = nsh_testing::Headless::start(cfg).unwrap();
        app.wait_for("Connect to your daemon").unwrap();
        app.press("Continue").unwrap();
        app.wait_for("Choose your private Profile").unwrap();
        app.press("Back").unwrap();
        app.wait_for("Connect to your daemon").unwrap();
        app.press("Continue").unwrap();
        app.wait_for("Choose your private Profile").unwrap();
        app.press("Continue").unwrap();
        app.wait_for("Add your first workspace").unwrap();
        app.press("Continue").unwrap();
        app.wait_for("Connect an agent when you are ready").unwrap();
        app.press("Finish setup").unwrap();
        app.wait_for("Your workspace is ready").unwrap();
        app.press("Open Dashboard").unwrap();
        app.wait_for("Project: alpha").unwrap();
    }

    #[test]
    fn a_locked_step_cannot_be_skipped_with_continue() {
        let mut cfg = EngineConfig::new(Root::of::<LockedGuide>());
        cfg.viewport = Size::new(px(1180.0), px(780.0));
        let mut app = nsh_testing::Headless::start(cfg).unwrap();
        app.wait_for("Connect to your daemon").unwrap();
        let _ = app.press("Continue");
        app.wait_for("Connect to your daemon").unwrap();
        assert!(!app.screen().shows("Choose your private Profile"));
    }

    #[test]
    fn rail_steps_unlock_when_required_results_arrive() {
        let mut cfg = EngineConfig::new(Root::of::<CompletingGuide>());
        cfg.viewport = Size::new(px(1180.0), px(780.0));
        let mut app = nsh_testing::Headless::start(cfg).unwrap();
        app.wait_for("Connect to your daemon").unwrap();
        let _ = app.press("Profile");
        assert!(!app.screen().shows("Choose your private Profile"));
        app.wait_for("3/3 · Ready").unwrap();
        app.press("Profile").unwrap();
        app.wait_for("Choose your private Profile").unwrap();
    }

    #[test]
    fn compact_layout_keeps_reactive_back_navigation() {
        let mut cfg = EngineConfig::new(Root::of::<ReadyGuide>());
        cfg.viewport = Size::new(px(800.0), px(780.0));
        let mut app = nsh_testing::Headless::start(cfg).unwrap();
        app.wait_for("Connect to your daemon").unwrap();
        app.press("Continue").unwrap();
        app.wait_for("Choose your private Profile").unwrap();
        app.press("Back").unwrap();
        app.wait_for("Connect to your daemon").unwrap();
    }

    #[test]
    fn completion_tracks_delayed_agent_sharing_results() {
        let mut cfg = EngineConfig::new(Root::of::<AgentStatusGuide>());
        cfg.viewport = Size::new(px(1180.0), px(780.0));
        let mut app = nsh_testing::Headless::start(cfg).unwrap();
        app.wait_for("Not shared").unwrap();
        app.wait_for("Shared with agents").unwrap();
    }

    #[test]
    fn finish_waits_for_the_agent_sharing_mutation() {
        let mut cfg = EngineConfig::new(Root::of::<BusyAgentsGuide>());
        cfg.viewport = Size::new(px(1180.0), px(780.0));
        let mut app = nsh_testing::Headless::start(cfg).unwrap();
        app.wait_for("Connect an agent when you are ready").unwrap();
        let _ = app.press("Finish setup");
        assert!(!app.screen().shows("Your workspace is ready"));
        app.wait_for("Shared with agents").unwrap();
        app.press("Finish setup").unwrap();
        app.wait_for("Your workspace is ready").unwrap();
    }

    #[test]
    fn runtime_checks_can_reopen_the_guide_at_the_optional_agent_step() {
        let mut cfg = EngineConfig::new(Root::of::<ReadyGuide>());
        cfg.viewport = Size::new(px(1180.0), px(780.0));
        let mut app = nsh_testing::Headless::start(cfg).unwrap();
        app.wait_for("Connect to your daemon").unwrap();
        app.press("Skip setup").unwrap();
        app.wait_for("Project: alpha").unwrap();
        app.press("Runtime checks").unwrap();
        app.wait_for("Reopen launch guide…").unwrap();
        app.press("Reopen launch guide…").unwrap();
        app.wait_for("Connect an agent when you are ready").unwrap();
    }

    #[test]
    fn skip_returns_to_the_dashboard_and_escape_also_leaves_the_guide() {
        let mut cfg = EngineConfig::new(Root::of::<ReadyGuide>());
        cfg.viewport = Size::new(px(1180.0), px(780.0));
        let mut app = nsh_testing::Headless::start(cfg.clone()).unwrap();
        app.wait_for("Connect to your daemon").unwrap();
        app.press("Skip setup").unwrap();
        app.wait_for("Project: alpha").unwrap();

        let mut app = nsh_testing::Headless::start(cfg).unwrap();
        app.wait_for("Connect to your daemon").unwrap();
        app.press_key("Escape").unwrap();
        app.wait_for("Project: alpha").unwrap();
    }
}
