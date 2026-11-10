//! Deterministic protocol fixture. This is NOT a real coding agent.
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
fn main() {
    if std::env::var_os("ACP_TEST_PARENT_SENTINEL").is_some() {
        std::process::exit(99);
    }
    let mode = std::env::args().nth(1).unwrap_or_default();
    if mode == "timeout" {
        std::thread::sleep(std::time::Duration::from_secs(60));
        return;
    }
    let mut prompt = Value::Null;
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { return };
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            return;
        };
        match v["method"].as_str() {
            Some("initialize") => reply(
                v["id"].clone(),
                json!({"protocolVersion":if mode=="version"{2}else{1},"agentCapabilities":{"loadSession":false},"agentInfo":{"name":"deterministic-fake-not-coding-agent","version":"1"}}),
            ),
            Some("session/new") => {
                if mode == "auth" {
                    emit(
                        json!({"jsonrpc":"2.0","id":v["id"],"error":{"code":-32000,"message":"Authentication required"}}),
                    );
                } else {
                    reply(v["id"].clone(), json!({"sessionId":"fake-session"}));
                }
            }
            Some("session/prompt") => {
                prompt = v["id"].clone();
                match mode.as_str() {
                    "crash" => std::process::exit(7),
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
                    "cancel" | "cancel-timeout" => emit(
                        json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"fake-session","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Waiting for explicit cancellation"}}}}),
                    ),
                    _ => {
                        emit(
                            json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"fake-session","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Synthetic ACP update 中文"}}}}),
                        );
                        reply(prompt.clone(), json!({"stopReason":"end_turn"}));
                        if mode == "duplicate-response" {
                            reply(prompt.clone(), json!({"stopReason":"end_turn"}));
                        }
                    }
                }
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
