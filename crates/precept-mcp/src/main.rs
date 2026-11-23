//! One stdio bridge for every registered project: it serves the project whose
//! root holds the session's folder (the client's MCP roots, else its own
//! working directory) through that project's agent capability, and nothing
//! anywhere else. Only the daemon reads storage/repository data.
use clap::Parser;
use precept_ipc::{Access, Client, Credential, Error, Location, MAX_FRAME, load_credential};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[derive(Parser)]
struct Args {
    #[command(flatten)]
    location: Location,
}
/// The id of the one request this server sends: `roots/list`.
const ROOTS_REQUEST: &str = "precept-roots";
const LEGACY: &str = "2025-11-25";
const STATELESS: &str = "2026-07-28";
fn info() -> Value {
    json!({"name":"precept","version":"0.1.0a0"})
}
/// Sent once in `initialize`: what the server is for and when to call it.
const INSTRUCTIONS: &str = "Precept serves the project of this session's folder (nothing outside a registered project): its confirmed engineering context: the developer's reviewed preferences and the repository's contracts. Call get_effective_context before planning or editing anything in this project, and again after your context was compacted. Treat confirmed preferences as the developer's standing instructions; current repository files and contracts outrank them, so report a conflict instead of choosing silently. When a rule you expected is missing or two seem to disagree, explain_rule says why. Call get_handoff with a task id to resume saved work. These are read-only. When the developer states a lasting preference or you learn a durable project convention, you may file it with suggest_rule: it only becomes a suggestion the developer reviews, never an active rule, and works only where they allowed it. Where allowed, record your work with start_session and update_work_state so the next agent (or you, after compaction) can continue, and keep what you learn about this project in its memory: remember facts about this machine, cautions, lessons and attempts (failures included), recall before trying something that may have been tried, reaffirm a note that held and forget one that no longer does. Memory is unreviewed notes: rules and current code win.";
fn tools() -> Value {
    let selector = json!({"type":"object","properties":{
        "agent_id":{"type":"string"},"path":{"type":"string"},"language":{"type":"string"},
        "task_tags":{"type":"array","items":{"type":"string"},"maxItems":64}
    },"additionalProperties":false});
    let item_list = json!({"type":"array","maxItems":12,"items":{"type":"object","properties":{
        "id":{"type":"string","maxLength":24,"description":"A stable lowercase id (letters, digits, hyphens); assigned when omitted"},
        "text":{"type":"string","maxLength":200},
        "depends_on":{"type":"array","maxItems":12,"items":{"type":"string","maxLength":24},"description":"Ids of items in the same list that must finish first"}
    },"required":["text"],"additionalProperties":false}});
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
        {"name":"start_session","description":"Start (or resume) your work session in this project and say what you are working on. Other agents of this project see open sessions at their start, so work can continue across agents and after compaction. Call it once your task is clear; pass the conversation id if you have one, so the same conversation finds its session again. Work items are objects with a text and an optional stable id; a todo item may name the ids it depends on.","inputSchema":{"type":"object","properties":{
            "goal":{"type":"string","maxLength":500},
            "conversation_id":{"type":"string","maxLength":128},
            "done":item_list,"doing":item_list,"todo":item_list
        },"required":["goal"],"additionalProperties":false}},
        {"name":"update_work_state","description":"Keep your session's work state current: what is done, what you are doing, what is next (each list replaces the previous one; keep the ids of items that stay). Pass expected_rev from the session's last answer so another agent's update is not silently overwritten; on Conflict, read the session again and retry. Update it when a step finishes and before your context is compacted.","inputSchema":{"type":"object","properties":{
            "session_id":{"type":"string"},
            "goal":{"type":"string","maxLength":500},
            "expected_rev":{"type":"integer","minimum":0},
            "done":item_list,"doing":item_list,"todo":item_list
        },"required":["session_id"],"additionalProperties":false}},
        {"name":"close_session","description":"Close your work session when the task is finished or handed over, with a short summary of the outcome and anything left for the next agent. Pass expected_rev from the session's last answer.","inputSchema":{"type":"object","properties":{
            "session_id":{"type":"string"},
            "summary":{"type":"string","maxLength":1000},
            "expected_rev":{"type":"integer","minimum":0},
            "todo":item_list
        },"required":["session_id"],"additionalProperties":false}},
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
fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
fn tool_error(message: &str) -> Value {
    json!({"content":[{"type":"text","text":message}],"isError":true})
}
/// What `suggest_rule` tells the agent: the outcome and what to do next.
fn suggestion_reply(result: Result<Value, Error>) -> Value {
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
fn memory_reply(method: &str, result: Result<Value, Error>) -> Value {
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

/// What a session tool tells the agent.
fn session_reply(result: Result<Value, Error>) -> Value {
    let text = match result {
        Ok(v) => {
            return json!({"content":[{"type":"text","text":format!(
                "Session `{}` ({}, rev {}). Keep it current with update_work_state, passing expected_rev={} so another agent's update is not overwritten; close it with close_session when done.",
                v.get("id").and_then(Value::as_str).unwrap_or("?"),
                v.get("status").and_then(Value::as_str).unwrap_or("?"),
                v.get("rev").and_then(Value::as_u64).unwrap_or(0),
                v.get("rev").and_then(Value::as_u64).unwrap_or(0)
            )}],"isError":false});
        }
        Err(Error::Unauthorized) => {
            "The developer has not allowed this project's agents to record work sessions (Precept, Agents page). Do not retry."
        }
        Err(Error::NotFound) => "No such session in this project; start one with start_session.",
        Err(Error::InvalidTransition) => "That session is closed; start a new one.",
        Err(Error::Conflict) => {
            "Another agent updated this session since you read it. Read the current work_sessions (get_effective_context) and retry with the new expected_rev."
        }
        Err(Error::InvalidRequest) => {
            "Invalid work state: a goal is required (at most 500 characters); at most 12 items per list, each an object with a text (at most 200 characters), an optional lowercase id and optional depends_on naming ids of the same list."
        }
        Err(_) => "Precept could not record the session. Do not retry; continue your task.",
    };
    tool_error(text)
}
struct Bridge {
    location: Location,
    /// The served project and its agent capability, once the session's
    /// folder resolved to one; `None` serves nothing.
    bound: Option<(String, Credential)>,
    /// The client offered MCP roots in `initialize`.
    roots: bool,
    initialized: bool,
    ready: bool,
    /// The client's self-reported `clientInfo` name and version, passed to
    /// the daemon's activity log only (bounded, printable).
    client: Option<String>,
    /// The work session this connection started, attached to later calls
    /// so the daemon's evidence knows which session they belong to.
    session: Option<String>,
}
/// `name version` from `clientInfo`, bounded and printable, or none.
fn client_label(params: &Value) -> Option<String> {
    let info = params.get("clientInfo")?;
    let name = info.get("name")?.as_str()?;
    let version = info.get("version").and_then(Value::as_str).unwrap_or("");
    let label: String = format!("{name} {version}")
        .chars()
        .filter(|c| !c.is_control())
        .take(64)
        .collect();
    let label = label.trim();
    (!label.is_empty()).then(|| label.to_owned())
}
/// What every tool says outside a registered project.
const UNBOUND: &str = "This folder is not part of a project registered in Precept (or its agent access is off), so Precept has nothing for it. Continue without Precept.";

/// The project whose registered root holds every one of `dirs`, through the
/// agent capabilities published in the runtime directory: each is asked for
/// its own root. `None` when no project or more than one matches.
async fn resolve(location: &Location, dirs: &[std::path::PathBuf]) -> Option<(String, Credential)> {
    let dirs: Vec<_> = dirs.iter().filter_map(|d| d.canonicalize().ok()).collect();
    if dirs.is_empty() {
        return None;
    }
    let runtime = location.root().ok()?;
    let mut names: Vec<String> = std::fs::read_dir(&runtime)
        .ok()?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| {
            n.strip_prefix("agent-")?
                .strip_suffix(".cap")
                .map(str::to_owned)
        })
        .filter(|n| precept_ipc::valid_project(n))
        .take(256)
        .collect();
    names.sort();
    let mut found = None;
    for name in names {
        let Ok(path) = location.credential_path(Some(&name)) else {
            continue;
        };
        let Ok(credential) = load_credential(location, &path) else {
            continue;
        };
        if credential.access
            != (Access::Agent {
                project: name.clone(),
            })
        {
            continue;
        }
        let Ok(mut client) = Client::connect(location, &credential).await else {
            continue;
        };
        let Ok(answer) = client.request("project.root", json!({})).await else {
            continue;
        };
        let Some(root) = answer["root"]
            .as_str()
            .and_then(|r| std::path::Path::new(r).canonicalize().ok())
        else {
            continue;
        };
        if dirs.iter().all(|d| d.starts_with(&root)) {
            if found.is_some() {
                return None;
            }
            found = Some((name, credential));
        }
    }
    found
}

/// Local folders named by a `roots/list` result (`file://` URIs only).
fn root_dirs(result: &Value) -> Vec<std::path::PathBuf> {
    result["roots"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["uri"].as_str()?.strip_prefix("file://"))
        .map(percent_decode)
        .map(|s| {
            // `file:///C:/...` leaves a leading slash before the drive letter;
            // Windows reads `/C:/...` as drive-relative and loses the folder.
            let b = s.as_bytes();
            if cfg!(windows)
                && b.len() >= 3
                && b[0] == b'/'
                && b[1].is_ascii_alphabetic()
                && b[2] == b':'
            {
                s[1..].to_owned()
            } else {
                s
            }
        })
        .map(std::path::PathBuf::from)
        .take(64)
        .collect()
}
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// One sanitized diagnostic line on stderr (never stdout, never arguments,
/// values or paths): MCP clients show a server's stderr in their own logs.
fn diagnose(what: &str) {
    eprintln!("precept-mcp: {what}");
}
impl Bridge {
    async fn tool(&mut self, name: &Value, args: &Value) -> Result<Value, Value> {
        let Some(args) = args.as_object() else {
            return Err(error(Value::Null, -32700, "Malformed input: ValueError"));
        };
        let (method, params) = match name.as_str() {
            Some("get_effective_context") => {
                if args
                    .keys()
                    .any(|k| k != "include_documents" && k != "selector")
                    || !args
                        .get("include_documents")
                        .unwrap_or(&Value::Bool(false))
                        .is_boolean()
                {
                    diagnose("get_effective_context refused: invalid arguments");
                    return Ok(tool_error("include_documents must be boolean"));
                }
                let mut params = json!({"include_documents":args.get("include_documents").unwrap_or(&Value::Bool(false))});
                if let Some(selector) = args.get("selector") {
                    let valid = serde_json::from_value::<precept_core::contracts::SelectorQuery>(
                        selector.clone(),
                    )
                    .is_ok_and(|q| q.validate().is_ok());
                    if !valid {
                        diagnose("get_effective_context refused: invalid selector");
                        return Ok(tool_error("Invalid selector"));
                    }
                    params["selector"] = selector.clone();
                }
                ("context.get", params)
            }
            Some("explain_rule") => {
                let key_ok = args
                    .get("key")
                    .and_then(Value::as_str)
                    .is_some_and(|k| !k.is_empty() && k.chars().count() <= 120);
                if !key_ok || args.keys().any(|k| k != "key" && k != "selector") {
                    diagnose("explain_rule refused: invalid arguments");
                    return Ok(tool_error(
                        "key (a rule key) is required; selector is optional",
                    ));
                }
                let mut params = json!({"key":args["key"]});
                if let Some(selector) = args.get("selector") {
                    let valid = serde_json::from_value::<precept_core::contracts::SelectorQuery>(
                        selector.clone(),
                    )
                    .is_ok_and(|q| q.validate().is_ok());
                    if !valid {
                        diagnose("explain_rule refused: invalid selector");
                        return Ok(tool_error("Invalid selector"));
                    }
                    params["selector"] = selector.clone();
                }
                ("context.explain", params)
            }
            Some("suggest_rule") => {
                const KEYS: [&str; 7] = [
                    "scope",
                    "key",
                    "value",
                    "tags",
                    "applies_to",
                    "enforcement",
                    "replaces",
                ];
                let replaces = args.get("replaces");
                let required: &[&str] = if replaces.is_some() {
                    &["scope", "value"]
                } else {
                    &["scope", "key", "value"]
                };
                if args.keys().any(|k| !KEYS.contains(&k.as_str()))
                    || !required
                        .iter()
                        .all(|k| args.get(*k).is_some_and(Value::is_string))
                    || args.get("key").is_some_and(|k| !k.is_string())
                    || replaces.is_some_and(|r| r.as_i64().is_none_or(|n| n <= 0))
                {
                    diagnose("suggest_rule refused: invalid arguments");
                    return Ok(tool_error(
                        "scope and value are required strings, and key unless replaces names the rule to update; only tags, applies_to, enforcement and replaces may be added",
                    ));
                }
                ("preference.suggest", Value::Object(args.clone()))
            }
            Some(tool @ ("remember" | "recall" | "reaffirm" | "forget")) => {
                let (allowed, required): (&[&str], &[&str]) = match tool {
                    "remember" => (&["kind", "title", "body"], &["kind", "title", "body"]),
                    "recall" => (&["query", "kind", "limit", "index", "id"], &[]),
                    "reaffirm" => (&["id", "basis"], &["id"]),
                    _ => (&["id"], &["id"]),
                };
                let typed = args.iter().all(|(k, v)| match k.as_str() {
                    "limit" => v.as_u64().is_some_and(|n| (1..=20).contains(&n)),
                    "index" => v.is_boolean(),
                    _ => v.is_string(),
                });
                // Fetching by id takes no search arguments.
                let exclusive = tool != "recall" || !args.contains_key("id") || args.len() == 1;
                if args.keys().any(|k| !allowed.contains(&k.as_str()))
                    || required.iter().any(|k| !args.contains_key(*k))
                    || !typed
                    || !exclusive
                {
                    diagnose("memory tool refused: invalid arguments");
                    return Ok(tool_error("Invalid arguments: see the tool's schema"));
                }
                (
                    match tool {
                        "remember" => "memory.remember",
                        "recall" if args.contains_key("id") => "memory.get",
                        "recall" => "memory.recall",
                        "reaffirm" => "memory.reaffirm",
                        _ => "memory.forget",
                    },
                    Value::Object(args.clone()),
                )
            }
            Some(tool @ ("start_session" | "update_work_state" | "close_session")) => {
                let allowed: &[&str] = match tool {
                    "start_session" => &["goal", "conversation_id", "done", "doing", "todo"],
                    "update_work_state" => &[
                        "session_id",
                        "goal",
                        "expected_rev",
                        "done",
                        "doing",
                        "todo",
                    ],
                    _ => &["session_id", "summary", "expected_rev", "todo"],
                };
                let lists_ok = ["done", "doing", "todo"].iter().all(|k| {
                    args.get(*k).is_none_or(|v| {
                        v.as_array().is_some_and(|a| {
                            a.len() <= 12
                                && a.iter().all(|i| {
                                    i.as_object().is_some_and(|o| {
                                        o.keys().all(|k| {
                                            ["id", "text", "depends_on"].contains(&k.as_str())
                                        }) && o.get("text").is_some_and(Value::is_string)
                                            && o.get("id").is_none_or(Value::is_string)
                                            && o.get("depends_on").is_none_or(|d| {
                                                d.as_array().is_some_and(|ds| {
                                                    ds.len() <= 12
                                                        && ds.iter().all(Value::is_string)
                                                })
                                            })
                                    })
                                })
                        })
                    })
                });
                let strings_ok = ["goal", "conversation_id", "session_id", "summary"]
                    .iter()
                    .all(|k| args.get(*k).is_none_or(Value::is_string));
                let rev_ok = args
                    .get("expected_rev")
                    .is_none_or(|v| v.as_u64().is_some());
                let required = if tool == "start_session" {
                    "goal"
                } else {
                    "session_id"
                };
                if args.keys().any(|k| !allowed.contains(&k.as_str()))
                    || !lists_ok
                    || !strings_ok
                    || !rev_ok
                    || args.get(required).is_none()
                {
                    diagnose("session tool refused: invalid arguments");
                    return Ok(tool_error("Invalid arguments: see the tool's schema"));
                }
                let mut params = serde_json::Map::new();
                for (k, v) in args {
                    let name = match k.as_str() {
                        "session_id" => "id",
                        "conversation_id" => "conversation",
                        other => other,
                    };
                    params.insert(name.into(), v.clone());
                }
                let method = match tool {
                    "start_session" => "session.start",
                    "update_work_state" => "session.update",
                    _ => "session.close",
                };
                (method, Value::Object(params))
            }
            Some("get_handoff") => {
                if args.len() != 1 || !args.get("task_id").is_some_and(Value::is_string) {
                    diagnose("get_handoff refused: invalid arguments");
                    return Ok(tool_error("task_id string required"));
                }
                ("handoff.get", json!({"task_id":args["task_id"]}))
            }
            _ => {
                diagnose("unknown tool requested");
                return Err(error(
                    Value::Null,
                    -32602,
                    &format!("Unknown tool: {}", name.as_str().unwrap_or("None")),
                ));
            }
        };
        let mut params = params;
        if let Some(client) = &self.client {
            params["client"] = json!(client);
        }
        if let Some(session) = &self.session
            && method != "session.start"
        {
            params["session"] = json!(session);
        }
        let Some((_, credential)) = &self.bound else {
            return Ok(tool_error(UNBOUND));
        };
        let result = match Client::connect(&self.location, credential).await {
            Ok(mut client) => client.request(method, params).await,
            Err(e) => Err(e),
        };
        if let Err(e) = &result {
            diagnose(&format!("{method} failed: {e}"));
        }
        if method == "preference.suggest" {
            return Ok(suggestion_reply(result));
        }
        if method.starts_with("memory.") {
            return Ok(memory_reply(method, result));
        }
        if method.starts_with("session.") {
            match (&result, method) {
                (Ok(v), "session.start") => {
                    self.session = v["id"].as_str().map(str::to_owned);
                }
                (Ok(_), "session.close") => self.session = None,
                _ => {}
            }
            return Ok(session_reply(result));
        }
        Ok(match result {
            Ok(data) => {
                json!({"content":[{"type":"text","text":data.to_string()}],"isError":false})
            }
            Err(Error::NotFound) => tool_error("Handoff not found"),
            Err(_) => tool_error("IPC unavailable or request denied"),
        })
    }
    /// Binds to the project of `dirs`; a change of project is logged.
    async fn bind(&mut self, dirs: &[std::path::PathBuf]) {
        let next = resolve(&self.location, dirs).await;
        let before = self.bound.as_ref().map(|(n, _)| n.clone());
        let after = next.as_ref().map(|(n, _)| n.clone());
        self.bound = next;
        if before != after {
            self.session = None;
            match &self.bound {
                Some((_, credential)) => {
                    diagnose("serving the project of this session's folder");
                    // Recorded as the project's "started" evidence.
                    if let Ok(mut c) = Client::connect(&self.location, credential).await {
                        let _ = c.request("daemon.health", json!({})).await;
                    }
                }
                None => diagnose("this folder is not a registered project; serving nothing"),
            }
        }
    }
    async fn handle(&mut self, message: Value) -> Option<Value> {
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        if !message.is_object() || message.get("jsonrpc") != Some(&json!("2.0")) {
            return Some(error(id, -32600, "Invalid JSON-RPC request"));
        }
        // The client's answer to our roots/list: its workspace folders.
        if id == json!(ROOTS_REQUEST) && message.get("method").is_none() {
            if let Some(result) = message.get("result") {
                let dirs = root_dirs(result);
                if !dirs.is_empty() {
                    self.bind(&dirs).await;
                }
            }
            return None;
        }
        if message.get("id").is_none() {
            let method = message.get("method").and_then(Value::as_str);
            if method == Some("notifications/initialized") {
                self.ready = self.initialized;
            }
            // Ask for the workspace folders once ready, and again when they change.
            if self.roots
                && matches!(
                    method,
                    Some("notifications/initialized" | "notifications/roots/list_changed")
                )
            {
                return Some(json!({"jsonrpc":"2.0","id":ROOTS_REQUEST,"method":"roots/list"}));
            }
            return None;
        }
        if !(id.is_string() || id.as_i64().is_some() || id.as_u64().is_some()) {
            return Some(error(Value::Null, -32600, "Invalid id"));
        }
        let params = message.get("params").cloned().unwrap_or(json!({}));
        if !params.is_object() {
            return Some(error(id, -32602, "params must be object"));
        }
        let stateless = params
            .get("_meta")
            .and_then(|v| v.get("io.modelcontextprotocol/protocolVersion"))
            == Some(&json!(STATELESS));
        let method = message.get("method").and_then(Value::as_str);
        let mut result = match method {
            Some("initialize") => {
                if !params.get("protocolVersion").is_some_and(Value::is_string) {
                    return Some(error(id, -32602, "protocolVersion required"));
                }
                self.initialized = true;
                self.ready = false;
                self.client = client_label(&params);
                self.roots = params
                    .pointer("/capabilities/roots")
                    .is_some_and(Value::is_object);
                json!({"protocolVersion":LEGACY,"serverInfo":info(),"capabilities":{"tools":{}},"instructions":INSTRUCTIONS})
            }
            Some("ping") => json!({}),
            _ if !(stateless || self.ready) => {
                diagnose("request before MCP initialization");
                return Some(error(id, -32000, "MCP initialization required"));
            }
            Some("tools/list") => json!({"tools":tools()}),
            Some("tools/call") => match self
                .tool(
                    params.get("name").unwrap_or(&Value::Null),
                    params.get("arguments").unwrap_or(&json!({})),
                )
                .await
            {
                Ok(v) => v,
                Err(mut v) => {
                    if v["error"]["code"] != json!(-32700) {
                        v["id"] = id;
                    }
                    return Some(v);
                }
            },
            _ => return Some(error(id, -32601, "Method not found")),
        };
        if stateless {
            result["_meta"] = json!({"io.modelcontextprotocol/serverInfo":info()});
        }
        Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
    }
}
// Bounded newline framing; unlike read_line this cannot allocate an unbounded line.
async fn line(reader: &mut BufReader<tokio::io::Stdin>) -> std::io::Result<Option<Vec<u8>>> {
    let mut data = Vec::new();
    let mut oversized = false;
    loop {
        let buf = reader.fill_buf().await?;
        if buf.is_empty() {
            return if data.is_empty() && !oversized {
                Ok(None)
            } else if oversized {
                Ok(Some(Vec::new()))
            } else {
                Ok(Some(data))
            };
        }
        let n = buf
            .iter()
            .position(|b| *b == b'\n')
            .map(|i| i + 1)
            .unwrap_or(buf.len());
        let done = buf[n - 1] == b'\n';
        if data.len() + n > MAX_FRAME {
            oversized = true;
            data.clear();
        } else if !oversized {
            data.extend_from_slice(&buf[..n]);
        }
        reader.consume(n);
        if done {
            return Ok(Some(data));
        }
    }
}
async fn run(args: Args) -> precept_ipc::Result<()> {
    args.location.root()?;
    let mut bridge = Bridge {
        location: args.location,
        bound: None,
        roots: false,
        initialized: false,
        ready: false,
        client: None,
        session: None,
    };
    // Until the client names its workspace folders, the working directory
    // the client started this server in decides.
    let cwd: Vec<_> = std::env::current_dir().into_iter().collect();
    bridge.bind(&cwd).await;
    let mut input = BufReader::new(tokio::io::stdin());
    let mut output = tokio::io::stdout();
    while let Some(raw) = line(&mut input).await.map_err(|_| Error::Unavailable)? {
        let response = match serde_json::from_slice::<Value>(&raw) {
            Ok(v) => bridge.handle(v).await,
            Err(_) => Some(error(
                Value::Null,
                -32700,
                "Malformed input: JSONDecodeError",
            )),
        };
        if let Some(value) = response {
            let mut raw = serde_json::to_vec(&value).map_err(|_| Error::Unavailable)?;
            if raw.len() > MAX_FRAME {
                raw = serde_json::to_vec(&error(value["id"].clone(), -32603, "Response too large"))
                    .map_err(|_| Error::Unavailable)?;
            }
            raw.push(b'\n');
            output
                .write_all(&raw)
                .await
                .map_err(|_| Error::Unavailable)?;
            output.flush().await.map_err(|_| Error::Unavailable)?;
        }
    }
    Ok(())
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run(Args::parse()).await {
        eprintln!("precept-mcp: {error}");
        std::process::exit(2);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schemas_expose_read_suggest_session_and_memory_tools_none_naming_a_project() {
        let v = tools();
        assert_eq!(v.as_array().unwrap().len(), 11);
        for t in v.as_array().unwrap() {
            assert_eq!(t["inputSchema"]["additionalProperties"], false);
            assert!(
                !t["inputSchema"]["properties"]
                    .as_object()
                    .unwrap()
                    .contains_key("project")
            );
        }
    }
    #[test]
    fn memory_replies_mark_notes_as_unreviewed_and_say_whether_to_retry() {
        let ok = memory_reply(
            "memory.recall",
            Ok(json!({"entries":[{"kind":"caution","title":"WSL","id":"ab","body":"slow"}]})),
        );
        let text = ok["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("unreviewed") && text.contains("- [caution] WSL (ab): slow"));
        let none = memory_reply("memory.recall", Ok(json!({"entries":[]})));
        assert!(
            none["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("No memory")
        );
        // Index lines carry no body and say how to fetch the full text.
        let index = memory_reply(
            "memory.recall",
            Ok(json!({"index":true,"entries":[
                {"kind":"caution","title":"WSL","id":"ab","weight":61.0}
            ]})),
        );
        let text = index["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("- [caution] WSL (ab weight 61.0)") && text.contains("recall id="));
        assert!(!text.contains("slow"));
        // A fetch by id and a stale notice.
        let got = memory_reply(
            "memory.get",
            Ok(
                json!({"entry":{"kind":"lesson","title":"Run","id":"cd","body":"cargo test",
                "notice":"Not verified for 9 days; check it."}}),
            ),
        );
        let text = got["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("- [lesson] Run (cd): cargo test")
                && text.contains("Not verified for 9 days")
        );
        for (e, needle) in [
            (Error::Unauthorized, "Do not retry"),
            (Error::NotFound, "recall"),
            (Error::InvalidRequest, "at most 120"),
        ] {
            let r = memory_reply("memory.remember", Err(e));
            assert_eq!(r["isError"], true);
            assert!(r["content"][0]["text"].as_str().unwrap().contains(needle));
        }
    }
    #[test]
    fn roots_name_local_folders_only() {
        let dirs = root_dirs(&json!({"roots":[
            {"uri":"file:///home/u/my%20repo","name":"repo"},
            {"uri":"https://example.invalid/x"}
        ]}));
        assert_eq!(dirs, vec![std::path::PathBuf::from("/home/u/my repo")]);
        assert!(root_dirs(&json!({})).is_empty());
        // A Windows drive URI keeps its drive letter, not a leading slash.
        let dirs = root_dirs(&json!({"roots":[
            {"uri":"file:///C:/Users/u/repo"},
            {"uri":"file:///c%3A/Users/u/other"}
        ]}));
        if cfg!(windows) {
            assert_eq!(
                dirs,
                vec![
                    std::path::PathBuf::from("C:/Users/u/repo"),
                    std::path::PathBuf::from("c:/Users/u/other")
                ]
            );
        } else {
            assert_eq!(dirs.len(), 2);
        }
    }
    #[test]
    fn suggestion_replies_say_it_is_not_active_and_whether_to_retry() {
        let ok = suggestion_reply(Ok(json!({"status":"suggested","id":"7"})));
        assert_eq!(ok["isError"], false);
        let text = ok["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("#7") && text.contains("not active"));
        for (e, needle) in [
            (Error::Unauthorized, "not allowed"),
            (Error::Unavailable, "writes are off"),
            (Error::Conflict, "already"),
            (Error::Capacity, "limit"),
            (Error::InvalidRequest, "Invalid suggestion"),
        ] {
            let r = suggestion_reply(Err(e));
            assert_eq!(r["isError"], true);
            assert!(r["content"][0]["text"].as_str().unwrap().contains(needle));
        }
        assert!(
            tools()
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["name"] == "suggest_rule")
        );
    }
}
