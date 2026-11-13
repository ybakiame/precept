//! An owned child process whose ownership begins the moment it exists.
//!
//! `Desktop::start` launches the native GUI before the window worker can even be
//! spawned, so every later initialization failure — a refused `init`, a window that
//! never appears, a bounded worker fault, a panic — must still terminate and reap *that*
//! child and stop *its* log thread. A plain `Child` handle dropped during unwinding does
//! neither: Rust leaves the process running. Test harness only; no product surface.
use crate::pipe::{self, JOIN_WAIT, Mode, POLL, Poll};
use std::{
    process::{Child, ChildStderr, ExitStatus},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// The GUI's own log is bounded; exceeding the cap is reported in the log instead of
/// silently dropping evidence.
const MAX_LOG_LINES: usize = 50_000;
/// Grace period for a well-behaved child to leave on its own before it is forced down.
const EXIT_WAIT: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnedCleanup {
    /// Which child this result belongs to, so a failure-path test can name it.
    pub pid: u32,
    pub reaped: bool,
    pub reader_joined: bool,
}

/// Refusal to adopt: the bounded read mode could not be established, so this child was
/// terminated and reaped rather than handed to a caller that would only have blocked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unfit {
    pub detail: String,
    pub pid: u32,
    pub reaped: bool,
}

/// Where a `Drop`'s cleanup result is reported, so failure-path tests can assert it
/// without parsing captured stdout. Test seam only.
static CLEANUP_SINK: OnceLock<Mutex<Option<Sender<OwnedCleanup>>>> = OnceLock::new();

/// Install (or clear with `None`) the cleanup sink. Not part of any protocol.
pub fn set_cleanup_sink(sink: Option<Sender<OwnedCleanup>>) {
    *CLEANUP_SINK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("cleanup sink lock") = sink;
}

fn send_cleanup(done: OwnedCleanup) {
    let sink = CLEANUP_SINK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("cleanup sink lock")
        .clone();
    if let Some(sink) = sink {
        let _ = sink.send(done);
    }
}

/// A child this harness launched, plus the thread that pumps its stderr.
#[derive(Debug)]
pub struct OwnedProcess {
    child: Child,
    name: &'static str,
    stop: Arc<AtomicBool>,
    reader: Option<(JoinHandle<()>, Receiver<()>)>,
    lines: Arc<Mutex<Vec<String>>>,
    cleanup: Option<OwnedCleanup>,
    exited: Option<ExitStatus>,
    read_mode: Mode,
}

