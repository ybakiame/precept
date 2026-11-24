use super::configuration::{configure, gate_row, group, hook_tile, levels, pointer_tile};
use super::state::{apply_pointer, caption, mcp_tile, word};
use super::*;

/// The integrations, by mechanism: MCP tools, the session-start briefing and
/// the AGENTS.md pointer. One switch per agent each.
pub(super) fn integrations(model: Signal<Model>) -> impl IntoElement {
    let row = || view().flex_row().flex_wrap().gap_2();
    surface(model)
        .title_fn(word(model, Msg::IntegrationsTitle))
        .child(caption(model, |_| Some(Msg::IntegrationsHelp)))
        .child(caption(model, |_| Some(Msg::OtherAgents)))
        .child(levels(model))
        .child(group(
            model,
            Msg::McpGroup,
            Msg::McpSummary,
            Msg::McpScope,
            row()
                .child(mcp_tile(model, "claude", Msg::TileClaude))
                .child(mcp_tile(model, "codex", Msg::TileCodex)),
        ))
        .child(group(
            model,
            Msg::HookGroup,
            Msg::HookSummary,
            Msg::HookScope,
            row()
                .child(hook_tile(model, "claude", "session_start", Msg::TileClaude))
                .child(hook_tile(model, "codex", "session_start", Msg::TileCodex)),
        ))
        .child(group(
            model,
            Msg::GateGroup,
            Msg::GateSummary,
            Msg::GateScope,
            view()
                .gap_2()
                .child(dynamic(move || gate_row(model)))
                .child(
                    row()
                        .child(hook_tile(model, "claude", "pre_tool", Msg::TileClaude))
                        .child(hook_tile(model, "codex", "pre_tool", Msg::TileCodex)),
                ),
        ))
        .child(group(
            model,
            Msg::ReadGroup,
            Msg::ReadSummary,
            Msg::ReadScope,
            row().child(hook_tile(model, "claude", "post_read", Msg::TileClaude)),
        ))
        .child(group(
            model,
            Msg::CompactGroup,
            Msg::CompactSummary,
            Msg::CompactScope,
            row().child(hook_tile(model, "claude", "pre_compact", Msg::TileClaude)),
        ))
        .child(group(
            model,
            Msg::PointerGroup,
            Msg::PointerSummary,
            Msg::PointerScope,
            // One tile wide, like the tiles of the groups above.
            row()
                .child(pointer_tile(model))
                .child(ui::tile_spacer())
                .child(ui::tile_spacer()),
        ))
        .child(
            text(move || {
                let m = model.get();
                m.agents
                    .hooks_error
                    .clone()
                    .or_else(|| m.agents.pointer_error.clone())
                    .unwrap_or_default()
            })
            .text_size(px(scale::text::CAPTION))
            .text_color(move || palette(model).error),
        )
}

/// The exact hook command, for setting a hook up by hand.
pub(super) fn hook_command(model: Signal<Model>) -> impl IntoElement {
    view()
        .gap_1()
        .style_when(
            move || {
                model
                    .get()
                    .agents
                    .hooks
                    .as_ref()
                    .and_then(|h| h.hooks.first())
                    .is_none()
            },
            |s| s.hidden(),
        )
        .child(caption(model, |_| Some(Msg::IntegrationsCommand)))
        .child(
            text(move || {
                model
                    .get()
                    .agents
                    .hooks
                    .as_ref()
                    .and_then(|h| h.get("claude"))
                    .map(|h| {
                        h.command
                            .replace("--vendor claude", "--vendor <claude|codex>")
                    })
                    .unwrap_or_default()
            })
            .font_family("monospace")
            .text_size(px(scale::text::CAPTION))
            .text_color(move || palette(model).muted),
        )
}

/// Manual setup, folded away: the MCP configuration to paste (for agents on
/// Windows or other clients) and the hook command.
pub(super) fn advanced(model: Signal<Model>) -> impl IntoElement {
    ui::disclosure(
        model,
        word(model, Msg::AdvancedSetup),
        word(model, Msg::AdvancedHint),
        view()
            .gap_6()
            .child(configure(model))
            .child(surface(model).child(hook_command(model))),
    )
}

/// The pointer confirmation, previewing the exact block; a sibling of the
/// page's scroll region.
pub(crate) fn pointer_dialog(model: Signal<Model>) -> impl IntoElement {
    ui::dialog(model, "")
        .title_fn(move || {
            let m = model.get();
            m.word(if m.agents.pointer_dialog == Some(false) {
                Msg::PointerRemoveTitle
            } else {
                Msg::PointerAddTitle
            })
            .into()
        })
        .description_fn(move || {
            let m = model.get();
            m.word(if m.agents.pointer_dialog == Some(false) {
                Msg::PointerRemoveBody
            } else {
                Msg::PointerAddBody
            })
            .into()
        })
        .content_fn(move || {
            view()
                .style_when(
                    move || model.get().agents.pointer_dialog != Some(true),
                    |s| s.hidden(),
                )
                .p_3()
                .rounded(px(scale::radius::CONTROL))
                .border_1()
                .border_color(move || palette(model).hairline)
                .bg(move || palette(model).background)
                .child(
                    text(move || {
                        model
                            .get()
                            .agents
                            .pointer
                            .as_ref()
                            .map(|p| p.block.clone())
                            .unwrap_or_default()
                    })
                    .font_family("monospace")
                    .text_size(px(scale::text::CAPTION)),
                )
                .into_element()
        })
        .cancel_fn(word(model, Msg::Cancel))
        .confirm_fn(move || {
            let m = model.get();
            m.word(if m.agents.pointer_dialog == Some(false) {
                Msg::RemoveConfirm
            } else {
                Msg::PointerApply
            })
            .into()
        })
        .confirm_disabled(move || model.get().agents.pointer_busy)
        .open(move || model.get().agents.pointer_dialog.is_some())
        .on_cancel(move || model.update(|m| m.agents.pointer_dialog = None))
        .on_confirm(move || {
            let add = model.get().agents.pointer_dialog == Some(true);
            apply_pointer(model, add)
        })
}
