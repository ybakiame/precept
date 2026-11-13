//! Bounded pipe IO shared by the acceptance driver's worker and GUI children.
//!
//! Neither direction may block past a single poll: a caller that cannot cancel a
//! blocked `read` or `write` also cannot join the thread doing it, which is exactly how
//! an unbounded window worker used to wedge a run. The setters therefore return a
//! `Result` and hand back only modes that are bounded by construction — there is no
//! "blocking" value a caller can keep going with. Test harness only; no product surface.
use std::{
    io::{ErrorKind, Read},
    thread,
    time::Duration,
};

/// Poll interval for every cooperative IO thread and its queues.
pub const POLL: Duration = Duration::from_millis(2);
/// Teardown waits no longer than this for a thread that ignores the stop flag, and
/// reports it as unjoined instead of pretending completion.
pub const JOIN_WAIT: Duration = Duration::from_secs(10);

/// An established bounded mode. `NONBLOCKING` never blocks a syscall; `PEEK_POLL`
/// asks how many bytes are there before reading them.
pub type Mode = &'static str;
pub const NONBLOCKING: Mode = "nonblocking";
pub const PEEK_POLL: Mode = "peek-poll";

/// One non-blocking observation of a byte stream.
pub enum Poll {
    Bytes(usize),
    Eof,
    Idle,
}

fn last_os_error() -> String {
    match std::io::Error::last_os_error().raw_os_error() {
        Some(code) => format!("os error {code}"),
        None => "unspecified os error".to_owned(),
    }
}

/// Unix honors `O_NONBLOCK` on both ends of a pipe, so both directions are set the
/// same way and either failure is fatal to the session.
#[cfg(unix)]
fn make_nonblocking(stream: &mut impl std::os::fd::AsFd) -> Result<Mode, String> {
    use std::os::fd::{AsFd, AsRawFd};
    let fd = stream.as_fd().as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(format!("F_GETFL failed for fd {fd}: {}", last_os_error()));
    }
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0 {
        return Err(format!(
            "F_SETFL O_NONBLOCK failed for fd {fd}: {}",
            last_os_error()
        ));
    }
    Ok(NONBLOCKING)
}

#[cfg(unix)]
pub fn set_write_nonblocking(stream: &mut impl std::os::fd::AsFd) -> Result<Mode, String> {
    make_nonblocking(stream)
}

#[cfg(unix)]
pub fn set_read_mode(stream: &mut impl std::os::fd::AsFd) -> Result<Mode, String> {
    make_nonblocking(stream)
}

#[cfg(unix)]
pub fn poll_bytes<S: Read + std::os::fd::AsFd>(
    stream: &mut S,
    chunk: &mut [u8],
) -> Result<Poll, String> {
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

/// Windows honors `PIPE_NOWAIT` only for the write end of an anonymous pipe. The read
/// end is polled with `PeekNamedPipe` instead, because a trust in the unsupported mode
/// leaves the reader parked in a blocking call that no stop flag can interrupt — which
/// is how a child holding an inherited stdout leaked a thread on the native host.
#[cfg(windows)]
pub fn set_write_nonblocking(
    stream: &mut impl std::os::windows::io::AsRawHandle,
) -> Result<Mode, String> {
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
    if ok == 0 {
        return Err(format!(
            "PIPE_NOWAIT refused on the write handle: {}",
            last_os_error()
        ));
    }
    Ok(NONBLOCKING)
}

#[cfg(windows)]
pub fn set_read_mode(_stream: &mut impl std::os::windows::io::AsRawHandle) -> Result<Mode, String> {
    Ok(PEEK_POLL)
}

#[cfg(windows)]
pub fn poll_bytes<S: Read + std::os::windows::io::AsRawHandle>(
    stream: &mut S,
    chunk: &mut [u8],
) -> Result<Poll, String> {
    use std::os::windows::io::AsRawHandle;
    const ERROR_BROKEN_PIPE: u32 = 109;
    let mut available = 0u32;
    let peeked = unsafe {
        windows_sys::Win32::System::Pipes::PeekNamedPipe(
            stream.as_raw_handle(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut available,
            std::ptr::null_mut(),
        )
    };
    if peeked == 0 {
        return match std::io::Error::last_os_error().raw_os_error() {
            Some(ERROR_BROKEN_PIPE) => Ok(Poll::Eof),
            Some(code) => Err(format!("reader peek failed with os error {code}")),
            None => Err("reader peek failed".to_owned()),
        };
    }
    if available == 0 {
        thread::sleep(POLL);
        return Ok(Poll::Idle);
    }
    // Bytes are already in the pipe, so this read returns instead of blocking.
    match stream.read(chunk) {
        Ok(0) => Ok(Poll::Eof),
        Ok(size) => Ok(Poll::Bytes(size)),
        Err(error) if error.kind() == ErrorKind::Interrupted => Ok(Poll::Idle),
        Err(error) => Err(format!("reader io {}", error.kind())),
    }
}
