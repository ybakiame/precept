use super::model::{Item, ItemState, Session, present};
use crate::{Error, Value, decode, json, worktree};
use precept_core::WorktreeInspector;
use precept_ipc::Access;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Get {
    id: String,
    #[serde(default, deserialize_with = "present")]
    #[allow(dead_code)]
    client: Option<String>,
}

pub fn handle(
    service: &crate::Service,
    authority: &precept_ipc::peer::Peer,
    method: &str,
    params: Value,
) -> precept_ipc::Result<Value> {
    let Access::Agent { project } = authority.access() else {
        return Err(Error::Unauthorized);
    };
    if !service.profiles.can_propose(&project) {
        return Err(Error::Unauthorized);
    }
    let state = service.profiles.state_dir();
    let session = match method {
        "session.get" => {
            let get: Get = decode(params)?;
            service.sessions.get(state, &project, &get.id)?
        }
        "session.start" => service.sessions.start(state, &project, decode(params)?)?,
        "session.update" => service
            .sessions
            .update(state, &project, decode(params)?, false)?,
        "session.close" => service
            .sessions
            .update(state, &project, decode(params)?, true)?,
        _ => return Err(Error::Unsupported),
    };
    serde_json::to_value(session).map_err(|_| Error::Unavailable)
}

pub fn handle_operator(
    service: &crate::Service,
    method: &str,
    params: Value,
) -> precept_ipc::Result<Value> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct P {
        project: String,
        #[serde(default, deserialize_with = "present")]
        id: Option<String>,
    }
    let p: P = decode(params)?;
    if !precept_ipc::valid_project(&p.project) || !service.profiles.listed(&p.project) {
        return Err(Error::NotFound);
    }
    let state = service.profiles.state_dir();
    match (method, p.id) {
        ("sessions.list", None) => Ok(json!({
            "sessions": service.sessions.list(state, &p.project)?,
        })),
        ("sessions.delete", Some(id)) => {
            service.sessions.delete(state, &p.project, &id)?;
            Ok(json!({"deleted": id}))
        }
        _ => Err(Error::InvalidRequest),
    }
}

pub fn handoff(
    service: &crate::Service,
    authority: &precept_ipc::peer::Peer,
    params: Value,
) -> precept_ipc::Result<Value> {
    let Access::OperatorWrite {
        profile_id,
        generation,
        owner_epoch,
    } = authority.access()
    else {
        return Err(Error::Unauthorized);
    };
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct P {
        project: String,
        id: String,
        task_id: String,
    }
    let p: P = decode(params)?;
    if !service.profiles.allowed(&p.project) {
        return Err(Error::NotFound);
    }
    let session = service
        .sessions
        .list(service.profiles.state_dir(), &p.project)?
        .into_iter()
        .find(|session| session.id == p.id)
        .ok_or(Error::NotFound)?;
    let (_, roots) = service.profiles.read()?;
    let root = roots.get(&p.project).ok_or(Error::NotFound)?.clone();
    let tree = worktree::GitInspector
        .inspect(
            &precept_core::Project {
                name: p.project.clone(),
                root,
            },
            true,
        )
        .map_err(|_| Error::Unavailable)?;
    let completed = render_items(&session, ItemState::Done);
    let mut next_action = render_items(&session, ItemState::Doing);
    for state in [ItemState::Blocked, ItemState::Todo, ItemState::Deferred] {
        let lines = render_items(&session, state);
        if lines.is_empty() {
            continue;
        }
        if !next_action.is_empty() {
            next_action.push_str("\n\n");
        }
        next_action.push_str(&lines);
    }
    if next_action.is_empty() {
        next_action = "Review the completed work and decide the next step.".to_owned();
    }
    let mut completed = completed;
    if let Some(summary) = &session.summary {
        if !completed.is_empty() {
            completed.push_str("\n\n");
        }
        completed.push_str(summary);
    }
    let request = json!({
        "version": 2,
        "binding": {"profile_id":profile_id,"generation":generation,"owner_epoch":owner_epoch},
        "operation_id": precept_ipc::operator::operation_id()?,
        "command": {
            "command": "put_handoff",
            "project": p.project,
            "task_id": p.task_id,
            "objective": session.goal,
            "completed": completed,
            "next_action": next_action,
            "validation_json": "[]",
            "snapshot": {
                "base_sha": tree.head.unwrap_or_default(),
                "branch": tree.branch.unwrap_or_default(),
                "state": match tree.dirty {
                    Some(true) => "dirty",
                    Some(false) => "clean",
                    None => "unknown",
                },
                "fingerprint": tree.worktree_fingerprint,
            },
            "expected": {"kind":"absent"},
        },
    });
    service.profiles.handle(
        authority,
        "profile.mutate",
        request,
        service,
        &service.evidence,
    )
}

fn render_items(session: &Session, state: ItemState) -> String {
    session
        .items
        .iter()
        .filter(|item| item.state == state)
        .map(render_item)
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_item(item: &Item) -> String {
    let mut line = format!(
        "- [{}] {} {} (state: {})",
        item.kind.as_str(),
        item.id,
        item.text,
        item.state.as_str()
    );
    if !item.depends_on.is_empty() {
        line.push_str(&format!("; depends on: {}", item.depends_on.join(", ")));
    }
    if let Some(owner) = &item.owner {
        line.push_str(&format!("; owner: {owner}"));
    }
    if let Some(reason) = &item.blocked_reason {
        line.push_str(&format!("; blocked: {reason}"));
    }
    line
}
