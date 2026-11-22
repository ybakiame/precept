//! One icon family: original outline vectors on a 24×24 grid, stroke 1.6,
//! round caps and joins, `currentColor`. Icons supplement labels and never
//! replace an unfamiliar action. Sizes come from [`scale::icon`].
use super::prelude::*;
use nsh_core::element::ImageSrc;

fn markup(path: &str) -> String {
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path d='{path}' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'/></svg>"
    )
}

/// An outline glyph at `size` points.
pub fn glyph(path: &str, size: f32) -> impl IntoElement {
    let markup = markup(path);
    view().child(svg(ImageSrc::svg_markup(&markup)).w(px(size)).h(px(size)))
}

/// A loading spinner: an arc that turns while work is in flight. It rests
/// (still drawn, not turning) under reduced motion.
pub fn spinner(size: f32) -> impl IntoElement {
    let markup = markup("M12 3A9 9 0 1 0 21 12");
    view().child(
        svg(ImageSrc::svg_markup(&markup))
            .w(px(size))
            .h(px(size))
            .animate_spin()
            .style_when(reduce_motion, |s| s.animate_none()),
    )
}
