use super::formatting::{channel_word, decision_word};
use super::*;

/// The History tab: bounded decision rows for the inspected id. A deleted record
/// keeps no content or history snapshot, so this falls back to the empty state.
pub(super) fn history_tab(model: Signal<Model>) -> AnyElement {
    let m = model.get();
    let mut body = view().gap_2().flex_shrink_0();
    let Some(inspect) = m
        .operator
        .inspect
        .as_ref()
        .filter(|i| m.operator.selected_id == Some(i.record.id))
    else {
        return body
            .child(
                ui::empty(model, m.word(Msg::ReviewNoSelection))
                    .description(m.word(Msg::ReviewQueueEmptyHelp)),
            )
            .into_element();
    };
    if inspect.decisions.is_empty() {
        return body
            .child(ui::empty(model, m.word(Msg::ReviewHistoryEmpty)))
            .into_element();
    }
    body = body.child(
        text(m.word(Msg::ReviewChannelNote))
            .text_size(px(ui::scale::text::CAPTION))
            .text_color(move || palette(model).muted),
    );
    let lines = inspect
        .decisions
        .iter()
        .take(8)
        .map(|d| {
            let mut line = format!(
                "{} · r{}→r{} · {} · {} · {} {}",
                decision_word(m.locale, d.action),
                d.from_revision,
                d.to_revision,
                channel_word(m.locale, d.channel),
                precept_ui_core::presentation::utc_minute(&d.decided_at),
                m.word(Msg::ReviewCommit),
                d.receipt_commit_sequence
            );
            if let Some(by) = d.replaced_by {
                line.push_str(&format!(
                    " · {}",
                    m.word(Msg::ReplacedBy).replace("{id}", &by.to_string())
                ));
            }
            if let Some(note) = &d.note {
                line.push_str(&format!(" — {}", sanitize(note, 500)));
            }
            line
        })
        .collect();
    body = body.child(card(model, m.word(Msg::ReviewHistoryTab), lines));
    body.into_element()
}
