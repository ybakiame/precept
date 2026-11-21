//! Observed work sessions: what an agent is doing in one project (its goal and
//! done / doing / todo lists), written by the agents themselves through MCP and
//! read back by every agent of the project at session start, so work moves
//! between agents and survives compaction. Agents' own records, not reviewed
//! rules: kept in the daemon's private state (one file per project), never in
//! the Profile, bounded, and deleted by the operator at will. Writes need the
//! operator's per-project permission for agent writes (`project.propose`).
use crate::state::StateDir;
use precept_ipc::Error;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// The most sessions kept per project (closed ones go first).
pub const MAX_SESSIONS: usize = 20;
pub const MAX_GOAL: usize = 500;
pub const MAX_ITEMS: usize = 12;
pub const MAX_ITEM: usize = 200;
pub const MAX_SUMMARY: usize = 1000;
/// The most sessions an agent's context carries.
pub const IN_CONTEXT: usize = 5;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub id: String,
    /// The agent's self-reported client name.
    pub agent: Option<String>,
    /// The harness's conversation id, to find the session again.
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

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    format: u32,
    sessions: Vec<Session>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Bounded text: trimmed, no control characters but line breaks; `None` when
/// empty, an error when too long.
fn text(raw: &str, max: usize) -> Result<Option<String>, Error> {
    let t = raw.trim();
    if t.chars().count() > max || t.chars().any(|c| c.is_control() && c != '\n') {
        return Err(Error::InvalidRequest);
    }
    Ok((!t.is_empty()).then(|| t.to_owned()))
}
fn items(raw: &[String]) -> Result<Vec<String>, Error> {
    if raw.len() > MAX_ITEMS {
        return Err(Error::InvalidRequest);
    }
    raw.iter()
        .map(|i| text(i, MAX_ITEM))
        .filter_map(Result::transpose)
        .collect()
}

/// What an agent sends to start or update a session.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub conversation: Option<String>,
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default)]
    pub done: Option<Vec<String>>,
    #[serde(default)]
    pub doing: Option<Vec<String>>,
    #[serde(default)]
    pub todo: Option<Vec<String>>,
    #[serde(default)]
    pub summary: Option<String>,
    /// The MCP client's self-reported name (also the activity label).
    #[serde(default)]
    pub client: Option<String>,
}

/// The session store; one lock for every project's file.
pub struct Sessions {
    lock: Mutex<()>,
}

fn file_name(project: &str) -> String {
    format!("sessions-{project}.json")
}

