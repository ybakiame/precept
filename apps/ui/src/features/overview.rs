//! Overview: the daemon, agent access, the current project's effective context,
//! the resume context of the looked-up task, and the way into review. Facts are
//! label / value rows (`ui::properties`), never colon-joined strings.
use crate::*;
use precept_ui_core::presentation::sanitize;
use precept_ui_core::{SetupTab, i18n::Msg};
use shell::target_words;
use tokens::{compact, palette};
use ui::{BadgeVariant, ButtonSize, ButtonVariant, badge, button, properties, scale, surface};

/// Guided onboarding card shown on Dashboard when setup is not complete yet.
pub(crate) fn onboarding(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    let conn_ok = super::setup::step_done(&m, SetupTab::Connection);
    let profile_ok = super::setup::step_done(&m, SetupTab::Profile);
    let ws_ok = super::setup::step_done(&m, SetupTab::Workspaces);
    let next_tab = if !conn_ok {
        SetupTab::Connection
    } else if !profile_ok {
        SetupTab::Profile
    } else {
        SetupTab::Workspaces
    };
    let step_row = |label: Msg, done: bool| {
        view()
            .flex_row()
            .items_center()
            .gap_3()
            .child(text(m.word(label)).flex_1())
            .child(
                badge(
                    model,
                    m.word(if done {
                        Msg::SetupStepComplete
                    } else {
                        Msg::SetupStepPending
                    }),
                )
                .variant(if done {
                    BadgeVariant::Success
                } else {
                    BadgeVariant::Neutral
                }),
            )
    };
    surface(model)
        .title(m.word(Msg::DashboardOnboardingTitle))
        .child(note(model, m.word(Msg::WorkspaceSetupIncomplete)))
        .child(
            view()
                .gap_2()
                .child(step_row(Msg::Connection, conn_ok))
                .child(step_row(Msg::ProfileSection, profile_ok))
                .child(step_row(Msg::Projects, ws_ok)),
        )
        .child(
            view().items_start().child(
                button(model, "")
                    .label_fn(move || model.get().word(Msg::GoToRuntimeChecks).into())
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Small)
                    .on_press(move || shell::go_setup(model, next_tab)),
            ),
        )
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
    context = context.child(note(model, m.word(Msg::ContextHelp)));
    for warning in p.context.warnings.iter().take(8) {
        context = context.child(note(model, sanitize(warning, 512)));
    }
    context = context.child(
        view().items_start().child(
            button(model, "")
                .label_fn(move || model.get().word(Msg::OpenPreferences).into())
                .size(ButtonSize::Small)
                .on_press(move || shell::go(model, shell::Destination::Preferences)),
        ),
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
        .child(text(m.word(Msg::Tools)))
        .child(text(m.word(Msg::ClientPresence)))
        .child(note(model, m.word(Msg::ClientHelp)))
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
        view().gap_4().child(daemon(model)).child(agent)
    } else {
        view()
            .flex_row()
            .items_start()
            .gap_4()
            .child(view().flex_1().min_w_0().child(daemon(model)))
            .child(view().flex_1().min_w_0().child(agent))
    };
    view()
        .flex_shrink_0()
        .gap_4()
        .child(context)
        .child(top)
        .child(resume)
        .child(review)
}
