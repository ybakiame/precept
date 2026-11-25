//! Guided next actions: a reusable recommendation card and the launch guide's
//! numbered step row. Both compose the app's semantic button and surface styles.
use super::prelude::*;
use super::{ButtonVariant, button};
use std::rc::Rc;

type StateFn = Rc<dyn Fn() -> bool>;
type PressFn = Rc<dyn Fn()>;

/// A calm recommendation with one primary action and an optional secondary
/// route. The caller owns the decision and rebuilds it when that decision changes.
pub struct NextAction {
    model: Signal<Model>,
    icon: &'static str,
    title: SharedString,
    description: SharedString,
    action_label: SharedString,
    on_action: PressFn,
    secondary: Option<(SharedString, PressFn)>,
}

/// A recommendation card with a named next action.
pub fn next_action(
    model: Signal<Model>,
    icon: &'static str,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    action_label: impl Into<SharedString>,
    on_action: impl Fn() + 'static,
) -> NextAction {
    NextAction {
        model,
        icon,
        title: title.into(),
        description: description.into(),
        action_label: action_label.into(),
        on_action: Rc::new(on_action),
        secondary: None,
    }
}

impl NextAction {
    /// A quieter alternative, such as reopening the full guide.
    pub fn secondary(
        mut self,
        label: impl Into<SharedString>,
        on_press: impl Fn() + 'static,
    ) -> Self {
        self.secondary = Some((label.into(), Rc::new(on_press)));
        self
    }
}

impl IntoElement for NextAction {
    fn into_element(self) -> AnyElement {
        let model = self.model;
        let title = self.title;
        let description = self.description;
        let action_label = self.action_label;
        let on_action = self.on_action;
        let secondary = self.secondary;
        let reveal = motion::reveal();
        let mut actions = view().flex_row().flex_wrap().items_center().gap_2().child(
            button(model, "")
                .test_id("next-step.primary")
                .label_fn(move || action_label.clone())
                .variant(ButtonVariant::Primary)
                .on_press(move || on_action()),
        );
        if let Some((label, on_press)) = secondary {
            actions = actions.child(
                button(model, "")
                    .test_id("next-step.secondary")
                    .label_fn(move || label.clone())
                    .variant(ButtonVariant::Ghost)
                    .on_press(move || on_press()),
            );
        }
        view()
            .test_id("next-step-card")
            .flex_shrink_0()
            .p_4()
            .gap_4()
            .rounded(px(scale::radius::CONTAINER))
            .border_1()
            .border_color(move || palette(model).hairline)
            .bg(move || palette(model).panel)
            .bind_motion(&reveal)
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .items_start()
                    .gap_4()
                    .child(
                        view()
                            .size(px(scale::disc::LG))
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .rounded(px(scale::radius::PILL))
                            .bg(move || palette(model).primary.opacity(super::tokens::tint::REST))
                            .text_color(move || palette(model).primary)
                            .child(icon::glyph(self.icon, scale::icon::XL)),
                    )
                    .child(
                        view()
                            .flex_1()
                            .min_w(px(240.0))
                            .gap_1()
                            .child(
                                text(title)
                                    .role(AccessibilityRole::Header)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(scale::text::SUBTITLE)),
                            )
                            .child(
                                text(description)
                                    .text_size(px(scale::text::BODY))
                                    .text_color(move || palette(model).muted),
                            )
                            .child(view().mt_1().child(actions)),
                    ),
            )
            .into_element()
    }
}

/// One numbered launch-guide step builder. The shared rail indicator may draw
/// selection behind it; otherwise the row draws its own selected fill.
pub struct GuideStep {
    model: Signal<Model>,
    number: u8,
    label: Rc<dyn Fn() -> SharedString>,
    description: Rc<dyn Fn() -> SharedString>,
    selected: StateFn,
    done: StateFn,
    disabled: StateFn,
    quiet_selection: StateFn,
    on_press: PressFn,
}

/// A numbered guide step with its visible label and explanation.
pub fn guide_step(
    model: Signal<Model>,
    number: u8,
    label: impl Fn() -> SharedString + 'static,
    description: impl Fn() -> SharedString + 'static,
) -> GuideStep {
    GuideStep {
        model,
        number,
        label: Rc::new(label),
        description: Rc::new(description),
        selected: Rc::new(|| false),
        done: Rc::new(|| false),
        disabled: Rc::new(|| false),
        quiet_selection: Rc::new(|| false),
        on_press: Rc::new(|| {}),
    }
}

