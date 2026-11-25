//! Overview: the daemon, agent access, the current project's effective context,
//! the resume context of the looked-up task, and the way into review. Facts are
//! label / value rows (`ui::properties`), never colon-joined strings.
use crate::*;
use precept_ui_core::presentation::sanitize;
use precept_ui_core::{OnboardingStep, SetupTab, i18n::Msg};
use shell::target_words;
use tokens::{compact, palette};
use ui::{BadgeVariant, ButtonSize, ButtonVariant, badge, button, properties, scale, surface};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Recommendation {
    Setup(OnboardingStep),
    Review(i64),
    Agents(String),
    Preference,
    Ready,
}

fn recommend(m: &Model) -> Recommendation {
    if !super::setup::step_done(m, SetupTab::Connection) {
        return Recommendation::Setup(OnboardingStep::Connection);
    }
    if !super::setup::step_done(m, SetupTab::Profile) {
        return Recommendation::Setup(OnboardingStep::Profile);
    }
    if !super::setup::step_done(m, SetupTab::Workspaces) {
        return Recommendation::Setup(OnboardingStep::Workspace);
    }
    if let Some(n) = m.review_count.filter(|n| *n > 0) {
        return Recommendation::Review(n);
    }
    if let Some(entry) = super::agents::active_entry(m)
        && !entry.agents
    {
        return Recommendation::Agents(entry.name);
    }
    if m.project()
        .is_some_and(|project| project.context.preferences.is_empty())
    {
        return Recommendation::Preference;
    }
    Recommendation::Ready
}

/// The Dashboard's one recommended action, chosen from observed setup and work.
/// It is guidance only: every route opens the existing workflow and its guards.
pub(crate) fn next_step(model: Signal<Model>) -> impl IntoElement {
    let recommendation = computed(move || recommend(&model.get()));
    dynamic(move || {
        let next = recommendation.get();
        let m = model.get_untracked();
        let card = match next {
            Recommendation::Setup(_) => ui::next_action(
                model,
                icons::BRAND,
                m.word(Msg::NextStepSetupTitle),
                m.word(Msg::NextStepSetupBody),
                m.word(Msg::LaunchGuideContinue),
                move || shell::reopen_onboarding(model),
            )
            .secondary(m.word(Msg::GoToRuntimeChecks), move || {
                shell::go_setup(model, SetupTab::Connection)
            }),
            Recommendation::Review(n) => ui::next_action(
                model,
                icons::REVIEW,
                m.word(Msg::NextStepReviewTitle)
                    .replace("{n}", &n.to_string()),
                m.word(Msg::NextStepReviewBody),
                m.word(Msg::OpenReview),
                move || shell::go(model, shell::Destination::Review),
            ),
            Recommendation::Agents(name) => ui::next_action(
                model,
                icons::AGENTS,
                m.word(Msg::NextStepAgentsTitle)
                    .replace("{name}", &sanitize(&name, 64)),
                m.word(Msg::NextStepAgentsBody),
                m.word(Msg::OpenAgentsPage),
                move || shell::go(model, shell::Destination::Agents),
            ),
            Recommendation::Preference => ui::next_action(
                model,
                icons::section_path(precept_ui_core::Section::Rules),
                m.word(Msg::NextStepPreferenceTitle),
                m.word(Msg::NextStepPreferenceBody),
                m.word(Msg::OpenPreferences),
                move || shell::go(model, shell::Destination::Rules),
            ),
            Recommendation::Ready => ui::next_action(
                model,
                icons::CHECK,
                m.word(Msg::NextStepReadyTitle),
                m.word(Msg::NextStepReadyBody),
                m.word(Msg::OpenHandoff),
                move || shell::go(model, shell::Destination::Handoff),
            )
            .secondary(m.word(Msg::OpenReview), move || {
                shell::go(model, shell::Destination::Review)
            }),
        };
        card.into_element()
    })
}

fn note(model: Signal<Model>, value: impl Into<SharedString>) -> impl IntoElement {
    text(value.into())
        .text_size(px(scale::text::CAPTION))
        .text_color(move || palette(model).muted)
}

