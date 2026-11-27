//! Waits sized for real daemon processes on slow hosted runners. The engine
//! waits 5 s at most, and a press fails at once on a control that is still
//! disabled while the connection, a read or a grant catches up.
use nsh_testing::Headless;
use std::time::Duration;

/// How long a process test waits for the screen to catch up.
pub const PATIENCE: Duration = Duration::from_secs(30);

pub trait Patient {
    /// `selector` shows within [`PATIENCE`].
    fn sees(&mut self, selector: &str) -> Result<(), String>;
    /// Presses `selector` once exactly one enabled control matches it.
    fn presses(&mut self, selector: &str) -> Result<(), String>;
}

impl Patient for Headless {
    fn sees(&mut self, selector: &str) -> Result<(), String> {
        self.wait_for_within(selector, PATIENCE)
    }

    fn presses(&mut self, selector: &str) -> Result<(), String> {
        // An ambiguous selector is left for `press` to report.
        self.wait_until(&format!("`{selector}` pressable"), PATIENCE, |s| {
            !matches!(s.press_target(selector), Ok(None))
        })?;
        self.press(selector)
    }
}
