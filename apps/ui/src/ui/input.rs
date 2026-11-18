//! The app's `Input`: a single-line text field built on the semantic `nsh/base`
//! input (editing, keyboard, submit, accessible label), styled from tokens with
//! a validation state. It takes states and slots, never colors or radii; the
//! caller owns its width and flex.
use super::prelude::*;
use std::rc::Rc;

type StateFn = Rc<dyn Fn() -> bool>;
type LabelFn = Rc<dyn Fn() -> SharedString>;
type ValueFn = Rc<dyn Fn() -> String>;
type ChangeFn = Rc<dyn Fn(&str)>;

/// A styled text field builder; create it with [`input`].
pub struct Input {
    model: Signal<Model>,
    label: LabelFn,
    value: ValueFn,
    on_change: Option<ChangeFn>,
    on_submit: Option<ChangeFn>,
    placeholder: Option<SharedString>,
    disabled: StateFn,
    invalid: StateFn,
    reference: Option<ElementRef>,
    large: bool,
    max_length: Option<u32>,
}

/// A medium, valid field with an accessible label.
pub fn input(model: Signal<Model>, label: impl Into<SharedString>) -> Input {
    let label = label.into();
    Input {
        model,
        label: Rc::new(move || label.clone()),
        value: Rc::new(String::new),
        on_change: None,
        on_submit: None,
        placeholder: None,
        disabled: Rc::new(|| false),
        invalid: Rc::new(|| false),
        reference: None,
        large: false,
        max_length: None,
    }
}

impl Input {
    /// A label that stays current (a localized word).
    pub fn label_fn(mut self, label: impl Fn() -> SharedString + 'static) -> Self {
        self.label = Rc::new(label);
        self
    }
    /// The current text (read from its owner each time it changes).
    pub fn value(mut self, value: impl Fn() -> String + 'static) -> Self {
        self.value = Rc::new(value);
        self
    }
    /// Called with the new text on every edit.
    pub fn on_change(mut self, on_change: impl Fn(&str) + 'static) -> Self {
        self.on_change = Some(Rc::new(on_change));
        self
    }
    /// Called with the text on Return / submit.
    pub fn on_submit(mut self, on_submit: impl Fn(&str) + 'static) -> Self {
        self.on_submit = Some(Rc::new(on_submit));
        self
    }
    /// Hint text shown while empty.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }
    /// No editing while this holds.
    pub fn disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
        self.disabled = Rc::new(disabled);
        self
    }
    /// Validation failed: the boundary takes the danger role and an error
    /// message belongs beside the field (the caller renders it).
    pub fn invalid(mut self, invalid: impl Fn() -> bool + 'static) -> Self {
        self.invalid = Rc::new(invalid);
        self
    }
    /// A handle for focusing / blurring the field from a handler.
    pub fn element_ref(mut self, reference: &ElementRef) -> Self {
        self.reference = Some(*reference);
        self
    }
    /// A large frame (prominent decisions); medium is the default.
    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }
    /// The maximum number of characters accepted.
    pub fn max_length(mut self, max_length: u32) -> Self {
        self.max_length = Some(max_length);
        self
    }
}

impl IntoElement for Input {
    fn into_element(self) -> AnyElement {
        let model = self.model;
        let label = self.label.clone();
        let value = self.value.clone();
        let change = self.on_change.clone();
        let submit = self.on_submit.clone();
        let disabled = self.disabled.clone();
        let invalid = self.invalid.clone();
        let invalid_focus = invalid.clone();
        let height = if self.large {
            scale::control::LG
        } else {
            scale::control::MD
        };
        let mut field = base_input()
            .label(move || label())
            .value(move || value())
            .h(px(height))
            .w_full()
            .px_2()
            .border_2()
            .rounded(px(scale::radius::CONTROL))
            .bg(move || palette(model).panel)
            .text_color(move || palette(model).text)
            .text_size(px(scale::text::BODY))
            .border_color(move || {
                let p = palette(model);
                if invalid() { p.danger } else { p.border }
            })
            .focus(move |s| {
                s.border_color(move || {
                    let p = palette(model);
                    if invalid_focus() { p.danger } else { p.primary }
                })
            })
            .disabled(move || disabled())
            .disabled_style(|s| s.opacity(0.45))
            .transition_colors()
            .duration(motion::ms(motion::INSTANT))
            .ease_out()
            .on_change(move |v: &str| {
                if let Some(cb) = &change {
                    cb(v);
                }
            });
        if let Some(submit) = submit {
            field = field.on_submit(move |v: &str| submit(v));
        }
        if let Some(placeholder) = self.placeholder {
            field = field.placeholder(placeholder.to_string());
        }
        if let Some(reference) = self.reference {
            field = field.element_ref(&reference);
        }
        if let Some(max_length) = self.max_length {
            field = field.max_length(max_length);
        }
        field.into_element()
    }
}