pub(crate) fn daemon(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    let transport = match m.connection.target {
        precept_ui_core::connection::Target::Synthetic => "—",
        precept_ui_core::connection::Target::Wsl => "wsl.exe --exec → Unix IPC",
        _ => {
            if cfg!(windows) {
                "Windows named pipe"
            } else {
                "Unix IPC"
            }
        }
    };
    let mut facts = properties(model)
        .row(m.word(Msg::TargetLabel), target_words(&m))
        .row(m.word(Msg::StateLabel), m.read_label())
        .row(m.word(Msg::TransportLabel), transport)
        .row(m.word(Msg::LastReadLabel), m.read_time());
    if let Some(d) = &m.data {
        facts = facts
            .row(m.word(Msg::Version), d.health.api_version.to_string())
            .row(m.word(Msg::Schema), sanitize(&d.health.schema, 64));
    }
    if let Some(duration) = m.last_duration_ms {
        facts = facts.row(m.word(Msg::Duration), format!("{duration} ms"));
    }
    surface(model)
        .title(m.word(Msg::Daemon))
        .child(facts)
        .child(note(
            model,
            m.word(if m.live {
                Msg::HealthHelp
            } else {
                Msg::SnapshotHealth
            }),
        ))
}

/// The daemon at a glance: where, its state, the last read; the transport,
/// versions and timing are in Diagnostics.
fn connection(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    surface(model)
        .title(m.word(Msg::Daemon))
        .child(
            properties(model)
                .row(m.word(Msg::TargetLabel), target_words(&m))
                .row(m.word(Msg::StateLabel), m.read_label())
                .row(m.word(Msg::LastReadLabel), m.read_time()),
        )
        .child(
            view().items_start().child(
                button(model, "")
                    .label_fn(move || model.get().word(Msg::ConnectionDetails).into())
                    .size(ButtonSize::Small)
                    .variant(ButtonVariant::Ghost)
                    .on_press(move || shell::go(model, shell::Destination::Diagnostics)),
            ),
        )
}

