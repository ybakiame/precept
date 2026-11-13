//! Bounded owned native-window worker pipe. Test harness only; no product surface.
//!
//! The synchronous window driver replies with one JSON line per request line, and a
//! reply can block forever once the owned GUI thread stops pumping window messages.
//! This module therefore owns the only deadline that bounds a single call.
use serde_json::Value;
use std::{
    io::{Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// A worker reply larger than this is a stream fault, never buffered input.
pub const MAX_RESPONSE_BYTES: usize = 64 * 1024;
/// Requests are tiny by contract; oversized or framed input never reaches the pipe.
pub const MAX_REQUEST_BYTES: usize = 8 * 1024;
const INIT_DEADLINE: Duration = Duration::from_secs(300);
const CAPTURE_DEADLINE: Duration = Duration::from_secs(180);
const DEFAULT_DEADLINE: Duration = Duration::from_secs(60);
const CLEANUP_WAIT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
pub enum Event {
    Line(String),
    Eof,
    /// The reader stopped on its own: an unreadable, truncated or over-long stream.
    Fault(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cleanup {
    pub reaped: bool,
    pub reader_joined: bool,
}

struct Outstanding {
    seq: u64,
    action: String,
}

/// One request/response pair at a time against a child the caller owns and may kill.
pub struct WorkerRpc {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Event>,
    reader: Option<(JoinHandle<()>, Receiver<()>)>,
    outstanding: Option<Outstanding>,
    fatal: Option<String>,
    cleanup: Option<Cleanup>,
    seq: u64,
    nonce: Option<String>,
    fixed_deadline: Option<Duration>,
}

impl WorkerRpc {
    pub fn spawn(program: &Path, args: &[&str]) -> std::io::Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child.stdin.take().expect("piped stdin");
        let mut stdout = child.stdout.take().expect("piped stdout");
        let (tx, rx) = channel::<Event>();
        let (done_tx, done_rx) = channel::<()>();
        let reader = thread::spawn(move || {
            read_lines(&mut stdout, &tx);
            let _ = done_tx.send(());
        });
        Ok(Self {
            child,
            stdin: Some(stdin),
            rx,
            reader: Some((reader, done_rx)),
            outstanding: None,
            fatal: None,
            cleanup: None,
            seq: 0,
            nonce: None,
            fixed_deadline: None,
        })
    }

    /// Force one deadline for every action; used by the boundedness tests.
    pub fn with_deadline(mut self, deadline: Duration) -> Self {
        self.fixed_deadline = Some(deadline);
        self
    }

    /// A reply that carries a nonce must carry exactly this one.
    pub fn expect_nonce(&mut self, nonce: &str) {
        self.nonce = Some(nonce.to_owned());
    }

    pub fn is_fatal(&self) -> bool {
        self.fatal.is_some()
    }

    pub fn fatal_reason(&self) -> Option<&str> {
        self.fatal.as_deref()
    }

    pub fn reaped(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }

    fn deadline_for(&self, action: &str) -> Duration {
        self.fixed_deadline.unwrap_or(match action {
            "init" => INIT_DEADLINE,
            "capture" => CAPTURE_DEADLINE,
            _ => DEFAULT_DEADLINE,
        })
    }

    /// Send one request and settle it with exactly one validated response, or fail
    /// inside its deadline. A failure makes the session unusable by design: a late
    /// reply must never be read as a later action's acknowledgement.
    pub fn request(&mut self, payload: &Value) -> Result<Value, String> {
        if let Some(reason) = &self.fatal {
            return Err(format!("worker session unusable after {reason}"));
        }
        let action = payload
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| "request carries no action".to_string())?
            .to_owned();
        let line =
            serde_json::to_string(payload).map_err(|_| "request is not encodable".to_string())?;
        if line.len() > MAX_REQUEST_BYTES || line.contains('\n') || line.contains('\r') {
            return Err(format!("request line out of bounds action={action}"));
        }
        // Anything already queued while idle means the worker spoke out of turn.
        match self.rx.try_recv() {
            Ok(Event::Line(_)) => {
                return Err(self.poison("unsolicited worker response while idle"));
            }
            Ok(Event::Eof) => return Err(self.poison("worker ended while idle")),
            Ok(Event::Fault(detail)) => {
                return Err(self.poison(format!("worker stream fault {detail}")));
            }
            Err(TryRecvError::Disconnected) => {
                return Err(self.poison("reader stopped while idle"));
            }
            Err(TryRecvError::Empty) => {}
        }
        self.seq += 1;
        let deadline = Instant::now() + self.deadline_for(&action);
        self.outstanding = Some(Outstanding {
            seq: self.seq,
            action: action.clone(),
        });
        let input = self
            .stdin
            .as_mut()
            .ok_or_else(|| "worker stdin is closed".to_string())?;
        if writeln!(input, "{line}")
            .and_then(|()| input.flush())
            .is_err()
        {
            return Err(self.poison(format!("worker stdin failed action={action}")));
        }
        // Exactly one settled response per request. `recv_timeout(ZERO)` returns
        // immediately once the deadline has passed, so this cannot block unbounded.
        match self
            .rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            Ok(Event::Line(text)) => {
                let outcome = serde_json::from_str::<Value>(&text)
                    .map_err(|_| format!("malformed json response bytes={}", text.len()))
                    .and_then(|value| {
                        validate(&value, self.nonce.as_deref())?;
                        require_shape(&value, &action)?;
                        Ok(value)
                    });
                match outcome {
                    Ok(value) => {
                        self.outstanding = None;
                        Ok(value)
                    }
                    Err(reason) => Err(self.poison(reason)),
                }
            }
            Ok(Event::Eof) => Err(self.poison(self.pending_reason("worker eof"))),
            Ok(Event::Fault(detail)) => {
                Err(self.poison(self.pending_reason(&format!("stream fault {detail}"))))
            }
            Err(RecvTimeoutError::Timeout) => {
                Err(self.poison(self.pending_reason("deadline exceeded")))
            }
            Err(RecvTimeoutError::Disconnected) => {
                Err(self.poison(self.pending_reason("reader stopped")))
            }
        }
    }

    fn pending_reason(&self, detail: &str) -> String {
        match &self.outstanding {
            Some(pending) => format!("{detail} seq={} action={}", pending.seq, pending.action),
            None => detail.to_owned(),
        }
    }

    /// Record the sanitized cause, then terminate and reap only this owned child.
    fn poison(&mut self, reason: impl Into<String>) -> String {
        let reason = reason.into();
        if self.fatal.is_none() {
            let announced = self.pending_reason(&reason);
            println!("DESKTOP_RPC_FATAL {announced}");
            self.fatal = Some(reason);
        }
        self.outstanding = None;
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.fatal.clone().expect("cause recorded above")
    }

    /// Close stdin, let the owned worker exit, kill it if it does not, then join the
    /// reader thread instead of detaching a blocked one.
    pub fn shutdown(&mut self) -> Cleanup {
        if let Some(done) = self.cleanup {
            return done;
        }
        self.stdin = None;
        let until = Instant::now() + CLEANUP_WAIT;
        while Instant::now() < until {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) => thread::sleep(Duration::from_millis(20)),
            }
        }
        let _ = self.child.kill();
        let reaped = self
            .child
            .try_wait()
            .map(|waited| waited.is_some())
            .unwrap_or(false);
        let mut reader_joined = false;
        if let Some((handle, done)) = self.reader.take() {
            reader_joined = done.recv_timeout(CLEANUP_WAIT).is_ok();
            if reader_joined {
                let _ = handle.join();
            }
        }
        println!("DESKTOP_WORKER_CLEANUP reaped={reaped} reader_joined={reader_joined}");
        let done = Cleanup {
            reaped,
            reader_joined,
        };
        self.cleanup = Some(done);
        done
    }
}

