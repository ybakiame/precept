use crate::work_session;
use precept_ipc::Error;
use serde_json::{Value, json};

pub(super) const LEGACY: &str = "2025-11-25";
pub(super) const STATELESS: &str = "2026-07-28";

pub(super) fn info() -> Value {
    json!({"name":"precept","version":env!("CARGO_PKG_VERSION")})
}

/// Sent once in `initialize`: what the server is for and when to call it.
pub(super) const INSTRUCTIONS: &str = "Precept serves the project of this session's folder (nothing outside a registered project): its confirmed engineering context: the developer's reviewed preferences and the repository's contracts. Call get_effective_context before planning or editing anything in this project, and again after your context was compacted. Treat confirmed preferences as the developer's standing instructions; current repository files and contracts outrank them, so report a conflict instead of choosing silently. When a rule you expected is missing or two seem to disagree, explain_rule says why. Call get_handoff with a task id to resume saved work. These are read-only. When the developer states a lasting preference or you learn a durable project convention, you may file it with suggest_rule: it only becomes a suggestion the developer reviews, never an active rule, and works only where they allowed it. Where allowed, record your work with start_session and update_work_state so the next agent (or you, after compaction) can continue, read full state with get_work_session, and keep what you learn about this project in its memory: remember facts about this machine, cautions, lessons and attempts (failures included), recall before trying something that may have been tried, reaffirm a note that held and forget one that no longer does. Memory is unreviewed notes: rules and current code win.";

pub(super) fn tools() -> Value {
    let selector = json!({"type":"object","properties":{
        "agent_id":{"type":"string"},"path":{"type":"string"},"language":{"type":"string"},
        "task_tags":{"type":"array","items":{"type":"string"},"maxItems":64}
    },"additionalProperties":false});
    let [
        start_session,
        update_work_state,
        close_session,
        get_work_session,
    ] = work_session::tools();
    json!([
        {"name":"get_effective_context","description":"Call this first, before planning or editing in this project: returns the developer's confirmed preferences (with tags) and the repository's effective contracts that apply. Optional selector labels (agent_id, path, language, task_tags) narrow applicability; they are not authorization.","inputSchema":{"type":"object","properties":{"include_documents":{"type":"boolean","default":false},"selector":selector},"additionalProperties":false}},
        {"name":"explain_rule","description":"Why a rule key applies to your request or not: every rule of that key (the developer's, the project's, the repository's), each delivered or not with the reason (not confirmed, switched off, applies elsewhere, replaced by a repository rule, value not allowed, outranked, uncommitted). Use it when a rule you expected is missing or two seem to disagree. Read-only.","inputSchema":{"type":"object","properties":{"key":{"type":"string","maxLength":120},"selector":selector},"required":["key"],"additionalProperties":false}},
        {"name":"get_handoff","description":"Resume saved work: returns the task's handoff (objective, completed work, next action, recorded checks) with the current git revision and drift warnings. Saved checks are historical; rerun them. Does not grant permissions.","inputSchema":{"type":"object","properties":{"task_id":{"type":"string"}},"required":["task_id"],"additionalProperties":false}},
        {"name":"suggest_rule","description":"File a lasting rule for the developer to review in Precept: a preference they stated, or a durable convention of this project you confirmed. It is NOT active until the developer confirms it, so keep following your current instructions. Use a short kebab-case key and state the rule itself as the value (one imperative sentence or a few lines). Do not file one-off task instructions, secrets or personal data. scope: project (this repository) or personal (the developer's habits across projects; facts about this machine go to remember). To update a rule that changed (a work-environment fact, a convention that moved on), pass replaces with its id from get_effective_context and the new value: the key comes from that rule, and the developer reviews the change against it.","inputSchema":{"type":"object","properties":{
            "scope":{"type":"string","enum":["project","personal"]},
            "key":{"type":"string","maxLength":120},
            "value":{"type":"string","maxLength":4096},
            "tags":{"type":"array","items":{"type":"string"},"maxItems":8,"description":"Short lowercase labels; defaults to [\"agent\"]"},
            "applies_to":{"type":"object","properties":{
                "agents":{"type":"array","items":{"type":"string"}},
                "languages":{"type":"array","items":{"type":"string"}},
                "paths":{"type":"array","items":{"type":"string"}},
                "task_tags":{"type":"array","items":{"type":"string"}}
            },"additionalProperties":false},
            "enforcement":{"type":"string","enum":["advisory","verify","hard"]},
            "replaces":{"type":"integer","minimum":1,"description":"The id of the confirmed rule this updates (same scope); key may then be left out"}
        },"required":["scope","value"],"additionalProperties":false}},
        start_session,
        update_work_state,
        close_session,
        get_work_session,
        {"name":"remember","description":"Keep a note in this project's memory so the next agent, harness or model knows it: kind environment (a fact about this machine: toolchain, path, service), caution (a pitfall or a step that must come first), lesson (a way of working that proved right) or attempt (what you tried and how it went, failures included). Notes are yours to keep current, not reviewed rules: rules and the current code win. Writing the same kind and title again updates that note (the same text reaffirms it). Keep notes short and specific; no secrets or personal data.","inputSchema":{"type":"object","properties":{
            "kind":{"type":"string","enum":["environment","caution","lesson","attempt"]},
            "title":{"type":"string","maxLength":120},
            "body":{"type":"string","maxLength":2000}
        },"required":["kind","title","body"],"additionalProperties":false}},
        {"name":"recall","description":"Search this project's memory (notes agents kept): by words in the title or text and/or by kind. With index=true, matches come back as one-line summaries (id, kind, title, weight) instead of full text; pass a note's id alone to fetch its full text. Your briefing lists the environment and the most relevant notes; recall the rest when a task touches them, before trying something that may have been tried.","inputSchema":{"type":"object","properties":{
            "query":{"type":"string","maxLength":200},
            "kind":{"type":"string","enum":["environment","caution","lesson","attempt"]},
            "limit":{"type":"integer","minimum":1,"maximum":20},
            "index":{"type":"boolean","default":false},
            "id":{"type":"string","description":"Fetch this note's full text instead of searching"}
        },"additionalProperties":false}},
        {"name":"reaffirm","description":"Mark a memory note (its id from the briefing or recall) as still true, after you relied on it and it held. Optionally state the basis: what verified it.","inputSchema":{"type":"object","properties":{
            "id":{"type":"string"},
            "basis":{"type":"string","maxLength":300}
        },"required":["id"],"additionalProperties":false}},
        {"name":"forget","description":"Remove a memory note (its id) that is no longer true. To change a note, remember it again with the same kind and title instead.","inputSchema":{"type":"object","properties":{
            "id":{"type":"string"}
        },"required":["id"],"additionalProperties":false}}
    ])
}

