//! The app's `Button`: one press feedback, one focus ring, one disabled look
//! everywhere. Built on the semantic `nsh/base` button (role, keyboard,
//! disabled and selected states), styled entirely from tokens. It takes a
//! variant, a size, states and slots — never colors or radii.
use super::prelude::*;
use std::rc::Rc;

/// What a button means in its decision area.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    /// An ordinary visible action: an inset fill with a hairline boundary.
    Default,
    /// The one emphasized commitment (the Return-key action), accent fill.
    Primary,
    /// A destructive commitment; pairs with a confirmation for serious work.
    Danger,
}

/// One size sets padding, text, icon and radius together.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonSize {
    Small,
    Medium,
    Large,
}

type StateFn = Rc<dyn Fn() -> bool>;
type PressFn = Rc<dyn Fn()>;
type LabelFn = Rc<dyn Fn() -> SharedString>;

/// A styled button builder. Create it with [`button`], set its meaning and
/// states, and pass it where an element is expected.
pub struct Button {
    model: Signal<Model>,
    label: LabelFn,
    variant: ButtonVariant,
    size: ButtonSize,
    disabled: StateFn,
    loading: StateFn,
    on_press: Option<PressFn>,
    icon: Option<&'static str>,
}

/// A default, medium button with a static label.
pub fn button(model: Signal<Model>, label: impl Into<SharedString>) -> Button {
    let label = label.into();
    Button {
        model,
        label: Rc::new(move || label.clone()),
        variant: ButtonVariant::Default,
        size: ButtonSize::Medium,
        disabled: Rc::new(|| false),
        loading: Rc::new(|| false),
        on_press: None,
        icon: None,
    }
}

impl Button {
    /// Its meaning (default, primary, danger).
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }
    /// Its size (small, medium, large).
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }
    /// No presses and a dimmed look while this holds.
    pub fn disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
        self.disabled = Rc::new(disabled);
        self
    }
    /// Work in flight: dims, disables to prevent a duplicate submit, and shows
    /// a spinner beside the label.
    pub fn loading(mut self, loading: impl Fn() -> bool + 'static) -> Self {
        self.loading = Rc::new(loading);
        self
    }
    /// A label that stays current (a localized word, a count).
    pub fn label_fn(mut self, label: impl Fn() -> SharedString + 'static) -> Self {
        self.label = Rc::new(label);
        self
    }
    /// A leading outline glyph from the app's icon family.
    pub fn icon(mut self, path: &'static str) -> Self {
        self.icon = Some(path);
        self
    }
    /// What activation does.
    pub fn on_press(mut self, on_press: impl Fn() + 'static) -> Self {
        self.on_press = Some(Rc::new(on_press));
        self
    }
}

fn rest_bg(model: Signal<Model>, v: ButtonVariant) -> AdaptiveColor {
    let p = palette(model);
    match v {
        ButtonVariant::Default => p.inset,
        ButtonVariant::Primary => p.primary,
        ButtonVariant::Danger => p.danger,
    }
}
fn rest_text(model: Signal<Model>, v: ButtonVariant) -> AdaptiveColor {
    let p = palette(model);
    match v {
        ButtonVariant::Default => p.text,
        ButtonVariant::Primary => p.on_primary,
        ButtonVariant::Danger => p.on_danger,
    }
}
fn rest_border(model: Signal<Model>, v: ButtonVariant) -> AdaptiveColor {
    let p = palette(model);
    match v {
        ButtonVariant::Default => p.border,
        ButtonVariant::Primary => p.primary,
        ButtonVariant::Danger => p.danger,
    }
}
fn hover_bg(model: Signal<Model>, v: ButtonVariant) -> AdaptiveColor {
    let p = palette(model);
    match v {
        ButtonVariant::Default => mix(p.inset, p.text, 0.07),
        ButtonVariant::Primary => p.primary.opacity(0.88),
        ButtonVariant::Danger => p.danger.opacity(0.88),
    }
}
fn press_bg(model: Signal<Model>, v: ButtonVariant) -> AdaptiveColor {
    let p = palette(model);
    match v {
        ButtonVariant::Default => mix(p.inset, p.text, 0.13),
        ButtonVariant::Primary => p.primary.opacity(0.78),
        ButtonVariant::Danger => p.danger.opacity(0.78),
    }
}
fn focus_border(model: Signal<Model>, v: ButtonVariant) -> AdaptiveColor {
    let p = palette(model);
    match v {
        ButtonVariant::Default => p.primary,
        ButtonVariant::Primary | ButtonVariant::Danger => p.text,
    }
}

impl IntoElement for Button {
    fn into_element(self) -> AnyElement {
        let model = self.model;
        let variant = self.variant;
        let size = self.size;
        let label = self.label.clone();
        let label_child = label.clone();
        let disabled = self.disabled.clone();
        let loading = self.loading.clone();
        let loading_disabled = loading.clone();
        let press = self.on_press.clone();
        let icon_path = self.icon;
        let (text_size, icon_size, radius) = match size {
            ButtonSize::Small => (scale::text::CAPTION, scale::icon::SM, scale::radius::SMALL),
            ButtonSize::Medium => (scale::text::BODY, scale::icon::MD, scale::radius::CONTROL),
            ButtonSize::Large => (
                scale::text::SUBTITLE,
                scale::icon::MD,
                scale::radius::CONTROL,
            ),
        };
        let b = base_button()
            .focusable(true)
            .accessibility_label(move || Some(label()))
            .disabled(move || disabled() || loading_disabled())
            .disabled_style(|s| s.opacity(0.45))
            .border_2()
            .rounded(px(radius))
            .text_size(px(text_size))
            .bg(move || rest_bg(model, variant))
            .text_color(move || rest_text(model, variant))
            .border_color(move || rest_border(model, variant))
            .hover(move |s| s.bg(move || hover_bg(model, variant)))
            .active(move |s| {
                s.scale(motion::press())
                    .bg(move || press_bg(model, variant))
            })
            .focus(move |s| s.border_color(move || focus_border(model, variant)))
            .on_press(move || {
                if let Some(cb) = &press {
                    cb();
                }
            });
        let b = motion::feedback(b);
        let b = match size {
            ButtonSize::Small => b.px_2().py_0p5().min_h(px(scale::control::SM)),
            ButtonSize::Medium if compact() => b.px_2().py_0p5().min_h(px(scale::control::MD)),
            ButtonSize::Medium => b.px_2().py_1().min_h(px(scale::control::MD)),
            ButtonSize::Large => b.px_3().py_1p5().min_h(px(scale::control::LG)),
        };
        let mut content = view().flex_row().items_center();
        content = match size {
            ButtonSize::Small => content.gap_1(),
            _ => content.gap_2(),
        };
        if let Some(path) = icon_path {
            content = content.child(icon::glyph(path, icon_size));
        }
        let loading_spinner = loading.clone();
        content = content.child(
            view()
                .style_when(move || !loading_spinner(), |s| s.hidden())
                .child(icon::spinner(icon_size)),
        );
        content = content.child(text(move || label_child()));
        b.child(content).into_element()
    }
}
