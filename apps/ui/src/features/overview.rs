//! Overview: the daemon, agent access, the current project's effective context,
//! the resume context of the looked-up task, and the way into review. Facts are
//! label / value rows (`ui::properties`), never colon-joined strings.
use crate::*;
use acp_ui_core::i18n::Msg;
use acp_ui_core::presentation::sanitize;
use shell::target_words;
use tokens::{compact, palette};
use ui::{ButtonSize, button, properties, scale, surface};

fn note(model: Signal<Model>, value: impl Into<SharedString>) -> impl IntoElement {
    text(value.into())
        .text_size(px(scale::text::CAPTION))
        .text_color(move || palette(model).muted)
}

pub(crate) fn daemon(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    let transport = match m.connection.target {
        acp_ui_core::connection::Target::Synthetic => "—",
        acp_ui_core::connection::Target::Wsl => "wsl.exe --exec → Unix IPC",
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
    resume = resume.child(note(model, m.word(Msg::EarlierNotCurrent)));

    let agent = surface(model)
        .title(m.word(Msg::ClientAccess))
        .child(text(m.word(Msg::Tools)))
        .child(text(m.word(Msg::ClientPresence)))
        .child(note(model, m.word(Msg::ClientHelp)));

    let review = surface(model)
        .title(m.word(Msg::ReviewSuggestions))
        .child(note(model, m.word(Msg::ReviewHelp)))
        .child(
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
