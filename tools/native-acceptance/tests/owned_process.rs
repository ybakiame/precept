//! Ownership regressions for the child processes this harness launches. They drive an
//! owned mock GUI only; they never constitute native GUI acceptance.
use precept_native_acceptance::process::{OwnedCleanup, OwnedProcess, set_cleanup_sink};
use std::{
    panic::{self, AssertUnwindSafe},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc::channel,
    thread,
    time::{Duration, Instant},
};

const FAKE: &str = env!("CARGO_BIN_EXE_precept-fake-desktop-worker");
/// Cleanup must finish inside this window, not merely "eventually".
const BOUNDED: Duration = Duration::from_secs(10);

fn mock_gui(mode: &str) -> Child {
    Command::new(Path::new(FAKE))
        .arg(mode)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("owned mock GUI launches")
}

fn wait_for_log(process: &OwnedProcess, marker: &str) {
    let end = Instant::now() + BOUNDED;
    while Instant::now() < end {
        if process
            .log()
            .lock()
            .expect("log lock")
            .iter()
            .any(|line| line.contains(marker))
        {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("owned mock GUI probe marker missing: {marker}");
}

#[test]
fn adopted_child_is_reaped_and_its_log_thread_joined_on_terminate() {
    let mut process = OwnedProcess::adopt(mock_gui("gui-hang"), "gui").expect("adoption");
    let pid = process.pid();
    assert_ne!(
        process.read_mode(),
        "blocking",
        "adoption requires bounded reads"
    );
    wait_for_log(&process, "PRECEPT_GUI_SYNTHETIC");
    let start = Instant::now();
    let done = process.terminate();
    assert_eq!(
        done,
        OwnedCleanup {
            pid,
            reaped: true,
            reader_joined: true
        },
        "the mock GUI must be reaped and its pump joined"
    );
    assert!(
        start.elapsed() < BOUNDED,
        "terminate took {:?}",
        start.elapsed()
    );
    // A repeated teardown is reported from the record, never re-executed against some
    // other process that reused the pid.
    assert_eq!(process.terminate(), done);
}

#[test]
fn a_clean_exit_is_reaped_without_being_forced_down() {
    // "eof" returns immediately: the guard must still observe and reap it, and must not
    // report the exit as a forced kill.
    let mut process = OwnedProcess::adopt(mock_gui("eof"), "gui").expect("adoption");
    let pid = process.pid();
    let start = Instant::now();
    let done = process.terminate();
    assert!(done.reaped && done.reader_joined, "{done:?}");
    assert_eq!(done.pid, pid);
    assert!(
        start.elapsed() < BOUNDED,
        "terminate took {:?}",
        start.elapsed()
    );
}

#[test]
fn a_panic_during_initialization_still_reaps_the_owned_child() {
    let (tx, rx) = channel::<OwnedCleanup>();
    set_cleanup_sink(Some(tx));
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        let process = OwnedProcess::adopt(mock_gui("gui-hang"), "gui").expect("adoption");
        wait_for_log(&process, "PRECEPT_GUI_SYNTHETIC");
        // Pretend the window worker refused its session half way through initialization.
        // Ownership already exists, so unwinding must clean the child up.
        panic!("injected initialization failure");
    }));
    panic::set_hook(hook);
    set_cleanup_sink(None);
    assert!(outcome.is_err(), "the injected failure must unwind");
    let reported: Vec<OwnedCleanup> = rx.try_iter().collect();
    assert!(!reported.is_empty(), "unwinding ownership reported nothing");
    for cleanup in &reported {
        assert!(
            cleanup.reaped && cleanup.reader_joined,
            "an owned child leaked through the initialization path: {cleanup:?}"
        );
    }
    assert!(
        reported.iter().any(|c| c.pid != 0),
        "the reported cleanup must name a pid: {reported:?}"
    );
}

#[test]
fn adoption_refuses_and_reaps_when_the_bounded_read_mode_is_unavailable() {
    let child = mock_gui("gui-hang");
    let pid = child.id();
    let error = OwnedProcess::adopt_with_read_mode_sabotage(child, "gui")
        .expect_err("an unbounded log pipe must refuse adoption rather than hand out a process");
    assert_eq!(error.pid, pid, "the refusal must name the child it reaped");
    assert!(
        error.reaped,
        "a refused child must not stay running: {error:?}"
    );
    assert!(error.detail.contains("injected refusal"), "{error:?}");
}
