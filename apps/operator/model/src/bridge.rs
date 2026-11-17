//! Windows-native GUI -> WSL2 backend bridge.
//!
//! One long-lived helper per WSL connection, launched with
//! `wsl.exe --distribution D --user U --exec /abs/acp-wsl-bridge --runtime-dir R --endpoint E`.
//! The helper speaks a versioned, length-framed JSON protocol on stdio and accepts
//! only a fixed set of typed requests. It loads Operator/OperatorWrite capabilities
//! inside WSL and calls the same daemon-authorized functions as a local client; it
//! never validates or approves mutations itself, opens SQLite or links the Writer.
//! Capability tokens never appear in argv, environment, frames, stdout or stderr.
use acp_core::operator::Mutation;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};

/// Protocol identity and version, checked in the helper's first frame.
pub const PROTOCOL: &str = "acp-wsl-bridge";
pub const VERSION: u32 = 1;
/// Client -> helper frame limit (a mutation is capped at 128 KiB by the daemon client).
pub const MAX_REQUEST: usize = 256 * 1024;
/// Helper -> client frame limit (a Snapshot is capped at 4 MiB).
pub const MAX_REPLY: usize = 5 * 1024 * 1024;
/// Concurrent in-flight requests the helper accepts (UI reads + session polling).
pub const MAX_IN_FLIGHT: usize = 8;
/// The exact session methods the existing Sessions UI uses.
pub const SESSION_METHODS: [&str; 6] = [
    "session.refresh",
    "session.start",
    "session.prompt",
    "session.permission",
    "session.cancel",
    "session.stop",
];
/// The exact Operator control/read methods (the daemon client re-checks this list).
pub const CONTROL_METHODS: [&str; 10] = [
    "profile.info",
    "profile.create",
    "profile.select",
    "profile.write.enable",
    "profile.write.disable",
    "profile.preferences.list",
    "profile.preferences.get",
    "profile.review.queue",
    "profile.review.inspect",
    "profile.handoff.get",
];
/// Every error string a helper may return. Anything else is replaced by
/// `Unavailable` on the Windows side: helper text is never displayed verbatim.
pub const KNOWN_ERRORS: [&str; 28] = [
    "Busy",
    "Capacity",
    "Conflict",
    "OutcomeUnknown",
    "Unauthorized",
    "RestartRequired",
    "StaleGeneration",
    "OwnerMismatch",
    "IdempotencyConflict",
    "InvalidRequest",
    "InvalidTransition",
    "ReviewMismatch",
    "Unavailable",
    "Unsupported",
    "Version",
    "TooLarge",
    "Cancelled",
    "Permission denied",
    "Local daemon unavailable or access refused",
    "Session unavailable",
    "Session access refused",
    "Invalid session capabilities",
    "Unsupported session version",
    "Session unavailable or request denied",
    "Invalid session state",
    "Invalid task ID",
    "Invalid WSL snapshot",
    "Client unavailable",
];
pub fn known_error(code: &str) -> &'static str {
    KNOWN_ERRORS
        .iter()
        .find(|k| **k == code)
        .copied()
        .unwrap_or("Unavailable")
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub bridge: String,
    pub version: u32,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub id: u64,
    pub op: Op,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    Snapshot {
        project: Option<String>,
        task: String,
    },
    Control {
        method: String,
        params: Value,
    },
    Mutate {
        mutation: Box<Mutation>,
    },
    Session {
        project: String,
        method: String,
        fields: Value,
    },
    /// Abandon an in-flight request. It produces no reply of its own.
    Cancel {
        target: u64,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub version: u32,
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ok: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
impl Reply {
    pub fn ok(id: u64, value: Value) -> Self {
        Self {
            version: VERSION,
            id,
            ok: Some(value),
            error: None,
        }
    }
    pub fn error(id: u64, code: &str) -> Self {
        Self {
            version: VERSION,
            id,
            ok: None,
            error: Some(code.into()),
        }
    }
    /// Exactly one of `ok` / `error`, in this protocol version.
    pub fn well_formed(&self) -> bool {
        self.version == VERSION && self.ok.is_some() != self.error.is_some()
    }
}

/// Fixed request allowlist and shape checks, applied by the helper before any IPC.
pub fn admit(op: &Op) -> Result<(), &'static str> {
    match op {
        Op::Snapshot { project, task } => {
            if project
                .as_deref()
                .is_some_and(|p| !acp_ipc::valid_project(p))
                || (!task.is_empty() && !crate::valid_task(task))
            {
                return Err("InvalidRequest");
            }
        }
        Op::Control { method, params } => {
            if !CONTROL_METHODS.contains(&method.as_str()) {
                return Err("Unsupported");
            }
            if !params.is_object() {
                return Err("InvalidRequest");
            }
        }
        Op::Mutate { .. } => {}
        Op::Session {
            project,
            method,
            fields,
        } => {
            if !SESSION_METHODS.contains(&method.as_str()) {
                return Err("Unsupported");
            }
            if !acp_ipc::valid_project(project) || !fields.is_object() {
                return Err("InvalidRequest");
            }
        }
        Op::Cancel { .. } => {}
    }
    Ok(())
}

