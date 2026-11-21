//! The change between two versions of a rule's text, in the app's colors:
//! NSH's DiffView, one column, long lines wrapped, changed words marked.
use super::prelude::*;
use nsh_base::text_view::PartStyle;
use nsh_base::{DiffInline, DiffLayout, DiffViewStyles, diff_view};
use nsh_core::element::{Style, StyleRefinement};

/// Rows the view shows at most before it scrolls inside.
const MAX_ROWS: usize = 16;
/// Characters a wrapped row holds in a narrow column (a sheet), so the
/// estimate errs tall rather than cutting rows off.
const ROW_CHARS: usize = 38;

/// Rows `text` takes once long lines wrap.
fn wrapped_rows(text: &str) -> usize {
    text.lines()
        .map(|l| l.chars().count().div_ceil(ROW_CHARS).max(1))
        .sum::<usize>()
        .max(1)
}

fn styles(model: Signal<Model>) -> DiffViewStyles {
    let p = palette(model);
    let part = |s: StyleRefinement| Some(PartStyle::from(Style::from(s)));
    let mut s = DiffViewStyles::default();
    s.side = part(StyleRefinement::default().bg(p.panel));
    s.gutter = part(StyleRefinement::default().bg(p.panel));
    s.line_number = part(StyleRefinement::default().text_color(p.muted));
    s.content = part(StyleRefinement::default().text_color(p.text));
    s.line_added = part(StyleRefinement::default().bg(p.success.opacity(0.12)));
    s.line_deleted = part(StyleRefinement::default().bg(p.danger.opacity(0.12)));
    s.word_added = part(StyleRefinement::default().bg(p.success.opacity(0.32)));
    s.word_deleted = part(StyleRefinement::default().bg(p.danger.opacity(0.32)));
    s.indicator_added = part(StyleRefinement::default().bg(p.success));
    s.indicator_deleted = part(StyleRefinement::default().bg(p.danger));
    s.separator = part(StyleRefinement::default().bg(p.inset).text_color(p.muted));
    s.filler = part(StyleRefinement::default().bg(p.inset));
    s
}

/// `old` → `new`, unified, every unchanged line shown (rules are short),
/// its height from the wrapped row count up to [`MAX_ROWS`].
pub fn text_diff(model: Signal<Model>, old: String, new: String) -> impl IntoElement {
    let rows = wrapped_rows(&old) + wrapped_rows(&new);
    let height = (rows.clamp(2, MAX_ROWS) as f32) * 20.0 + 8.0;
    view()
        .w_full()
        .h(px(height))
        .rounded(px(scale::radius::CONTROL))
        .border_1()
        .border_color(move || palette(model).hairline)
        .overflow_hidden()
        .child(
            diff_view()
                .old_value(old)
                .new_value(new)
                .language("markdown")
                .layout(DiffLayout::Unified)
                .inline(DiffInline::WordAlt)
                .soft_wrap(true)
                .expand_all(true)
                .styles(move || styles(model))
                .flex_1(),
        )
}
