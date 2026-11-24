use crate::{
    binding::{UNBOUND, client_label, resolve, root_dirs},
    protocol::{
        INSTRUCTIONS, LEGACY, STATELESS, error, info, memory_reply, suggestion_reply, tool_error,
        tools,
    },
    work_session,
};
use precept_ipc::{Client, Credential, Error, Location};
use serde_json::{Value, json};
use std::path::PathBuf;

/// The id of the one request this server sends: `roots/list`.
const ROOTS_REQUEST: &str = "precept-roots";

pub(super) struct Bridge {
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

impl Bridge {
    pub(super) fn new(location: Location) -> Self {
        Self {
            location,
            bound: None,
            roots: false,
            initialized: false,
            ready: false,
            client: None,
            session: None,
        }
    }

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
            Some(
                tool @ ("start_session" | "update_work_state" | "close_session"
                | "get_work_session"),
            ) => match work_session::request(tool, args) {
                Ok(request) => request,
                Err(message) => {
                    diagnose("session tool refused: invalid arguments");
                    return Ok(tool_error(&message));
                }
            },
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
                (Ok(v), "session.start" | "session.get") => {
                    self.session = v["id"].as_str().map(str::to_owned);
                }
                (Ok(_), "session.close") => self.session = None,
                _ => {}
            }
            return Ok(work_session::session_reply(result));
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
    pub(super) async fn bind(&mut self, dirs: &[PathBuf]) {
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

    pub(super) async fn handle(&mut self, message: Value) -> Option<Value> {
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

/// One sanitized diagnostic line on stderr (never stdout, never arguments,
/// values or paths): MCP clients show a server's stderr in their own logs.
fn diagnose(what: &str) {
    eprintln!("precept-mcp: {what}");
}
