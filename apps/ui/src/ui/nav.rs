//! The app's `NavItem`: one sidebar destination. A ghost button that fills
//! the sidebar's width, with its icon in a fixed lane and one persistent
//! selected fill (never a leading bar). Keyboard, focus ring and press
//! feedback are the button's.
use super::prelude::*;
use super::{ButtonVariant, button};

/// A destination: its icon path, a label that stays current, whether it is the
/// current one and what choosing it does.
pub fn nav_item(
    model: Signal<Model>,
    icon_path: &'static str,
    label: impl Fn() -> SharedString + 'static,
    selected: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    button(model, "")
        .label_fn(label)
        .variant(ButtonVariant::Ghost)
        .icon(icon_path)
        .full_width()
        .selected(selected)
        .quiet_selection(move || !compact())
        .on_press(on_press)
}

/// A destination with work waiting: a count pill in the trailing lane while
/// `count` is known and above zero (it pops in when the count changes), and
/// `spoken` naming the count for screen readers ("To review, 3 waiting").
pub fn nav_item_counted(
    model: Signal<Model>,
    icon_path: &'static str,
    label: impl Fn() -> SharedString + 'static,
    count: impl Fn() -> Option<i64> + 'static,
    spoken: impl Fn() -> SharedString + 'static,
    selected: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    let count = computed(count);
    let pill = dynamic(move || {
        let Some(n) = count.get() else {
            return view().into_element();
        };
        let shown = if n > 99 {
            "99+".to_owned()
        } else {
            n.to_string()
        };
        let pop = motion::pop();
        view()
            .min_w(px(scale::icon::LG))
            .px_1p5()
            .items_center()
            .rounded(px(scale::radius::PILL))
            .bg(move || palette(model).primary)
            .bind_motion(&pop)
            .child(
                text(shown)
                    .tabular_nums()
                    .text_size(px(scale::text::CAPTION))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(move || palette(model).on_primary),
            )
            .into_element()
    });
    button(model, "")
        .label_fn(label)
        .spoken(spoken)
        .variant(ButtonVariant::Ghost)
        .icon(icon_path)
        .full_width()
        .selected(selected)
        .quiet_selection(move || !compact())
        .trailing(pill)
        .on_press(on_press)
}

/// A setup step tab: its icon, visible label, and a success check in the
/// trailing lane once complete.
/// The check grows in when the step completes.
pub fn step_item(
    model: Signal<Model>,
    icon_path: &'static str,
    label: impl Fn() -> SharedString + 'static,
    spoken: impl Fn() -> SharedString + 'static,
    selected: impl Fn() -> bool + 'static,
    done: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    let done = computed(done);
    let check = dynamic(move || {
        if !done.get() {
            return view().into_element();
        }
        let pop = motion::pop();
        view()
            .text_color(move || palette(model).success)
            .bind_motion(&pop)
            .child(icon::glyph(super::icons::CHECK, scale::icon::SM))
            .into_element()
    });
    button(model, "")
        .label_fn(label)
        .spoken(spoken)
        .variant(ButtonVariant::Ghost)
        .icon(icon_path)
        .full_width()
        .selected(selected)
        .trailing(check)
        .on_press(on_press)
}

/// One selection for a column of rows in several groups (the sidebar): the
/// rows register as they are built, and one indicator behind them slides to
/// the current row (`base`, ease-in-out; it jumps under reduced motion and
/// on its first placement). The rows draw no selected fill of their own
/// meanwhile ([`super::button::Button::quiet_selection`]). In the compact,
/// wrapped layout it hides and the rows draw their own fills.
pub struct Slide<K: Copy + PartialEq + 'static> {
    rows: std::rc::Rc<std::cell::RefCell<Vec<(K, ElementRef)>>>,
    /// Bumped whenever a row or a group moves, to measure again.
    moved: Signal<u32>,
}

impl<K: Copy + PartialEq + 'static> Default for Slide<K> {
    fn default() -> Self {
        Self {
            rows: Default::default(),
            moved: signal(0),
        }
    }
}

impl<K: Copy + PartialEq + 'static> Slide<K> {
    /// `child` as the row for `key`.
    pub fn row(&self, key: K, child: impl IntoElement) -> AnyElement {
        let reference = element_ref();
        self.rows.borrow_mut().push((key, reference));
        let moved = self.moved;
        view()
            .element_ref(&reference)
            .on_layout(move |_: &Bounds<Points>| moved.update(|m| *m = m.wrapping_add(1)))
            .child(child)
            .into_element()
    }

    /// `child` as a group of rows: when it moves (a group above it grew),
    /// the indicator measures again.
    pub fn group(&self, child: impl IntoElement) -> AnyElement {
        let moved = self.moved;
        view()
            .on_layout(move |_: &Bounds<Points>| moved.update(|m| *m = m.wrapping_add(1)))
            .child(child)
            .into_element()
    }

    /// The indicator, the first child of `container` (whose ref it is),
    /// placed under the row `current` names. Build it after the rows.
    pub fn indicator(
        &self,
        model: Signal<Model>,
        container: ElementRef,
        current: impl Fn() -> K + 'static,
    ) -> AnyElement {
        let rows = self.rows.clone();
        let y = animated_value(0.0);
        let (x, w, h) = (signal(0.0_f32), signal(0.0_f32), signal(0.0_f32));
        let shown = signal(false);
        let placed = stored(std::cell::Cell::new(false));
        let last_target = stored(std::cell::Cell::new(None::<(K, f32)>));
        let generation = signal(0_u64);
        // Bumped to measure again once layout has happened.
        let retry = signal(0_u8);
        let moved = self.moved;
        let current = computed(current);
        effect(move || {
            let key = current.get();
            let attempt = retry.get();
            let _ = moved.get();
            let _ = window();
            let run = generation.get_untracked().wrapping_add(1);
            generation.set(run);
            if compact() {
                shown.set(false);
                return;
            }
            let Some(row) = rows
                .borrow()
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, r)| *r)
            else {
                shown.set(false);
                return;
            };
            spawn(async move {
                let (Some(at), Some(frame)) = (row.measure().await, container.measure().await)
                else {
                    if attempt < 8 {
                        request_animation_frame(move |_| retry.update(|r| *r += 1));
                    }
                    return;
                };
                if generation.get_untracked() != run {
                    return;
                }
                // Only the newest layout measurement may move or restart the slide.
                let top = at.page.y.0 - frame.page.y.0;
                x.set(at.page.x.0 - frame.page.x.0);
                w.set(at.frame.size.width.0);
                h.set(at.frame.size.height.0);
                let same_target = last_target.with(|last| last.get() == Some((key, top)));
                if !placed.with(|p| p.get()) || reduce_motion() {
                    y.set(top);
                    placed.with(|p| p.set(true));
                } else if same_target {
                    y.set(top);
                } else {
                    y.timing(top, motion::BASE, Easing::EaseInOut);
                }
                last_target.with(|last| last.set(Some((key, top))));
                shown.set(true);
            });
        });
        view()
            .absolute()
            .top_0()
            .left(move || px(x.get()))
            .w(move || px(w.get()))
            .h(move || px(h.get()))
            .bind_translate_y(&y)
            .rounded(px(scale::radius::CONTROL))
            .bg(move || palette(model).inset)
            .style_when(move || !shown.get(), |s| s.hidden())
            .pointer_events(PointerEvents::None)
            .accessibility_elements_hidden(true)
            .into_element()
    }
}