impl OwnedProcess {
    /// Adopt an already-spawned child whose stderr is a pipe. Failing to make that pipe
    /// bounded terminates and reaps the child and refuses to return a process.
    pub fn adopt(mut child: Child, name: &'static str) -> Result<Self, Unfit> {
        let pid = child.id();
        let mut stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                return Err(Unfit {
                    detail: "adopted process has no piped stderr".to_owned(),
                    pid,
                    reaped: abandon(&mut child),
                });
            }
        };
        let read_mode = match pipe::set_read_mode(&mut stderr) {
            Ok(mode) => mode,
            Err(detail) => {
                return Err(Unfit {
                    detail,
                    pid,
                    reaped: abandon(&mut child),
                });
            }
        };
        Ok(Self::pumping(child, name, stderr, read_mode))
    }

    /// Test seam: pretend the mandatory bounded read mode failed, so the refusal and the
    /// owned-child cleanup are exercised on hosts where the real call does not fail.
    pub fn adopt_with_read_mode_sabotage(
        mut child: Child,
        name: &'static str,
    ) -> Result<Self, Unfit> {
        let pid = child.id();
        let stderr = child.stderr.take();
        // The handle is dropped unconditionally: an unread pipe must not outlive refusal.
        drop(stderr);
        Err(Unfit {
            detail: format!("{name} bounded read mode unavailable: injected refusal"),
            pid,
            reaped: abandon(&mut child),
        })
    }

    fn pumping(child: Child, name: &'static str, mut stderr: ChildStderr, read_mode: Mode) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let lines = Arc::new(Mutex::new(Vec::new()));
        let (done_tx, done_rx) = channel::<()>();
        let reader_stop = stop.clone();
        let reader_lines = lines.clone();
        let reader = thread::spawn(move || {
            pump(&mut stderr, &reader_lines, &reader_stop);
            let _ = done_tx.send(());
        });
        Self {
            child,
            name,
            stop,
            reader: Some((reader, done_rx)),
            lines,
            cleanup: None,
            exited: None,
            read_mode,
        }
    }

    /// Terminate and reap only this child, then stop and join its log thread. Repeating
    /// the call returns the recorded result instead of touching the process again.
    pub fn terminate(&mut self) -> OwnedCleanup {
        if let Some(done) = self.cleanup {
            return done;
        }
        self.stop.store(true, Ordering::SeqCst);
        let until = Instant::now() + EXIT_WAIT;
        while Instant::now() < until && !self.reaped() {
            thread::sleep(POLL);
        }
        let _ = self.child.kill();
        if let Ok(status) = self.child.wait() {
            self.exited = Some(status);
        }
        let reaped = self.reaped();
        let reader_joined = match self.reader.take() {
            Some((handle, done)) => done.recv_timeout(JOIN_WAIT).is_ok() && handle.join().is_ok(),
            None => true,
        };
        println!(
            "OWNED_PROCESS_CLEANUP name={} pid={} reaped={reaped} reader_joined={reader_joined}",
            self.name,
            self.pid()
        );
        let done = OwnedCleanup {
            pid: self.pid(),
            reaped,
            reader_joined,
        };
        self.cleanup = Some(done);
        if !(done.reaped && done.reader_joined) {
            println!(
                "OWNED_PROCESS_TEARDOWN_INCOMPLETE name={} {done:?}",
                self.name
            );
        }
        send_cleanup(done);
        done
    }

    /// True once the child has exited and been reaped by this harness.
    pub fn reaped(&mut self) -> bool {
        if self.exited.is_some() {
            return true;
        }
        match self.child.try_wait() {
            Ok(Some(status)) => {
                self.exited = Some(status);
                true
            }
            Ok(None) => false,
            Err(_) => true,
        }
    }

    /// The exit status once the child has exited, without waiting for it. `close()` uses
    /// this to assert the GUI left on its own after an owned window close.
    pub fn exited(&mut self) -> Option<ExitStatus> {
        self.reaped();
        self.exited
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn read_mode(&self) -> Mode {
        self.read_mode
    }

    pub fn log(&self) -> Arc<Mutex<Vec<String>>> {
        self.lines.clone()
    }
}

impl Drop for OwnedProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}

/// Force this child down and report whether it was reaped, for paths where the caller
/// never got to own it.
fn abandon(child: &mut Child) -> bool {
    let pid = child.id();
    let _ = child.kill();
    let reaped = child.wait().is_ok();
    println!("OWNED_PROCESS_ABANDONED pid={pid} reaped={reaped}");
    reaped
}

/// Collect the child's `ACP_`-prefixed probe lines. The loop checks the stop flag every
/// poll, so it can never outlive the harness that joins it.
fn pump(stderr: &mut ChildStderr, lines: &Arc<Mutex<Vec<String>>>, stop: &AtomicBool) {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut truncated = false;
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let size = match pipe::poll_bytes(stderr, &mut chunk) {
            Ok(Poll::Idle) => continue,
            Ok(Poll::Bytes(size)) => size,
            // The log stream ending or faulting loses evidence but must never hang: it
            // is recorded in the same collection the driver writes out.
            Ok(Poll::Eof) => {
                record(lines, "ACP_LOG_EOF".to_owned());
                return;
            }
            Err(detail) => {
                record(lines, format!("ACP_LOG_FAULT {detail}"));
                return;
            }
        };
        for byte in &chunk[..size] {
            if *byte != b'\n' {
                buf.push(*byte);
                continue;
            }
            let text = String::from_utf8_lossy(&std::mem::take(&mut buf)).to_string();
            if text.starts_with("ACP_") && !record(lines, text) && !truncated {
                truncated = true;
                record(lines, "ACP_LOG_TRUNCATED".to_owned());
            }
        }
    }
}

fn record(lines: &Arc<Mutex<Vec<String>>>, text: String) -> bool {
    let mut log = lines.lock().expect("gui log lock");
    if log.len() >= MAX_LOG_LINES {
        return false;
    }
    log.push(text);
    true
}
