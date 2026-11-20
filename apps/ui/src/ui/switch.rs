//! The app's `Switch` and quick-setting tile: an on/off setting that takes
//! effect at once. Built on the semantic `nsh/base` switch (switch role,
//! checked state for screen readers, Space/Return, disabled), styled here:
//! a pill track that fills with `primary` and a thumb that slides across it.
//! The tile is the gallery's quick setting: the whole card is the switch, its
//! icon well lights up and its edge turns `primary` while on. In a terminal
//! the base switch shows its own `[on ]` / `[off]` mark beside the label.
use super::prelude::*;
use std::rc::Rc;

const TRACK_W: f32 = 34.0;
const TRACK_H: f32 = 20.0;
const THUMB: f32 = 14.0;
/// A quick-setting tile's narrowest width; tiles in a wrapped row share it evenly.
const TILE_MIN: f32 = 180.0;

fn track(
    model: Signal<Model>,
    on: Rc<dyn Fn() -> bool>,
) -> impl Fn(nsh_core::element::StyleRefinement) -> nsh_core::element::StyleRefinement {
    move |s| {
        let p = palette(model);
        s.w(px(TRACK_W))
            .h(px(TRACK_H))
            .flex_shrink_0()
            .rounded(px(scale::radius::PILL))
            .border_1()
            .border_color(if on() { p.primary } else { p.border })
            .bg(if on() { p.primary } else { p.inset })
            .justify_center()
            .transition_colors()
            .duration(motion::ms(motion::FAST)())
    }
}

fn thumb(
    model: Signal<Model>,
    on: Rc<dyn Fn() -> bool>,
) -> impl Fn(nsh_core::element::StyleRefinement) -> nsh_core::element::StyleRefinement {
    move |s| {
        let p = palette(model);
        s.size(px(THUMB))
            .rounded(px(scale::radius::PILL))
            .bg(if on() { p.on_primary } else { p.muted })
            .ml(px(if on() { TRACK_W - THUMB - 4.0 } else { 2.0 }))
            .shadow_sm()
            .transition()
            .duration(motion::ms(motion::FAST)())
            .ease_out()
    }
}

/// A plain switch named `label` (for screen readers and, in a terminal, beside
/// its mark).
pub fn switch(
    model: Signal<Model>,
    label: impl Fn() -> SharedString + 'static,
    on: impl Fn() -> bool + 'static,
    disabled: impl Fn() -> bool + 'static,
    on_change: impl Fn(bool) + 'static,
) -> impl IntoElement {
    let on: Rc<dyn Fn() -> bool> = Rc::new(on);
    let checked = on.clone();
    nsh_base::switch()
        .checked(move || checked())
        .disabled(disabled)
        .accessibility_label(move || Some(label()))
        .on_change(move |next: &bool| on_change(*next))
        .track_style(track(model, on.clone()))
        .thumb_style(thumb(model, on))
        .focus(move |s| {
            s.ring(px(2.0))
                .ring_color(palette(model).primary.opacity(0.55))
        })
        .rounded(px(scale::radius::PILL))
}

/// A quick-setting tile builder: the card is the switch. Create it with
/// [`quick_tile`], then give it a title, a spoken name, a detail line and,
/// when needed, a disabled condition.
pub struct QuickTile {
    model: Signal<Model>,
    icon: &'static str,
    on: Rc<dyn Fn() -> bool>,
    on_change: Rc<dyn Fn(bool)>,
    title: Rc<dyn Fn() -> SharedString>,
    spoken: Option<Rc<dyn Fn() -> SharedString>>,
    detail: Rc<dyn Fn() -> SharedString>,
    disabled: Rc<dyn Fn() -> bool>,
}

