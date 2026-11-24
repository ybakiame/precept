use super::model::{DAY_MS, Entry, FADE_AFTER_DAYS, Kind, NOTICE_AFTER_DAYS, UNSTABLE_WINDOW_MS};

const UNSTABLE_CHANGES: usize = 3;

impl Kind {
    fn base(self) -> f64 {
        match self {
            Kind::Environment => 100.0,
            Kind::Caution => 60.0,
            Kind::Lesson => 40.0,
            Kind::Attempt => 20.0,
        }
    }
}

impl Entry {
    /// The entry's weight at `now`: its kind, pin, confirmations, recalls and
    /// an operator's edit, less fading after [`FADE_AFTER_DAYS`].
    pub fn weight(&self, now: u64) -> f64 {
        let base = self.kind.base();
        let idle_days = now.saturating_sub(self.confirmed_ms) / DAY_MS;
        let fade = (idle_days.saturating_sub(FADE_AFTER_DAYS) as f64 * 0.5).min(base);
        base + if self.pinned { 1000.0 } else { 0.0 }
            + 4.0 * f64::from(self.confirmations.min(10))
            + f64::from(self.recalls.min(20))
            + if self.edited_by_operator { 10.0 } else { 0.0 }
            - fade
    }
    /// Changed often lately: shown so, never boosted for it.
    pub fn unstable(&self, now: u64) -> bool {
        self.changes
            .iter()
            .filter(|t| now.saturating_sub(**t) <= UNSTABLE_WINDOW_MS)
            .count()
            >= UNSTABLE_CHANGES
    }
    /// Fading: not confirmed for [`FADE_AFTER_DAYS`].
    pub fn fading(&self, now: u64) -> bool {
        now.saturating_sub(self.confirmed_ms) / DAY_MS > FADE_AFTER_DAYS
    }
    /// The entry with its weight and flags, as reads return it.
    pub fn view(&self, now: u64) -> serde_json::Value {
        let mut v = serde_json::to_value(self).unwrap_or_default();
        v["weight"] = serde_json::json!((self.weight(now) * 10.0).round() / 10.0);
        v["unstable"] = serde_json::json!(self.unstable(now));
        v["fading"] = serde_json::json!(self.fading(now));
        let idle_days = now.saturating_sub(self.confirmed_ms) / DAY_MS;
        v["unverified_days"] = serde_json::json!(idle_days);
        if idle_days >= NOTICE_AFTER_DAYS {
            v["notice"] = serde_json::json!(format!(
                "Not verified for {idle_days} days; check it against the current code before relying on it, then reaffirm or correct it."
            ));
        }
        v
    }
}

/// A query's search terms: its words, lowercase; a run of CJK characters
/// longer than two also gives its overlapping pairs, so a phrase finds notes
/// that use its parts. At most 32 terms.
pub(super) fn terms(query: &str) -> Vec<String> {
    let cjk = |c: char| matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF);
    let mut out: Vec<String> = Vec::new();
    for word in query.split_whitespace() {
        let word = word.to_lowercase();
        out.push(word.clone());
        let mut run = Vec::new();
        for c in word.chars().chain(std::iter::once(' ')) {
            if cjk(c) {
                run.push(c);
                continue;
            }
            if run.len() > 2 {
                for pair in run.windows(2) {
                    out.push(pair.iter().collect());
                }
            }
            run.clear();
        }
    }
    out.sort();
    out.dedup();
    out.truncate(32);
    out
}
