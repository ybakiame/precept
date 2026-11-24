use precept_ipc::Error;
use serde_json::{Map, Value, json};

fn item_schema() -> Value {
    json!({"type":"object","properties":{
        "id":{"type":"string","maxLength":24,"description":"Stable lowercase id (letters, digits, hyphens); assigned when omitted"},
        "text":{"type":"string","minLength":1,"maxLength":200},
        "kind":{"type":"string","enum":["milestone","task"],"default":"task"},
        "state":{"type":"string","enum":["todo","doing","blocked","deferred","done"],"default":"todo"},
        "depends_on":{"type":"array","maxItems":36,"items":{"type":"string","maxLength":24},"description":"Ids of items in the same session that must finish first"},
        "owner":{"type":"string","maxLength":64},
        "blocked_reason":{"type":"string","maxLength":200}
    },"required":["text"],"additionalProperties":false})
}

fn item_list_schema() -> Value {
    json!({"type":"array","maxItems":36,"items":item_schema()})
}

fn operation_schema() -> Value {
    let id = json!({"type":"string","maxLength":24});
    json!({"oneOf":[
        {"type":"object","properties":{"op":{"const":"add"},"item":item_schema()},"required":["op","item"],"additionalProperties":false},
        {"type":"object","properties":{
            "op":{"const":"update"},"id":id,"text":{"type":"string","maxLength":200},
            "kind":{"type":"string","enum":["milestone","task"]},
            "depends_on":{"type":"array","maxItems":36,"items":{"type":"string","maxLength":24}},
            "owner":{"type":"string","maxLength":64},"blocked_reason":{"type":"string","maxLength":200}
        },"required":["op","id"],"additionalProperties":false},
        {"type":"object","properties":{"op":{"const":"start"},"id":id},"required":["op","id"],"additionalProperties":false},
        {"type":"object","properties":{"op":{"const":"complete"},"id":id},"required":["op","id"],"additionalProperties":false},
        {"type":"object","properties":{"op":{"const":"defer"},"id":id},"required":["op","id"],"additionalProperties":false},
        {"type":"object","properties":{"op":{"const":"block"},"id":id,"reason":{"type":"string","minLength":1,"maxLength":200}},"required":["op","id","reason"],"additionalProperties":false},
        {"type":"object","properties":{"op":{"const":"reopen"},"id":id},"required":["op","id"],"additionalProperties":false},
        {"type":"object","properties":{"op":{"const":"remove"},"id":id},"required":["op","id"],"additionalProperties":false}
    ]})
}

fn operation_list_schema() -> Value {
    json!({"type":"array","maxItems":36,"items":operation_schema()})
}

pub(super) fn tools() -> [Value; 4] {
    [
        json!({"name":"start_session","description":"Start a work session with one goal and flat structured items, or resume an open session by id or conversation. Items have kind milestone/task, state todo/doing/blocked/deferred/done, optional dependencies, owner and blocked reason. Dependencies may cross states but must stay in the same session and form an acyclic graph.","inputSchema":{"type":"object","properties":{
            "goal":{"type":"string","minLength":1,"maxLength":500},
            "conversation_id":{"type":"string","maxLength":128},
            "resume_session_id":{"type":"string","maxLength":32},
            "expected_rev":{"type":"integer","minimum":0,"description":"Required when resume_session_id is supplied"},
            "items":item_list_schema()
        },"required":["goal"],"additionalProperties":false}}),
        json!({"name":"update_work_state","description":"Apply single-item operations to an open session without replacing unrelated work. Use expected_rev from the latest session answer; on Conflict, call get_work_session and retry against its revision. Supported operations: add, update, start, complete, defer, block, reopen, remove.","inputSchema":{"type":"object","properties":{
            "session_id":{"type":"string","maxLength":32},
            "goal":{"type":"string","minLength":1,"maxLength":500},
            "expected_rev":{"type":"integer","minimum":0},
            "operations":operation_list_schema()
        },"required":["session_id","expected_rev"],"additionalProperties":false}}),
        json!({"name":"close_session","description":"Close a finished or handed-over session with a short summary. Doing items return to todo so unfinished work remains visible. Pass expected_rev from the latest session answer.","inputSchema":{"type":"object","properties":{
            "session_id":{"type":"string","maxLength":32},
            "summary":{"type":"string","maxLength":1000},
            "expected_rev":{"type":"integer","minimum":0}
        },"required":["session_id","expected_rev"],"additionalProperties":false}}),
        json!({"name":"get_work_session","description":"Read the full current state of one work session in this project, including item ids, states, dependencies, owners and revision. Use it after Conflict or before resuming work from another conversation.","inputSchema":{"type":"object","properties":{
            "session_id":{"type":"string","maxLength":32}
        },"required":["session_id"],"additionalProperties":false}}),
    ]
}

