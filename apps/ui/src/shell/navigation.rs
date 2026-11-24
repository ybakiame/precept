use crate::*;
use features::{connection, profile, review};
use precept_ui_core::{Section, SetupTab, i18n::Msg, operator::ReviewMode};

/// Every top-level sidebar destination: flat, unambiguous, mutually exclusive.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Destination {
    Dashboard,
    /// Preferences & contracts.
    Rules,
    Agents,
    /// Agents' work sessions.
    Sessions,
    /// Agents' project memory.
    Memory,
    Handoff,
    Review,
    ReviewedRules,
    RuntimeChecks,
    Workspaces,
    PersonalSync,
    Diagnostics,
    Settings,
}

pub(crate) fn current(m: &Model) -> Destination {
    if m.operator.open {
        if m.operator.mode == ReviewMode::Inbox {
            Destination::Review
        } else {
            Destination::ReviewedRules
        }
    } else if m.setup_open {
        if m.setup_tab == SetupTab::Workspaces {
            Destination::Workspaces
        } else {
            Destination::RuntimeChecks
        }
    } else if m.sync_open {
        Destination::PersonalSync
    } else if m.agents_page {
        Destination::Agents
    } else if m.sessions_page {
        Destination::Sessions
    } else if m.memory_page {
        Destination::Memory
    } else if m.settings {
        Destination::Settings
    } else {
        match m.section {
            Section::Dashboard => Destination::Dashboard,
            Section::Projects => Destination::Workspaces,
            Section::Rules => Destination::Rules,
            Section::Handoff => Destination::Handoff,
            Section::Diagnostics => Destination::Diagnostics,
        }
    }
}

fn leave_workspaces(m: &mut Model) {
    m.operator.open = false;
    m.operator.review = None;
    m.settings = false;
    m.setup_open = false;
    m.sync_open = false;
    m.agents_page = false;
    m.sessions_page = false;
    m.memory_page = false;
}

/// Opens Runtime checks directly on `tab` (Connection, Profile, or Workspaces).
pub(crate) fn go_setup(model: Signal<Model>, tab: SetupTab) {
    model.update(|m| {
        leave_workspaces(m);
        m.setup_open = true;
        m.setup_tab = tab;
    });
    if tab == SetupTab::Connection {
        connection::opened(model);
    }
}

/// Opens Context directly on the pending queue or a reviewed record scope.
pub(crate) fn go_review(model: Signal<Model>, mode: ReviewMode) {
    review::open_mode(model, mode);
}

/// Goes to `destination` (each opener keeps its own reads and guards).
pub(crate) fn go(model: Signal<Model>, destination: Destination) {
    match destination {
        Destination::Review => review::open(model),
        Destination::ReviewedRules => review::open_records(model),
        Destination::RuntimeChecks => {
            let tab = model.get().setup_tab;
            go_setup(
                model,
                if tab == SetupTab::Workspaces {
                    SetupTab::Connection
                } else {
                    tab
                },
            );
        }
        Destination::Workspaces => go_setup(model, SetupTab::Workspaces),
        Destination::PersonalSync => {
            model.update(|m| {
                leave_workspaces(m);
                m.sync_open = true;
            });
            profile::read(model);
        }
        Destination::Settings => model.update(|m| {
            leave_workspaces(m);
            m.settings = true;
        }),
        Destination::Agents => model.update(|m| {
            leave_workspaces(m);
            m.agents_page = true;
        }),
        Destination::Sessions => model.update(|m| {
            leave_workspaces(m);
            m.sessions_page = true;
        }),
        Destination::Memory => model.update(|m| {
            leave_workspaces(m);
            m.memory_page = true;
        }),
        Destination::Dashboard => model.update(|m| {
            leave_workspaces(m);
            m.set_section(Section::Dashboard);
        }),
        Destination::Rules => model.update(|m| {
            leave_workspaces(m);
            m.set_section(Section::Rules);
        }),
        Destination::Handoff => model.update(|m| {
            leave_workspaces(m);
            m.set_section(Section::Handoff);
        }),
        Destination::Diagnostics => model.update(|m| {
            leave_workspaces(m);
            m.set_section(Section::Diagnostics);
        }),
    }
}

pub(super) fn title_key(destination: Destination) -> Msg {
    match destination {
        Destination::Dashboard => Msg::Dashboard,
        Destination::Rules => Msg::RulesAndContracts,
        Destination::Agents => Msg::AgentsDestination,
        Destination::Sessions => Msg::SessionsDestination,
        Destination::Memory => Msg::MemoryDestination,
        Destination::Handoff => Msg::Handoff,
        Destination::Review => Msg::ReviewHub,
        Destination::ReviewedRules => Msg::RulesDestination,
        Destination::RuntimeChecks => Msg::RuntimeChecks,
        Destination::Workspaces => Msg::Projects,
        Destination::PersonalSync => Msg::SyncDestination,
        Destination::Diagnostics => Msg::Diagnostics,
        Destination::Settings => Msg::Settings,
    }
}

/// Whether a destination displays one workspace's data.
pub(crate) fn workspace_scoped(d: Destination) -> bool {
    matches!(
        d,
        Destination::Dashboard
            | Destination::Rules
            | Destination::Agents
            | Destination::Sessions
            | Destination::Memory
            | Destination::Handoff
    )
}

/// Whether the current destination has a copyable read-only snapshot page.
pub(super) fn copyable_destination(d: Destination) -> bool {
    matches!(
        d,
        Destination::Dashboard
            | Destination::Rules
            | Destination::Handoff
            | Destination::Diagnostics
    )
}
