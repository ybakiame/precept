//! Disclosure: a quiet header row that shows or hides a section of rarely
//! needed content (an "Advanced" area). The header is one button announcing
//! its expanded state; its chevron turns as the region springs open (NSH's
//! `Collapsible` reveal, off under reduced motion).
use super::prelude::*;

/// A right-pointing chevron; turned a quarter when open.
const CHEVRON: &str = "M9 6L15 12L9 18";

/// A collapsed-by-default region under a header that reads `label`, with
/// `hint` beside it while closed.
pub fn disclosure(
    model: Signal<Model>,
    label: impl Fn() -> SharedString + 'static,
    hint: impl Fn() -> SharedString + 'static,
    content: impl IntoElement,
) -> impl IntoElement {
    let open = signal(false);
    let label = std::rc::Rc::new(label);
    let spoken = label.clone();
    let header = view()
        .role(AccessibilityRole::Button)
        .label(move || spoken())
        .accessibility_state(move || AccessibilityState {
            expanded: Some(open.get()),
            ..Default::default()
        })
        .on_press(move || open.update(|o| *o = !*o))
        .flex_row()
        .items_center()
        .gap_2()
        .px_2()
        .py_1p5()
        .rounded(px(scale::radius::CONTROL))
        .transition_colors()
        .duration(motion::ms(motion::FAST))
        .hover(move |s| s.bg(mix(palette(model).background, palette(model).text, 0.05)))
        .focus(move |s| {
            s.ring(px(2.0))
                .ring_color(palette(model).primary.opacity(0.55))
        })
        .child(
            view()
                .text_color(move || palette(model).muted)
                .rotate(move || {
                    Radians(if open.get() {
                        std::f32::consts::FRAC_PI_2
                    } else {
                        0.0
                    })
                })
                .transition_transform()
                .duration(motion::ms(motion::FAST))
                .child(icon::glyph(CHEVRON, scale::icon::SM)),
        )
        .child(text(move || label()).font_weight(FontWeight::MEDIUM))
        .child(
            text(move || {
                if open.get() {
                    SharedString::default()
                } else {
                    hint()
                }
            })
            .flex_1()
            .text_size(px(scale::text::CAPTION))
            .text_color(move || palette(model).muted),
        );
    let header = motion::feedback(header);
    nsh_base::collapsible()
        .open(open)
        .reveal(true)
        .content(view().pt_3().gap_6().child(content))
        .child(header)
}
