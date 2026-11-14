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

/// A destructive-confirmation builder; create it with [`alert_dialog`].
pub struct AlertDialog {
    model: Signal<Model>,
    title: LabelFn,
    description: Option<LabelFn>,
    cancel: LabelFn,
    confirm: LabelFn,
    open: StateFn,
    on_cancel: Option<PressFn>,
    on_confirm: Option<PressFn>,
}

/// A confirmation titled for its decision (e.g. `Delete “Roadmap”?`).
pub fn alert_dialog(model: Signal<Model>, title: impl Into<SharedString>) -> AlertDialog {
    let title = title.into();
    AlertDialog {
        model,
        title: Rc::new(move || title.clone()),
        description: None,
        cancel: Rc::new(|| "Cancel".into()),
        confirm: Rc::new(|| "Confirm".into()),
        open: Rc::new(|| false),
        on_cancel: None,
        on_confirm: None,
    }
}

impl AlertDialog {
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
    /// Whether the dialog is presented; the caller owns this state.
    pub fn open(mut self, open: impl Fn() -> bool + 'static) -> Self {
        self.open = Rc::new(open);
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
        let cancel = self.cancel.clone();
        let confirm = self.confirm.clone();
        let open = self.open.clone();
        let on_cancel = self.on_cancel.clone();
        let on_confirm = self.on_confirm.clone();
        // Present while open; leaving it out of the view dismisses it. The
        // motion is created fresh each time it opens, so the grow plays once.
        dynamic(move || {
            if !open() {
                return view().into_element();
            }
            let scrim = motion::scrim();
            let grow = motion::grow();
            let cancel_cb = on_cancel.clone();
            let confirm_cb = on_confirm.clone();
            let modal_cancel = on_cancel.clone();
            let title_text = title.clone();
            let cancel_text = cancel.clone();
            let confirm_text = confirm.clone();
            let mut panel = view()
                .w_full()
                .max_w_80()
                .p_4()
                .gap_3()
                .rounded(px(scale::radius::CONTAINER))
                .border_1()
                .border_color(move || palette(model).border)
                .bg(move || palette(model).panel)
                .text_color(move || palette(model).text)
                .shadow_lg()
                .child(
                    text(move || title_text())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(scale::text::SUBTITLE)),
                );
            if let Some(description) = description.clone() {
                panel = panel.child(
                    text(move || description())
                        .text_size(px(scale::text::BODY))
                        .text_color(move || palette(model).muted),
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
                .variant(super::button::ButtonVariant::Danger)
                .on_press(move || {
                    if let Some(cb) = &confirm_cb {
                        cb();
                    }
                });
            panel = panel.child(
                view()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .child(cancel_button)
                    .child(confirm_button),
            );
            base_dialog()
                .close_on_escape(true)
                .animation(ModalAnimation::None)
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
        .into_element()
    }
}
