//! The app's `AlertDialog`: explicit confirmation of a consequential,
//! irreversible action. Built on the semantic `nsh/base` dialog (a native modal
//! layer that traps focus and reports Escape / back as a cancel), styled from
//! tokens: a fading scrim and a panel that grows into place, a default Cancel
//! and a danger commitment named for its result. The caller owns whether it is
//! open and removes it on cancel or confirm.
use super::prelude::*;
use std::rc::Rc;

type LabelFn = Rc<dyn Fn() -> SharedString>;
type StateFn = Rc<dyn Fn() -> bool>;
type PressFn = Rc<dyn Fn()>;
type ContentFn = Rc<dyn Fn() -> AnyElement>;

/// A dialog's width, chosen by its decision (DESIGN.md §8).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DialogSize {
    /// 400: a confirmation.
    Small,
    /// 520: a short form.
    Medium,
    /// 640: a form with a rich value.
    Large,
}
impl DialogSize {
    fn width(self) -> f32 {
        match self {
            Self::Small => scale::layout::DIALOG_SM,
            Self::Medium => scale::layout::DIALOG_MD,
            Self::Large => scale::layout::DIALOG_LG,
        }
    }
}

/// The tone of a dialog's header icon.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DialogTone {
    Accent,
    Danger,
}

/// A destructive-confirmation builder; create it with [`alert_dialog`].
pub struct AlertDialog {
    model: Signal<Model>,
    title: LabelFn,
    description: Option<LabelFn>,
    content: Option<ContentFn>,
    cancel: LabelFn,
    confirm: LabelFn,
    confirm_disabled: StateFn,
    confirm_variant: super::button::ButtonVariant,
    size: DialogSize,
    icon: Option<(&'static str, DialogTone)>,
    open: StateFn,
    initial_focus: Option<ElementRef>,
    on_cancel: Option<PressFn>,
    on_confirm: Option<PressFn>,
}

/// A confirmation titled for its decision (e.g. `Delete “Roadmap”?`).
/// A focused decision or short form: the same modal, committing with a
/// primary (non-destructive) button.
pub fn dialog(model: Signal<Model>, title: impl Into<SharedString>) -> AlertDialog {
    alert_dialog(model, title)
        .primary()
        .size(DialogSize::Medium)
}

pub fn alert_dialog(model: Signal<Model>, title: impl Into<SharedString>) -> AlertDialog {
    let title = title.into();
    AlertDialog {
        model,
        title: Rc::new(move || title.clone()),
        description: None,
        content: None,
        cancel: Rc::new(|| "Cancel".into()),
        confirm: Rc::new(|| "Confirm".into()),
        confirm_disabled: Rc::new(|| false),
        confirm_variant: super::button::ButtonVariant::Danger,
        size: DialogSize::Small,
        icon: None,
        open: Rc::new(|| false),
        initial_focus: None,
        on_cancel: None,
        on_confirm: None,
    }
}

impl AlertDialog {
    /// Its width (Small for confirmations, the default of `alert_dialog`;
    /// Medium for `dialog`).
    pub fn size(mut self, size: DialogSize) -> Self {
        self.size = size;
        self
    }
    /// A header icon in a tinted disc beside the title.
    pub fn icon(mut self, path: &'static str, tone: DialogTone) -> Self {
        self.icon = Some((path, tone));
        self
    }
    /// Only new scope, consequence or recovery information; never a restatement
    /// of the title.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        let description = description.into();
        self.description = Some(Rc::new(move || description.clone()));
        self
    }
    /// The action that leaves without committing (default `Cancel`).
    pub fn cancel(mut self, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        self.cancel = Rc::new(move || label.clone());
        self
    }
    /// A live content slot between the description and the actions (e.g. a
    /// retype field). Rebuilt each time the dialog opens; its bindings read
    /// their owner so the value stays current.
    pub fn content_fn(mut self, content: impl Fn() -> AnyElement + 'static) -> Self {
        self.content = Some(Rc::new(content));
        self
    }
    /// The destructive commitment cannot fire while this holds (e.g. until the
    /// exact ID is retyped). A disabled commitment refuses blind activation.
    /// A non-destructive commitment (a `Dialog`'s Apply): the primary button.
    pub fn primary(mut self) -> Self {
        self.confirm_variant = super::button::ButtonVariant::Primary;
        self
    }
    pub fn confirm_disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
        self.confirm_disabled = Rc::new(disabled);
        self
    }
    /// The destructive commitment, named for its result (e.g. `Delete`).
    pub fn confirm(mut self, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        self.confirm = Rc::new(move || label.clone());
        self
    }
    /// A title that stays current (a localized decision).
    pub fn title_fn(mut self, title: impl Fn() -> SharedString + 'static) -> Self {
        self.title = Rc::new(title);
        self
    }
    /// A description that stays current (a localized consequence).
    pub fn description_fn(mut self, description: impl Fn() -> SharedString + 'static) -> Self {
        self.description = Some(Rc::new(description));
        self
    }
    /// A cancel label that stays current (a localized word).
    pub fn cancel_fn(mut self, label: impl Fn() -> SharedString + 'static) -> Self {
        self.cancel = Rc::new(label);
        self
    }
    /// A commitment label that stays current (a localized result verb).
    pub fn confirm_fn(mut self, label: impl Fn() -> SharedString + 'static) -> Self {
        self.confirm = Rc::new(label);
        self
    }
    /// Whether the dialog is presented; the caller owns this state.
    pub fn open(mut self, open: impl Fn() -> bool + 'static) -> Self {
        self.open = Rc::new(open);
        self
    }
    /// Focuses this field after the native modal has appeared and established
    /// its focus scope, never while the field is still being mounted.
    pub fn initial_focus(mut self, reference: &ElementRef) -> Self {
        self.initial_focus = Some(*reference);
        self
    }
    /// Called when the user dismisses without committing (Escape, back, Cancel).
    pub fn on_cancel(mut self, on_cancel: impl Fn() + 'static) -> Self {
        self.on_cancel = Some(Rc::new(on_cancel));
        self
    }
    /// Called when the user commits the destructive action.
    pub fn on_confirm(mut self, on_confirm: impl Fn() + 'static) -> Self {
        self.on_confirm = Some(Rc::new(on_confirm));
        self
    }
}

