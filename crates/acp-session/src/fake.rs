//! Deterministic protocol fixture. This is NOT a real coding agent.
mod fake_mcp;
use serde_json::{Value, json};
use std::io::{BufRead, Write};
fn emit(v: Value) {
    let mut out = std::io::stdout().lock();
    writeln!(out, "{v}").unwrap();
    out.flush().unwrap();
}
fn reply(id: Value, result: Value) {
    emit(json!({"jsonrpc":"2.0","id":id,"result":result}));
}
fn initialized(mode: &str) -> Value {
    let mut result = json!({"protocolVersion":if mode=="version"{2}else{1},
        "agentCapabilities":{"loadSession":false,"promptCapabilities":{"image":true,"audio":false,"embeddedContext":true},"mcpCapabilities":{"http":true,"sse":false},"auth":{"logout":{}}},
        "agentInfo":{"name":"deterministic-fake-not-coding-agent","version":"1"},
        "authMethods":[{"id":"login","type":"terminal","name":"SENSITIVE_AUTH_TEST","description":"SENSITIVE_AUTH_TEST","args":["--SENSITIVE_AUTH_TEST"],"env":{"KEY":"SENSITIVE_AUTH_TEST"},"_meta":{"terminal-auth":{"args":["--SENSITIVE_AUTH_TEST"]}}}]});
    match mode {
        "auth-malformed" => result["authMethods"] = json!("SENSITIVE_AUTH_TEST"),
        "auth-duplicate" => result["authMethods"] = json!([{"id":"login"},{"id":"login"}]),
        "auth-unknown-type" => result["authMethods"][0]["type"] = json!("future-login"),
        "auth-bad-id" => result["authMethods"][0]["id"] = json!("SENSITIVE_AUTH_TEST\n"),
        "auth-bad-type" => result["authMethods"][0]["type"] = json!(42),
        "auth-huge-list" => {
            result["authMethods"] = json!(
                (0..9)
                    .map(|n| json!({"id":format!("login-{n}")}))
                    .collect::<Vec<_>>()
            )
        }
        "auth-bad-capability" => {
            result["agentCapabilities"]["promptCapabilities"]["image"] =
                json!("SENSITIVE_AUTH_TEST")
        }
        _ => {}
    }
    result
}
fn metadata(session: &str, kind: &str) {
    emit(
        json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":session,"update":{"sessionUpdate":kind,"SENSITIVE_AUTH_TEST":"SENSITIVE_AUTH_TEST"}}}),
    );
}
fn main() {
    if std::env::var_os("ACP_TEST_PARENT_SENTINEL").is_some() {
        std::process::exit(99);
    }
    let mode = std::env::args().nth(1).unwrap_or_default();
    // Dedicated Rust negative tests can receive the real read-only attachment
    // while exercising malformed/auth/permission responses, without calling tools.
    let accepts_context = std::env::args().nth(2).as_deref() == Some("--test-context");
    if mode == "timeout" {
        std::thread::sleep(std::time::Duration::from_secs(60));
        return;
    }
    let mut prompt = Value::Null;
    let mut mcp = None;
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { return };
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            return;
        };
        match v["method"].as_str() {
            Some("initialize") => reply(v["id"].clone(), initialized(&mode)),
            Some("session/new") => {
                if !mode.starts_with("mcp")
                    && !accepts_context
                    && v["params"]["mcpServers"] != json!([])
                {
                    std::process::exit(9);
                }
                if mode == "mcp-reject" {
                    emit(
                        json!({"jsonrpc":"2.0","id":v["id"],"error":{"code":-32602,"message":"SENSITIVE_AUTH_TEST"}}),
                    );
                    continue;
                }
                if mode == "mcp" {
                    mcp = Some(fake_mcp::Mcp::new(&v["params"]["mcpServers"]));
                }
                let ready = std::env::var_os("HOME")
                    .is_some_and(|p| std::path::Path::new(&p).join(".p1h-ready").is_file());
                if mode == "auth" || (mode == "auth-recover" && !ready) {
                    emit(
                        json!({"jsonrpc":"2.0","id":v["id"],"error":{"code":-32000,"message":"Authentication required"}}),
                    );
                } else if mode == "auth-rejected" {
                    emit(
                        json!({"jsonrpc":"2.0","id":v["id"],"error":{"code":-32602,"message":"SENSITIVE_AUTH_TEST","data":{"token":"SENSITIVE_AUTH_TEST"}}}),
                    );
                } else {
                    if mode.starts_with("metadata-") {
                        let count = if mode == "metadata-overflow" { 9 } else { 1 };
                        for _ in 0..count {
                            metadata(
                                if mode == "metadata-wrong-scope" {
                                    "other-session"
                                } else {
                                    "fake-session"
                                },
                                "available_commands_update",
                            );
                        }
                    }
                    reply(v["id"].clone(), json!({"sessionId":"fake-session"}));
                    if mode == "metadata-early" {
                        metadata("fake-session", "current_mode_update");
                    }
                }
            }
            Some("session/prompt") => {
                prompt = v["id"].clone();
                match mode.as_str() {
                    "auth-midturn" => emit(
                        json!({"jsonrpc":"2.0","id":v["id"],"error":{"code":-32000,"message":"SENSITIVE_AUTH_TEST"}}),
                    ),
                    "auth-unsupported" => emit(
                        json!({"jsonrpc":"2.0","id":"unsupported-auth","method":"authenticate","params":{"methodId":"login","token":"SENSITIVE_AUTH_TEST"}}),
                    ),
                    "mcp-unsupported-notification" => emit(
                        json!({"jsonrpc":"2.0","method":"_UNTRUSTED_SHOULD_NOT_BE_LOGGED","params":{"token":"SENSITIVE_AUTH_TEST"}}),
                    ),
                    "mcp" => {
                        mcp.as_mut().unwrap().call_both();
                        emit(
                            json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"fake-session","update":{"sessionUpdate":"tool_call","status":"completed"}}}),
                        );
                        reply(prompt.clone(), json!({"stopReason":"end_turn"}));
                    }
                    "crash" | "mcp-crash" => std::process::exit(7),
                    "malformed" => {
                        println!("not-json");
                        std::io::stdout().flush().unwrap();
                    }
                    "huge" => {
                        println!("{}", "x".repeat(65537));
                        std::io::stdout().flush().unwrap();
                    }
                    "wrong-id" => reply(json!(999), json!({"stopReason":"end_turn"})),
                    "wrong-session" => emit(
                        json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"other","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"must not leak"}}}}),
                    ),
                    "permission" | "malformed-permission" | "duplicate" => {
                        let request = json!({"jsonrpc":"2.0","id":format!("permission-{prompt}"),"method":"session/request_permission","params":{"sessionId":"fake-session","toolCall":{"toolCallId":"call-1","title":"Synthetic test operation","kind":"read","rawInput":{"fixture":true}},"options":if mode=="malformed-permission"{json!([{"bad":true}])}else{json!([{"optionId":"once","name":"Allow one synthetic operation","kind":"allow_once"},{"optionId":"always","name":"Always","kind":"allow_always"}])}}});
                        emit(request.clone());
                        if mode == "duplicate" {
                            emit(request)
                        }
                    }
                    "cancel" | "cancel-timeout" | "mcp-cancel" => emit(
                        json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"fake-session","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Waiting for explicit cancellation"}}}}),
                    ),
                    _ => {
                        emit(
                            json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"fake-session","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Synthetic ACP update 中文"}}}}),
                        );
                        reply(prompt.clone(), json!({"stopReason":"end_turn"}));
                        if mode == "metadata-early" {
                            metadata("fake-session", "usage_update");
                        }
                        if mode == "duplicate-response" {
                            reply(prompt.clone(), json!({"stopReason":"end_turn"}));
                        }
                    }
                }
            }
            Some("authenticate" | "logout") => std::process::exit(9),
            None if v["id"] == "unsupported-auth" => {
                if v["error"]["code"] != -32601 {
                    std::process::exit(9);
                }
                reply(prompt.clone(), json!({"stopReason":"end_turn"}));
            }
            Some("session/cancel") => {
                if !prompt.is_null() && mode != "cancel-timeout" {
                    reply(prompt.clone(), json!({"stopReason":"cancelled"}));
                    prompt = Value::Null;
                }
            }
            None if v["id"]
                .as_str()
                .is_some_and(|s| s.starts_with("permission-")) =>
            {
                let allowed = v["result"]["outcome"]["optionId"] == "once";
                emit(
                    json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"fake-session","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":if allowed{"Permission explicitly allowed once"}else{"Permission denied"}}}}}),
                );
                reply(prompt.clone(), json!({"stopReason":"end_turn"}));
                prompt = Value::Null;
            }
            _ => {}
        }
    }
}