impl GuideStep {
    /// Whether this is the current step.
    pub fn selected(mut self, selected: impl Fn() -> bool + 'static) -> Self {
        self.selected = Rc::new(selected);
        self
    }
    /// Whether this step's required result has been observed.
    pub fn done(mut self, done: impl Fn() -> bool + 'static) -> Self {
        self.done = Rc::new(done);
        self
    }
    /// Whether prerequisites prevent entering this step.
    pub fn disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
        self.disabled = Rc::new(disabled);
        self
    }
    /// Whether a shared indicator draws the selected fill instead of this row.
    pub fn quiet_selection(mut self, quiet: impl Fn() -> bool + 'static) -> Self {
        self.quiet_selection = Rc::new(quiet);
        self
    }
    /// Enters this step.
    pub fn on_press(mut self, on_press: impl Fn() + 'static) -> Self {
        self.on_press = Rc::new(on_press);
        self
    }
}

impl IntoElement for GuideStep {
    fn into_element(self) -> AnyElement {
        let model = self.model;
        let number = self.number;
        let selected_fn = self.selected.clone();
        let done_fn = self.done.clone();
        let disabled_fn = self.disabled.clone();
        let quiet_fn = self.quiet_selection.clone();
        let selected = computed(move || selected_fn());
        let done = computed(move || done_fn());
        let disabled = computed(move || disabled_fn());
        let quiet = computed(move || quiet_fn());
        let accessibility_label = self.label.clone();
        let visible_label = self.label;
        let visible_description = self.description;
        let on_press = self.on_press;
        base_button()
            .accessibility_label(move || Some(accessibility_label()))
            .selected(move || selected.get())
            .disabled(move || disabled.get())
            .disabled_style(|s| s.opacity(0.45))
            .w_full()
            .min_h(px(64.0))
            .px_3()
            .py_2()
            .gap_3()
            .flex_row()
            .items_start()
            .rounded(px(scale::radius::CONTROL))
            .border_1()
            .border_color(move || palette(model).inset.opacity(0.0))
            .bg(move || {
                if selected.get() && !quiet.get() {
                    palette(model).inset
                } else {
                    palette(model).inset.opacity(0.0)
                }
            })
            .hover(move |s| {
                s.bg(move || {
                    super::tokens::mix(
                        palette(model).inset,
                        palette(model).text,
                        super::tokens::wash::HOVER,
                    )
                    .opacity(0.7)
                })
            })
            .active(move |s| s.bg(move || palette(model).inset))
            .focus(move |s| {
                s.border_color(move || palette(model).primary)
                    .ring(px(2.0))
                    .ring_color(move || palette(model).primary.opacity(0.55))
            })
            .on_press(move || on_press())
            .child(
                view()
                    .size(px(scale::disc::MD))
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .rounded(px(scale::radius::PILL))
                    .border_1()
                    .border_color(move || {
                        if done.get() {
                            palette(model).success
                        } else {
                            palette(model).hairline
                        }
                    })
                    .bg(move || {
                        if done.get() {
                            palette(model).success.opacity(super::tokens::tint::REST)
                        } else {
                            palette(model).panel
                        }
                    })
                    .text_color(move || {
                        if done.get() {
                            palette(model).success
                        } else {
                            palette(model).muted
                        }
                    })
                    .child(dynamic(move || {
                        if done.get() {
                            let pop = motion::pop();
                            view()
                                .bind_motion(&pop)
                                .child(icon::glyph(super::icons::CHECK, scale::icon::SM))
                                .into_element()
                        } else {
                            text(number.to_string())
                                .tabular_nums()
                                .text_size(px(scale::text::CAPTION))
                                .font_weight(FontWeight::SEMIBOLD)
                                .into_element()
                        }
                    })),
            )
            .child(
                view()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        text(move || visible_label())
                            .font_weight(FontWeight::MEDIUM)
                            .text_size(px(scale::text::BODY))
                            .text_color(move || palette(model).text),
                    )
                    .child(
                        text(move || visible_description())
                            .text_size(px(scale::text::CAPTION))
                            .text_color(move || palette(model).muted),
                    ),
            )
            .into_element()
    }
}
