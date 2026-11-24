//! Minimal native operator panel; all business transitions live in shared::Model.
//! The Queue / Detail / History review workspace, the destructive delete tier and
//! their motion are rendered here from the one shared Model.
use crate::*;
use precept_ui_core::{
    connection::Target,
    i18n::{Locale, Msg, tr},
    operator::{
        Channel, Command, DecisionAction, DeleteArm, Origin, Pane, Review, ReviewMode, ReviewTab,
        Scope,
    },
    presentation::sanitize,
};
use shell::{card, word_action};
use tokens::palette;

mod actions;
mod delete;
mod detail;
mod dialogs;
mod formatting;
mod history;
mod import;
mod panel;
mod queue;
mod records;
#[cfg(test)]
mod tests;

use actions::{available, refresh, review, write_grant};
pub(crate) use detail::export_dialog;
pub(crate) use panel::panel;

pub(crate) fn open(model: Signal<Model>) {
    open_mode(model, ReviewMode::Inbox);
}
/// Opens the reviewed rule records, retaining the project scope when one is active.
pub(crate) fn open_records(model: Signal<Model>) {
    let mode = if model.get().operator.mode == ReviewMode::Project
        && model.get().selected_project.is_some()
    {
        ReviewMode::Project
    } else {
        ReviewMode::Personal
    };
    open_mode(model, mode);
}
/// Opens Context on the pending queue or one of the reviewed scopes:
/// Personal (global records) or Project (the selected workspace).
pub(crate) fn open_mode(model: Signal<Model>, mode: ReviewMode) {
    model.update(|m| {
        let global = match mode {
            // The inbox keeps its scope, but a project scope needs a project.
            ReviewMode::Inbox => m.operator.global || m.selected_project.is_none(),
            ReviewMode::Personal => true,
            ReviewMode::Project => false,
        };
        if global != m.operator.global
            || mode != m.operator.mode
            || (mode != ReviewMode::Inbox && m.operator.pane != Pane::Preferences)
        {
            m.operator.invalidate();
            m.operator.global = global;
            m.operator.after_id = 0;
        }
        if mode != ReviewMode::Inbox {
            m.operator.pane = Pane::Preferences;
        }
        // Entering the workspace re-reads the Profile (writes may have changed
        // in Runtime checks while it was closed). Cleared here, before that read
        // is built: clearing it later would make the read's reply stale.
        if !m.operator.open {
            m.operator.info = None;
        }
        if m.operator.mode != mode {
            m.operator.batch.clear();
        }
        m.operator.mode = mode;
        m.operator.open = true;
        m.settings = false;
        m.setup_open = false;
        m.agents_page = false;
        m.sessions_page = false;
        m.memory_page = false;
        m.operator.review = None;
        m.operator.delete_arm = None;
        // The pending queue loads with the page, as the record lists do; the
        // page's Refresh reads it again.
        if mode == ReviewMode::Inbox && m.operator.queue.is_none() {
            m.operator.follow_up = Some("profile.review.queue");
        }
    });
    // A snapshot completion changes the read guard. Starting an Operator read
    // before it settles would fence that reply and leave the workspace busy.
    if model.get().phase != precept_ui_core::Phase::Loading {
        refresh(model, "profile.info");
    }
}
