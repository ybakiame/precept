//! Work sessions as the daemon reports them (`sessions.list`): agents' own
//! structured coordination notes. Parsed and bounded here; the Sessions page
//! shows the items and the operator can delete a session.
use crate::presentation::sanitize;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WorkItemKind {
    Milestone,
    Task,
}

impl WorkItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Milestone => "milestone",
            Self::Task => "task",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WorkItemState {
    Todo,
    Doing,
    Blocked,
    Deferred,
    Done,
}

impl WorkItemState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Doing => "doing",
            Self::Blocked => "blocked",
            Self::Deferred => "deferred",
            Self::Done => "done",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct WorkItem {
    pub id: String,
    pub text: String,
    pub kind: WorkItemKind,
    pub state: WorkItemState,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub blocked_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct WorkSession {
    pub id: String,
    pub agent: Option<String>,
    pub conversation: Option<String>,
    pub status: String,
    pub started_ms: u64,
    pub updated_ms: u64,
    pub closed_ms: Option<u64>,
    pub goal: String,
    pub items: Vec<WorkItem>,
    pub summary: Option<String>,
}

#[derive(Deserialize)]
struct Reply {
    sessions: Vec<WorkSession>,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 24
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Parses and bounds a daemon reply; every text is sanitized for display.
pub fn parse(v: serde_json::Value) -> Result<Vec<WorkSession>, &'static str> {
    let raw = serde_json::from_value::<Reply>(v).map_err(|_| "Unavailable")?;
    if raw.sessions.len() > 50 {
        return Err("Unavailable");
    }
    let mut list = raw.sessions;
    for session in &mut list {
        session.id = sanitize(&session.id, 32);
        session.agent = session.agent.as_deref().map(|a| sanitize(a, 64));
        session.goal = sanitize(&session.goal, 500);
        if session.items.len() > 36 {
            return Err("Unavailable");
        }
        for item in &mut session.items {
            if !valid_id(&item.id) {
                return Err("Unavailable");
            }
            item.text = sanitize(&item.text, 200);
            if item.depends_on.len() > 36 || item.depends_on.iter().any(|id| !valid_id(id)) {
                return Err("Unavailable");
            }
            item.owner = item.owner.as_deref().map(|owner| sanitize(owner, 64));
            item.blocked_reason = item
                .blocked_reason
                .as_deref()
                .map(|reason| sanitize(reason, 200));
        }
        session.summary = session.summary.as_deref().map(|t| sanitize(t, 1000));
    }
    Ok(list)
}

/// The Sessions page's state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionsPage {
    /// The project the loaded list belongs to.
    pub project: Option<String>,
    pub list: Option<Vec<WorkSession>>,
    pub busy: bool,
    pub error: Option<String>,
    /// The session whose detail is open.
    pub open: Option<String>,
    /// The session armed for deletion (confirmation shown).
    pub delete: Option<String>,
    /// The session being saved as a Handoff, and the task id typed for it.
    pub handoff: Option<String>,
    pub task: String,
}

/// A Handoff task id from a goal: lowercase ASCII words joined by hyphens.
pub fn task_id(goal: &str) -> String {
    let mut out = String::new();
    for c in goal.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= 60 {
            break;
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "handoff".into()
    } else {
        out
    }
}

impl SessionsPage {
    pub fn opened(&self) -> Option<&WorkSession> {
        let id = self.open.as_deref()?;
        self.list.as_ref()?.iter().find(|s| s.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_bounds_and_sanitizes_structured_items() {
        let reply = json!({"sessions":[{"id":"ab","agent":"claude\u{7}","conversation":null,"status":"open",
            "started_ms":1,"updated_ms":2,"closed_ms":null,"goal":"Add import","items":[
                {"id":"t1","text":"tests\u{7}","kind":"task","state":"doing","owner":"agent\u{7}"},
                {"id":"d1","text":"docs","kind":"milestone","state":"todo","depends_on":["t1"]}
            ],"summary":null}]});
        let list = parse(reply).unwrap();
        assert_eq!(list[0].agent.as_deref(), Some("claude"));
        assert_eq!(list[0].items[0].text, "tests");
        assert_eq!(list[0].items[0].owner.as_deref(), Some("agent"));
        assert_eq!(list[0].items[1].kind, WorkItemKind::Milestone);
        assert!(parse(json!({"sessions":"x"})).is_err());
    }
}
