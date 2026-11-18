//! Overview: daemon status, agent access and the effective context summary.
use crate::*;
use acp_operator_core::i18n::Msg;
use acp_operator_core::presentation::sanitize;
use shell::*;
use tokens::compact;

pub(crate) fn daemon(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    let target = match m.connection.target {
        acp_operator_core::connection::Target::Synthetic => Msg::SyntheticTarget,
        acp_operator_core::connection::Target::Local => Msg::LocalTarget,
        acp_operator_core::connection::Target::Wsl => Msg::WslTarget,
    };
    let transport = match m.connection.target {
        acp_operator_core::connection::Target::Synthetic => "—",
        acp_operator_core::connection::Target::Wsl => "wsl.exe --exec → Unix IPC",
        _ => {
            if cfg!(windows) {
                "Windows named pipe"
            } else {
                "Unix IPC"
            }
        }
    };
    let mut lines = vec![
        format!("{} · {}", m.word(target), m.read_label()),
        format!("{}: {}", m.word(Msg::TransportLabel), transport),
        m.read_time(),
    ];
    if let Some(d) = &m.data {
        lines.push(format!(
            "{}: {} · {}: {}",
            m.word(Msg::Version),
            d.health.api_version,
            m.word(Msg::Schema),
            sanitize(&d.health.schema, 64)
        ));
    }
    if let Some(duration) = m.last_duration_ms {
        lines.push(format!("{}: {} ms", m.word(Msg::Duration), duration));
    }
    lines.push(
        m.word(if m.live {
            Msg::HealthHelp
        } else {
            Msg::SnapshotHealth
        })
        .into(),
    );
    card(model, m.word(Msg::Daemon), lines)
}
pub(crate) fn dashboard(model: Signal<Model>) -> impl IntoElement {
    let m = model.get();
    let p = m.project().expect("ready project");
    let mut context = vec![
        format!("{}: {}", m.word(Msg::Project), sanitize(&p.name, 120)),
        format!(
            "{}: {} · HEAD: {}",
            m.word(Msg::Branch),
            sanitize(
                p.context
                    .repository
                    .branch
                    .as_deref()
                    .unwrap_or(m.word(Msg::Unknown)),
                80
            ),
            sanitize(
                p.context
                    .repository
                    .head
                    .as_deref()
                    .unwrap_or(m.word(Msg::Unknown)),
                80
            )
        ),
        format!(
            "{}: {}",
            m.word(Msg::Worktree),
            m.word(match p.context.repository.dirty {
                Some(true) => Msg::HasChanges,
                Some(false) => Msg::NoChanges,
                None => Msg::Unknown,
            })
        ),
        format!(
            "{}: {} · {}: {}",
            m.word(Msg::ConfirmedPreferences),
            p.context.preferences.len(),
            m.word(Msg::ContractReferences),
            p.context.documents.len()
        ),
        m.word(Msg::ContextHelp).into(),
    ];
    context.extend(p.context.warnings.iter().take(8).cloned());
    let mut resume = vec![m.word(Msg::EarlierNotCurrent).into()];
    if let Some(h) = p.handoff.as_ref().filter(|h| h.task_id == m.queried_task) {
        resume.push(format!("{} {}", m.word(Msg::Handoff), h.task_id));
        resume.push(format!("{}: {}", m.word(Msg::Objective), h.objective));
        resume.push(format!("{}: {}", m.word(Msg::NextAction), h.next_action));
        resume.extend(h.resume_warnings.iter().take(8).cloned());
    } else if m.queried_task.is_empty() {
        resume.push(m.word(Msg::NoHandoff).into());
    } else {
        resume.push(format!(
            "{} {} {}",
            m.word(Msg::HandoffMissing),
            m.queried_task,
            m.word(Msg::InProject)
        ));
    }
    let agent = card(
        model,
        m.word(Msg::ClientAccess),
        vec![
            m.word(Msg::ClientPresence).into(),
            m.word(Msg::Tools).into(),
            m.word(Msg::ClientHelp).into(),
        ],
    );
    let cards = if compact() {
        view()
            .gap_2()
            .flex_shrink_0()
            .child(daemon(model))
            .child(agent)
    } else {
        view()
            .flex_row()
            .gap_2()
            .flex_shrink_0()
            .child(view().flex_1().child(daemon(model)))
            .child(view().flex_1().child(agent))
    };
    view()
        .flex_shrink_0()
        .gap_3()
        .child(cards)
        .child(card(model, m.word(Msg::EffectiveContext), context))
        .child(card(model, m.word(Msg::ResumeSummary), resume))
        .child(card(
            model,
            m.word(Msg::ReviewSuggestions),
            vec![
                m.word(Msg::ReviewDeferred).into(),
                m.word(Msg::ReviewHelp).into(),
            ],
        ))
}
