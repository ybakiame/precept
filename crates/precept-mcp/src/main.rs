//! Project-pinned stdio bridge. Only the daemon reads storage/repository data.
use clap::Parser;
use precept_ipc::{Access, Client, Credential, Error, Location, MAX_FRAME, load_credential};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[derive(Parser)]
struct Args {
    #[command(flatten)]
    location: Location,
    #[arg(long)]
    project: String,
    #[arg(long)]
    capability_file: std::path::PathBuf,
}
const LEGACY: &str = "2025-11-25";
const STATELESS: &str = "2026-07-28";
fn info() -> Value {
    json!({"name":"precept","version":"0.1.0a0"})
}
/// Sent once in `initialize`: what the server is for and when to call it.
const INSTRUCTIONS: &str = "Precept serves this project's confirmed engineering context: the developer's reviewed preferences and the repository's contracts. Call get_effective_context before planning or editing anything in this project, and again after your context was compacted. Treat confirmed preferences as the developer's standing instructions; current repository files and contracts outrank them, so report a conflict instead of choosing silently. Call get_handoff with a task id to resume saved work. Both are read-only. When the developer states a lasting preference or you learn a durable project convention, you may file it with suggest_rule: it only becomes a suggestion the developer reviews, never an active rule, and works only where they allowed it.";
fn tools() -> Value {
    let selector = json!({"type":"object","properties":{
        "agent_id":{"type":"string"},"path":{"type":"string"},"language":{"type":"string"},
        "task_tags":{"type":"array","items":{"type":"string"},"maxItems":64}
    },"additionalProperties":false});
    json!([
        {"name":"get_effective_context","description":"Call this first, before planning or editing in this project: returns the developer's confirmed preferences (with tags) and the repository's effective contracts that apply. Optional selector labels (agent_id, path, language, task_tags) narrow applicability; they are not authorization.","inputSchema":{"type":"object","properties":{"include_documents":{"type":"boolean","default":false},"selector":selector},"additionalProperties":false}},
        {"name":"get_handoff","description":"Resume saved work: returns the task's handoff (objective, completed work, next action, recorded checks) with the current git revision and drift warnings. Saved checks are historical; rerun them. Does not grant permissions.","inputSchema":{"type":"object","properties":{"task_id":{"type":"string"}},"required":["task_id"],"additionalProperties":false}},
        {"name":"suggest_rule","description":"File a lasting rule for the developer to review in Precept: a preference they stated, or a durable convention of this project you confirmed. It is NOT active until the developer confirms it, so keep following your current instructions. Use a short kebab-case key and state the rule itself as the value (one imperative sentence or a few lines). Do not file one-off task instructions, secrets or personal data. scope: project (this repository), environment (facts about this machine, for this project) or personal (the developer's habits across projects).","inputSchema":{"type":"object","properties":{
            "scope":{"type":"string","enum":["project","environment","personal"]},
            "key":{"type":"string","maxLength":120},
            "value":{"type":"string","maxLength":4096},
            "tags":{"type":"array","items":{"type":"string"},"maxItems":8,"description":"Short lowercase labels; defaults to [\"agent\"]"},
            "applies_to":{"type":"object","properties":{
                "agents":{"type":"array","items":{"type":"string"}},
                "languages":{"type":"array","items":{"type":"string"}},
                "paths":{"type":"array","items":{"type":"string"}},
                "task_tags":{"type":"array","items":{"type":"string"}}
            },"additionalProperties":false},
            "enforcement":{"type":"string","enum":["advisory","verify","hard"]}
        },"required":["scope","key","value"],"additionalProperties":false}}
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
        Err(Error::ScopeMismatch | Error::InvalidRequest | Error::InvalidTransition) => {
            "Invalid suggestion: check the key (short, kebab-case), value, tags, applies_to and enforcement (environment notes are advisory only)."
        }
        Err(_) => {
            "Precept could not file the suggestion. Do not retry; mention the rule in your reply instead."
        }
    };
    tool_error(text)
}
struct Bridge {
    location: Location,
    credential: Credential,
    initialized: bool,
    ready: bool,
    /// The client's self-reported `clientInfo` name and version, passed to
    /// the daemon's activity log only (bounded, printable).
    client: Option<String>,
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
/// One sanitized diagnostic line on stderr (never stdout, never arguments,
/// values or paths): MCP clients show a server's stderr in their own logs.
fn diagnose(what: &str) {
    eprintln!("precept-mcp: {what}");
}
impl Bridge {
    async fn tool(&self, name: &Value, args: &Value) -> Result<Value, Value> {
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
            Some("suggest_rule") => {
                const KEYS: [&str; 6] =
                    ["scope", "key", "value", "tags", "applies_to", "enforcement"];
                if args.keys().any(|k| !KEYS.contains(&k.as_str()))
                    || !["scope", "key", "value"]
                        .iter()
                        .all(|k| args.get(*k).is_some_and(Value::is_string))
                {
                    diagnose("suggest_rule refused: invalid arguments");
                    return Ok(tool_error(
                        "scope, key and value are required strings; only tags, applies_to and enforcement may be added",
                    ));
                }
                ("preference.suggest", Value::Object(args.clone()))
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
        let result = match Client::connect(&self.location, &self.credential).await {
            Ok(mut client) => client.request(method, params).await,
            Err(e) => Err(e),
        };
        if let Err(e) = &result {
            diagnose(&format!("{method} failed: {e}"));
        }
        if method == "preference.suggest" {
            return Ok(suggestion_reply(result));
        }
        Ok(match result {
            Ok(data) => {
                json!({"content":[{"type":"text","text":data.to_string()}],"isError":false})
            }
            Err(Error::NotFound) => tool_error("Handoff not found"),
            Err(_) => tool_error("IPC unavailable or request denied"),
        })
    }
    async fn handle(&mut self, message: Value) -> Option<Value> {
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        if !message.is_object() || message.get("jsonrpc") != Some(&json!("2.0")) {
            return Some(error(id, -32600, "Invalid JSON-RPC request"));
        }
        if message.get("id").is_none() {
            if message.get("method") == Some(&json!("notifications/initialized")) {
                self.ready = self.initialized;
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
    if !precept_ipc::valid_project(&args.project) {
        return Err(Error::InvalidRequest);
    }
    let credential = load_credential(&args.location, &args.capability_file)?;
    if credential.access
        != (Access::Agent {
            project: args.project,
        })
    {
        return Err(Error::Unauthorized);
    }
    let mut client = Client::connect(&args.location, &credential).await?;
    client.request("daemon.health", json!({})).await?;
    drop(client);
    diagnose("connected to the daemon; waiting for the MCP client");
    let mut bridge = Bridge {
        location: args.location,
        credential,
        initialized: false,
        ready: false,
        client: None,
    };
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
    fn schemas_expose_two_read_tools_and_suggest_rule_none_naming_a_project() {
        let v = tools();
        assert_eq!(v.as_array().unwrap().len(), 3);
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
