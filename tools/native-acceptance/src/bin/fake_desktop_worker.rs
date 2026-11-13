//! Owned fake window worker for bounded-RPC tests only. It drives no real window and
//! its PASS never stands in for native GUI acceptance.
use serde_json::{Value, json};
use std::{
    env,
    io::{self, BufRead, Write},
    process::{self, Command, Stdio},
    thread,
    time::Duration,
};

fn reply(rpc_id: &Value) -> String {
    json!({
        "status": "ok",
        "rpc_id": rpc_id,
        "images": "synthetic-images",
        "file": "synthetic-shot.png",
        "foreground": true,
        "input_layout": null
    })
    .to_string()
}

fn emit(line: &str) {
    let out = io::stdout();
    let _ = writeln!(out.lock(), "{line}");
    let _ = out.lock().flush();
}

fn main() {
    let mode = env::args().nth(1).unwrap_or_default();
    // Keeps an inherited stdout pipe open without ever writing to it.
    if mode == "grandchild-hold" {
        thread::sleep(Duration::from_secs(120));
        return;
    }
    // A worker that stops consuming stdin: the harness must still stay bounded.
    if mode == "no-stdin-read" {
        thread::sleep(Duration::from_secs(60));
        return;
    }
    // Leaves the pipe open through a child the harness never owned, then exits itself.
    if mode == "hold-stdout" {
        let holder = env::current_exe()
            .and_then(|exe| {
                Command::new(exe)
                    .arg("grandchild-hold")
                    .stdin(Stdio::null())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::null())
                    .spawn()
            })
            .is_ok();
        if holder {
            // The grandchild must be scheduled before this worker exits, so the pipe
            // is demonstrably held by a process the harness never owned.
            thread::sleep(Duration::from_millis(150));
        }
        return;
    }
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut line = String::new();
    let mut calls = 0u64;
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        calls += 1;
        let request: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
        let rpc_id = request.get("rpc_id").cloned().unwrap_or(Value::Null);
        match mode.as_str() {
            // Reads the request and never settles it.
            "silent" => thread::sleep(Duration::from_secs(3600)),
            // Ends the stream without a response.
            "eof" => return,
            // Terminates abnormally after accepting the request.
            "crash" => process::exit(3),
            "malformed" => emit("not-a-json-response"),
            // Exceeds the bounded reply without ever terminating the line.
            "oversized" => {
                emit(&"a".repeat(200_000));
                thread::sleep(Duration::from_secs(60));
                return;
            }
            // Settles the request only after the caller's deadline has passed.
            "late" => {
                thread::sleep(Duration::from_millis(700));
                emit(&reply(&rpc_id));
            }
            // Two replies for one request, both naming that request.
            "duplicate" => {
                emit(&reply(&rpc_id));
                emit(&reply(&rpc_id));
            }
            // The stale reply for the previous call arrives only after this newer
            // request was written: correlation must refuse it, with no sleep involved.
            "stale-after-next" => {
                if calls > 1 {
                    let previous = rpc_id.as_u64().unwrap_or(1) - 1;
                    emit(&reply(&Value::from(previous)));
                }
                emit(&reply(&rpc_id));
            }
            // A well-formed acknowledgement that omits the field the action needs.
            "shapeless" => emit(&json!({"status":"ok","rpc_id":rpc_id}).to_string()),
            // A reply naming a window this driver does not own.
            "bad-nonce" => {
                emit(&json!({"status":"ok","rpc_id":rpc_id,"nonce":"ffffffffffffffff"}).to_string())
            }
            "refuse" => emit(
                &json!({"status":"refused","rpc_id":rpc_id,"reason":"synthetic refusal"})
                    .to_string(),
            ),
            // A reply that answers no request the caller ever sent.
            "unsolicited" => emit(&reply(&Value::from(9999))),
            _ => emit(&reply(&rpc_id)),
        }
    }
}