pub(crate) fn dashboard(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    let p = m.project().expect("ready project");
    let r = &p.context.repository;
    let unknown = m.word(Msg::Unknown);
    let mut context = surface(model).title(m.word(Msg::EffectiveContext)).child(
        properties(model)
            .row(m.word(Msg::Project), sanitize(&p.name, 120))
            .row(
                m.word(Msg::Branch),
                sanitize(r.branch.as_deref().unwrap_or(unknown), 80),
            )
            .row("HEAD", sanitize(r.head.as_deref().unwrap_or(unknown), 80))
            .row(
                m.word(Msg::Worktree),
                m.word(match r.dirty {
                    Some(true) => Msg::HasChanges,
                    Some(false) => Msg::NoChanges,
                    None => Msg::Unknown,
                }),
            )
            .row(
                m.word(Msg::ConfirmedPreferences),
                p.context.preferences.len().to_string(),
            )
            .row(
                m.word(Msg::ContractReferences),
                p.context.documents.len().to_string(),
            ),
    );
    // What the context promises and does not, behind its info button.
    let about = std::iter::once(m.word(Msg::ContextHelp).to_owned())
        .chain(
            p.context
                .warnings
                .iter()
                .take(8)
                .map(|w| precept_ui_core::presentation::warning(m.locale, w)),
        )
        .collect::<Vec<_>>()
        .join("\n\n");
    context = context.child(
        view()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                button(model, "")
                    .label_fn(move || model.get().word(Msg::OpenPreferences).into())
                    .size(ButtonSize::Small)
                    .on_press(move || shell::go(model, shell::Destination::Rules)),
            )
            .child(ui::info(
                model,
                move || model.get().word(Msg::AboutContext).into(),
                move || about.clone().into(),
            )),
    );

    let mut resume = surface(model).title(m.word(Msg::ResumeSummary));
    if let Some(h) = p.handoff.as_ref().filter(|h| h.task_id == m.queried_task) {
        resume = resume.child(
            properties(model)
                .row(m.word(Msg::TaskLabel), sanitize(&h.task_id, 120))
                .row(m.word(Msg::Objective), &h.objective)
                .row(m.word(Msg::NextAction), &h.next_action),
        );
        for warning in h.resume_warnings.iter().take(8) {
            resume = resume.child(note(model, sanitize(warning, 512)));
        }
    } else if m.queried_task.is_empty() {
        resume = resume.child(note(model, m.word(Msg::NoHandoff)));
    } else {
        resume = resume.child(note(
            model,
            format!(
                "{} {} {}",
                m.word(Msg::HandoffMissing),
                sanitize(&m.queried_task, 120),
                m.word(Msg::InProject)
            ),
        ));
    }
    resume = resume
        .child(note(model, m.word(Msg::EarlierNotCurrent)))
        .child(
            view().items_start().child(
                button(model, "")
                    .label_fn(move || model.get().word(Msg::OpenHandoff).into())
                    .size(ButtonSize::Small)
                    .on_press(move || shell::go(model, shell::Destination::Handoff)),
            ),
        );

    let agent = surface(model)
        .title(m.word(Msg::ClientAccess))
        // The card's title already names it: no second heading.
        .child(super::agents::level_rows(model, false))
        .child(
            view().items_start().child(
                button(model, "")
                    .label_fn(move || model.get().word(Msg::OpenAgentsPage).into())
                    .size(ButtonSize::Small)
                    .on_press(move || shell::go(model, shell::Destination::Agents)),
            ),
        );

    let mut review = surface(model).title(m.word(Msg::ReviewSuggestions));
    if let Some(n) = m.review_count.filter(|n| *n > 0) {
        review = review.child(
            view().items_start().child(
                badge(
                    model,
                    m.word(Msg::ReviewWaiting).replace("{n}", &n.to_string()),
                )
                .variant(BadgeVariant::Info),
            ),
        );
    }
    let review = review.child(note(model, m.word(Msg::ReviewHelp))).child(
        view().items_start().child(
            button(model, "")
                .label_fn(move || model.get().word(Msg::OpenReview).into())
                .size(ButtonSize::Small)
                .on_press(move || shell::go(model, shell::Destination::Review)),
        ),
    );

    let top = if compact() {
        view().gap_4().child(connection(model)).child(agent)
    } else {
        view()
            .flex_row()
            .items_start()
            .gap_4()
            .child(view().flex_1().min_w_0().child(connection(model)))
            .child(view().flex_1().min_w_0().child(agent))
    };
    view()
        .flex_shrink_0()
        .gap_4()
        .child(next_step(model))
        .child(context)
        .child(top)
        .child(resume)
        .child(review)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> Model {
        let mut model = Model::fixture();
        model.live = true;
        *model.profile_info = Some(precept_ui_core::operator::ProfileInfo {
            selected: true,
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

    #[test]
    fn recommendations_prioritize_setup_then_review_then_agent_access() {
        let mut model = ready();
        model.review_count = Some(3);
        assert_eq!(recommend(&model), Recommendation::Review(3));

        std::sync::Arc::make_mut(model.data.as_mut().unwrap())
            .directory
            .iter_mut()
            .for_each(|entry| {
                entry.agents = false;
            });
        assert!(matches!(recommend(&model), Recommendation::Review(3)));
        model.review_count = None;
        assert!(matches!(
            recommend(&model),
            Recommendation::Agents(name) if name == "alpha"
        ));

        std::sync::Arc::make_mut(model.data.as_mut().unwrap())
            .directory
            .iter_mut()
            .for_each(|entry| {
                entry.agents = true;
            });
        std::sync::Arc::make_mut(model.data.as_mut().unwrap()).projects[0]
            .context
            .preferences
            .clear();
        assert_eq!(recommend(&model), Recommendation::Preference);
    }

    #[test]
    fn incomplete_setup_is_always_the_first_recommendation() {
        let mut model = ready();
        model.review_count = Some(9);
        model.live = false;
        assert_eq!(
            recommend(&model),
            Recommendation::Setup(OnboardingStep::Connection)
        );
        model.live = true;
        model.profile_info.as_mut().unwrap().selected = false;
        std::sync::Arc::make_mut(model.data.as_mut().unwrap())
            .health
            .schema = "Unselected".into();
        assert_eq!(
            recommend(&model),
            Recommendation::Setup(OnboardingStep::Profile)
        );
        model.profile_info.as_mut().unwrap().selected = true;
        std::sync::Arc::make_mut(model.data.as_mut().unwrap())
            .projects
            .clear();
        assert_eq!(
            recommend(&model),
            Recommendation::Setup(OnboardingStep::Workspace)
        );
    }
}
