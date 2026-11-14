//! The app's motion tokens and the helpers that build control motion from them.
//!
//! Motion explains a change; it is never ambient decoration. Durations,
//! easings, distances and scales live here, beside the theme, so every control
//! moves with one character. Every helper honors `reduce_motion()`: durations
//! collapse to zero and scales rest at 1, so each state stays understandable
//! with all motion off.
use nsh_core::prelude::*;

/// A hover wash, a color change.
pub const INSTANT: f32 = 120.0;
/// A press, a control's state feedback (desktop pointer feedback stays < 200ms).
pub const FAST: f32 = 180.0;
/// Moving into place: a selection sliding, a glyph swapping, a menu leaving.
/// Reserved for the review panel's sliding selection and glyph swaps (Commit B/C).
#[allow(dead_code)]
pub const BASE: f32 = 240.0;
/// Reveals: a dialog or popup opening.
pub const SLOW: f32 = 360.0;
/// Unhurried entrances: a list's wave. Reserved for the queue's arrival wave
/// (Commit B/C).
#[allow(dead_code)]
pub const SLOWER: f32 = 500.0;

/// The soft settle curve for presses (`cubic-bezier(0.22, 1, 0.36, 1)`), an
/// ease-out family that also suits a hover wash coming to rest.
pub const SETTLE: (f32, f32, f32, f32) = (0.22, 1.0, 0.36, 1.0);

/// Whole-control press scale.
pub const PRESS: f32 = 0.97;
/// Icon-button press scale. Reserved for the review panel's icon buttons
/// (Commit B/C).
#[allow(dead_code)]
pub const ICON_PRESS: f32 = 0.95;
/// A popup's entrance scale.
pub const ENTER: f32 = 0.97;
/// A popup's entrance rise (points).
pub const RISE: f32 = 8.0;
/// A changed label's rise (points). Reserved for the review panel's changing
/// status labels (Commit B/C).
#[allow(dead_code)]
pub const LABEL_RISE: f32 = 4.0;

/// A transition duration that collapses to zero under reduced motion.
pub fn ms(token: f32) -> impl Fn() -> f32 {
    move || if reduce_motion() { 0.0 } else { token }
}

/// A press scale that rests at 1 under reduced motion.
pub fn press() -> impl Fn() -> f32 {
    move || if reduce_motion() { 1.0 } else { PRESS }
}

/// Layered pointer feedback for a control: its hover wash, press scale and
/// focus ring animate on the soft settle. Colors, opacity and transforms move;
/// layout never does.
pub fn feedback<S: Styled>(s: S) -> S {
    s.transition()
        .duration(ms(FAST))
        .ease_bezier(SETTLE.0, SETTLE.1, SETTLE.2, SETTLE.3)
}

/// A popup growing from where it belongs: scale up from [`ENTER`], rise
/// [`RISE`] points and fade in, on `slow` ease-out. Lands at rest under
/// reduced motion. Create it once in a component body and bind it with
/// `bind_motion`.
pub fn grow() -> Motion {
    motion().entering(
        MotionSpec::new(KeyframeTiming::new(SLOW).easing(Easing::EaseOut))
            .values(MotionChannel::Scale, [ENTER, 1.0])
            .values(MotionChannel::TranslateY, [RISE, 0.0])
            .values(MotionChannel::Opacity, [0.0, 1.0]),
    )
}

/// A scrim fading in on `fast` ease-out. Lands at rest under reduced motion.
pub fn scrim() -> Motion {
    motion().entering(
        MotionSpec::new(KeyframeTiming::new(FAST).easing(Easing::EaseOut))
            .values(MotionChannel::Opacity, [0.0, 1.0]),
    )
}