impl IntoElement for AlertDialog {
    fn into_element(self) -> AnyElement {
        let model = self.model;
        let title = self.title.clone();
        let description = self.description.clone();
        let content = self.content.clone();
        let cancel = self.cancel.clone();
        let confirm = self.confirm.clone();
        let confirm_disabled = self.confirm_disabled.clone();
        let open = self.open.clone();
        let initial_focus = self.initial_focus;
        let confirm_variant = self.confirm_variant;
        let width = self.size.width();
        let header_icon = self.icon;
        let is_open = computed(move || open());
        let on_cancel = self.on_cancel.clone();
        let on_confirm = self.on_confirm.clone();
        // Present while open; leaving it out of the view dismisses it. The
        // motion is created fresh each time it opens, so the grow plays once.
        when(is_open, move || {
            // Content bindings own their updates. Reading a model while making
            // the content must not replace the native modal on every keystroke.
            untracked(|| {
                let scrim = motion::scrim();
                let grow = motion::grow();
                let cancel_cb = on_cancel.clone();
                let confirm_cb = on_confirm.clone();
                let modal_cancel = on_cancel.clone();
                let title_text = title.clone();
                let cancel_text = cancel.clone();
                let confirm_text = confirm.clone();
                let confirm_disabled = confirm_disabled.clone();
                let mut panel = view()
                    .test_id("dialog-panel")
                    .w_full()
                    .max_w(px(width))
                    // Never taller than the window: 48 points of the page show
                    // above and below, and the content scrolls between the
                    // fixed header and actions.
                    .max_h(move || px((window().height.0 - 48.0).max(240.0)))
                    .p_6()
                    .gap_4()
                    .rounded(px(scale::radius::OVERLAY))
                    .border_1()
                    .border_color(move || palette(model).hairline)
                    .bg(move || palette(model).raised)
                    .text_color(move || palette(model).text)
                    .shadow_xl()
                    .on_layout(move |_bounds: &Bounds<Points>| {
                        if std::env::var_os("PRECEPT_UI_PROBE").is_some() {
                            eprintln!(
                                "PRECEPT_UI_DIALOG panel_width={} panel_height={}",
                                _bounds.size.width.0, _bounds.size.height.0
                            );
                        }
                    });
                // Header: an optional tone disc, then the title over its
                // description. The disc keeps a fixed lane so text aligns.
                let mut heading = view().flex_1().min_w_0().gap_1().child(
                    text(move || title_text())
                        .role(AccessibilityRole::Header)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(scale::text::SUBTITLE)),
                );
                if let Some(description) = description.clone() {
                    heading = heading.child(
                        text(move || description())
                            .text_size(px(scale::text::BODY))
                            .text_color(move || palette(model).muted),
                    );
                }
                let mut header = view().flex_row().items_start().gap_3();
                if let Some((path, tone)) = header_icon {
                    let tint = move || {
                        let p = palette(model);
                        match tone {
                            DialogTone::Accent => p.primary,
                            DialogTone::Danger => p.danger,
                        }
                    };
                    header = header.child(
                        view()
                            .size(px(scale::disc::MD))
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .rounded(px(scale::radius::PILL))
                            .bg(move || tint().opacity(crate::ui::tokens::tint::REST))
                            .text_color(tint)
                            .child(icon::glyph(path, scale::icon::MD)),
                    );
                }
                panel = panel.child(header.child(heading));
                if let Some(content) = content.clone() {
                    // Content always fills the panel's content width.
                    panel = panel.child(
                        scroll()
                            .w_full()
                            .flex_shrink(1.0)
                            .min_h_0()
                            .child(view().w_full().pb_1().child(content())),
                    );
                }
                let cancel_button = super::button::button(model, "")
                    .label_fn(move || cancel_text())
                    .on_press(move || {
                        if let Some(cb) = &cancel_cb {
                            cb();
                        }
                    });
                let confirm_button = super::button::button(model, "")
                    .label_fn(move || confirm_text())
                    .variant(confirm_variant)
                    .disabled(move || confirm_disabled())
                    .on_press(move || {
                        if let Some(cb) = &confirm_cb {
                            cb();
                        }
                    });
                panel = panel.child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .justify_end()
                        .gap_2()
                        .pt_4()
                        .border_t_1()
                        .border_color(move || palette(model).hairline)
                        .child(cancel_button)
                        .child(confirm_button),
                );
                base_dialog()
                    .flex_1()
                    .close_on_escape(true)
                    .animation(ModalAnimation::None)
                    .on_open(move || {
                        if std::env::var_os("PRECEPT_UI_PROBE").is_some() {
                            eprintln!(
                                "PRECEPT_UI_DIALOG opened=true initial_focus={} focus_bound={}",
                                initial_focus.is_some(),
                                initial_focus.is_some_and(|reference| reference.node().is_some())
                            );
                        }
                        if let Some(reference) = initial_focus {
                            reference.focus();
                        }
                    })
                    .on_layout(move |_bounds: &Bounds<Points>| {
                        if std::env::var_os("PRECEPT_UI_PROBE").is_some() {
                            eprintln!(
                                "PRECEPT_UI_DIALOG root_width={} root_height={}",
                                _bounds.size.width.0, _bounds.size.height.0
                            );
                        }
                    })
                    .on_cancel(move || {
                        if let Some(cb) = &modal_cancel {
                            cb();
                        }
                    })
                    .child(
                        view()
                            .size_full()
                            .flex_1()
                            .items_center()
                            .justify_center()
                            .p_4()
                            .bg(black().opacity(0.5))
                            .bind_motion(&scrim)
                            .child(panel.bind_motion(&grow)),
                    )
                    .into_element()
            })
        })
        .into_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nsh_engine::{EngineConfig, Root};
    use std::cell::Cell;

    #[component]
    fn RetainedDialog() -> impl IntoElement {
        let model = signal(Model::loading());
        let mounts = Rc::new(Cell::new(0_u32));
        let field = element_ref();
        view()
            .flex_1()
            .child(
                super::super::button::button(model, "Open retained dialog")
                    .on_press(move || model.update(|m| m.operator.open = true)),
            )
            .child(
                alert_dialog(model, "Retained dialog")
                    .open(move || model.get().operator.open)
                    .initial_focus(&field)
                    .content_fn(move || {
                        mounts.set(mounts.get() + 1);
                        view()
                            .child(text(format!("Content mounts: {}", mounts.get())))
                            .child(
                                super::super::input::input(model, "Retained field")
                                    .element_ref(&field)
                                    .placeholder(
                                        model.get().word(precept_ui_core::i18n::Msg::OperatorKey),
                                    )
                                    .value(move || model.get().operator.key)
                                    .on_change(move |value| {
                                        model.update(|m| m.operator.key = value.to_owned())
                                    }),
                            )
                            .into_element()
                    }),
            )
    }

    #[component]
    fn LargeFormDialog() -> impl IntoElement {
        let model = signal(Model::loading());
        view().flex_1().child(
            dialog(model, "Add a personal preference")
                .size(DialogSize::Large)
                .icon(crate::ui::icons::PLUS, DialogTone::Accent)
                .description("Saved as a suggestion.")
                .open(|| true)
                .content_fn(move || {
                    view()
                        .w_full()
                        .gap_4()
                        .child(
                            super::super::input::input(model, "Key")
                                .test_id("probe-key")
                                .value(|| "commit-style".into()),
                        )
                        .child(super::super::markdown::markdown_input(
                            model,
                            "probe-value",
                            || "Value".into(),
                            || "Prefer **small** commits.".into(),
                            |_| {},
                        ))
                        .into_element()
                }),
        )
    }

    fn bounds_of(app: &nsh_testing::Headless, id: &str) -> Bounds<Points> {
        app.screen()
            .layout_nodes()
            .into_iter()
            .find(|n| n.test_id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("no {id}"))
            .bounds
    }

    /// The dialog grows to its size and no field reaches past the panel's
    /// content edge, on a roomy window and on a narrow one (the regression:
    /// fixed-width content once spilled out of a 320-point panel).
    #[test]
    fn dialog_content_stays_inside_the_panel_at_every_window_size() {
        let mut cfg = EngineConfig::new(Root::of::<LargeFormDialog>());
        cfg.viewport = Size::new(px(1080.0), px(760.0));
        let mut app = nsh_testing::Headless::start(cfg).unwrap();
        app.wait_for("Add a personal preference").unwrap();
        for (width, expected) in [(1080.0, scale::layout::DIALOG_LG), (520.0, 520.0 - 32.0)] {
            app.set_viewport(Size::new(px(width), px(760.0))).unwrap();
            app.wait_for("Add a personal preference").unwrap();
            let panel = bounds_of(&app, "dialog-panel");
            assert!(
                (panel.size.width.0 - expected).abs() < 0.5,
                "panel {} wide at window {width}, expected {expected}",
                panel.size.width.0
            );
            let right = panel.origin.x.0 + panel.size.width.0;
            for id in ["probe-key", "probe-value"] {
                let field = bounds_of(&app, id);
                assert!(
                    field.origin.x.0 >= panel.origin.x.0 + 23.5
                        && field.origin.x.0 + field.size.width.0 <= right - 23.5,
                    "{id} {:?} leaves the panel {:?} at window {width}",
                    field,
                    panel
                );
            }
        }
    }

    #[test]
    fn editing_dialog_content_does_not_remount_the_native_modal() {
        let mut app =
            nsh_testing::Headless::start(EngineConfig::new(Root::of::<RetainedDialog>())).unwrap();
        app.wait_for("Open retained dialog").unwrap();
        app.press("Open retained dialog").unwrap();
        app.wait_for("Content mounts: 1").unwrap();
        app.press_key("9").unwrap();
        app.press_key("9").unwrap();
        app.press_key("9").unwrap();
        assert_eq!(
            app.screen().input_text("Retained field").unwrap(),
            Some("999")
        );
        app.wait_for("Content mounts: 1").unwrap();
        assert!(
            !app.screen().texts().iter().any(|text| {
                text.starts_with("Content mounts: ") && *text != "Content mounts: 1"
            })
        );
    }
}