fn string(value: &Value, max: usize, field: &str) -> Result<(), String> {
    if value
        .as_str()
        .is_some_and(|text| text.chars().count() <= max)
    {
        Ok(())
    } else {
        Err(format!(
            "{field} must be a string of at most {max} characters"
        ))
    }
}

fn item(value: &Value, field: &str) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Err(format!("{field} must be an object"));
    };
    if object.keys().any(|key| {
        ![
            "id",
            "text",
            "kind",
            "state",
            "depends_on",
            "owner",
            "blocked_reason",
        ]
        .contains(&key.as_str())
    }) {
        return Err(format!("{field} contains an unknown field"));
    }
    string(
        object.get("text").unwrap_or(&Value::Null),
        200,
        &format!("{field}.text"),
    )?;
    if let Some(id) = object.get("id") {
        string(id, 24, &format!("{field}.id"))?;
    }
    if let Some(kind) = object.get("kind")
        && !matches!(kind.as_str(), Some("milestone" | "task"))
    {
        return Err(format!("{field}.kind must be milestone or task"));
    }
    if let Some(state) = object.get("state")
        && !matches!(
            state.as_str(),
            Some("todo" | "doing" | "blocked" | "deferred" | "done")
        )
    {
        return Err(format!("{field}.state is invalid"));
    }
    if let Some(deps) = object.get("depends_on") {
        let Some(deps) = deps.as_array() else {
            return Err(format!("{field}.depends_on must be an array"));
        };
        if deps.len() > 36 {
            return Err(format!("{field}.depends_on has too many entries"));
        }
        for (index, dep) in deps.iter().enumerate() {
            string(dep, 24, &format!("{field}.depends_on[{index}]"))?;
        }
    }
    if let Some(owner) = object.get("owner") {
        string(owner, 64, &format!("{field}.owner"))?;
    }
    if let Some(reason) = object.get("blocked_reason") {
        string(reason, 200, &format!("{field}.blocked_reason"))?;
    }
    Ok(())
}

fn items(value: &Value, field: &str) -> Result<(), String> {
    let Some(values) = value.as_array() else {
        return Err(format!("{field} must be an array"));
    };
    if values.len() > 36 {
        return Err(format!("{field} has more than 36 items"));
    }
    for (index, value) in values.iter().enumerate() {
        item(value, &format!("{field}[{index}]"))?;
    }
    Ok(())
}