impl Sessions {
    pub fn new() -> Self {
        Self {
            lock: Mutex::new(()),
        }
    }
    fn load(state: &StateDir, project: &str) -> Result<Vec<Session>, Error> {
        let Some(text) = state.read(&file_name(project))? else {
            return Ok(Vec::new());
        };
        let file: File = serde_json::from_str(&text).map_err(|_| Error::InvalidRequest)?;
        if file.format != 1 || file.sessions.len() > MAX_SESSIONS {
            return Err(Error::InvalidRequest);
        }
        Ok(file.sessions)
    }
    fn save(state: &StateDir, project: &str, mut sessions: Vec<Session>) -> Result<(), Error> {
        // Newest first; past the bound, closed sessions go before open ones.
        sessions.sort_by_key(|s| std::cmp::Reverse(s.updated_ms));
        while sessions.len() > MAX_SESSIONS {
            let at = sessions
                .iter()
                .rposition(|s| s.status == "closed")
                .unwrap_or(sessions.len() - 1);
            sessions.remove(at);
        }
        let body = serde_json::to_vec_pretty(&File {
            format: 1,
            sessions,
        })
        .map_err(|_| Error::Unavailable)?;
        state.write(&file_name(project), &body)
    }
    /// Every session of `project`, newest first (the operator's read).
    pub fn list(&self, state: &StateDir, project: &str) -> Result<Vec<Session>, Error> {
        let _held = self.lock.lock().unwrap();
        let mut s = Self::load(state, project)?;
        s.sort_by_key(|x| std::cmp::Reverse(x.updated_ms));
        Ok(s)
    }
    /// What an agent's context carries: open sessions, then recently closed
    /// ones, newest first, at most [`IN_CONTEXT`].
    pub fn for_context(&self, state: &StateDir, project: &str) -> Vec<Session> {
        let mut s = self.list(state, project).unwrap_or_default();
        s.sort_by_key(|x| (x.status != "open", std::cmp::Reverse(x.updated_ms)));
        s.truncate(IN_CONTEXT);
        s
    }
    /// Starts a session, or returns the open one this agent and conversation
    /// already has (updating its goal when one is given).
    pub fn start(&self, state: &StateDir, project: &str, u: Update) -> Result<Session, Error> {
        let goal = text(u.goal.as_deref().unwrap_or(""), MAX_GOAL)?.ok_or(Error::InvalidRequest)?;
        let conversation = match u.conversation.as_deref() {
            Some(c) => text(c, 128)?,
            None => None,
        };
        let agent = u.client.as_deref().and_then(crate::activity::client_label);
        let _held = self.lock.lock().unwrap();
        let mut all = Self::load(state, project)?;
        let now = now_ms();
        if conversation.is_some()
            && let Some(s) = all
                .iter_mut()
                .find(|s| s.status == "open" && s.conversation == conversation && s.agent == agent)
        {
            s.goal = goal;
            s.updated_ms = now;
            let found = s.clone();
            Self::save(state, project, all)?;
            return Ok(found);
        }
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).map_err(|_| Error::Unavailable)?;
        let session = Session {
            id: nonce.iter().map(|b| format!("{b:02x}")).collect(),
            agent,
            conversation,
            status: "open".into(),
            started_ms: now,
            updated_ms: now,
            closed_ms: None,
            goal,
            done: items(u.done.as_deref().unwrap_or(&[]))?,
            doing: items(u.doing.as_deref().unwrap_or(&[]))?,
            todo: items(u.todo.as_deref().unwrap_or(&[]))?,
            summary: None,
        };
        all.push(session.clone());
        Self::save(state, project, all)?;
        Ok(session)
    }
    /// Replaces the given parts of an open session's work state; with
    /// `close`, also closes it (with its summary).
    pub fn update(
        &self,
        state: &StateDir,
        project: &str,
        u: Update,
        close: bool,
    ) -> Result<Session, Error> {
        let id = u.id.as_deref().ok_or(Error::InvalidRequest)?;
        let _held = self.lock.lock().unwrap();
        let mut all = Self::load(state, project)?;
        let s = all.iter_mut().find(|s| s.id == id).ok_or(Error::NotFound)?;
        if s.status != "open" {
            return Err(Error::InvalidTransition);
        }
        if let Some(g) = &u.goal {
            s.goal = text(g, MAX_GOAL)?.ok_or(Error::InvalidRequest)?;
        }
        if let Some(v) = &u.done {
            s.done = items(v)?;
        }
        if let Some(v) = &u.doing {
            s.doing = items(v)?;
        }
        if let Some(v) = &u.todo {
            s.todo = items(v)?;
        }
        let now = now_ms();
        s.updated_ms = now;
        if close {
            s.status = "closed".into();
            s.closed_ms = Some(now);
            s.doing.clear();
            if let Some(sum) = &u.summary {
                s.summary = text(sum, MAX_SUMMARY)?;
            }
        }
        let out = s.clone();
        Self::save(state, project, all)?;
        Ok(out)
    }
    /// The operator deletes one session.
    pub fn delete(&self, state: &StateDir, project: &str, id: &str) -> Result<(), Error> {
        let _held = self.lock.lock().unwrap();
        let mut all = Self::load(state, project)?;
        let before = all.len();
        all.retain(|s| s.id != id);
        if all.len() == before {
            return Err(Error::NotFound);
        }
        Self::save(state, project, all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> (tempfile::TempDir, StateDir) {
        let dir = tempfile::tempdir().unwrap();
        let state = StateDir::open(&dir.path().join("state")).unwrap();
        (dir, state)
    }
    fn update(id: &str) -> Update {
        Update {
            id: Some(id.into()),
            ..Default::default()
        }
    }

    #[test]
    fn start_finds_the_open_session_again_updates_and_closes() {
        let (_d, st) = state();
        let s = Sessions::new();
        let start = |goal: &str| Update {
            goal: Some(goal.into()),
            conversation: Some("conv-1".into()),
            client: Some("claude-code 2.1".into()),
            ..Default::default()
        };
        let a = s.start(&st, "alpha", start("Add import")).unwrap();
        let again = s
            .start(&st, "alpha", start("Add import from files"))
            .unwrap();
        assert_eq!(a.id, again.id, "same agent and conversation");
        assert_eq!(again.goal, "Add import from files");
        let other = s
            .start(
                &st,
                "alpha",
                Update {
                    goal: Some("Other".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_ne!(other.id, a.id);
        let mut u = update(&a.id);
        u.doing = Some(vec!["Writing tests".into(), "  ".into()]);
        u.todo = Some(vec!["Docs".into()]);
        let w = s.update(&st, "alpha", u, false).unwrap();
        assert_eq!(w.doing, ["Writing tests"]);
        let mut c = update(&a.id);
        c.summary = Some("Done; docs left.".into());
        let closed = s.update(&st, "alpha", c, true).unwrap();
        assert_eq!(closed.status, "closed");
        assert!(closed.doing.is_empty());
        assert_eq!(
            s.update(&st, "alpha", update(&a.id), false).err(),
            Some(Error::InvalidTransition)
        );
        // Open first in context.
        let ctx = s.for_context(&st, "alpha");
        assert_eq!(ctx[0].id, other.id);
        assert!(s.list(&st, "beta").unwrap().is_empty(), "per project");
        s.delete(&st, "alpha", &a.id).unwrap();
        assert_eq!(s.list(&st, "alpha").unwrap().len(), 1);
    }

    #[test]
    fn bounds_are_enforced_and_closed_sessions_go_first() {
        let (_d, st) = state();
        let s = Sessions::new();
        let goal = |g: String| Update {
            goal: Some(g),
            ..Default::default()
        };
        assert_eq!(
            s.start(&st, "alpha", goal("x".repeat(MAX_GOAL + 1))).err(),
            Some(Error::InvalidRequest)
        );
        assert_eq!(
            s.start(&st, "alpha", goal(String::new())).err(),
            Some(Error::InvalidRequest)
        );
        let mut too_many = goal("g".into());
        too_many.todo = Some(vec!["t".into(); MAX_ITEMS + 1]);
        assert_eq!(
            s.start(&st, "alpha", too_many).err(),
            Some(Error::InvalidRequest)
        );
        let first = s.start(&st, "alpha", goal("first".into())).unwrap();
        s.update(&st, "alpha", update(&first.id), true).unwrap();
        for i in 0..MAX_SESSIONS {
            std::thread::sleep(std::time::Duration::from_millis(2));
            s.start(&st, "alpha", goal(format!("g{i}"))).unwrap();
        }
        let all = s.list(&st, "alpha").unwrap();
        assert_eq!(all.len(), MAX_SESSIONS);
        assert!(
            all.iter().all(|x| x.status == "open"),
            "the closed one went first"
        );
    }
}
