//! Tag colors: one color per tag name, the same on every preference that
//! carries it. Presentation only, kept in `settings.toml` (never in the
//! Profile, never sent to agents): a color never changes what a preference
//! says or who receives it, so changing one needs no review.
//!
//! A tag without a chosen color shows its generated one, an OKLCH color whose
//! hue comes from the name (so `git` is always the same hue). Colors made for
//! the operator, and those pinned when a suggestion first uses a tag, are
//! written as CSS `oklch()`; a color the operator picks is kept as the
//! picker writes it (also `oklch()`). The UI parses them; one that does not
//! parse falls back to the generated color.
use crate::operator::normalize_tag;
use std::collections::BTreeMap;

/// The longest color text kept.
pub const MAX_COLOR: usize = 48;
/// The most colors kept (more are dropped, by name order).
pub const MAX_COLORS: usize = 512;

/// The generated color of `tag`: `oklch(68% 0.13 <hue>)`, the hue an FNV-1a
/// hash of the name in whole degrees. Mid lightness and chroma read as a dot
/// and a tint on light and dark surfaces alike.
pub fn generated(tag: &str) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    for b in tag.bytes() {
        hash ^= u32::from(b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("oklch(68% 0.13 {})", hash % 360)
}

/// Whether `color` may be kept: short printable text in a CSS color form
/// the UI reads (`oklch(...)`, `#hex`, `rgb(...)`, `hsl(...)`).
pub fn plausible(color: &str) -> bool {
    let c = color.trim();
    !c.is_empty()
        && c.len() <= MAX_COLOR
        && c.bytes().all(|b| (0x20..0x7f).contains(&b))
        && (c.starts_with("oklch(")
            || c.starts_with('#')
            || c.starts_with("rgb(")
            || c.starts_with("hsl("))
}

/// The colors kept from `entries`: canonical tag names, plausible colors.
pub fn sanitize(entries: BTreeMap<String, String>) -> BTreeMap<String, String> {
    entries
        .into_iter()
        .filter_map(|(name, color)| {
            let name = normalize_tag(&name)?;
            plausible(&color).then(|| (name, color.trim().to_owned()))
        })
        .take(MAX_COLORS)
        .collect()
}

impl crate::Model {
    /// The color `tag` shows: the chosen one, else the generated one.
    pub fn tag_color(&self, tag: &str) -> String {
        self.tag_colors
            .get(tag)
            .cloned()
            .unwrap_or_else(|| generated(tag))
    }
    /// Chooses `color` for `tag` (an implausible color changes nothing).
    pub fn set_tag_color(&mut self, tag: &str, color: &str) {
        if let Some(name) = normalize_tag(tag)
            && plausible(color)
            && (self.tag_colors.len() < MAX_COLORS || self.tag_colors.contains_key(&name))
        {
            self.tag_colors.insert(name, color.trim().to_owned());
        }
    }
    /// Back to the generated color.
    pub fn reset_tag_color(&mut self, tag: &str) {
        self.tag_colors.remove(tag);
    }
    /// Pins the generated `oklch()` color of each of `tags` not colored yet:
    /// a tag a suggestion brings keeps its color from then on.
    pub fn pin_tag_colors(&mut self, tags: &[String]) {
        for t in tags {
            if !self.tag_colors.contains_key(t) && self.tag_colors.len() < MAX_COLORS {
                self.tag_colors.insert(t.clone(), generated(t));
            }
        }
    }
    /// Every tag the app knows here (in use on this project, on loaded
    /// records, or colored), with how many confirmed rows carry it.
    pub fn known_tags(&self) -> Vec<(String, usize)> {
        let mut all: BTreeMap<String, usize> = self.tag_counts().into_iter().collect();
        let records = self
            .operator
            .page
            .iter()
            .flat_map(|p| &p.records)
            .chain(self.operator.queue.iter().flat_map(|q| &q.records));
        for t in records.flat_map(|r| &r.tags).chain(self.tag_colors.keys()) {
            all.entry(t.clone()).or_default();
        }
        let mut all: Vec<_> = all.into_iter().collect();
        all.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_colors_are_stable_oklch_per_name() {
        assert_eq!(generated("git"), generated("git"));
        assert!(generated("git").starts_with("oklch(68% 0.13 "));
        assert_ne!(generated("git"), generated("style"));
        assert!(plausible(&generated("测试")));
    }

    #[test]
    fn colors_are_chosen_reset_pinned_and_sanitized() {
        let mut m = crate::Model::loading();
        assert_eq!(m.tag_color("git"), generated("git"));
        m.set_tag_color("Git", "oklch(60% 0.2 30)");
        assert_eq!(m.tag_color("git"), "oklch(60% 0.2 30)");
        m.set_tag_color("git", "url(javascript:x)");
        assert_eq!(m.tag_color("git"), "oklch(60% 0.2 30)");
        m.reset_tag_color("git");
        assert_eq!(m.tag_color("git"), generated("git"));
        m.pin_tag_colors(&["docs".into()]);
        assert_eq!(m.tag_colors["docs"], generated("docs"));
        m.set_tag_color("docs", "#112233");
        m.pin_tag_colors(&["docs".into()]);
        assert_eq!(m.tag_colors["docs"], "#112233");
        let kept = sanitize(BTreeMap::from([
            ("Style".to_owned(), " #abcdef ".to_owned()),
            ("a/b".to_owned(), "#000".to_owned()),
            ("ok".to_owned(), "red".to_owned()),
        ]));
        assert_eq!(
            kept,
            BTreeMap::from([("style".to_owned(), "#abcdef".to_owned())])
        );
    }
}
