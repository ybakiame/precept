//! Bounded owned native-window worker pipe. Test harness only; no product surface.
//!
//! The synchronous window driver replies with one JSON line per request line, and a
//! reply can block forever once the owned GUI thread stops pumping window messages.
//! Both directions are therefore bounded and cancelable here: every request carries an
//! `rpc_id` that the worker must echo, the caller waits on that correlation instead of
//! on pipe luck, and both IO threads stop through an explicit flag so teardown joins
//! them rather than abandoning them.
use serde_json::Value;
use std::{
    io::{ErrorKind, Read, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel},
    },
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
/// Poll interval for the cooperative IO threads and their queues.
const POLL: Duration = Duration::from_millis(2);
/// Teardown waits no longer than this for a thread that ignores the stop flag, and
/// reports it as unjoined instead of pretending completion.
const JOIN_WAIT: Duration = Duration::from_secs(10);
/// Grace period for a worker to notice that its stdin closed before it is killed.
const EXIT_WAIT: Duration = Duration::from_millis(250);

#[derive(Clone, Debug)]
pub enum Event {
    Line(String),
    Eof,
    /// The reader stopped on its own: an unreadable, truncated or over-long stream.
    Fault(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    Done,
    /// The pipe refused the bytes, or the deadline passed while it was full.
    Failed,
    /// The caller cancelled or tore the session down before the write finished.
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cleanup {
    pub reaped: bool,
    pub reader_joined: bool,
    pub writer_joined: bool,
}

/// Harness test seam: bytes plus the instant by which the write must be finished.
struct Job {
    bytes: Vec<u8>,
    deadline: Instant,
    generation: u64,
}

struct Outstanding {
    seq: u64,
    action: String,
}

/// One request/response pair at a time against a child the caller owns and may kill.
pub struct WorkerRpc {
    child: Child,
    event_rx: Receiver<Event>,
    job_tx: Sender<Job>,
    ack_rx: Receiver<WriteOutcome>,
    /// Cancel the in-flight write; set by the caller before abandoning a request.
    cancel_write: Arc<AtomicBool>,
    /// Generation of the newest accepted write; older results are stale.
    accepted: Arc<AtomicGeneration>,
    stop: Arc<AtomicBool>,
    reader: Option<(JoinHandle<()>, Receiver<()>)>,
    writer: Option<(JoinHandle<()>, Receiver<()>)>,
    outstanding: Option<Outstanding>,
    fatal: Option<String>,
    cleanup: Option<Cleanup>,
    io_modes: (&'static str, &'static str),
    seq: u64,
    nonce: Option<String>,
    fixed_deadline: Option<Duration>,
}

/// Newest accepted write generation, shared with the writer thread.
#[derive(Debug)]
pub struct AtomicGeneration {
    newest: std::sync::atomic::AtomicU64,
}

impl Default for AtomicGeneration {
    fn default() -> Self {
        Self {
            newest: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

impl AtomicGeneration {
    fn publish(&self, generation: u64) {
        self.newest.store(generation, Ordering::SeqCst);
    }

    fn is_stale(&self, generation: u64) -> bool {
        self.newest.load(Ordering::SeqCst) > generation
    }
}

impl WorkerRpc {
    pub fn spawn(program: &Path, args: &[&str]) -> std::io::Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut stdin = child.stdin.take().expect("piped stdin");
        let mut stdout = child.stdout.take().expect("piped stdout");
        let stdin_mode = set_write_nonblocking(&mut stdin);
        let stdout_mode = set_read_mode(&mut stdout);
        let stop = Arc::new(AtomicBool::new(false));
        let cancel_write = Arc::new(AtomicBool::new(false));
        let accepted = Arc::new(AtomicGeneration::default());
        let (event_tx, event_rx) = channel::<Event>();
        let (job_tx, job_rx) = channel::<Job>();
        let (ack_tx, ack_rx) = channel::<WriteOutcome>();

        let reader_stop = stop.clone();
        let (reader_done_tx, reader_done_rx) = channel::<()>();
        let reader = thread::spawn(move || {
            read_stream(&mut stdout, &event_tx, &reader_stop);
            let _ = reader_done_tx.send(());
        });

        let writer_stop = stop.clone();
        let writer_cancel = cancel_write.clone();
        let writer_accepted = accepted.clone();
        let (writer_done_tx, writer_done_rx) = channel::<()>();
        let writer = thread::spawn(move || {
            write_jobs(
                &mut stdin,
                &job_rx,
                &ack_tx,
                &writer_stop,
                &writer_cancel,
                &writer_accepted,
            );
            let _ = writer_done_tx.send(());
        });

        Ok(Self {
            child,
            event_rx,
            job_tx,
            ack_rx,
            cancel_write,
            accepted,
            stop,
            reader: Some((reader, reader_done_rx)),
            writer: Some((writer, writer_done_rx)),
            outstanding: None,
            fatal: None,
            cleanup: None,
            io_modes: (stdin_mode, stdout_mode),
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

    /// How each pipe was made bounded, so a stalled peer is released by the deadline
    /// instead of by process death. The read side differs per platform, so this is
    /// recorded in acceptance evidence rather than assumed.
    pub fn io_modes(&self) -> (&'static str, &'static str) {
        self.io_modes
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

    /// Send one request and settle it with exactly one correlated, validated response,
    /// or fail inside one shared deadline covering both the write and the reply. A
    /// failure makes the session unusable: no later action may adopt an older reply.
    pub fn request(&mut self, payload: &Value) -> Result<Value, String> {
        if let Some(reason) = &self.fatal {
            return Err(format!("worker session unusable after {reason}"));
        }
        let mut object = match payload.as_object() {
            Some(object) => object.clone(),
            None => return Err("request is not a JSON object".to_owned()),
        };
        let Some(action) = object
            .get("action")
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            return Err("request carries no action".to_owned());
        };
        self.seq += 1;
        let seq = self.seq;
        object.insert("rpc_id".to_owned(), Value::from(seq));
        let line = match serde_json::to_string(&Value::Object(object)) {
            Ok(line) => line,
            Err(_) => return Err("request is not encodable".to_owned()),
        };
        if line.len() > MAX_REQUEST_BYTES {
            return Err(format!("request line out of bounds action={action}"));
        }
        // A request the worker cannot terminate is a request that never happened:
        // both the driver and the fake worker read line-by-line.
        let mut bytes = line.into_bytes();
        bytes.push(b'\n');
        // Anything already queued while idle means the worker spoke out of turn.
        match self.event_rx.try_recv() {
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
        let deadline_at = Instant::now() + self.deadline_for(&action);
        self.outstanding = Some(Outstanding {
            seq,
            action: action.clone(),
        });
        // Stale acknowledgements from a cancelled write must never settle this call.
        while self.ack_rx.try_recv().is_ok() {}
        self.cancel_write.store(false, Ordering::SeqCst);
        self.accepted.publish(seq);
        if self
            .job_tx
            .send(Job {
                bytes,
                deadline: deadline_at,
                generation: seq,
            })
            .is_err()
        {
            return Err(self.poison("writer thread is gone"));
        }
        let written = match self.ack_rx.recv_timeout(remaining(deadline_at)) {
            Ok(WriteOutcome::Done) => Ok(()),
            Ok(WriteOutcome::Cancelled) => Err("write cancelled"),
            Ok(WriteOutcome::Failed) => Err("write refused"),
            Err(RecvTimeoutError::Timeout) => {
                self.cancel_write.store(true, Ordering::SeqCst);
                Err("write deadline exceeded")
            }
            Err(RecvTimeoutError::Disconnected) => Err("writer stopped"),
        };
        if let Err(reason) = written {
            return Err(self.poison(reason.to_owned()));
        }
        let reply = match self.event_rx.recv_timeout(remaining(deadline_at)) {
            Ok(Event::Line(text)) => text,
            Ok(Event::Eof) => return Err(self.poison("worker eof")),
            Ok(Event::Fault(detail)) => {
                return Err(self.poison(format!("stream fault {detail}")));
            }
            Err(RecvTimeoutError::Timeout) => {
                return Err(self.poison("reply deadline exceeded"));
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(self.poison("reader stopped"));
            }
        };
        let outcome = serde_json::from_str::<Value>(&reply)
            .map_err(|_| format!("malformed json response bytes={}", reply.len()))
            .and_then(|value| {
                validate(&value, self.nonce.as_deref(), seq)?;
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

    fn pending_reason(&self, detail: &str) -> String {
        match &self.outstanding {
            Some(pending) => format!("{detail} seq={} action={}", pending.seq, pending.action),
            None => detail.to_owned(),
        }
    }

    /// Record the sanitized cause, stop both IO threads, then terminate and reap only
    /// this owned child.
    fn poison(&mut self, reason: impl Into<String>) -> String {
        if self.fatal.is_none() {
            let announced = self.pending_reason(&reason.into());
            println!("DESKTOP_RPC_FATAL {announced}");
            self.fatal = Some(announced);
        }
        self.outstanding = None;
        self.stop.store(true, Ordering::SeqCst);
        self.cancel_write.store(true, Ordering::SeqCst);
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.fatal.clone().expect("cause recorded above")
    }

    /// Test seam: exercise the write path on its own, without a request/response
    /// exchange, so a worker that stops consuming stdin can be driven until its pipe
    /// refuses more bytes. Not part of the worker protocol.
    pub fn write_probe(&mut self, bytes: &[u8], deadline: Duration) -> Result<(), String> {
        self.seq += 1;
        let generation = self.seq;
        self.accepted.publish(generation);
        self.cancel_write.store(false, Ordering::SeqCst);
        while self.ack_rx.try_recv().is_ok() {}
        let job = Job {
            bytes: bytes.to_vec(),
            deadline: Instant::now() + deadline,
            generation,
        };
        if self.job_tx.send(job).is_err() {
            return Err("writer thread is gone".to_owned());
        }
        match self.ack_rx.recv_timeout(deadline) {
            Ok(WriteOutcome::Done) => Ok(()),
            Ok(WriteOutcome::Cancelled) => Err("write cancelled".to_owned()),
            Ok(WriteOutcome::Failed) => Err("write refused".to_owned()),
            Err(RecvTimeoutError::Timeout) => {
                self.cancel_write.store(true, Ordering::SeqCst);
                Err("write deadline exceeded".to_owned())
            }
            Err(RecvTimeoutError::Disconnected) => Err("writer stopped".to_owned()),
        }
    }

    /// Stop both owned IO threads, terminate and reap only the owned child, then join
    /// the threads. An unjoined thread is reported, never treated as success.
    pub fn shutdown(&mut self) -> Cleanup {
        if let Some(done) = self.cleanup {
            return done;
        }
        self.stop.store(true, Ordering::SeqCst);
        self.cancel_write.store(true, Ordering::SeqCst);
        // Exiting the writer drops our stdin handle, so a well-behaved worker sees EOF
        // and leaves on its own. Only then does the harness terminate it by force.
        let until = Instant::now() + EXIT_WAIT;
        while Instant::now() < until {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) => thread::sleep(POLL),
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let reaped = self.reaped();
        let reader_joined = join_thread(self.reader.take());
        let writer_joined = join_thread(self.writer.take());
        println!(
            "DESKTOP_WORKER_CLEANUP reaped={reaped} reader_joined={reader_joined} writer_joined={writer_joined}"
        );
        let done = Cleanup {
            reaped,
            reader_joined,
            writer_joined,
        };
        self.cleanup = Some(done);
        if !(done.reaped && done.reader_joined && done.writer_joined) {
            println!("DESKTOP_TEARDOWN_INCOMPLETE {done:?}");
        }
        done
    }
}

impl Drop for WorkerRpc {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn remaining(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

fn join_thread(slot: Option<(JoinHandle<()>, Receiver<()>)>) -> bool {
    let Some((handle, done)) = slot else {
        return true;
    };
    if done.recv_timeout(JOIN_WAIT).is_err() {
        return false;
    }
    handle.join().is_ok()
}

fn write_jobs(
    stdin: &mut ChildStdin,
    job_rx: &Receiver<Job>,
    ack_tx: &Sender<WriteOutcome>,
    stop: &AtomicBool,
    cancel: &AtomicBool,
    accepted: &AtomicGeneration,
) {
    loop {
        let job = match job_rx.recv_timeout(POLL) {
            Ok(job) => job,
            Err(RecvTimeoutError::Timeout) => {
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if stop.load(Ordering::SeqCst) || accepted.is_stale(job.generation) {
            let _ = ack_tx.send(WriteOutcome::Cancelled);
            continue;
        }
        let mut outcome = WriteOutcome::Done;
        let mut offset = 0usize;
        loop {
            if stop.load(Ordering::SeqCst)
                || cancel.load(Ordering::SeqCst)
                || accepted.is_stale(job.generation)
            {
                outcome = WriteOutcome::Cancelled;
                break;
            }
            match stdin.write(&job.bytes[offset..]) {
                Ok(0) => {
                    outcome = WriteOutcome::Failed;
                    break;
                }
                Ok(size) => {
                    offset += size;
                    if offset < job.bytes.len() {
                        continue;
                    }
                    match stdin.flush() {
                        Ok(()) => break,
                        Err(error) if error.kind() == ErrorKind::WouldBlock => {
                            if Instant::now() >= job.deadline {
                                outcome = WriteOutcome::Failed;
                                break;
                            }
                            thread::sleep(POLL);
                        }
                        Err(error) if error.kind() == ErrorKind::Interrupted => {}
                        Err(_) => {
                            outcome = WriteOutcome::Failed;
                            break;
                        }
                    }
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    if Instant::now() >= job.deadline {
                        outcome = WriteOutcome::Failed;
                        break;
                    }
                    thread::sleep(POLL);
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(_) => {
                    outcome = WriteOutcome::Failed;
                    break;
                }
            }
        }
        let _ = ack_tx.send(outcome);
    }
}

/// One non-blocking observation of the reply stream. Neither variant may block past a
/// single poll, so the stop flag always gets a turn and teardown can join the reader.
enum Poll {
    Bytes(usize),
    Eof,
    Idle,
}

/// Unix relies on O_NONBLOCK, which is honored on both pipe ends.
#[cfg(unix)]
fn poll_bytes(stream: &mut ChildStdout, chunk: &mut [u8]) -> Result<Poll, String> {
    match stream.read(chunk) {
        Ok(0) => Ok(Poll::Eof),
        Ok(size) => Ok(Poll::Bytes(size)),
        Err(error) if error.kind() == ErrorKind::WouldBlock => {
            thread::sleep(POLL);
            Ok(Poll::Idle)
        }
        Err(error) if error.kind() == ErrorKind::Interrupted => Ok(Poll::Idle),
        Err(error) => Err(format!("reader io {}", error.kind())),
    }
}

/// Windows does not honor `PIPE_NOWAIT` for the read end of an anonymous pipe, so the
/// reader asks `PeekNamedPipe` first and only then reads bytes that are already there.
/// Without this, a grandchild that inherits the write end would hold the reader in a
/// blocking call forever and the thread could never be joined.
#[cfg(windows)]
fn poll_bytes(stream: &mut ChildStdout, chunk: &mut [u8]) -> Result<Poll, String> {
    use std::os::windows::io::AsRawHandle;
    const ERROR_BROKEN_PIPE: u32 = 109;
    let handle = stream.as_raw_handle();
    let mut available = 0u32;
    let peeked = unsafe {
        windows_sys::Win32::System::Pipes::PeekNamedPipe(
            handle,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut available,
            std::ptr::null_mut(),
        )
    };
    if peeked == 0 {
        let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        // Every writer closed the pipe: this is the only EOF signal that is reliable
        // when a process the harness never owns still holds the handle.
        if code == ERROR_BROKEN_PIPE {
            return Ok(Poll::Eof);
        }
        return Err(format!("reader peek failed with os error {code}"));
    }
    if available == 0 {
        thread::sleep(POLL);
        return Ok(Poll::Idle);
    }
    match stream.read(chunk) {
        Ok(0) => Ok(Poll::Eof),
        Ok(size) => Ok(Poll::Bytes(size)),
        Err(error) if error.kind() == ErrorKind::Interrupted => Ok(Poll::Idle),
        Err(error) => Err(format!("reader io {}", error.kind())),
    }
}

fn read_stream(stdout: &mut ChildStdout, tx: &Sender<Event>, stop: &AtomicBool) {
    let mut line: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let polled = match poll_bytes(stdout, &mut chunk) {
            Ok(poll) => poll,
            Err(detail) => {
                let _ = tx.send(Event::Fault(detail));
                return;
            }
        };
        let size = match polled {
            Poll::Idle => continue,
            Poll::Bytes(size) => size,
            Poll::Eof => {
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
        };
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
}

#[cfg(unix)]
fn set_write_nonblocking(stream: &mut ChildStdin) -> &'static str {
    make_nonblocking(stream)
}

#[cfg(unix)]
fn set_read_mode(stream: &mut ChildStdout) -> &'static str {
    make_nonblocking(stream)
}

#[cfg(unix)]
fn make_nonblocking(stream: &mut impl std::os::fd::AsFd) -> &'static str {
    use std::os::fd::{AsFd, AsRawFd};
    let fd = stream.as_fd().as_raw_fd();
    let ok = unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        flags >= 0 && libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) == 0
    };
    if ok { "nonblocking" } else { "blocking" }
}

/// The write end does honor `PIPE_NOWAIT`; the bounded-write path depends on it.
#[cfg(windows)]
fn set_write_nonblocking(stream: &mut ChildStdin) -> &'static str {
    const PIPE_NOWAIT: u32 = 0x0000_0001;
    let handle = std::os::windows::io::AsRawHandle::as_raw_handle(stream);
    let ok = unsafe {
        windows_sys::Win32::System::Pipes::SetNamedPipeHandleState(
            handle,
            &PIPE_NOWAIT,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if ok != 0 { "nonblocking" } else { "blocking" }
}

#[cfg(windows)]
fn set_read_mode(stream: &mut ChildStdout) -> &'static str {
    let _ = stream;
    "peek-poll"
}

/// A reply must name the request it answers. Anything else fails closed, so a late or
/// duplicated reply for an older call can never settle a newer one.
fn validate(value: &Value, expected_nonce: Option<&str>, expected_rpc: u64) -> Result<(), String> {
    match value.get("rpc_id") {
        None => {
            return Err(format!(
                "response carries no rpc_id expected={expected_rpc}"
            ));
        }
        Some(Value::Number(observed)) => {
            if observed.as_u64() != Some(expected_rpc) {
                return Err(format!(
                    "correlation mismatch observed={observed} expected={expected_rpc}"
                ));
            }
        }
        Some(_) => return Err("response rpc_id is not a number".to_owned()),
    }
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
    let required: Option<(&str, bool)> = match action {
        "init" => Some(("images", has_text(value, "images"))),
        "capture" => Some(("file", has_text(value, "file"))),
        "observe" => Some((
            "foreground",
            value.get("foreground").and_then(Value::as_bool).is_some(),
        )),
        _ => None,
    };
    if let Some((field, false)) = required {
        return Err(format!("{action} acknowledgement lacks {field}"));
    }
    Ok(())
}

fn has_text(value: &Value, field: &str) -> bool {
    value
        .get(field)
        .and_then(Value::as_str)
        .is_some_and(|text| !text.is_empty())
}
