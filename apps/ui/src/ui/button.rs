//! The app's `Button`: one press feedback, one focus ring, one disabled look
//! everywhere. Built on the semantic `nsh/base` button (role, keyboard,
//! disabled and selected states), styled entirely from tokens. It takes a
//! variant, a size, states and slots — never colors or radii.
use super::prelude::*;
use crate::ui::tokens::wash;
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
    /// A quiet, familiar action (a sidebar destination, a toolbar command): no
    /// fill or boundary at rest, a wash on hover, a fill when selected.
    Ghost,
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
    selected: StateFn,
    quiet_selection: StateFn,
    full_width: bool,
    icon_only: bool,
    leading: Option<AnyElement>,
    trailing: Option<AnyElement>,
    spoken: Option<LabelFn>,
    // Explicit native-acceptance observer only; never a control or authority.
    #[cfg(not(target_arch = "wasm32"))]
    probe: Option<(std::path::PathBuf, String)>,
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
        selected: Rc::new(|| false),
        quiet_selection: Rc::new(|| false),
        full_width: false,
        icon_only: false,
        leading: None,
        trailing: None,
        spoken: None,
        #[cfg(not(target_arch = "wasm32"))]
        probe: None,
    }
}

impl Button {
    /// Its meaning (default, primary, danger, ghost).
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
    /// A persistent selected state (the current destination, an open menu's
    /// trigger): a fill distinct from hover, announced to screen readers.
    pub fn selected(mut self, selected: impl Fn() -> bool + 'static) -> Self {
        self.selected = Rc::new(selected);
        self
    }
    /// While this holds, the selected state keeps its stronger label and its
    /// announcement but draws no fill of its own: a shared indicator behind a
    /// group of rows (the sidebar's sliding selection) draws it instead.
    pub fn quiet_selection(mut self, quiet: impl Fn() -> bool + 'static) -> Self {
        self.quiet_selection = Rc::new(quiet);
        self
    }
    /// What screen readers say when the visible label alone is ambiguous,
    /// e.g. "Remove alpha" for one of several "Remove…" rows.
    pub fn spoken(mut self, spoken: impl Fn() -> SharedString + 'static) -> Self {
        self.spoken = Some(Rc::new(spoken));
        self
    }
    /// Fill the caller's width with the content at the leading edge (rows of
    /// destinations or choices).
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }
    /// A leading element in the icon lane instead of an icon (a status dot).
    pub fn leading(mut self, leading: impl IntoElement) -> Self {
        self.leading = Some(leading.into_element());
        self
    }
    /// A trailing lane after the label (a done check, a count). The lane
    /// keeps its place so rows stay aligned.
    pub fn trailing(mut self, trailing: impl IntoElement) -> Self {
        self.trailing = Some(trailing.into_element());
        self
    }
    /// What activation does.
    pub fn on_press(mut self, on_press: impl Fn() + 'static) -> Self {
        self.on_press = Some(Rc::new(on_press));
        self
    }
    /// Observe this existing focus stop in an explicitly owned acceptance run.
    /// The caller supplies a closed catalog key, never a label or user content.
    pub fn probe_key(self, key: impl FnOnce() -> String) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut button = self;
            static SINK: std::sync::LazyLock<Option<std::path::PathBuf>> =
                std::sync::LazyLock::new(|| {
                    std::env::var_os("ACP_NATIVE_FOCUS_FILE")
                        .map(std::path::PathBuf::from)
                        .filter(|path| safe_probe_file(path, 0))
                });
            if let Some(path) = SINK.as_ref() {
                button.probe = Some((path.clone(), key()));
            }
            button
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = key;
            self
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn safe_probe_file(path: &std::path::Path, additional: usize) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::MetadataExt;
            if m.file_attributes() & 0x400 != 0 {
                return false;
            }
        }
        m.is_file() && !m.file_type().is_symlink() && m.len() + additional as u64 <= 256 * 1024
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn emit_focus_probe(probe: &(std::path::PathBuf, String)) {
    use std::io::Write;
    let (path, key) = probe;
    if safe_probe_file(path, key.len() + 1)
        && let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(path)
    {
        let _ = writeln!(file, "{key}");
        let _ = file.flush();
    }
}

/// A square icon-only button of the control height: a ghost by default, with
/// its accessible name (and the name a tooltip would show) in `label`.
pub fn icon_button(
    model: Signal<Model>,
    path: &'static str,
    label: impl Fn() -> SharedString + 'static,
) -> Button {
    let mut b = button(model, "")
        .label_fn(label)
        .variant(ButtonVariant::Ghost);
    b.icon = Some(path);
    b.icon_only = true;
    b
}

