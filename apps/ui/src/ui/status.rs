//! The app's status dot: a small shape that reinforces a state word beside it
//! (DESIGN.md §3). Shape and color differ by state, so it never relies on
//! color alone; motion marks what is live (a ring pinging out) or in progress
//! (a breathing dot) and stops under reduced motion.
use super::prelude::*;

/// What the dot says.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// Answering now: filled success, a ring pings out.
    Live,
    /// Working (a read in flight): filled warning, breathing.
    Busy,
    /// Nothing live (an offline preview): filled muted, still.
    Idle,
    /// Unreachable or refused: hollow error, still.
    Down,
    /// Needs the user's attention: hollow warning, still.
    Attention,
}

impl Status {
    fn hollow(self) -> bool {
        matches!(self, Self::Down | Self::Attention)
    }
}

fn tone(model: Signal<Model>, status: Status) -> AdaptiveColor {
    let p = palette(model);
    match status {
        Status::Live => p.success,
        Status::Busy | Status::Attention => p.warning,
        Status::Idle => p.muted,
        Status::Down => p.error,
    }
}

/// A status dot following `status`; decorative to screen readers, because the
/// word beside it is what they read.
pub fn status_dot(model: Signal<Model>, status: impl Fn() -> Status + 'static) -> impl IntoElement {
    let status = std::rc::Rc::new(status);
    let (fill, edge, ring, ring_shown, pulsing) = (
        status.clone(),
        status.clone(),
        status.clone(),
        status.clone(),
        status,
    );
    let dot = scale::disc::DOT;
    view()
        .accessibility_elements_hidden(true)
        .size(px(dot))
        .flex_shrink_0()
        // The ping ring: the same circle, growing and fading out (Live only).
        .child(
            view()
                .absolute()
                .inset_0()
                .rounded(px(scale::radius::PILL))
                .bg(move || tone(model, ring()))
                .style_when(
                    move || ring_shown() != Status::Live || reduce_motion(),
                    |s| s.hidden(),
                )
                .animate_ping(),
        )
        .child(
            view()
                .absolute()
                .inset_0()
                .rounded(px(scale::radius::PILL))
                .border_1()
                .border_color(move || tone(model, edge()))
                .bg(move || {
                    let s = fill();
                    if s.hollow() {
                        tone(model, s).opacity(0.0)
                    } else {
                        tone(model, s)
                    }
                })
                .style_when(
                    move || pulsing() == Status::Busy && !reduce_motion(),
                    |s| s.animate_pulse(),
                ),
        )
        .into_element()
}