fn operation(value: &Value, field: &str) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Err(format!("{field} must be an object"));
    };
    let Some(op) = object.get("op").and_then(Value::as_str) else {
        return Err(format!("{field}.op is required"));
    };
    let allowed: &[&str] = match op {
        "add" => &["op", "item"],
        "update" => &[
            "op",
            "id",
            "text",
            "kind",
            "depends_on",
            "owner",
            "blocked_reason",
        ],
        "block" => &["op", "id", "reason"],
        "start" | "complete" | "defer" | "reopen" | "remove" => &["op", "id"],
        _ => return Err(format!("{field}.op is invalid")),
    };
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("{field} contains a field not allowed for {op}"));
    }
    if op == "add" {
        return item(
            object.get("item").unwrap_or(&Value::Null),
            &format!("{field}.item"),
        );
    }
    string(
        object.get("id").unwrap_or(&Value::Null),
        24,
        &format!("{field}.id"),
    )?;
    if op == "block" {
        string(
            object.get("reason").unwrap_or(&Value::Null),
            200,
            &format!("{field}.reason"),
        )?;
    }
    if op == "update" {
        if let Some(text) = object.get("text") {
            string(text, 200, &format!("{field}.text"))?;
        }
        if let Some(kind) = object.get("kind")
            && !matches!(kind.as_str(), Some("milestone" | "task"))
        {
            return Err(format!("{field}.kind must be milestone or task"));
        }
        if let Some(deps) = object.get("depends_on") {
            let Some(deps) = deps.as_array() else {
                return Err(format!("{field}.depends_on must be an array"));
            };
            if deps.len() > 36 {
                return Err(format!("{field}.depends_on has too many entries"));
            }
            for (index, dep) in deps.iter().enumerate() {
                string(dep, 24, &format!("{field}.depends_on[{index}]"))?;
            }
        }
        if let Some(owner) = object.get("owner") {
            string(owner, 64, &format!("{field}.owner"))?;
        }
        if let Some(reason) = object.get("blocked_reason") {
            string(reason, 200, &format!("{field}.blocked_reason"))?;
        }
    }
    Ok(())
}

fn operations(value: &Value, field: &str) -> Result<(), String> {
    let Some(values) = value.as_array() else {
        return Err(format!("{field} must be an array"));
    };
    if values.len() > 36 {
        return Err(format!("{field} has more than 36 operations"));
    }
    for (index, value) in values.iter().enumerate() {
        operation(value, &format!("{field}[{index}]"))?;
    }
    Ok(())
}

pub(super) fn request(
    tool: &str,
    args: &Map<String, Value>,
) -> Result<(&'static str, Value), String> {
    let allowed: &[&str] = match tool {
        "start_session" => &[
            "goal",
            "conversation_id",
            "resume_session_id",
            "expected_rev",
            "items",
        ],
        "update_work_state" => &["session_id", "goal", "expected_rev", "operations"],
        "close_session" => &["session_id", "summary", "expected_rev"],
        "get_work_session" => &["session_id"],
        _ => return Err("unknown work-session tool".into()),
    };
    if args.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("work-session arguments contain an unknown field".into());
    }
    for (key, max) in [
        ("goal", 500),
        ("conversation_id", 128),
        ("resume_session_id", 32),
        ("session_id", 32),
        ("summary", 1000),
    ] {
        if let Some(value) = args.get(key) {
            string(value, max, key)?;
        }
    }
    if let Some(value) = args.get("expected_rev")
        && value.as_u64().is_none()
    {
        return Err("expected_rev must be a non-negative integer".into());
    }
    if let Some(value) = args.get("items") {
        items(value, "items")?;
    }
    if let Some(value) = args.get("operations") {
        operations(value, "operations")?;
    }
    match tool {
        "start_session" => {
            if !args.contains_key("goal") {
                return Err("goal is required".into());
            }
            if args.contains_key("resume_session_id") && !args.contains_key("expected_rev") {
                return Err("expected_rev is required with resume_session_id".into());
            }
        }
        "update_work_state" => {
            if !args.contains_key("session_id") || !args.contains_key("expected_rev") {
                return Err("session_id and expected_rev are required".into());
            }
            if !args.contains_key("goal") && !args.contains_key("operations") {
                return Err("goal or operations is required".into());
            }
        }
        "close_session" => {
            if !args.contains_key("session_id") || !args.contains_key("expected_rev") {
                return Err("session_id and expected_rev are required".into());
            }
        }
        "get_work_session" if !args.contains_key("session_id") => {
            return Err("session_id is required".into());
        }
        "get_work_session" => {}
        _ => {}
    }
    let mut params = Map::new();
    for (key, value) in args {
        let name = match key.as_str() {
            "session_id" | "resume_session_id" => "id",
            "conversation_id" => "conversation",
            other => other,
        };
        params.insert(name.into(), value.clone());
    }
    let method = match tool {
        "start_session" => "session.start",
        "update_work_state" => "session.update",
        "close_session" => "session.close",
        "get_work_session" => "session.get",
        _ => unreachable!(),
    };
    Ok((method, Value::Object(params)))
}