fn rest_bg(model: Signal<Model>, v: ButtonVariant, selected: bool) -> AdaptiveColor {
    let p = palette(model);
    match v {
        ButtonVariant::Default if selected => mix(p.inset, p.text, wash::PRESS),
        ButtonVariant::Default => p.inset,
        ButtonVariant::Primary => p.primary,
        ButtonVariant::Danger => p.danger,
        ButtonVariant::Ghost if selected => p.inset,
        ButtonVariant::Ghost => p.inset.opacity(0.0),
    }
}
fn rest_text(model: Signal<Model>, v: ButtonVariant, selected: bool) -> AdaptiveColor {
    let p = palette(model);
    match v {
        ButtonVariant::Default => p.text,
        ButtonVariant::Primary => p.on_primary,
        ButtonVariant::Danger => p.on_danger,
        ButtonVariant::Ghost if selected => p.text,
        ButtonVariant::Ghost => p.muted,
    }
}
fn rest_border(model: Signal<Model>, v: ButtonVariant) -> AdaptiveColor {
    let p = palette(model);
    match v {
        ButtonVariant::Default => p.hairline,
        ButtonVariant::Primary => p.primary,
        ButtonVariant::Danger => p.danger,
        ButtonVariant::Ghost => p.inset.opacity(0.0),
    }
}
fn hover_bg(model: Signal<Model>, v: ButtonVariant) -> AdaptiveColor {
    let p = palette(model);
    match v {
        ButtonVariant::Default => mix(p.inset, p.text, wash::HOVER),
        ButtonVariant::Primary => p.primary.opacity(0.88),
        ButtonVariant::Danger => p.danger.opacity(0.88),
        ButtonVariant::Ghost => mix(p.inset, p.text, wash::HOVER).opacity(0.7),
    }
}
fn press_bg(model: Signal<Model>, v: ButtonVariant) -> AdaptiveColor {
    let p = palette(model);
    match v {
        ButtonVariant::Default => mix(p.inset, p.text, wash::PRESS),
        ButtonVariant::Primary => p.primary.opacity(0.78),
        ButtonVariant::Danger => p.danger.opacity(0.78),
        ButtonVariant::Ghost => mix(p.inset, p.text, wash::PRESS),
    }
}
fn focus_border(model: Signal<Model>, v: ButtonVariant) -> AdaptiveColor {
    let p = palette(model);
    match v {
        ButtonVariant::Default | ButtonVariant::Ghost => p.primary,
        ButtonVariant::Primary | ButtonVariant::Danger => p.text,
    }
}