/// Length-prefixed (u32 big-endian) frame. Refuses empty or oversized frames.
pub fn write_frame(out: &mut impl Write, body: &[u8], limit: usize) -> std::io::Result<()> {
    if body.is_empty() || body.len() > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "frame size",
        ));
    }
    out.write_all(&(body.len() as u32).to_be_bytes())?;
    out.write_all(body)?;
    out.flush()
}
/// `Ok(None)` is a clean end of stream before a frame starts.
pub fn read_frame(input: &mut impl Read, limit: usize) -> std::io::Result<Option<Vec<u8>>> {
    let mut header = [0u8; 4];
    let mut filled = 0;
    while filled < 4 {
        match input.read(&mut header[filled..])? {
            0 if filled == 0 => return Ok(None),
            0 => return Err(std::io::ErrorKind::UnexpectedEof.into()),
            n => filled += n,
        }
    }
    let size = u32::from_be_bytes(header) as usize;
    if size == 0 || size > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "frame size",
        ));
    }
    let mut body = vec![0u8; size];
    input.read_exact(&mut body)?;
    Ok(Some(body))
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub use native_bridge::*;

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
mod native_bridge {
    use super::*;
    use acp_ipc::Location;
    use std::{
        collections::{HashMap, HashSet},
        process::{Child, ChildStdin, Command, Stdio},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicU64, Ordering},
            mpsc,
        },
        time::Duration,
    };

    // ---------------------------------------------------------------- helper side

    /// WSL-side helper loop: stdin requests, stdout replies, until stdin closes.
    /// Each request runs the same daemon-authorized call a local client makes.
    pub async fn serve(location: Location) -> i32 {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Reply>(64);
        let writer = tokio::spawn(async move {
            let mut out = tokio::io::stdout();
            while let Some(reply) = rx.recv().await {
                let Ok(body) = serde_json::to_vec(&reply) else {
                    continue;
                };
                if body.len() > MAX_REPLY {
                    let fallback =
                        serde_json::to_vec(&Reply::error(reply.id, "TooLarge")).unwrap_or_default();
                    if write_async(&mut out, &fallback).await.is_err() {
                        return;
                    }
                    continue;
                }
                if write_async(&mut out, &body).await.is_err() {
                    return;
                }
            }
            let _ = out.flush().await;
        });
        async fn write_async(out: &mut tokio::io::Stdout, body: &[u8]) -> std::io::Result<()> {
            out.write_all(&(body.len() as u32).to_be_bytes()).await?;
            out.write_all(body).await?;
            out.flush().await
        }
        let hello = Hello {
            bridge: PROTOCOL.into(),
            version: VERSION,
        };
        // The hello is a Reply-shaped frame with id 0 so the client reads one schema.
        if tx
            .send(Reply::ok(
                0,
                serde_json::to_value(hello).unwrap_or_default(),
            ))
            .await
            .is_err()
        {
            return 2;
        }
        let tasks: Arc<Mutex<HashMap<u64, tokio::task::AbortHandle>>> = Arc::default();
        let mut input = tokio::io::stdin();
        let code = loop {
            let mut header = [0u8; 4];
            match input.read_exact(&mut header).await {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break 0,
                Err(_) => break 2,
            }
            let size = u32::from_be_bytes(header) as usize;
            if size == 0 || size > MAX_REQUEST {
                // Frame boundaries are lost; refuse and stop instead of resynchronizing.
                let _ = tx.send(Reply::error(0, "TooLarge")).await;
                break 2;
            }
            let mut body = vec![0u8; size];
            if input.read_exact(&mut body).await.is_err() {
                break 2;
            }
            let Ok(request) = serde_json::from_slice::<Request>(&body) else {
                let _ = tx.send(Reply::error(0, "InvalidRequest")).await;
                continue;
            };
            if request.version != VERSION {
                let _ = tx.send(Reply::error(request.id, "Version")).await;
                continue;
            }
            if request.id == 0 {
                let _ = tx.send(Reply::error(0, "InvalidRequest")).await;
                continue;
            }
            if let Op::Cancel { target } = request.op {
                // Take the handle and release the lock before any await.
                let handle = tasks.lock().unwrap().remove(&target);
                if let Some(handle) = handle {
                    handle.abort();
                    let _ = tx.send(Reply::error(target, "Cancelled")).await;
                }
                continue;
            }
            if let Err(code) = admit(&request.op) {
                let _ = tx.send(Reply::error(request.id, code)).await;
                continue;
            }
            let refusal = {
                let running = tasks.lock().unwrap();
                if running.contains_key(&request.id) {
                    Some("InvalidRequest")
                } else if running.len() >= MAX_IN_FLIGHT {
                    Some("Busy")
                } else {
                    None
                }
            };
            if let Some(code) = refusal {
                let _ = tx.send(Reply::error(request.id, code)).await;
                continue;
            }
            let id = request.id;
            let location = location.clone();
            let reply_to = tx.clone();
            let registry = tasks.clone();
            let handle = tokio::spawn(async move {
                let reply = match execute(&location, request.op).await {
                    Ok(value) => Reply::ok(id, value),
                    Err(code) => Reply::error(id, code),
                };
                // Removing first means a racing Cancel finds nothing to abort.
                if registry.lock().unwrap().remove(&id).is_some() {
                    let _ = reply_to.send(reply).await;
                }
            });
            tasks.lock().unwrap().insert(id, handle.abort_handle());
            if handle.is_finished() {
                tasks.lock().unwrap().remove(&id);
            }
        };
        // Stdin closed (GUI closed or bridge replaced): let in-flight requests,
        // including a submitted mutation, finish briefly, then exit. The daemon
        // and all Profile/auth/runtime files are untouched.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while !tasks.lock().unwrap().is_empty() && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        drop(tx);
        let _ = tokio::time::timeout(Duration::from_secs(2), writer).await;
        code
    }

    async fn execute(location: &Location, op: Op) -> Result<Value, &'static str> {
        match op {
            Op::Snapshot { project, task } => {
                crate::native::snapshot_at(location, project.as_deref(), &task)
                    .await
                    .and_then(|s| serde_json::to_value(s).map_err(|_| "Unavailable"))
            }
            Op::Control { method, params } => {
                crate::operator::operator_at(location, &method, params, None).await
            }
            Op::Mutate { mutation } => {
                crate::operator::operator_at(
                    location,
                    "profile.mutate",
                    Value::Null,
                    Some(&mutation),
                )
                .await
            }
            Op::Session {
                project,
                method,
                fields,
            } => crate::session::session_raw(location, &project, &method, fields).await,
            Op::Cancel { .. } => Err("InvalidRequest"),
        }
    }

    // ---------------------------------------------------------------- client side

    /// Identity of one WSL connection; a different key replaces the bridge.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Key {
        pub distribution: String,
        pub user: String,
        pub helper: String,
        pub runtime: String,
        pub endpoint: String,
    }
    /// Why a call produced no daemon answer. `Lost`/`Timeout` happen only after
    /// the request was (possibly) delivered, so a mutation's outcome is unknown.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CallError {
        NotSent,
        Lost,
        Timeout,
        Remote(&'static str),
    }
    struct Inner {
        stdin: Mutex<Option<ChildStdin>>,
        child: Mutex<Child>,
        pending: Mutex<HashMap<u64, mpsc::SyncSender<Reply>>>,
        abandoned: Mutex<HashSet<u64>>,
        next: AtomicU64,
        alive: AtomicBool,
    }
    #[derive(Clone)]
    pub struct Client {
        inner: Arc<Inner>,
    }
    impl Client {
        /// Starts `command` with piped stdio and waits for the protocol hello.
        pub fn connect(
            mut command: Command,
            hello_timeout: Duration,
        ) -> Result<Self, &'static str> {
            command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            let mut child = command.spawn().map_err(|_| "WSL bridge unavailable")?;
            let stdin = child.stdin.take();
            let mut stdout = child.stdout.take().ok_or("WSL bridge unavailable")?;
            let inner = Arc::new(Inner {
                stdin: Mutex::new(stdin),
                child: Mutex::new(child),
                pending: Mutex::default(),
                abandoned: Mutex::default(),
                next: AtomicU64::new(1),
                alive: AtomicBool::new(true),
            });
            let (hello_tx, hello_rx) = mpsc::sync_channel::<bool>(1);
            let reader = inner.clone();
            std::thread::spawn(move || {
                let hello = read_frame(&mut stdout, MAX_REPLY)
                    .ok()
                    .flatten()
                    .and_then(|b| serde_json::from_slice::<Reply>(&b).ok())
                    .filter(|r| r.id == 0 && r.well_formed())
                    .and_then(|r| r.ok)
                    .and_then(|v| serde_json::from_value::<Hello>(v).ok())
                    .is_some_and(|h| {
                        h == Hello {
                            bridge: PROTOCOL.into(),
                            version: VERSION,
                        }
                    });
                let _ = hello_tx.send(hello);
                if !hello {
                    reader.shutdown();
                    return;
                }
                while let Ok(Some(body)) = read_frame(&mut stdout, MAX_REPLY) {
                    let Some(reply) = serde_json::from_slice::<Reply>(&body)
                        .ok()
                        .filter(Reply::well_formed)
                    else {
                        break; // Protocol violation: stop trusting this stream.
                    };
                    let waiter = reader.pending.lock().unwrap().remove(&reply.id);
                    match waiter {
                        Some(waiter) => {
                            let _ = waiter.send(reply);
                        }
                        None if reader.abandoned.lock().unwrap().remove(&reply.id) => {}
                        None => break, // Unsolicited or mismatched request ID.
                    }
                }
                reader.shutdown();
            });
            match hello_rx.recv_timeout(hello_timeout) {
                Ok(true) => Ok(Self { inner }),
                _ => {
                    inner.shutdown();
                    Err("WSL bridge unavailable")
                }
            }
        }
        pub fn alive(&self) -> bool {
            self.inner.alive.load(Ordering::SeqCst)
        }
        /// One typed request. Never retried here.
        pub fn call(&self, op: Op, timeout: Duration) -> Result<Value, CallError> {
            if !self.alive() {
                return Err(CallError::NotSent);
            }
            let id = self.inner.next.fetch_add(1, Ordering::SeqCst);
            let body = serde_json::to_vec(&Request {
                version: VERSION,
                id,
                op,
            })
            .map_err(|_| CallError::NotSent)?;
            if body.len() > MAX_REQUEST {
                return Err(CallError::Remote("TooLarge"));
            }
            let (tx, rx) = mpsc::sync_channel(1);
            self.inner.pending.lock().unwrap().insert(id, tx);
            {
                let mut stdin = self.inner.stdin.lock().unwrap();
                let written = match stdin.as_mut() {
                    Some(s) => write_frame(s, &body, MAX_REQUEST),
                    None => {
                        self.inner.pending.lock().unwrap().remove(&id);
                        return Err(CallError::NotSent);
                    }
                };
                if written.is_err() {
                    drop(stdin);
                    self.inner.pending.lock().unwrap().remove(&id);
                    self.inner.shutdown();
                    // Some bytes may have reached the helper.
                    return Err(CallError::Lost);
                }
            }
            match rx.recv_timeout(timeout) {
                Ok(reply) => match (reply.ok, reply.error) {
                    (Some(value), None) => Ok(value),
                    (None, Some(code)) => Err(CallError::Remote(known_error(&code))),
                    _ => Err(CallError::Lost),
                },
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if self.inner.pending.lock().unwrap().remove(&id).is_some() {
                        self.inner.abandoned.lock().unwrap().insert(id);
                        let _ = self.send_cancel(id);
                    }
                    Err(CallError::Timeout)
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => Err(CallError::Lost),
            }
        }
        fn send_cancel(&self, target: u64) -> std::io::Result<()> {
            let id = self.inner.next.fetch_add(1, Ordering::SeqCst);
            let body = serde_json::to_vec(&Request {
                version: VERSION,
                id,
                op: Op::Cancel { target },
            })?;
            match self.inner.stdin.lock().unwrap().as_mut() {
                Some(s) => write_frame(s, &body, MAX_REQUEST),
                None => Err(std::io::ErrorKind::BrokenPipe.into()),
            }
        }
        /// Closes stdin (the helper exits after in-flight work), then reaps the
        /// process; kills it only if it does not exit in time. The WSL daemon and
        /// all Profile/auth/runtime files are never touched.
        pub fn close(&self, grace: Duration) {
            self.inner.stdin.lock().unwrap().take();
            let deadline = std::time::Instant::now() + grace;
            loop {
                if self
                    .inner
                    .child
                    .lock()
                    .unwrap()
                    .try_wait()
                    .ok()
                    .flatten()
                    .is_some()
                {
                    break;
                }
                if std::time::Instant::now() >= deadline {
                    let _ = self.inner.child.lock().unwrap().kill();
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = self.inner.child.lock().unwrap().wait();
            self.inner.alive.store(false, Ordering::SeqCst);
        }
        #[doc(hidden)]
        pub fn process_id(&self) -> u32 {
            self.inner.child.lock().unwrap().id()
        }
    }
    impl Inner {
        fn shutdown(&self) {
            self.alive.store(false, Ordering::SeqCst);
            self.stdin.lock().unwrap().take();
            let _ = self.child.lock().unwrap().kill();
            // Dropping senders wakes every waiter with `Lost`.
            self.pending.lock().unwrap().clear();
        }
    }

    static ACTIVE: Mutex<Option<(Key, Client)>> = Mutex::new(None);

    /// Returns the live bridge for `key`. Only an explicit user-initiated read
    /// (`start = true`: connect, Reconnect, navigation) may start a new helper;
    /// polling and Operator/Session calls never start or restart one, so a lost
    /// bridge or stopped distribution stays an explicit unavailable state.
    pub fn bridge(
        key: &Key,
        start: bool,
        command: impl FnOnce() -> Result<Command, &'static str>,
    ) -> Result<Client, &'static str> {
        let mut active = ACTIVE.lock().unwrap();
        if let Some((current, client)) = active.as_ref()
            && current == key
            && client.alive()
        {
            return Ok(client.clone());
        }
        if !start {
            return Err("WSL bridge disconnected; reconnect explicitly");
        }
        if let Some((_, old)) = active.take() {
            old.close(Duration::from_secs(3));
        }
        let client = Client::connect(command()?, Duration::from_secs(30))?;
        *active = Some((key.clone(), client.clone()));
        Ok(client)
    }
    /// Closes the active bridge, e.g. when the connection target changes or the
    /// GUI exits. Never stops the WSL daemon.
    pub fn close_active() {
        if let Some((_, client)) = ACTIVE.lock().unwrap().take() {
            client.close(Duration::from_secs(3));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn frames_round_trip_and_refuse_empty_oversized_or_truncated_input() {
        let mut buf = Vec::new();
        write_frame(&mut buf, b"{}", 16).unwrap();
        assert_eq!(read_frame(&mut buf.as_slice(), 16).unwrap().unwrap(), b"{}");
        assert!(write_frame(&mut Vec::new(), b"", 16).is_err());
        assert!(write_frame(&mut Vec::new(), &[b'x'; 17], 16).is_err());
        assert!(read_frame(&mut &[][..], 16).unwrap().is_none());
        let mut oversized = 17u32.to_be_bytes().to_vec();
        oversized.extend([b'x'; 17]);
        assert!(read_frame(&mut oversized.as_slice(), 16).is_err());
        assert!(read_frame(&mut &0u32.to_be_bytes()[..], 16).is_err());
        assert!(read_frame(&mut &[0u8, 0][..], 16).is_err());
        let mut short = 4u32.to_be_bytes().to_vec();
        short.extend(b"{}");
        assert!(read_frame(&mut short.as_slice(), 16).is_err());
    }

    #[test]
    fn requests_are_strict_typed_and_allowlisted() {
        for raw in [
            json!({"version":1,"id":1,"op":{"type":"shell","command":"id"}}),
            json!({"version":1,"id":1,"op":{"type":"control","method":"profile.info","params":{},"extra":1}}),
            json!({"version":1,"id":1,"op":{"type":"snapshot","project":null,"task":""},"token":"x"}),
            json!({"version":1,"op":{"type":"snapshot","project":null,"task":""}}),
        ] {
            assert!(serde_json::from_value::<Request>(raw).is_err());
        }
        let control = |m: &str| Op::Control {
            method: m.into(),
            params: json!({}),
        };
        for m in CONTROL_METHODS {
            assert_eq!(admit(&control(m)), Ok(()));
        }
        for m in [
            "profile.mutate",
            "daemon.shutdown",
            "session.start",
            "project.register",
            "",
        ] {
            assert_eq!(admit(&control(m)), Err("Unsupported"));
        }
        assert_eq!(
            admit(&Op::Control {
                method: "profile.info".into(),
                params: json!([])
            }),
            Err("InvalidRequest")
        );
        let session = |p: &str, m: &str| Op::Session {
            project: p.into(),
            method: m.into(),
            fields: json!({}),
        };
        for m in SESSION_METHODS {
            assert_eq!(admit(&session("alpha", m)), Ok(()));
        }
        for m in [
            "session.list",
            "session.context",
            "session.capabilities",
            "session.exec",
        ] {
            assert_eq!(admit(&session("alpha", m)), Err("Unsupported"));
        }
        assert_eq!(
            admit(&session("../x", "session.start")),
            Err("InvalidRequest")
        );
        assert_eq!(
            admit(&Op::Snapshot {
                project: Some("bad name".into()),
                task: String::new()
            }),
            Err("InvalidRequest")
        );
    }

    #[test]
    fn replies_need_exactly_one_outcome_and_errors_are_sanitized() {
        assert!(Reply::ok(1, json!(1)).well_formed());
        assert!(Reply::error(1, "Busy").well_formed());
        let both = Reply {
            version: 1,
            id: 1,
            ok: Some(json!(1)),
            error: Some("Busy".into()),
        };
        assert!(!both.well_formed());
        let neither = Reply {
            ok: None,
            error: None,
            ..both.clone()
        };
        assert!(!neither.well_formed());
        assert!(
            !Reply {
                version: 2,
                ..Reply::ok(1, json!(1))
            }
            .well_formed()
        );
        assert_eq!(known_error("StaleGeneration"), "StaleGeneration");
        assert_eq!(known_error("/home/user/secret path"), "Unavailable");
    }
}
