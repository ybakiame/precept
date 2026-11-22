//! The app's keycaps: a keystroke drawn as the keys to press, each in a small
//! cap (`Ctrl` `Shift` `L`), as NSH labels it for this platform (`⇧⌘L` on Apple
//! platforms is one cap). A keystroke NSH cannot parse is shown as written.
use super::prelude::*;
use nsh_core::keymap::Keystroke;

/// The keys of `keystroke` as this platform names them.
pub fn key_parts(keystroke: &str) -> Vec<String> {
    if Keystroke::parse_with(keystroke, apple_keys()).is_err() {
        return vec![keystroke.to_owned()];
    }
    let label = key_label(keystroke);
    if label.contains('+') && label.len() > 1 {
        // `Ctrl++` names the plus key last.
        let mut parts: Vec<String> = label.split('+').map(str::to_owned).collect();
        if label.ends_with("++") {
            parts.retain(|p| !p.is_empty());
            parts.push("+".into());
        }
        parts.into_iter().filter(|p| !p.is_empty()).collect()
    } else {
        vec![label]
    }
}

/// `keystroke` as keycaps.
pub fn keycaps(model: Signal<Model>, keystroke: &str) -> impl IntoElement {
    let parts = key_parts(keystroke);
    view()
        .flex_row()
        .items_center()
        .gap_1()
        .children(parts.into_iter().map(move |part| {
            view()
                .min_w(px(scale::control::SM - 4.0))
                .px_1p5()
                .py_0p5()
                .items_center()
                .rounded(px(scale::radius::SMALL))
                .border_1()
                .border_color(move || palette(model).hairline)
                .bg(move || palette(model).inset)
                .child(
                    text(part)
                        .font_family("monospace")
                        .text_size(px(scale::text::CAPTION))
                        .text_color(move || palette(model).text),
                )
                .into_element()
        }))
        .into_element()
}
