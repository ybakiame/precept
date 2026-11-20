//! Agent activity: a bounded, in-memory record of what agents asked this
//! daemon, for the operator to see whether a configured MCP client actually
//! started `acp-mcp` and read its project.
//!
//! Recorded per agent request: when, which project, how it arrived (an
//! external MCP client or an ACP session), the MCP client's self-reported
//! name, what it read, the outcome and counts. Never preference values,
//! selectors, task ids, paths, tokens or response bodies. Refused handshakes
//! are recorded without a project (the daemon cannot know it). Nothing is
//! written to disk; a daemon restart starts an empty log.
use acp_ipc::Error;
use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// The most events kept; older ones are dropped first.
pub const CAPACITY: usize = 500;
/// The most events one `activity.list` returns.
pub const PAGE: usize = 200;
/// The longest client label kept, in characters.
pub const MAX_CLIENT: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// `acp-mcp` started and checked the daemon (`daemon.health`).
    Started,
    /// `get_effective_context` (`context.get`).
    Context,
    /// `get_handoff` (`handoff.get`).
    Handoff,
    /// The connection's credential was refused before any request.
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Via {
    /// An MCP client started `acp-mcp` itself with the project's credential.
    Mcp,
    /// Unknown (a refused handshake).
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub preferences: usize,
    pub contracts: usize,
    pub documents: usize,
    /// `contract_resolution.status` (a fixed word).
    pub contract_status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Event {
    pub id: u64,
    /// Unix milliseconds.
    pub at_ms: u64,
    pub kind: Kind,
    pub via: Via,
    pub project: Option<String>,
    pub client: Option<String>,
    /// `ok`, else the IPC error code.
    pub outcome: String,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts: Option<Counts>,
}

#[derive(Default)]
struct Inner {
    next: u64,
    events: VecDeque<Event>,
}

/// The shared log.
#[derive(Clone)]
pub struct Activity {
    started_ms: u64,
    inner: Arc<Mutex<Inner>>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A client's self-reported name as a bounded, printable label, or none.
pub fn client_label(raw: &str) -> Option<String> {
    let label: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_CLIENT)
        .collect();
    let label = label.trim();
    (!label.is_empty()).then(|| label.to_owned())
}

/// What a context answer contained, by count only.
pub fn context_counts(result: &Value) -> Counts {
    let len = |k: &str| result.get(k).and_then(Value::as_array).map_or(0, Vec::len);
    let status = result
        .pointer("/contract_resolution/status")
        .and_then(Value::as_str)
        .filter(|s| s.len() <= 32 && s.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
        .unwrap_or("unknown");
    Counts {
        preferences: len("preferences"),
        contracts: len("effective_contracts"),
        documents: len("documents"),
        contract_status: status.to_owned(),
    }
}

/// An outcome word: `ok` or the error code.
pub fn outcome<T>(result: &Result<T, Error>) -> String {
    match result {
        Ok(_) => "ok".into(),
        Err(e) => e.to_string(),
    }
}

impl Default for Activity {
    fn default() -> Self {
        Self {
            started_ms: now_ms(),
            inner: Arc::default(),
        }
    }
}

pub struct Record {
    pub kind: Kind,
    pub via: Via,
    pub project: Option<String>,
    pub client: Option<String>,
    pub outcome: String,
    pub duration_ms: u64,
    pub counts: Option<Counts>,
}

impl Activity {
    pub fn record(&self, r: Record) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.next += 1;
        let event = Event {
            id: inner.next,
            at_ms: now_ms(),
            kind: r.kind,
            via: r.via,
            project: r.project,
            client: r.client,
            outcome: r.outcome,
            duration_ms: r.duration_ms,
            counts: r.counts,
        };
        if inner.events.len() == CAPACITY {
            inner.events.pop_front();
        }
        inner.events.push_back(event);
    }

    /// Newest first: the events of `project` (plus refused handshakes, which
    /// carry no project), or every event; at most [`PAGE`], older than
    /// `before` when given.
    pub fn list(&self, project: Option<&str>, before: Option<u64>) -> Value {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut events: Vec<&Event> = inner
            .events
            .iter()
            .rev()
            .filter(|e| before.is_none_or(|b| e.id < b))
            .filter(|e| match project {
                None => true,
                Some(p) => e.project.as_deref().is_none_or(|q| q == p),
            })
            .take(PAGE + 1)
            .collect();
        let more = events.len() > PAGE;
        events.truncate(PAGE);
        serde_json::json!({
            "daemon_started_ms": self.started_ms,
            "capacity": CAPACITY,
            "events": events,
            "more": more,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rec(project: Option<&str>, kind: Kind) -> Record {
        Record {
            kind,
            via: Via::Mcp,
            project: project.map(str::to_owned),
            client: None,
            outcome: "ok".into(),
            duration_ms: 1,
            counts: None,
        }
    }

    #[test]
    fn bounded_newest_first_and_filtered_by_project() {
        let log = Activity::default();
        for i in 0..CAPACITY + 10 {
            log.record(rec(Some(if i % 2 == 0 { "a" } else { "b" }), Kind::Context));
        }
        log.record(rec(None, Kind::Refused));
        assert_eq!(log.inner.lock().unwrap().events.len(), CAPACITY);
        let page = log.list(Some("a"), None);
        let events = page["events"].as_array().unwrap();
        assert_eq!(events.len(), PAGE);
        assert_eq!(page["more"], json!(true));
        assert_eq!(events[0]["kind"], json!("refused"));
        assert!(events[1..].iter().all(|e| e["project"] == json!("a")));
        let ids: Vec<u64> = events.iter().map(|e| e["id"].as_u64().unwrap()).collect();
        assert!(ids.windows(2).all(|w| w[0] > w[1]));
        let older = log.list(Some("a"), Some(*ids.last().unwrap()));
        assert!(older["events"][0]["id"].as_u64().unwrap() < *ids.last().unwrap());
    }

    #[test]
    fn labels_and_counts_carry_no_content() {
        assert_eq!(client_label("  Antigravity\n"), Some("Antigravity".into()));
        assert_eq!(client_label("\u{7}"), None);
        assert_eq!(client_label(&"x".repeat(200)).unwrap().len(), MAX_CLIENT);
        let c = context_counts(&json!({
            "preferences":[{"value":"secret"}],"effective_contracts":[],
            "documents":[{},{}],"contract_resolution":{"status":"not_configured"}
        }));
        assert_eq!(
            c,
            Counts {
                preferences: 1,
                contracts: 0,
                documents: 2,
                contract_status: "not_configured".into()
            }
        );
        let weird = context_counts(&json!({"contract_resolution":{"status":"<x>"}}));
        assert_eq!(weird.contract_status, "unknown");
    }
}
