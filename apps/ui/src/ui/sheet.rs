//! The app's `Sheet`: supplementary work beside the page, from the trailing
//! edge over a scrim (the `nsh/base` sheet: focus, Escape and backdrop
//! dismissal, a dialog role named by its title). A header with the title and
//! a Close button, a scrolling body and a footer for the sheet's actions.
//! The caller owns whether it is open and closes it in `on_close`.
use super::prelude::*;
use nsh_base::{SheetEvent, sheet as base_sheet};
use std::rc::Rc;

type LabelFn = Rc<dyn Fn() -> SharedString>;
type ContentFn = Rc<dyn Fn() -> AnyElement>;

pub struct Sheet {
    model: Signal<Model>,
    title: LabelFn,
    close: LabelFn,
    open: Rc<dyn Fn() -> bool>,
    on_close: Option<Rc<dyn Fn()>>,
    body: Option<ContentFn>,
    footer: Option<ContentFn>,
}

pub fn sheet(model: Signal<Model>) -> Sheet {
    Sheet {
        model,
        title: Rc::new(|| "".into()),
        close: Rc::new(|| "Close".into()),
        open: Rc::new(|| false),
        on_close: None,
        body: None,
        footer: None,
    }
}

impl Sheet {
    pub fn title_fn(mut self, title: impl Fn() -> SharedString + 'static) -> Self {
        self.title = Rc::new(title);
        self
    }
    /// The Close button's name (localized).
    pub fn close_fn(mut self, label: impl Fn() -> SharedString + 'static) -> Self {
        self.close = Rc::new(label);
        self
    }
    pub fn open(mut self, open: impl Fn() -> bool + 'static) -> Self {
        self.open = Rc::new(open);
        self
    }
    pub fn on_close(mut self, on_close: impl Fn() + 'static) -> Self {
        self.on_close = Some(Rc::new(on_close));
        self
    }
    /// The content (made once; keep it current with bindings or `dynamic`).
    pub fn body(mut self, body: impl Fn() -> AnyElement + 'static) -> Self {
        self.body = Some(Rc::new(body));
        self
    }
    /// The sheet's actions, pinned below the body.
    pub fn footer(mut self, footer: impl Fn() -> AnyElement + 'static) -> Self {
        self.footer = Some(Rc::new(footer));
        self
    }
}

impl IntoElement for Sheet {
    fn into_element(self) -> AnyElement {
        let Sheet {
            model,
            title,
            close,
            open,
            on_close,
            body,
            footer,
        } = self;
        let close_cb = on_close.clone();
        let changed = on_close.clone();
        let label = title.clone();
        base_sheet()
            .open(move || Some(open()))
            .side(Placement::Right)
            .accessibility_label(move || Some(label()))
            .on_open_change(move |e: &SheetEvent| {
                if let SheetEvent::OpenChange(false, _) = e
                    && let Some(cb) = &changed
                {
                    cb();
                }
            })
            .backdrop_style(|s| s.bg(black().opacity(0.4)))
            .popup_style(move |s| {
                s.w(px(if compact() {
                    360.0
                } else {
                    scale::layout::SHEET
                }))
                .h_full()
                .border_l_1()
                .border_color(palette(model).border)
                .bg(palette(model).panel)
                .text_color(palette(model).text)
                .shadow_lg()
            })
            .child({
                let mut panel = view()
                    .size_full()
                    .child(
                        view()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .px_4()
                            .py_3()
                            .border_b_1()
                            .border_color(move || palette(model).border)
                            .child(
                                text({
                                    let title = title.clone();
                                    move || title()
                                })
                                .role(AccessibilityRole::Header)
                                .flex_1()
                                .min_w_0()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(scale::text::SUBTITLE)),
                            )
                            .child(
                                super::button::button(model, "")
                                    .label_fn({
                                        let close = close.clone();
                                        move || close()
                                    })
                                    .variant(super::button::ButtonVariant::Ghost)
                                    .size(super::button::ButtonSize::Small)
                                    .on_press(move || {
                                        if let Some(cb) = &close_cb {
                                            cb();
                                        }
                                    }),
                            ),
                    )
                    .child(
                        scroll().flex_1().min_h_0().child(
                            view()
                                .p_4()
                                .gap_4()
                                .flex_shrink_0()
                                .children(body.as_ref().map(|b| b())),
                        ),
                    );
                if let Some(footer) = &footer {
                    panel = panel.child(
                        view()
                            .px_4()
                            .py_3()
                            .border_t_1()
                            .border_color(move || palette(model).border)
                            .child(footer()),
                    );
                }
                panel
            })
            .into_element()
    }
}
