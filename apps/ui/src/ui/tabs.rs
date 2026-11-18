//! The app's `Tabs`: a tab list built on the semantic `nsh/base` tabs. The base
//! owns the WAI-ARIA tablist keyboard pattern (one Tab stop, arrows / Home / End
//! move between tabs, Space / Return activate) and a single selection indicator
//! that slides to the chosen tab natively, honoring reduced motion. The app
//! styles the indicator and each tab from tokens; in a terminal the base draws
//! the selected tab bold on a band and the others muted, with no indicator.
use super::prelude::*;
use std::rc::Rc;

type LabelFn = Rc<dyn Fn() -> SharedString>;
type StateFn = Rc<dyn Fn() -> bool>;
type PressFn = Rc<dyn Fn()>;

/// A tab list builder; create it with [`tabs`] and add tabs in reading order.
pub struct Tabs {
    model: Signal<Model>,
    items: Vec<AnyElement>,
}

/// An empty tab list with the app's sliding selection.
pub fn tabs(model: Signal<Model>) -> Tabs {
    Tabs {
        model,
        items: Vec::new(),
    }
}

impl Tabs {
    /// Add a tab with a static label, its selected state and its activation.
    pub fn add(
        self,
        label: impl Into<SharedString>,
        selected: impl Fn() -> bool + 'static,
        on_press: impl Fn() + 'static,
    ) -> Self {
        let label = label.into();
        self.add_inner(Rc::new(move || label.clone()), selected, on_press)
    }

    /// Add a tab whose label stays current (a localized word).
    pub fn add_fn(
        self,
        label: impl Fn() -> SharedString + 'static,
        selected: impl Fn() -> bool + 'static,
        on_press: impl Fn() + 'static,
    ) -> Self {
        self.add_inner(Rc::new(label), selected, on_press)
    }

    fn add_inner(
        mut self,
        label: LabelFn,
        selected: impl Fn() -> bool + 'static,
        on_press: impl Fn() + 'static,
    ) -> Self {
        let model = self.model;
        let selected: StateFn = Rc::new(selected);
        let on_press: PressFn = Rc::new(on_press);
        let color_sel = selected.clone();
        let a11y_label = label.clone();
        let tab = base_tab()
            .selected(move || selected())
            .accessibility_label(move || Some(a11y_label()))
            .on_press(move || on_press())
            .px_3()
            .py_1p5()
            .rounded(px(scale::radius::CONTROL))
            .text_size(px(scale::text::BODY))
            .text_color(move || {
                let p = palette(model);
                if color_sel() { p.text } else { p.muted }
            })
            .hover(move |s| s.text_color(palette(model).text))
            .focus(move |s| s.text_color(palette(model).text))
            .child(text(move || label()));
        self.items.push(tab.into_element());
        self
    }
}

impl IntoElement for Tabs {
    fn into_element(self) -> AnyElement {
        let model = self.model;
        base_tabs()
            .flex_row()
            .flex_wrap()
            .gap_1()
            // One selection background, sliding to the chosen tab natively.
            .indicator_style(move |s| {
                s.top_0p5()
                    .bottom_0p5()
                    .rounded(px(scale::radius::CONTROL))
                    .bg(move || palette(model).inset)
            })
            .children(self.items)
            .into_element()
    }
}
