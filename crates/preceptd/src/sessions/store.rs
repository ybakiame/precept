use super::model::{
    self, IN_CONTEXT, Item, ItemState, LegacySession, MAX_GOAL, MAX_ITEM, MAX_ITEMS, MAX_SESSIONS,
    MAX_SUMMARY, Operation, Session, Update, invalid, required_text, valid_item_id,
};
use crate::state::StateDir;
use precept_ipc::Error;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Deserialize)]
struct Version {
    format: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileV2 {
    format: u32,
    sessions: Vec<Session>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileV1 {
    #[serde(rename = "format")]
    _format: u32,
    sessions: Vec<LegacySession>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn generated_id() -> Result<String, Error> {
    let mut nonce = [0_u8; 4];
    getrandom::fill(&mut nonce).map_err(|_| Error::Unavailable)?;
    Ok(nonce.iter().map(|b| format!("{b:02x}")).collect())
}

fn unique_id(items: &[Item]) -> Result<String, Error> {
    for _ in 0..32 {
        let id = generated_id()?;
        if !items.iter().any(|item| item.id == id) {
            return Ok(id);
        }
    }
    Err(Error::Unavailable)
}

fn file_name(project: &str) -> String {
    format!("sessions-{project}.json")
}

pub struct Sessions {
    lock: Mutex<()>,
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
        let version: Version = serde_json::from_str(&text).map_err(|_| Error::Unavailable)?;
        let mut sessions = match version.format {
            1 => {
                let file: FileV1 = serde_json::from_str(&text).map_err(|_| Error::Unavailable)?;
                file.sessions
                    .into_iter()
                    .map(LegacySession::into_session)
                    .collect()
            }
            2 => {
                let file: FileV2 = serde_json::from_str(&text).map_err(|_| Error::Unavailable)?;
                file.sessions
            }
            _ => return Err(Error::Unavailable),
        };
        if sessions.len() > MAX_SESSIONS {
            return Err(Error::Unavailable);
        }
        for session in &mut sessions {
            for (index, item) in session.items.iter_mut().enumerate() {
                if let Some(owner) = &item.owner {
                    item.owner = model::owner(owner, &format!("items[{index}].owner"))?;
                }
            }
            model::validate_items(&session.items)?;
        }
        Ok(sessions)
    }

    fn save(
        state: &StateDir,
        project: &str,
        mut sessions: Vec<Session>,
        keep: Option<&str>,
    ) -> Result<(), Error> {
        sessions.sort_by_key(|s| std::cmp::Reverse(s.updated_ms));
        while sessions.len() > MAX_SESSIONS {
            let at = sessions
                .iter()
                .rposition(|s| s.status == "closed" && Some(s.id.as_str()) != keep)
                .or_else(|| sessions.iter().rposition(|s| Some(s.id.as_str()) != keep))
                .ok_or(Error::Unavailable)?;
            sessions.remove(at);
        }
        let body = serde_json::to_vec_pretty(&FileV2 {
            format: 2,
            sessions,
        })
        .map_err(|_| Error::Unavailable)?;
        state.write(&file_name(project), &body)
    }

    pub fn list(&self, state: &StateDir, project: &str) -> Result<Vec<Session>, Error> {
        let _held = self.lock.lock().unwrap();
        let mut sessions = Self::load(state, project)?;
        sessions.sort_by_key(|s| std::cmp::Reverse(s.updated_ms));
        Ok(sessions)
    }

    pub fn get(&self, state: &StateDir, project: &str, id: &str) -> Result<Session, Error> {
        let _held = self.lock.lock().unwrap();
        Self::load(state, project)?
            .into_iter()
            .find(|session| session.id == id)
            .ok_or(Error::NotFound)
    }

    pub fn for_context(&self, state: &StateDir, project: &str) -> Vec<Session> {
        let mut sessions = self.list(state, project).unwrap_or_default();
        sessions.sort_by_key(|s| (s.status != "open", std::cmp::Reverse(s.updated_ms)));
        sessions.truncate(IN_CONTEXT);
        sessions
    }

    pub fn start(&self, state: &StateDir, project: &str, update: Update) -> Result<Session, Error> {
        if update.operations.is_some() {
            return Err(invalid("operations", "not allowed when starting a session"));
        }
        if update.summary.is_some() {
            return Err(invalid("summary", "not allowed when starting a session"));
        }
        let goal = required_text(update.goal.as_deref().unwrap_or(""), MAX_GOAL, "goal")?;
        let conversation = update
            .conversation
            .as_deref()
            .map(|value| model::text(value, 128, "conversation_id"))
            .transpose()?
            .flatten();
        if update.id.is_some() && conversation.is_some() {
            return Err(invalid(
                "conversation_id",
                "cannot be combined with resume_session_id",
            ));
        }
        let agent = update
            .client
            .as_deref()
            .and_then(crate::activity::client_label);
        let has_items = update.items.is_some();
        let initial = update.items.unwrap_or_default();
        let _held = self.lock.lock().unwrap();
        let mut all = Self::load(state, project)?;
        let now = now_ms();
        if let Some(id) = update.id.as_deref() {
            if has_items {
                return Err(invalid(
                    "items",
                    "not allowed when resuming an existing session",
                ));
            }
            let expected = update
                .expected_rev
                .ok_or_else(|| invalid("expected_rev", "required when resuming a session"))?;
            let session = all
                .iter_mut()
                .find(|session| session.id == id)
                .ok_or(Error::NotFound)?;
            if session.status != "open" {
                return Err(Error::InvalidTransition);
            }
            if expected != session.rev {
                return Err(Error::Conflict);
            }
            session.goal = goal;
            session.updated_ms = now;
            session.rev += 1;
            let found = session.clone();
            Self::save(state, project, all, Some(&found.id))?;
            return Ok(found);
        }
        if conversation.is_some()
            && let Some(session) = all.iter_mut().find(|session| {
                session.status == "open"
                    && session.conversation == conversation
                    && session.agent == agent
            })
        {
            if has_items {
                return Err(invalid(
                    "items",
                    "not allowed when resuming a conversation; update the existing session",
                ));
            }
            if update.expected_rev.is_some_and(|rev| rev != session.rev) {
                return Err(Error::Conflict);
            }
            session.goal = goal;
            session.updated_ms = now;
            session.rev += 1;
            let found = session.clone();
            Self::save(state, project, all, Some(&found.id))?;
            return Ok(found);
        }
        if update.expected_rev.is_some() {
            return Err(invalid(
                "expected_rev",
                "only allowed when resuming an existing session",
            ));
        }
        if initial.len() > MAX_ITEMS {
            return Err(invalid("items", format!("at most {MAX_ITEMS} items")));
        }
        let mut items = Vec::with_capacity(initial.len());
        for new in initial {
            let id = match new.id.as_deref() {
                Some(id) if valid_item_id(id) => id.to_owned(),
                Some(_) => {
                    return Err(invalid(
                        "items.id",
                        "use at most 24 lowercase letters, digits or hyphens",
                    ));
                }
                None => unique_id(&items)?,
            };
            items.push(new.into_item(id, agent.clone()));
        }
        model::validate_items(&items)?;
        let mut nonce = [0_u8; 8];
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
            items,
            summary: None,
            rev: 1,
        };
        all.push(session.clone());
        Self::save(state, project, all, Some(&session.id))?;
        Ok(session)
    }

    pub fn update(
        &self,
        state: &StateDir,
        project: &str,
        update: Update,
        close: bool,
    ) -> Result<Session, Error> {
        let id = update
            .id
            .as_deref()
            .ok_or_else(|| invalid("session_id", "required"))?;
        let expected = update
            .expected_rev
            .ok_or_else(|| invalid("expected_rev", "required"))?;
        if update.items.is_some() {
            return Err(invalid(
                "items",
                "not allowed when updating a session; use an add operation",
            ));
        }
        if update.conversation.is_some() {
            return Err(invalid(
                "conversation_id",
                "not allowed when updating a session",
            ));
        }
        if close {
            if update.goal.is_some() {
                return Err(invalid("goal", "not allowed when closing a session"));
            }
            if update.operations.is_some() {
                return Err(invalid("operations", "not allowed when closing a session"));
            }
        } else if update.summary.is_some() {
            return Err(invalid("summary", "only allowed when closing a session"));
        }
        let _held = self.lock.lock().unwrap();
        let mut all = Self::load(state, project)?;
        let session = all
            .iter_mut()
            .find(|session| session.id == id)
            .ok_or(Error::NotFound)?;
        if session.status != "open" {
            return Err(Error::InvalidTransition);
        }
        if expected != session.rev {
            return Err(Error::Conflict);
        }
        if let Some(goal) = &update.goal {
            session.goal = required_text(goal, MAX_GOAL, "goal")?;
        }
        let agent = update
            .client
            .as_deref()
            .and_then(crate::activity::client_label);
        if let Some(operations) = &update.operations {
            for (index, operation) in operations.iter().enumerate() {
                apply_operation(&mut session.items, operation, agent.as_deref(), index)?;
            }
        }
        if update.goal.is_none()
            && update.operations.as_ref().is_none_or(|ops| ops.is_empty())
            && !close
        {
            return Err(invalid("request", "no changes supplied"));
        }
        model::validate_items(&session.items)?;
        let now = now_ms();
        session.updated_ms = now;
        session.rev += 1;
        if close {
            session.status = "closed".into();
            session.closed_ms = Some(now);
            for item in &mut session.items {
                if item.state == ItemState::Doing {
                    item.state = ItemState::Todo;
                    item.blocked_reason = None;
                }
            }
            if let Some(summary) = &update.summary {
                session.summary = model::text(summary, MAX_SUMMARY, "summary")?;
            }
        }
        let out = session.clone();
        Self::save(state, project, all, Some(&out.id))?;
        Ok(out)
    }

    pub fn delete(&self, state: &StateDir, project: &str, id: &str) -> Result<(), Error> {
        let _held = self.lock.lock().unwrap();
        let mut sessions = Self::load(state, project)?;
        let before = sessions.len();
        sessions.retain(|session| session.id != id);
        if sessions.len() == before {
            return Err(Error::NotFound);
        }
        Self::save(state, project, sessions, None)
    }
}

fn apply_operation(
    items: &mut Vec<Item>,
    operation: &Operation,
    default_owner: Option<&str>,
    index: usize,
) -> Result<(), Error> {
    let field = format!("operations[{index}]");
    match operation {
        Operation::Add { item } => {
            if items.len() >= MAX_ITEMS {
                return Err(invalid(field, format!("at most {MAX_ITEMS} items")));
            }
            let id = match item.id.as_deref() {
                Some(id) if valid_item_id(id) => id.to_owned(),
                Some(_) => {
                    return Err(invalid(
                        format!("{field}.item.id"),
                        "use at most 24 lowercase letters, digits or hyphens",
                    ));
                }
                None => unique_id(items)?,
            };
            items.push(item.clone().into_item(id, default_owner.map(str::to_owned)));
        }
        Operation::Update {
            id,
            text,
            kind,
            depends_on,
            owner,
            blocked_reason,
        } => {
            let item = items
                .iter_mut()
                .find(|item| &item.id == id)
                .ok_or_else(|| invalid(format!("{field}.id"), "item not found"))?;
            if let Some(value) = text {
                item.text = required_text(value, MAX_ITEM, &format!("{field}.text"))?;
            }
            if let Some(value) = kind {
                item.kind = *value;
            }
            if let Some(value) = depends_on {
                item.depends_on = value.clone();
            }
            if let Some(value) = owner {
                item.owner = model::owner(value, &format!("{field}.owner"))?;
            }
            if let Some(value) = blocked_reason {
                item.blocked_reason = Some(required_text(
                    value,
                    MAX_ITEM,
                    &format!("{field}.blocked_reason"),
                )?);
            }
        }
        Operation::Start { id } => {
            let index = items
                .iter()
                .position(|item| &item.id == id)
                .ok_or_else(|| invalid(format!("{field}.id"), "item not found"))?;
            if !matches!(
                items[index].state,
                ItemState::Todo | ItemState::Blocked | ItemState::Deferred
            ) {
                return Err(invalid(
                    format!("{field}.id"),
                    "only todo, blocked or deferred items can start",
                ));
            }
            if !items[index].depends_on.iter().all(|dep| {
                items
                    .iter()
                    .any(|item| &item.id == dep && item.state == ItemState::Done)
            }) {
                return Err(invalid(
                    format!("{field}.id"),
                    "dependencies must be done before start",
                ));
            }
            let item = &mut items[index];
            item.state = ItemState::Doing;
            item.blocked_reason = None;
            if item.owner.is_none() {
                item.owner = default_owner.map(str::to_owned);
            }
        }
        Operation::Complete { id } => {
            let index = items
                .iter()
                .position(|item| &item.id == id)
                .ok_or_else(|| invalid(format!("{field}.id"), "item not found"))?;
            if items[index].state == ItemState::Done {
                return Err(invalid(format!("{field}.id"), "item is already done"));
            }
            if !items[index].depends_on.iter().all(|dep| {
                items
                    .iter()
                    .any(|item| &item.id == dep && item.state == ItemState::Done)
            }) {
                return Err(invalid(
                    format!("{field}.id"),
                    "dependencies must be done before complete",
                ));
            }
            let item = &mut items[index];
            item.state = ItemState::Done;
            item.blocked_reason = None;
        }
        Operation::Defer { id } => {
            let item = items
                .iter_mut()
                .find(|item| &item.id == id)
                .ok_or_else(|| invalid(format!("{field}.id"), "item not found"))?;
            if item.state == ItemState::Done {
                return Err(invalid(
                    format!("{field}.id"),
                    "done items cannot be deferred; reopen first",
                ));
            }
            item.state = ItemState::Deferred;
            item.blocked_reason = None;
        }
        Operation::Block { id, reason } => {
            let reason = required_text(reason, MAX_ITEM, &format!("{field}.reason"))?;
            let item = items
                .iter_mut()
                .find(|item| &item.id == id)
                .ok_or_else(|| invalid(format!("{field}.id"), "item not found"))?;
            if item.state == ItemState::Done {
                return Err(invalid(
                    format!("{field}.id"),
                    "done items cannot be blocked; reopen first",
                ));
            }
            item.state = ItemState::Blocked;
            item.blocked_reason = Some(reason);
        }
        Operation::Reopen { id } => {
            let item = items
                .iter_mut()
                .find(|item| &item.id == id)
                .ok_or_else(|| invalid(format!("{field}.id"), "item not found"))?;
            if !matches!(
                item.state,
                ItemState::Done | ItemState::Blocked | ItemState::Deferred
            ) {
                return Err(invalid(
                    format!("{field}.id"),
                    "only done, blocked or deferred items can reopen",
                ));
            }
            item.state = ItemState::Todo;
            item.blocked_reason = None;
        }
        Operation::Remove { id } => {
            let before = items.len();
            items.retain(|item| &item.id != id);
            if items.len() == before {
                return Err(invalid(format!("{field}.id"), "item not found"));
            }
        }
    }
    Ok(())
}
