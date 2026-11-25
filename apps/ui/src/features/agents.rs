//! Agents (MCP): per-project external agent sharing, the exact configuration
//! to paste into Claude Code, Codex or any MCP client, and recent agent
//! activity for the currently selected workspace.
//!
//! `precept-mcp` is a stdio program the client starts itself (no port, no
//! listener). The daemon supplies the launch line (`project.mcp`); the page only
//! formats it. Sharing is switched per project by the daemon (`project.agents`),
//! which withdraws or republishes that project's agent credential.
use crate::*;
use precept_ui_core::{
    ProjectEntry,
    connection::Target,
    i18n::Msg,
    mcp::{Client, Hooks, Host, Launch, McpEntries, Pointer, SERVER, snippet},
};
use tokens::palette;
use ui::{BadgeVariant, ButtonVariant, badge, button, scale, surface, tabs};

mod access;
mod configuration;
mod integrations;
mod recent;
mod state;

use access::project_access;
pub(crate) use configuration::level_rows;
pub(crate) use integrations::pointer_dialog;
use integrations::{advanced, integrations};
pub(crate) use recent::{activity, activity_row};
pub(crate) use state::active_entry;
use state::{directory, word};
pub(crate) use state::{fetch, load_activity, watch};

/// The optional agent step in the first-launch guide. It reuses the same
/// per-project switch and daemon guards as the Agents page.
pub(crate) fn onboarding(model: Signal<Model>) -> impl IntoElement {
    view()
        .gap_4()
        .child(
            text(move || model.get().word(Msg::LaunchGuideAgentsBody))
                .text_size(px(scale::text::BODY))
                .text_color(move || palette(model).muted),
        )
        .child(project_access(model))
        .child(
            view().items_start().child(
                badge(
                    model,
                    model.get_untracked().word(Msg::LaunchGuideAgentsOptional),
                )
                .variant(BadgeVariant::Neutral),
            ),
        )
}

/// The Agents page (mounted by the shell outside the per-change page rebuild).
pub(crate) fn page(model: Signal<Model>) -> impl IntoElement {
    let empty_state = dynamic(move || {
        let m = model.get();
        if !m.live {
            return surface(model)
                .child(
                    ui::empty(model, m.word(Msg::AgentsOfflineTitle))
                        .icon(icons::AGENTS)
                        .description(m.word(Msg::AgentsOfflineBody))
                        .action(
                            button(model, "")
                                .label_fn(word(model, Msg::GoToRuntimeChecks))
                                .variant(ButtonVariant::Primary)
                                .on_press(move || {
                                    shell::go_setup(model, precept_ui_core::SetupTab::Connection)
                                }),
                        ),
                )
                .into_element();
        }
        if directory(&m).is_empty() {
            return surface(model)
                .child(
                    ui::empty(model, m.word(Msg::AgentsNeedsProject))
                        .icon(icons::AGENTS)
                        .description(m.word(Msg::AgentsNeedsProjectHelp))
                        .action(
                            button(model, "")
                                .label_fn(word(model, Msg::GoToWorkspaces))
                                .variant(ButtonVariant::Primary)
                                .on_press(move || {
                                    shell::go_setup(model, precept_ui_core::SetupTab::Workspaces)
                                }),
                        ),
                )
                .into_element();
        }
        view().into_element()
    });
    view()
        .gap_6()
        .max_w(px(scale::layout::CONTENT))
        .child(empty_state)
        .child(
            view()
                .gap_6()
                .style_when(
                    move || {
                        let m = model.get();
                        !m.live || active_entry(&m).is_none()
                    },
                    |s| s.hidden(),
                )
                .child(project_access(model))
                .child(integrations(model))
                .child(advanced(model)),
        )
}
