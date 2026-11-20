//! Agent activity as the daemon reports it (`activity.list`): when an MCP
//! client started `precept-mcp` for a project, what it read (by count only) and
//! what was refused. The daemon keeps it in memory since it started; this only
//! parses and words it. Client names are self-reported labels, sanitized.
use crate::i18n::Msg;
use crate::presentation::sanitize;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Counts {
    pub preferences: usize,
    pub contracts: usize,
    pub documents: usize,
    pub contract_status: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Event {
    pub id: u64,
    pub at_ms: u64,
    /// `started`, `context`, `handoff` or `refused`.
    pub kind: String,
    /// `mcp`, `session` or `unknown`.
    pub via: String,
    pub project: Option<String>,
    pub client: Option<String>,
    /// `ok`, else an IPC error code.
    pub outcome: String,
    pub duration_ms: u64,
    #[serde(default)]
    pub counts: Option<Counts>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Page {
    pub daemon_started_ms: u64,
    pub events: Vec<Event>,
    #[serde(default)]
    pub more: bool,
}

/// The activity panel's state on the Agents page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Activity {
    /// The project the loaded events belong to.
    pub project: Option<String>,
    pub page: Option<Page>,
    pub busy: bool,
    /// The last refusal (a fixed code).
    pub error: Option<String>,
}

impl Page {
    /// Parses and bounds a daemon reply.
    pub fn parse(value: serde_json::Value) -> Result<Self, &'static str> {
        let mut page: Self = serde_json::from_value(value).map_err(|_| "Unavailable")?;
        if page.events.len() > 500 {
            return Err("Unavailable");
        }
        for e in &mut page.events {
            e.client = e.client.as_deref().map(|c| sanitize(c, 64));
            e.outcome = sanitize(&e.outcome, 32);
        }
        Ok(page)
    }
    /// The most recent successful read, if any.
    pub fn last_read(&self) -> Option<&Event> {
        self.events
            .iter()
            .find(|e| e.outcome == "ok" && matches!(e.kind.as_str(), "context" | "handoff"))
    }
}

/// `HH:MM:SS UTC` of a Unix millisecond time.
pub fn clock(ms: u64) -> String {
    format!(
        "{:02}:{:02}:{:02} UTC",
        (ms / 3_600_000) % 24,
        (ms / 60_000) % 60,
        (ms / 1000) % 60
    )
}

/// `YYYY-MM-DD HH:MM UTC` of a Unix millisecond time (civil from days).
pub fn date_time(ms: u64) -> String {
    let days = (ms / 86_400_000) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        (ms / 3_600_000) % 24,
        (ms / 60_000) % 60
    )
}

impl crate::Model {
    /// What happened, in the operator's words.
    pub fn activity_summary(&self, e: &Event) -> String {
        let ok = e.outcome == "ok";
        match (e.kind.as_str(), ok) {
            ("started", true) => self.word(Msg::ActivityStarted).to_owned(),
            ("started", false) => self.word(Msg::ActivityStartFailed).to_owned(),
            ("context", true) => {
                let c = e.counts.clone().unwrap_or_default();
                self.word(Msg::ActivityContext)
                    .replace("{preferences}", &c.preferences.to_string())
                    .replace("{contracts}", &c.contracts.to_string())
                    .replace("{documents}", &c.documents.to_string())
            }
            ("context", false) => self.word(Msg::ActivityContextFailed).to_owned(),
            ("handoff", true) => self.word(Msg::ActivityHandoff).to_owned(),
            ("handoff", false) if e.outcome == "NotFound" => {
                self.word(Msg::ActivityHandoffMissing).to_owned()
            }
            ("handoff", false) => self.word(Msg::ActivityHandoffFailed).to_owned(),
            ("refused", _) => self.word(Msg::ActivityRefused).to_owned(),
            _ => self.word(Msg::ActivityOther).to_owned(),
        }
    }
    /// Who asked: the client's own name, else how it arrived.
    pub fn activity_who(&self, e: &Event) -> String {
        match (&e.client, e.via.as_str()) {
            (Some(c), _) => c.clone(),
            (None, "session") => self.word(Msg::ActivityViaSession).to_owned(),
            (None, "unknown") => self.word(Msg::ActivityUnknownClient).to_owned(),
            (None, _) => self.word(Msg::ActivityViaMcp).to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pages_parse_bounded_and_read_in_words() {
        let page = Page::parse(json!({"daemon_started_ms":1_763_640_000_000u64,"capacity":500,"more":false,"events":[
            {"id":3,"at_ms":1_763_640_061_000u64,"kind":"context","via":"mcp","project":"alpha",
             "client":"agy\u{7} 1.2","outcome":"ok","duration_ms":4,
             "counts":{"preferences":2,"contracts":0,"documents":3,"contract_status":"ready"}},
            {"id":2,"at_ms":1_763_640_060_000u64,"kind":"started","via":"mcp","project":"alpha",
             "client":null,"outcome":"ok","duration_ms":1},
            {"id":1,"at_ms":1_763_640_001_000u64,"kind":"refused","via":"unknown","project":null,
             "client":null,"outcome":"Unauthorized","duration_ms":0}
        ]}))
        .unwrap();
        assert_eq!(page.events[0].client.as_deref(), Some("agy 1.2"));
        assert_eq!(page.last_read().unwrap().id, 3);
        let m = crate::Model::loading();
        assert_eq!(
            m.activity_summary(&page.events[0]),
            "Read context: 2 preferences, 0 contracts, 3 documents"
        );
        assert_eq!(m.activity_who(&page.events[1]), "MCP client");
        assert!(m.activity_summary(&page.events[2]).contains("refused"));
        assert_eq!(clock(page.events[0].at_ms), "12:01:01 UTC");
        assert_eq!(date_time(1_763_640_000_000), "2025-11-20 12:00 UTC");
        assert!(Page::parse(json!({"events":"x"})).is_err());
    }
}
