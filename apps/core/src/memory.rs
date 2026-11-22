//! Project memory as the daemon reports it (`memories.list`): agents' notes
//! on one project (environment, cautions, lessons, attempts), weighted.
//! Parsed and bounded here; the Memory page shows, edits, pins, restores and
//! deletes them, and moves confirmed environment notes of the Profile in.
use crate::i18n::{Locale, Msg, tr};
use crate::presentation::sanitize;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Environment,
    Caution,
    Lesson,
    Attempt,
}
impl Kind {
    pub const ALL: [Kind; 4] = [
        Kind::Environment,
        Kind::Caution,
        Kind::Lesson,
        Kind::Attempt,
    ];
    /// The heading of the kind's group.
    pub fn heading(self) -> Msg {
        match self {
            Kind::Environment => Msg::MemoryKindEnvironment,
            Kind::Caution => Msg::MemoryKindCaution,
            Kind::Lesson => Msg::MemoryKindLesson,
            Kind::Attempt => Msg::MemoryKindAttempt,
        }
    }
    /// The kind's base weight (as the daemon weighs it).
    pub fn base(self) -> f64 {
        match self {
            Kind::Environment => 100.0,
            Kind::Caution => 60.0,
            Kind::Lesson => 40.0,
            Kind::Attempt => 20.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Earlier {
    pub body: String,
    pub until_ms: u64,
    pub by: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Entry {
    pub id: String,
    pub kind: Kind,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub confirmations: u32,
    #[serde(default)]
    pub recalls: u32,
    pub updated_ms: u64,
    #[serde(default)]
    pub edited_by_operator: bool,
    pub by: Option<String>,
    #[serde(default)]
    pub history: Vec<Earlier>,
    pub weight: f64,
    #[serde(default)]
    pub unstable: bool,
    #[serde(default)]
    pub fading: bool,
}

/// Text that keeps its line breaks: no other control or bidi characters,
/// at most `limit` characters.
fn block(value: &str, limit: usize) -> String {
    value
        .lines()
        .map(|l| sanitize(l, limit))
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(limit)
        .collect()
}

#[derive(Deserialize)]
struct Reply {
    entries: Vec<Entry>,
}

/// Parses and bounds a daemon reply; every text is sanitized for display.
pub fn parse(v: serde_json::Value) -> Result<Vec<Entry>, &'static str> {
    let mut list = serde_json::from_value::<Reply>(v)
        .map_err(|_| "Unavailable")?
        .entries;
    if list.len() > 300 {
        return Err("Unavailable");
    }
    for e in &mut list {
        e.id = sanitize(&e.id, 16);
        e.title = sanitize(&e.title, 120);
        e.body = block(&e.body, 2000);
        e.by = e.by.as_deref().map(|b| sanitize(b, 64));
        e.history.truncate(5);
        for h in &mut e.history {
            h.body = block(&h.body, 2000);
            h.by = h.by.as_deref().map(|b| sanitize(b, 64));
        }
    }
    Ok(list)
}

/// A confirmed environment note of the Profile, as the move takes it.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct EnvironmentNote {
    pub id: i64,
    pub revision: i64,
    pub proposal_digest: String,
    pub key: String,
    pub value: String,
}
#[derive(Deserialize)]
struct EnvironmentReply {
    records: Vec<EnvironmentNote>,
}
pub fn parse_environment(v: serde_json::Value) -> Result<Vec<EnvironmentNote>, &'static str> {
    let mut list = serde_json::from_value::<EnvironmentReply>(v)
        .map_err(|_| "Unavailable")?
        .records;
    list.truncate(100);
    for r in &mut list {
        r.key = sanitize(&r.key, 120);
        r.value = block(&r.value, 2000);
    }
    Ok(list)
}

/// Who wrote it, in words: an agent's label, "you", or "an agent".
pub fn author(locale: Locale, by: Option<&str>) -> String {
    match by {
        Some("operator") => tr(locale, Msg::MemoryByYou).to_owned(),
        Some(b) => b.to_owned(),
        None => tr(locale, Msg::MemoryByAgent).to_owned(),
    }
}

/// Why an entry weighs what it does: the kind's base and each addition.
pub fn why(locale: Locale, e: &Entry) -> String {
    let w = |m: Msg| tr(locale, m);
    let mut parts = vec![format!("{} {}", w(e.kind.heading()), e.kind.base())];
    if e.pinned {
        parts.push(w(Msg::MemoryWhyPinned).to_owned());
    }
    if e.confirmations > 0 {
        parts.push(
            w(Msg::MemoryWhyConfirmed)
                .replace("{n}", &e.confirmations.to_string())
                .replace("{w}", &(4 * e.confirmations.min(10)).to_string()),
        );
    }
    if e.recalls > 0 {
        parts.push(
            w(Msg::MemoryWhyRecalled)
                .replace("{n}", &e.recalls.to_string())
                .replace("{w}", &e.recalls.min(20).to_string()),
        );
    }
    if e.edited_by_operator {
        parts.push(w(Msg::MemoryWhyEdited).to_owned());
    }
    if e.fading {
        parts.push(w(Msg::MemoryWhyFading).to_owned());
    }
    w(Msg::MemoryWeight)
        .replace("{w}", &format!("{}", e.weight.round()))
        .replace("{parts}", &parts.join(" · "))
}

/// The Memory page's state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemoryPage {
    /// The project the loaded list belongs to.
    pub project: Option<String>,
    pub list: Option<Vec<Entry>>,
    pub busy: bool,
    pub error: Option<String>,
    /// The entry whose detail is open.
    pub open: Option<String>,
    /// The entry being edited, and its draft title and text.
    pub edit: Option<String>,
    pub title: String,
    pub body: String,
    /// The entry armed for deletion; the clear-all confirmation.
    pub delete: Option<String>,
    pub clear: bool,
    /// Confirmed environment notes of the Profile still to move, and the
    /// move's confirmation.
    pub environment: Vec<EnvironmentNote>,
    pub moving: bool,
}
impl MemoryPage {
    pub fn opened(&self) -> Option<&Entry> {
        let id = self.open.as_deref()?;
        self.list.as_ref()?.iter().find(|e| e.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_bounds_and_explains_weight() {
        let list = parse(json!({"entries":[{
            "id":"0123456789abcdef","kind":"lesson","title":"Run\u{202e} tests","body":"a\nb\u{7}",
            "pinned":true,"confirmations":3,"recalls":2,"created_ms":1,"updated_ms":2,
            "confirmed_ms":2,"edited_by_operator":true,"by":"operator","history":[],
            "changes":[],"weight":1064.0,"unstable":false,"fading":false,"session":null
        }]}))
        .unwrap();
        let e = &list[0];
        assert_eq!(e.title, "Run tests");
        assert_eq!(e.body, "a\nb");
        assert_eq!(author(Locale::En, e.by.as_deref()), "you");
        let text = why(Locale::En, e);
        assert!(text.starts_with("Weight 1064: Lessons 40 · pinned +1000 · 3 confirmations +12 · 2 recalls +2 · edited by you +10"), "{text}");
        assert!(parse(json!({"entries":"x"})).is_err());
    }
}