impl Drop for WorkerRpc {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn read_lines(stream: &mut impl Read, tx: &Sender<Event>) {
    let mut line: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => {
                if line.is_empty() {
                    let _ = tx.send(Event::Eof);
                } else {
                    let _ = tx.send(Event::Fault(format!(
                        "truncated reply bytes={}",
                        line.len()
                    )));
                }
                return;
            }
            Ok(size) => {
                for byte in &chunk[..size] {
                    if *byte != b'\n' {
                        line.push(*byte);
                        if line.len() > MAX_RESPONSE_BYTES {
                            let _ = tx.send(Event::Fault(format!(
                                "oversized response line bytes>{}",
                                MAX_RESPONSE_BYTES
                            )));
                            return;
                        }
                        continue;
                    }
                    let Ok(text) = String::from_utf8(std::mem::take(&mut line)) else {
                        let _ = tx.send(Event::Fault("non-utf8 response line".to_owned()));
                        return;
                    };
                    if tx.send(Event::Line(text)).is_err() {
                        return;
                    }
                }
            }
            Err(error) => {
                let _ = tx.send(Event::Fault(format!("reader io {}", error.kind())));
                return;
            }
        }
    }
}

fn validate(value: &Value, expected_nonce: Option<&str>) -> Result<(), String> {
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| "response carries no status".to_string())?;
    if !matches!(status, "ok" | "foreground_required" | "refused") {
        return Err(format!("unrecognized response status {status}"));
    }
    if let Some(nonce) = value.get("nonce") {
        let observed = nonce
            .as_str()
            .ok_or_else(|| "response nonce is not a string".to_string())?;
        if expected_nonce != Some(observed) {
            return Err("mismatched response nonce".to_owned());
        }
    }
    match status {
        "refused" => value
            .get("reason")
            .and_then(Value::as_str)
            .map(|_| ())
            .ok_or_else(|| "refusal without reason".to_string()),
        "foreground_required" => {
            if value.get("nonce").and_then(Value::as_str).is_some() {
                Ok(())
            } else {
                Err("foreground-required without nonce".to_owned())
            }
        }
        _ => Ok(()),
    }
}

/// Enforce the acknowledgement fields the owned window driver must produce.
pub fn require_shape(value: &Value, action: &str) -> Result<(), String> {
    if value["status"] != "ok" {
        return Ok(());
    }
    for (field, present) in match action {
        "init" => vec![("images", has_text(value, "images"))],
        "capture" => vec![("file", has_text(value, "file"))],
        "observe" => vec![("foreground", value.get("foreground").is_some())],
        _ => return Ok(()),
    } {
        if !present {
            return Err(format!("{action} acknowledgement lacks {field}"));
        }
    }
    Ok(())
}

fn has_text(value: &Value, field: &str) -> bool {
    value
        .get(field)
        .and_then(Value::as_str)
        .is_some_and(|text| !text.is_empty())
}
