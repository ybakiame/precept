//! The app's `Properties`: read-only facts as label / value rows on two fixed
//! lanes, so values line up down the region and a missing value never moves
//! the next row. Values are sanitized display text; long values wrap.
use super::prelude::*;
use acp_operator_core::presentation::sanitize;

/// A properties builder; create it with [`properties`].
pub struct Properties {
    model: Signal<Model>,
    rows: Vec<(SharedString, String)>,
}

/// An empty list.
pub fn properties(model: Signal<Model>) -> Properties {
    Properties {
        model,
        rows: Vec::new(),
    }
}

impl Properties {
    /// One fact: its label and its value (display text, sanitized here).
    pub fn row(mut self, label: impl Into<SharedString>, value: impl AsRef<str>) -> Self {
        self.rows
            .push((label.into(), sanitize(value.as_ref(), 512)));
        self
    }
}

impl IntoElement for Properties {
    fn into_element(self) -> AnyElement {
        let model = self.model;
        view()
            .gap_2()
            .children(self.rows.into_iter().map(move |(label, value)| {
                // One element for assistive technology: "Label: value".
                let spoken = format!("{label}: {value}");
                view()
                    .accessible(true)
                    .label(spoken)
                    .flex_row()
                    .gap_3()
                    .style_when(compact, |s| s.flex_col().gap_0p5())
                    .child(
                        text(label)
                            .w(px(scale::layout::LABEL_LANE))
                            .flex_shrink_0()
                            .text_size(px(scale::text::BODY))
                            .text_color(move || palette(model).muted),
                    )
                    .child(
                        text(value)
                            .flex_1()
                            .min_w_0()
                            .text_size(px(scale::text::BODY))
                            .text_color(move || palette(model).text),
                    )
            }))
            .into_element()
    }
}
