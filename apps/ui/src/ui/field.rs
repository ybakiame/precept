//! The app's `Field`: a visible label, its control and optional help, stacked
//! with the spacing of one control's parts. The label stays next to the control
//! it names; help and validation sit directly under it, never elsewhere.
use super::prelude::*;
use std::rc::Rc;

type TextFn = Rc<dyn Fn() -> SharedString>;

/// A field builder; create it with [`field`].
pub struct Field {
    model: Signal<Model>,
    label: TextFn,
    control: AnyElement,
    help: Option<TextFn>,
}

/// A labeled control (usually a `ui::input`).
pub fn field(
    model: Signal<Model>,
    label: impl Fn() -> SharedString + 'static,
    control: impl IntoElement,
) -> Field {
    Field {
        model,
        label: Rc::new(label),
        control: control.into_element(),
        help: None,
    }
}

impl Field {
    /// A short sentence under the control: what it expects or how to recover.
    pub fn help(mut self, help: impl Fn() -> SharedString + 'static) -> Self {
        self.help = Some(Rc::new(help));
        self
    }
}

impl IntoElement for Field {
    fn into_element(self) -> AnyElement {
        let model = self.model;
        let label = self.label;
        let mut body = view()
            .gap_1()
            .child(
                text(move || label())
                    .text_size(px(scale::text::CAPTION))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(move || palette(model).text),
            )
            .child(self.control);
        if let Some(help) = self.help {
            body = body.child(
                text(move || help())
                    .text_size(px(scale::text::CAPTION))
                    .text_color(move || palette(model).muted),
            );
        }
        body.into_element()
    }
}
