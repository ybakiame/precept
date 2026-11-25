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
    on_focus: Option<Rc<dyn Fn()>>,
    placeholder: Option<SharedString>,
    disabled: StateFn,
    invalid: StateFn,
    reference: Option<ElementRef>,
    large: bool,
    password: StateFn,
    max_length: Option<u32>,
    lines: Option<u8>,
    test_id: Option<&'static str>,
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
        on_focus: None,
        placeholder: None,
        disabled: Rc::new(|| false),
        invalid: Rc::new(|| false),
        reference: None,
        large: false,
        password: Rc::new(|| false),
        max_length: None,
        lines: None,
        test_id: None,
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
    /// Called when the field takes focus.
    pub fn on_focus(mut self, on_focus: impl Fn() + 'static) -> Self {
        self.on_focus = Some(Rc::new(on_focus));
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
    /// A stable test id (selectors use `id:...`).
    pub fn test_id(mut self, id: &'static str) -> Self {
        self.test_id = Some(id);
        self
    }
    /// Several lines of text (a long value); it grows to `lines` rows.
    pub fn multiline(mut self, lines: u8) -> Self {
        self.lines = Some(lines.max(2));
        self
    }
    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }
    /// A secure native input; accessibility does not expose the entered secret.
    pub fn password(mut self) -> Self {
        self.password = Rc::new(|| true);
        self
    }
    /// Mask the value until an explicit reveal action changes this state.
    pub fn masked(mut self, masked: impl Fn() -> bool + 'static) -> Self {
        self.password = Rc::new(masked);
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
        let height = if let Some(lines) = self.lines {
            scale::control::MD + f32::from(lines - 1) * scale::text::BODY * 1.5
        } else if self.large {
            scale::control::LG
        } else {
            scale::control::MD
        };
        let mut field = base_input()
            .label(move || label())
            .value(move || value())
            .multiline(self.lines.is_some())
            .masked(move || (self.password)())
            .h(px(height))
            .w_full()
            .px_3()
            .border_1()
            .rounded(px(scale::radius::CONTROL))
            .bg(move || palette(model).panel)
            .text_color(move || palette(model).text)
            .text_size(px(scale::text::BODY))
            .border_color(move || {
                let p = palette(model);
                if invalid() { p.danger } else { p.border }
            })
            .focus(move |s| {
                let invalid_ring = invalid_focus.clone();
                s.border_color(move || {
                    let p = palette(model);
                    if invalid_focus() { p.danger } else { p.primary }
                })
                .ring(px(3.0))
                .ring_color(move || {
                    let p = palette(model);
                    if invalid_ring() { p.danger } else { p.primary }.opacity(0.25)
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
        if let Some(focused) = self.on_focus {
            field = field.on_focus(move || focused());
        }
        if let Some(placeholder) = self.placeholder {
            field = field.placeholder(placeholder.to_string());
        }
        if let Some(reference) = self.reference {
            field = field.element_ref(&reference);
        }
        if let Some(id) = self.test_id {
            field = field.test_id(id);
        }
        if let Some(max_length) = self.max_length {
            field = field.max_length(max_length);
        }
        field.into_element()
    }
}
