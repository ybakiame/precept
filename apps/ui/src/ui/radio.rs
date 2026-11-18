//! The app's `RadioGroup`: one choice from a small visible set (a WSL
//! distribution, a connection target). Built on the semantic `nsh/base` radio
//! group: one Tab stop, arrows move and check, Space checks, the radio role and
//! checked state for screen readers. Each option is a full-width row with the
//! app's indicator, hover wash, pressed wash and a persistent checked fill; in a
//! terminal the base draws `(•)` / `( )` and the app's dot is left out.
use super::prelude::*;
use nsh_base::{radio as base_radio, radio_group as base_radio_group};
use std::rc::Rc;

type LabelFn = Rc<dyn Fn() -> SharedString>;
type StateFn = Rc<dyn Fn() -> bool>;
type PressFn = Rc<dyn Fn()>;

/// A radio group builder; create it with [`radio_group`] and add options in
/// reading order.
pub struct RadioGroup {
    model: Signal<Model>,
    label: LabelFn,
    options: Vec<AnyElement>,
}

/// An empty group; `label` is what screen readers call the set.
pub fn radio_group(model: Signal<Model>, label: impl Fn() -> SharedString + 'static) -> RadioGroup {
    RadioGroup {
        model,
        label: Rc::new(label),
        options: Vec::new(),
    }
}

impl RadioGroup {
    /// Add an option: its label, whether it is the chosen one, and what
    /// choosing it does. The app owns the value; choosing never unchecks.
    pub fn option(
        mut self,
        label: impl Into<SharedString>,
        checked: impl Fn() -> bool + 'static,
        on_select: impl Fn() + 'static,
    ) -> Self {
        let model = self.model;
        let label: SharedString = label.into();
        let checked: StateFn = Rc::new(checked);
        let on_select: PressFn = Rc::new(on_select);
        let (c_row, c_dot, c_text, c_a11y) = (
            checked.clone(),
            checked.clone(),
            checked.clone(),
            label.clone(),
        );
        let row = base_radio()
            .checked(move || checked())
            .accessibility_label(move || Some(c_a11y.clone()))
            .on_change(move |_: &bool| on_select())
            .flex_row()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .min_h(px(scale::control::MD))
            .rounded(px(scale::radius::CONTROL))
            .border_2()
            .border_color(move || palette(model).panel.opacity(0.0))
            .bg(move || {
                if c_row() {
                    palette(model).inset
                } else {
                    palette(model).panel.opacity(0.0)
                }
            })
            .hover(move |s| s.bg(move || mix(palette(model).inset, palette(model).text, 0.07)))
            .active(move |s| s.bg(move || mix(palette(model).inset, palette(model).text, 0.13)))
            .focus(move |s| s.border_color(move || palette(model).primary))
            .child(indicator(model, c_dot))
            .child(
                text(label.clone())
                    .text_size(px(scale::text::BODY))
                    .text_color(move || {
                        if c_text() {
                            palette(model).text
                        } else {
                            palette(model).muted
                        }
                    }),
            );
        self.options.push(motion::feedback(row).into_element());
        self
    }
}

/// The app's radio dot: a ring that fills with the accent when checked.
fn indicator(model: Signal<Model>, checked: StateFn) -> impl IntoElement {
    let ring = checked.clone();
    view()
        .style_when(terminal, |s| s.hidden())
        .size(px(scale::icon::SM))
        .items_center()
        .justify_center()
        .rounded(px(scale::radius::PILL))
        .border_2()
        .border_color(move || {
            if ring() {
                palette(model).primary
            } else {
                palette(model).border
            }
        })
        .child(
            view()
                .size(px(scale::icon::SM / 2.0 - 1.0))
                .rounded(px(scale::radius::PILL))
                .bg(move || palette(model).primary)
                .style_when(move || !checked(), |s| s.hidden()),
        )
}

impl IntoElement for RadioGroup {
    fn into_element(self) -> AnyElement {
        let label = self.label;
        base_radio_group()
            .accessibility_label(move || Some(label()))
            .gap_0p5()
            .children(self.options)
            .into_element()
    }
}