pub(super) fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

pub(super) fn tool_error(message: &str) -> Value {
    json!({"content":[{"type":"text","text":message}],"isError":true})
}

/// What `suggest_rule` tells the agent: the outcome and what to do next.
pub(super) fn suggestion_reply(result: Result<Value, Error>) -> Value {
    let text = match result {
        Ok(v) => {
            let id = v.get("id").and_then(Value::as_str).unwrap_or("?");
            return json!({"content":[{"type":"text","text":format!(
                "Filed as suggestion #{id}. It is not active: the developer reviews it in Precept. Keep following your current instructions; you may mention the suggestion in your reply."
            )}],"isError":false});
        }
        Err(Error::Unauthorized) => {
            "The developer has not allowed this project's agents to suggest rules (Precept, Agents page). Do not retry; mention the rule in your reply instead."
        }
        Err(Error::Unavailable) => {
            "Precept is not accepting suggestions now (writes are off for this daemon run). Do not retry; mention the rule in your reply instead."
        }
        Err(Error::Conflict) => {
            "This exact rule is already suggested or confirmed. Nothing was filed."
        }
        Err(Error::Capacity) => {
            "The suggestion limit is reached for now. Do not file more; mention further rules in your reply."
        }
        Err(Error::NotFound) => {
            "replaces names no confirmed rule of that scope. Check the id and scope in get_effective_context, or file a new rule without replaces."
        }
        Err(Error::ScopeMismatch | Error::InvalidRequest | Error::InvalidTransition) => {
            "Invalid suggestion: check the key (short, kebab-case), value, tags, applies_to and enforcement."
        }
        Err(_) => {
            "Precept could not file the suggestion. Do not retry; mention the rule in your reply instead."
        }
    };
    tool_error(text)
}

/// What a memory tool tells the agent.
pub(super) fn memory_reply(method: &str, result: Result<Value, Error>) -> Value {
    let text = match result {
        Ok(v) => {
            let line = |e: &Value| {
                let mut out = format!(
                    "- [{}] {} ({}): {}",
                    e["kind"].as_str().unwrap_or("?"),
                    e["title"].as_str().unwrap_or("?"),
                    e["id"].as_str().unwrap_or("?"),
                    e["body"].as_str().unwrap_or("")
                );
                if let Some(notice) = e["notice"].as_str() {
                    out.push_str(&format!(" — {notice}"));
                }
                out
            };
            let text = match method {
                "memory.remember" => format!(
                    "Memory {}: {}",
                    v["status"].as_str().unwrap_or("kept"),
                    line(&v["entry"])
                ),
                "memory.get" => format!("Memory note: {}", line(&v["entry"])),
                "memory.recall" => {
                    let entries = v["entries"].as_array().cloned().unwrap_or_default();
                    if entries.is_empty() {
                        "No memory notes match.".to_owned()
                    } else if v["index"] == json!(true) {
                        let mut out = String::from(
                            "Memory index (agents' notes, unreviewed; fetch a note's full text with recall id=...):\n",
                        );
                        for e in &entries {
                            out.push_str(&format!(
                                "- [{}] {} ({} weight {})\n",
                                e["kind"].as_str().unwrap_or("?"),
                                e["title"].as_str().unwrap_or("?"),
                                e["id"].as_str().unwrap_or("?"),
                                e["weight"]
                            ));
                        }
                        out
                    } else {
                        let mut out = String::from(
                            "Memory notes (agents' notes, unreviewed; rules and current code win):\n",
                        );
                        for e in &entries {
                            out.push_str(&line(e));
                            out.push('\n');
                        }
                        out
                    }
                }
                "memory.reaffirm" => "Reaffirmed.".to_owned(),
                _ => "Forgotten.".to_owned(),
            };
            return json!({"content":[{"type":"text","text":text}],"isError":false});
        }
        Err(Error::Unauthorized) => {
            "The developer has not allowed this project's agents to write (Precept, Agents page). Do not retry; mention the note in your reply instead."
        }
        Err(Error::NotFound) => "No memory note has that id; recall to find it.",
        Err(Error::InvalidRequest) => {
            "Invalid note: a kind, a title (at most 120 characters) and a text (at most 2000), without control characters."
        }
        Err(Error::Conflict) => "Another note of that kind already has this title.",
        Err(_) => "Precept could not reach project memory. Do not retry; continue your task.",
    };
    tool_error(text)
}
