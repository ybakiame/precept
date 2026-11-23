//! Work sessions as the daemon reports them (`sessions.list`): agents' own
//! notes on what they are doing in one project. Parsed and bounded here;
//! the Sessions page shows them and the operator can delete one.
use crate::presentation::sanitize;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct WorkSession {
    pub id: String,
    pub agent: Option<String>,
    pub conversation: Option<String>,
    /// `open` or `closed`.
    pub status: String,
    pub started_ms: u64,
    pub updated_ms: u64,
    pub closed_ms: Option<u64>,
    pub goal: String,
    pub done: Vec<String>,
    pub doing: Vec<String>,
    pub todo: Vec<String>,
    pub summary: Option<String>,
}

#[derive(Deserialize)]
struct RawItem {
    text: String,
}

/// A session as the daemon reports it: work items are objects; the page
/// shows their text.
#[derive(Deserialize)]
struct RawSession {
    id: String,
    agent: Option<String>,
    conversation: Option<String>,
    status: String,
    started_ms: u64,
    updated_ms: u64,
    closed_ms: Option<u64>,
    goal: String,
    done: Vec<RawItem>,
    doing: Vec<RawItem>,
    todo: Vec<RawItem>,
    summary: Option<String>,
}

#[derive(Deserialize)]
struct Reply {
    sessions: Vec<RawSession>,
}

/// Parses and bounds a daemon reply; every text is sanitized for display.
pub fn parse(v: serde_json::Value) -> Result<Vec<WorkSession>, &'static str> {
    let raw = serde_json::from_value::<Reply>(v).map_err(|_| "Unavailable")?;
    if raw.sessions.len() > 50 {
        return Err("Unavailable");
    }
    let texts = |items: Vec<RawItem>| items.into_iter().map(|i| i.text).collect();
    let mut list: Vec<WorkSession> = raw
        .sessions
        .into_iter()
        .map(|s| WorkSession {
            id: s.id,
            agent: s.agent,
            conversation: s.conversation,
            status: s.status,
            started_ms: s.started_ms,
            updated_ms: s.updated_ms,
            closed_ms: s.closed_ms,
            goal: s.goal,
            done: texts(s.done),
            doing: texts(s.doing),
            todo: texts(s.todo),
            summary: s.summary,
        })
        .collect();
    for s in &mut list {
        s.id = sanitize(&s.id, 32);
        s.agent = s.agent.as_deref().map(|a| sanitize(a, 64));
        s.goal = sanitize(&s.goal, 500);
        for l in [&mut s.done, &mut s.doing, &mut s.todo] {
            l.truncate(12);
            for i in l.iter_mut() {
                *i = sanitize(i, 200);
            }
        }
        s.summary = s.summary.as_deref().map(|t| sanitize(t, 1000));
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
    fn parses_bounds_and_sanitizes() {
        let reply = json!({"sessions":[{"id":"ab","agent":"claude\u{7}","conversation":null,"status":"open",
            "started_ms":1,"updated_ms":2,"closed_ms":null,"goal":"Add import","done":[],
            "doing":[{"id":"t1","text":"tests"}],
            "todo":[{"id":"d1","text":"docs"}],"summary":null,"rev":2}]});
        let list = parse(reply).unwrap();
        assert_eq!(list[0].agent.as_deref(), Some("claude"));
        assert_eq!(list[0].doing, ["tests"]);
        assert!(parse(json!({"sessions":"x"})).is_err());
    }
}