/// A tile whose `icon` lights up while `on`; `on_change` gets the asked value.
pub fn quick_tile(
    model: Signal<Model>,
    icon: &'static str,
    on: impl Fn() -> bool + 'static,
    on_change: impl Fn(bool) + 'static,
) -> QuickTile {
    QuickTile {
        model,
        icon,
        on: Rc::new(on),
        on_change: Rc::new(on_change),
        title: Rc::new(SharedString::default),
        spoken: None,
        detail: Rc::new(SharedString::default),
        disabled: Rc::new(|| false),
    }
}

impl QuickTile {
    pub fn title_fn(mut self, f: impl Fn() -> SharedString + 'static) -> Self {
        self.title = Rc::new(f);
        self
    }
    /// The switch's name for screen readers, unique on the page (default: the title).
    pub fn spoken(mut self, f: impl Fn() -> SharedString + 'static) -> Self {
        self.spoken = Some(Rc::new(f));
        self
    }
    /// What on means, or why it is off.
    pub fn detail(mut self, f: impl Fn() -> SharedString + 'static) -> Self {
        self.detail = Rc::new(f);
        self
    }
    pub fn disabled(mut self, f: impl Fn() -> bool + 'static) -> Self {
        self.disabled = Rc::new(f);
        self
    }
}

impl IntoElement for QuickTile {
    fn into_element(self) -> AnyElement {
        let QuickTile {
            model,
            icon: icon_path,
            on,
            on_change,
            title,
            spoken,
            detail,
            disabled,
        } = self;
        let spoken = spoken.unwrap_or_else(|| title.clone());
        let (checked, glyph, dim) = (on.clone(), on.clone(), disabled.clone());
        // One tracked value for every lit part (a signal is Copy).
        let lit = computed(move || glyph());
        let tile = nsh_base::switch()
            .checked(move || checked())
            .disabled(move || dim())
            .accessibility_label(move || Some(spoken()))
            .on_change(move |next: &bool| on_change(*next))
            .flex_1()
            .min_w(px(TILE_MIN))
            .p_3()
            .gap_0p5()
            .rounded(px(scale::radius::CONTAINER))
            .border_1()
            .border_color(move || {
                let p = palette(model);
                if lit.get() { p.primary } else { p.hairline }
            })
            .bg(move || {
                let p = palette(model);
                if lit.get() {
                    mix(p.panel, p.primary, 0.08)
                } else {
                    p.panel
                }
            })
            .transition_colors()
            .duration(motion::ms(motion::FAST))
            .hover(move |s| s.bg(mix(palette(model).panel, palette(model).text, 0.04)))
            .active(move |s| s.scale(motion::press()))
            .focus(move |s| {
                s.ring(px(2.0))
                    .ring_color(palette(model).primary.opacity(0.55))
            })
            .opacity(move || if disabled() { 0.55_f32 } else { 1.0_f32 })
            .track_style({
                let base = track(model, on.clone());
                move |s| base(s).absolute().top(px(14.0)).right(px(12.0))
            })
            .thumb_style(thumb(model, on.clone()));
        let tile = motion::feedback(tile);
        tile.child(
            view()
                .style_when(terminal, |s| s.hidden())
                .size(px(32.0))
                .mb_2()
                .rounded(px(scale::radius::PILL))
                .items_center()
                .justify_center()
                .border_1()
                .border_color(move || {
                    let p = palette(model);
                    if lit.get() { p.primary } else { p.border }
                })
                .bg(move || {
                    let p = palette(model);
                    if lit.get() { p.primary } else { p.inset }
                })
                .transition_colors()
                .duration(motion::ms(motion::FAST))
                .child(dynamic(move || {
                    let on = lit.get();
                    view()
                        .text_color(move || {
                            let p = palette(model);
                            if on { p.on_primary } else { p.muted }
                        })
                        .child(icon::glyph(icon_path, scale::icon::SM))
                        .into_element()
                })),
        )
        .child(
            text(move || title())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || palette(model).text),
        )
        .child(
            text(move || detail())
                .text_size(px(scale::text::CAPTION))
                .text_color(move || palette(model).muted),
        )
        .into_element()
    }
}