/// What a session tool tells the agent.
pub(super) fn session_reply(result: Result<Value, Error>) -> Value {
    let text = match result {
        Ok(value) => {
            let id = value.get("id").and_then(Value::as_str).unwrap_or("?");
            let status = value.get("status").and_then(Value::as_str).unwrap_or("?");
            let rev = value.get("rev").and_then(Value::as_u64).unwrap_or(0);
            return json!({"content":[{"type":"text","text":format!(
                "Session `{id}` ({status}, rev {rev}). Use expected_rev={rev} for the next update; call get_work_session after Conflict.\n{}",
                value
            )}],"isError":false});
        }
        Err(Error::Unauthorized) => {
            "The developer has not allowed this project's agents to record work sessions (Precept, Agents page). Do not retry.".to_owned()
        }
        Err(Error::NotFound) => {
            "No such session in this project; start one with start_session or choose an id from get_effective_context.".to_owned()
        }
        Err(Error::InvalidTransition) => "That session is closed; start a new one.".to_owned(),
        Err(Error::Conflict) => {
            "Another agent updated this session. Call get_work_session for the full current state and retry with its expected_rev.".to_owned()
        }
        Err(Error::InvalidWorkState { field, reason }) => {
            format!("Invalid work state at {field}: {reason}.")
        }
        Err(Error::InvalidRequest) => {
            "Invalid work-session arguments; check the tool schema and field types.".to_owned()
        }
        Err(_) => {
            "Precept could not record the session. Do not retry; continue your task.".to_owned()
        }
    };
    crate::protocol::tool_error(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_requests_map_to_single_item_ipc_operations() {
        let args = serde_json::from_value(json!({
            "session_id":"ab",
            "expected_rev":4,
            "operations":[
                {"op":"add","item":{"id":"ui","text":"UI","kind":"task","depends_on":["api"]}},
                {"op":"start","id":"ui"},
                {"op":"block","id":"ui","reason":"Waiting for API review"}
            ]
        }))
        .unwrap();
        let (method, params) = request("update_work_state", &args).unwrap();
        assert_eq!(method, "session.update");
        assert_eq!(params["id"], "ab");
        assert_eq!(params["expected_rev"], 4);
        assert_eq!(params["operations"][0]["item"]["depends_on"][0], "api");
    }

    #[test]
    fn invalid_operations_name_the_exact_field() {
        let args = serde_json::from_value(json!({
            "session_id":"ab",
            "expected_rev":4,
            "operations":[{"op":"block","id":"ui"}]
        }))
        .unwrap();
        let error = request("update_work_state", &args).unwrap_err();
        assert!(error.contains("operations[0].reason"), "{error}");
    }

    #[test]
    fn replies_return_full_state_and_precise_daemon_validation() {
        let ok = session_reply(Ok(json!({
            "id":"ab","status":"open","rev":4,"goal":"Ship","items":[{"id":"api","text":"API"}]
        })));
        let text = ok["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("expected_rev=4") && text.contains("\"items\""));
        let bad = session_reply(Err(Error::InvalidWorkState {
            field: "operations[0].id".into(),
            reason: "item not found".into(),
        }));
        assert_eq!(bad["isError"], true);
        assert_eq!(
            bad["content"][0]["text"],
            "Invalid work state at operations[0].id: item not found."
        );
    }
}
