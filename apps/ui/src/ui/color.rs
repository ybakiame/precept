//! Data colors and the app's color picker.
//!
//! A tag's color is the operator's data, not a theme role: it is parsed from
//! its CSS text (`oklch()`, hex, `rgb()`, `hsl()`) and used only as a dot and
//! a light tint behind theme text, so a chip stays readable in every theme
//! and its meaning stays in its name.
//!
//! [`color_picker`] styles the semantic `nsh/base` ColorPicker: a saturation
//! and brightness area, an OKLCH hue slider, a preview swatch beside a field
//! that reads any CSS color and writes `oklch()`, and preset swatches. It
//! reports while dragging (`on_preview`) and once per finished change
//! (`on_commit`), so saving happens once, not on every frame.
use super::prelude::*;
use nsh_base::{
    ColorChannel, ColorFormat, ColorPickerEvent, ColorSpace, color_area, color_field,
    color_picker as base_color_picker, color_slider, color_swatch,
};
use std::rc::Rc;
use std::str::FromStr;

/// `text` as a color, the same in light and dark (data, not a role).
pub fn data_color(text: &str) -> Option<AdaptiveColor> {
    let c = Hsla::from(Rgba::from_str(text.trim()).ok()?);
    Some(AdaptiveColor::new(c, c))
}

/// The color `tag` shows now (its chosen color, else its generated one).
pub fn tag_color(model: Signal<Model>, tag: &str) -> AdaptiveColor {
    let chosen = model.get().tag_color(tag);
    data_color(&chosen)
        .or_else(|| data_color(&precept_ui_core::tag_colors::generated(tag)))
        .unwrap_or_else(|| palette(model).muted)
}

/// A round dot of `color`.
pub fn color_dot(color: impl Fn() -> AdaptiveColor + 'static, size: f32) -> impl IntoElement {
    view()
        .size(px(size))
        .flex_shrink_0()
        .rounded(px(scale::radius::PILL))
        .bg(color)
}

/// A picker thumb: a round handle ringed in the raised surface.
fn thumb(
    s: nsh_core::element::StyleRefinement,
    model: Signal<Model>,
) -> nsh_core::element::StyleRefinement {
    s.size(px(16.0))
        .rounded(px(scale::radius::PILL))
        .border_2()
        .border_color(move || palette(model).raised)
        .shadow_md()
}

/// The presets: ten evenly spaced hues at the generated lightness and chroma.
fn presets() -> Vec<String> {
    (0..10)
        .map(|i| format!("oklch(68% 0.13 {})", i * 36))
        .collect()
}

/// A picker of `value` (CSS color text); `label` names it for screen readers.
pub fn color_picker(
    model: Signal<Model>,
    label: impl Fn() -> SharedString + 'static,
    value: impl Fn() -> String + 'static,
    on_preview: impl Fn(String) + 'static,
    on_commit: impl Fn(String) + 'static,
) -> impl IntoElement {
    let label: Rc<dyn Fn() -> SharedString> = Rc::new(label);
    let current = move || {
        data_color(&value())
            .map(|c| c.light)
            .unwrap_or_else(|| palette(model).muted.light)
    };
    let (area_label, hue_label, field_label, preview_label) =
        (label.clone(), label.clone(), label.clone(), label.clone());
    let mut swatches = view().flex_row().flex_wrap().gap_1p5();
    for preset in presets() {
        let color = data_color(&preset).map(|c| c.light);
        let spoken = preset.clone();
        swatches = swatches.child(
            color_swatch()
                .color(color)
                .accessibility_label(Some(SharedString::from(spoken)))
                .size(px(scale::icon::LG))
                .rounded(px(scale::radius::PILL))
                .border_2()
                .border_color(move || palette(model).panel)
                .selected_style(move |s| s.border_color(palette(model).text))
                .focus(move |s| {
                    s.ring(px(2.0))
                        .ring_color(palette(model).primary.opacity(0.55))
                }),
        );
    }
    base_color_picker()
        .value(move || Some(current()))
        .format(ColorFormat::Oklch)
        .on_change(move |e: &ColorPickerEvent| {
            if let ColorPickerEvent::Change(_, formatted) = e {
                on_preview(formatted.to_string());
            }
        })
        .on_change_end(move |e: &ColorPickerEvent| {
            if let ColorPickerEvent::ChangeEnd(_, formatted) = e {
                on_commit(formatted.to_string());
            }
        })
        .child(
            view()
                .w_full()
                .gap_3()
                .child(
                    color_area()
                        .accessibility_label(Some(area_label()))
                        .w_full()
                        .h(px(140.0))
                        .panel_style(|s| s.rounded(px(scale::radius::CONTROL)))
                        .thumb_style(move |s| thumb(s, model)),
                )
                .child(
                    color_slider()
                        .color_space(ColorSpace::Oklch)
                        .channel(ColorChannel::Hue)
                        .accessibility_label(Some(hue_label()))
                        .w_full()
                        .h(px(20.0))
                        .justify_center()
                        .track_style(|s| s.h(px(10.0)).rounded(px(scale::radius::PILL)))
                        .thumb_style(move |s| thumb(s, model)),
                )
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            color_swatch()
                                .accessibility_label(Some(preview_label()))
                                .size(px(scale::control::MD))
                                .flex_shrink_0()
                                .rounded(px(scale::radius::CONTROL))
                                .border_1()
                                .border_color(move || palette(model).hairline),
                        )
                        .child(
                            color_field()
                                .format(ColorFormat::Oklch)
                                .accessibility_label(Some(field_label()))
                                .flex_1()
                                .min_w_0()
                                .h(px(scale::control::MD))
                                .px_3()
                                .border_1()
                                .rounded(px(scale::radius::CONTROL))
                                .bg(move || palette(model).panel)
                                .text_color(move || palette(model).text)
                                .text_size(px(scale::text::BODY))
                                .border_color(move || palette(model).border)
                                .focus(move |s| {
                                    s.border_color(palette(model).primary)
                                        .ring(px(3.0))
                                        .ring_color(palette(model).primary.opacity(0.25))
                                }),
                        ),
                )
                .child(swatches),
        )
}