impl IntoElement for Button {
    fn into_element(self) -> AnyElement {
        let model = self.model;
        let variant = self.variant;
        let size = self.size;
        let label = self.spoken.clone().unwrap_or_else(|| self.label.clone());
        let label_child = self.label.clone();
        let disabled = self.disabled.clone();
        let loading = self.loading.clone();
        let loading_disabled = loading.clone();
        let press = self.on_press.clone();
        let icon_path = self.icon;
        let selected = self.selected.clone();
        let (selected_bg, selected_text, selected_a11y) =
            (selected.clone(), selected.clone(), selected.clone());
        let quiet = self.quiet_selection.clone();
        let full_width = self.full_width;
        let icon_only = self.icon_only;
        let trailing = self.trailing;
        let leading = self.leading;
        let (text_size, icon_size, radius) = match size {
            ButtonSize::Small => (scale::text::CAPTION, scale::icon::SM, scale::radius::SMALL),
            ButtonSize::Medium => (scale::text::BODY, scale::icon::MD, scale::radius::CONTROL),
            ButtonSize::Large => (
                scale::text::SUBTITLE,
                scale::icon::MD,
                scale::radius::CONTROL,
            ),
        };
        #[cfg(target_os = "windows")]
        let keyboard_ref = {
            // Win32's custom-view focus path does not activate press targets.
            // Use the same focused-key lifecycle as nsh-base's other controls;
            // no app-wide binding may take Enter/Space from an input or sibling.
            let reference = element_ref();
            let focused = signal(false);
            let probe = self.probe;
            reference.on_focus_in(move || {
                focused.set(true);
                if let Some(probe) = &probe {
                    emit_focus_probe(probe);
                }
            });
            reference.on_focus_out(move || focused.set(false));
            let disabled = self.disabled.clone();
            let loading = self.loading.clone();
            let press = self.on_press.clone();
            effect(move || {
                if !focused.get() || disabled() || loading() {
                    return;
                }
                for key in ["enter", "space"] {
                    let press = press.clone();
                    on_key_with(
                        key,
                        KeyOptions {
                            when: nsh_core::keymap::When::NotEditing,
                            for_focus: true,
                            ..Default::default()
                        },
                        move |_| {
                            if let Some(cb) = &press {
                                cb();
                            }
                        },
                    );
                }
            });
            reference
        };
        #[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
        let probe_ref = self.probe.map(|probe| {
            let reference = element_ref();
            reference.on_focus_in(move || emit_focus_probe(&probe));
            reference
        });
        let b = base_button()
            .accessibility_label(move || Some(label()))
            .disabled(move || disabled() || loading_disabled())
            .disabled_style(|s| s.opacity(0.45))
            .border_1()
            .rounded(px(radius))
            .text_size(px(text_size))
            .selected(move || selected_a11y())
            .bg(move || {
                if quiet() {
                    rest_bg(model, variant, false)
                } else {
                    rest_bg(model, variant, selected_bg())
                }
            })
            .text_color(move || rest_text(model, variant, selected_text()))
            .border_color(move || rest_border(model, variant))
            .hover(move |s| s.bg(move || hover_bg(model, variant)))
            .active(move |s| {
                let (press, icon_press) = (motion::press(), motion::icon_press());
                s.scale(move || if icon_only { icon_press() } else { press() })
                    .bg(move || press_bg(model, variant))
            })
            // The ring sits outside the edge, so fills stay intact; the edge
            // also takes the focus color where a ring would be clipped.
            .focus(move |s| {
                s.border_color(move || focus_border(model, variant))
                    .ring(px(2.0))
                    .ring_color(move || palette(model).primary.opacity(0.55))
            })
            .on_press(move || {
                if let Some(cb) = &press {
                    cb();
                }
            });
        #[cfg(target_os = "windows")]
        let b = b.element_ref(&keyboard_ref);
        #[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
        let b = if let Some(reference) = probe_ref {
            b.element_ref(&reference)
        } else {
            b
        };
        let b = motion::feedback(b);
        let height = match size {
            ButtonSize::Small => scale::control::SM,
            ButtonSize::Medium => scale::control::MD,
            ButtonSize::Large => scale::control::LG,
        };
        // The label is centered in the frame whatever the height.
        let b = b.justify_center();
        let b = if icon_only {
            b.size(px(height)).items_center()
        } else {
            match size {
                ButtonSize::Small => b.px_2().py_0p5().min_h(px(height)),
                ButtonSize::Medium if compact() => b.px_2().py_0p5().min_h(px(height)),
                ButtonSize::Medium => b.px_3().py_1().min_h(px(height)),
                ButtonSize::Large => b.px_4().py_1p5().min_h(px(height)),
            }
        };
        let b = if full_width {
            b.style_when(move || !compact(), |s| s.w_full())
        } else {
            b
        };
        let mut content = view().flex_row().items_center();
        content = match size {
            ButtonSize::Small => content.gap_1(),
            _ => content.gap_2(),
        };
        if let Some(path) = icon_path {
            content = content.child(icon::glyph(path, icon_size));
        } else if let Some(leading) = leading {
            content = content.child(
                view()
                    .size(px(icon_size))
                    .items_center()
                    .justify_center()
                    .child(leading),
            );
        }
        let loading_spinner = loading.clone();
        content = content.child(
            view()
                .style_when(move || !loading_spinner(), |s| s.hidden())
                .child(icon::spinner(icon_size)),
        );
        if icon_only {
            // The name is spoken; in a terminal, where icons are hidden, it is shown.
            content = content
                .child(text(move || label_child()).style_when(move || !terminal(), |s| s.hidden()));
            return b.child(content).into_element();
        }
        let label_view = text(move || label_child());
        content = content.child(if full_width {
            label_view.flex_1().min_w_0()
        } else {
            label_view
        });
        if let Some(trailing) = trailing {
            content = content.child(
                view()
                    .min_w(px(scale::icon::MD))
                    .items_center()
                    .justify_center()
                    .child(trailing),
            );
        }
        b.child(content).into_element()
    }
}

/// How long a copy button shows its result before it can copy again.
const COPIED_FOR: std::time::Duration = std::time::Duration::from_millis(1500);

/// A copy command whose result shows on the button itself: `copy` returns
/// whether it copied; then the copy glyph swaps to a check (it pops in) and
/// the label to `copied` for a moment, so no notification is needed.
pub fn copy_button(
    model: Signal<Model>,
    label: impl Fn() -> SharedString + 'static,
    copied: impl Fn() -> SharedString + 'static,
    copy: impl Fn() -> bool + 'static,
) -> Button {
    let done = signal(false);
    let glyph = dynamic(move || {
        if done.get() {
            let pop = motion::pop();
            view()
                .text_color(move || palette(model).success)
                .bind_motion(&pop)
                .child(icon::glyph(super::icons::CHECK, scale::icon::SM))
                .into_element()
        } else {
            icon::glyph(super::icons::COPY, scale::icon::SM).into_element()
        }
    });
    button(model, "")
        .label_fn(move || if done.get() { copied() } else { label() })
        .leading(glyph)
        .on_press(move || {
            if copy() {
                done.set(true);
                set_timeout(COPIED_FOR, move || done.set(false));
            }
        })
}
