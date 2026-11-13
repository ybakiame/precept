//! Owned fake window worker for bounded-RPC tests only. It drives no real window and
//! its PASS never stands in for native GUI acceptance.
use std::{
    io::{self, BufRead, Write},
    thread,
    time::Duration,
};

const HEALTHY: &str = "{\"status\":\"ok\",\"images\":\"synthetic-images\",\"file\":\"synthetic-shot.png\",\"foreground\":true,\"input_layout\":null}";

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut out = io::stdout();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        match mode.as_str() {
            // Reads the request and never settles it.
            "silent" => thread::sleep(Duration::from_secs(3600)),
            // Ends the stream without a response.
            "eof" => return,
            "malformed" => {
                writeln!(out, "not-a-json-response").unwrap();
                out.flush().unwrap();
            }
            // Exceeds the bounded reply without ever terminating the line.
            "oversized" => {
                let _ = write!(out, "{}", "a".repeat(200_000));
                let _ = out.flush();
                thread::sleep(Duration::from_secs(60));
                return;
            }
            // Settles the request only after the caller's deadline has passed.
            "late" => {
                thread::sleep(Duration::from_millis(700));
                writeln!(out, "{HEALTHY}").unwrap();
                out.flush().unwrap();
            }
            // Two replies for one request: the second must never be read as the
            // acknowledgement of the next action.
            "duplicate" => {
                writeln!(out, "{HEALTHY}").unwrap();
                writeln!(out, "{HEALTHY}").unwrap();
                out.flush().unwrap();
            }
            // A well-formed acknowledgement carrying no nonce for an action that
            // must produce one.
            "shapeless" => {
                writeln!(out, "{{\"status\":\"ok\"}}").unwrap();
                out.flush().unwrap();
            }
            // A reply naming a window this driver does not own.
            "bad-nonce" => {
                writeln!(out, "{{\"status\":\"ok\",\"nonce\":\"ffffffffffffffff\"}}").unwrap();
                out.flush().unwrap();
            }
            "refuse" => {
                writeln!(
                    out,
                    "{{\"status\":\"refused\",\"reason\":\"synthetic refusal\"}}"
                )
                .unwrap();
                out.flush().unwrap();
            }
            _ => {
                writeln!(out, "{HEALTHY}").unwrap();
                out.flush().unwrap();
            }
        }
    }
}
